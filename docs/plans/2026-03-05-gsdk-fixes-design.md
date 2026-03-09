# gravity-sdk Phase 3 Security Audit — All Findings Fix Design

Date: 2026-03-05
Scope: GSDK-017 through GSDK-027 (excluding GSDK-025 which is invalid)

---

## GSDK-017: Nested Mutex Holding in BlockBufferManager (HIGH)

**Problem:** `BlockBufferManager` has two `tokio::sync::Mutex` fields — `block_state_machine` and `latest_epoch_change_block_number` — that are frequently acquired in nested fashion. For example, in `get_executed_res` (line 579+626), `release_inflight_blocks` (line 941+942), and `set_compute_res`/`calculate_new_epoch_state` (line 697+674). With `tokio::sync::Mutex`, nested acquisition in different orderings across tasks can cause deadlocks since these are not reentrant.

**Fix:** Merge `latest_epoch_change_block_number` into `BlockStateMachine` as a regular field, eliminating the second mutex entirely. This removes all nested locking patterns. Since all accesses to `latest_epoch_change_block_number` already occur while `block_state_machine` is held (or could be restructured to do so), the field naturally belongs in the same critical section.

**Reference Code:**
```rust
// before (crates/block-buffer-manager/src/block_buffer_manager.rs:164-199)
pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockKey, BlockState>,
    profile: HashMap<BlockKey, BlockProfile>,
    latest_commit_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
    current_epoch: u64,
    next_epoch: Option<u64>,
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    latest_epoch_change_block_number: Mutex<u64>,
    ready_notifier: Arc<Notify>,
}

// after
pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockKey, BlockState>,
    profile: HashMap<BlockKey, BlockProfile>,
    latest_commit_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
    current_epoch: u64,
    next_epoch: Option<u64>,
    /// Moved from separate Mutex to eliminate nested locking (GSDK-017).
    latest_epoch_change_block_number: u64,
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    // REMOVED: latest_epoch_change_block_number: Mutex<u64>,
    ready_notifier: Arc<Notify>,
}
```

Then update all access sites. Example for `release_inflight_blocks`:
```rust
// before (line 940-966)
pub async fn release_inflight_blocks(&self) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    let latest_epoch_change_block_number = *self.latest_epoch_change_block_number.lock().await;
    let old_epoch = block_state_machine.current_epoch;
    // ...
    block_state_machine
        .blocks
        .retain(|key, _| key.block_number <= latest_epoch_change_block_number);
    // ...
}

// after
pub async fn release_inflight_blocks(&self) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    let latest_epoch_change_block_number =
        block_state_machine.latest_epoch_change_block_number;
    let old_epoch = block_state_machine.current_epoch;
    // ...
    block_state_machine
        .blocks
        .retain(|key, _| key.block_number <= latest_epoch_change_block_number);
    // ...
}
```

Example for `calculate_new_epoch_state`:
```rust
// before (line 674)
*self.latest_epoch_change_block_number.lock().await = block_num;

// after — block_state_machine is already held by the caller
block_state_machine.latest_epoch_change_block_number = block_num;
```

Example for `get_executed_res`:
```rust
// before (line 630-631)
let latest_epoch_change_block_number =
    *self.latest_epoch_change_block_number.lock().await;

// after — block_state_machine is already held
let latest_epoch_change_block_number =
    block_state_machine.latest_epoch_change_block_number;
```

Example for `init`:
```rust
// before (line 279)
*self.latest_epoch_change_block_number.lock().await = latest_commit_block_number;

// after
block_state_machine.latest_epoch_change_block_number = latest_commit_block_number;
```

Example for `latest_epoch_change_block_number` accessor:
```rust
// before (line 335-337)
pub async fn latest_epoch_change_block_number(&self) -> u64 {
    *self.latest_epoch_change_block_number.lock().await
}

// after
pub async fn latest_epoch_change_block_number(&self) -> u64 {
    let bsm = self.block_state_machine.lock().await;
    bsm.latest_epoch_change_block_number
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

---

## GSDK-018: Epoch Transition Stale Block Waste — No Cancellation (HIGH)

**Problem:** When an epoch transition occurs, `release_inflight_blocks` retains only blocks at or below `latest_epoch_change_block_number`. Blocks that are currently in-flight (being executed in the reth execution layer) are discarded from the buffer. However, the execution layer continues working on them until its own timeout (up to 2 seconds in `max_wait_timeout`), wasting CPU and delaying the new epoch's first block.

**Fix:** Introduce a `CancellationToken` (from `tokio_util`) per epoch. When an epoch transition occurs, cancel the token for the old epoch. The execution layer checks the token before starting and during execution, allowing it to abort stale work immediately.

**Reference Code:**
```rust
// before — BlockBufferManager fields
pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    ready_notifier: Arc<Notify>,
}

// after — add epoch cancellation token
use tokio_util::sync::CancellationToken;

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    ready_notifier: Arc<Notify>,
    /// Cancellation token for the current epoch. Cancelled on epoch transition
    /// to abort in-flight work from the old epoch (GSDK-018).
    epoch_cancel_token: Mutex<CancellationToken>,
}
```

In `release_inflight_blocks`, cancel the old token and create a new one:
```rust
// after — in release_inflight_blocks
pub async fn release_inflight_blocks(&self) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    // ...existing epoch update logic...

    // GSDK-018: Cancel in-flight execution for the old epoch
    {
        let mut token = self.epoch_cancel_token.lock().await;
        token.cancel();
        *token = CancellationToken::new();
    }

    block_state_machine
        .blocks
        .retain(|key, _| key.block_number <= latest_epoch_change_block_number);
    self.buffer_state.store(BufferState::EpochChange as u8, Ordering::SeqCst);
    // ...
}
```

Expose the token for the execution layer to check:
```rust
/// Returns a clone of the current epoch's cancellation token.
/// The execution layer should check this token during block execution
/// to abort early if the epoch has changed.
pub async fn current_epoch_cancel_token(&self) -> CancellationToken {
    self.epoch_cancel_token.lock().await.clone()
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`
- `bin/gravity_node/src/reth_cli.rs` (check token in execution loop)

---

## GSDK-019: Silent Block Dropping on Epoch Mismatch (MEDIUM)

**Problem:** In `set_ordered_blocks`, when a block's epoch does not match `current_epoch`, it is silently dropped with `return Ok(())` (lines 392-405). This hides legitimate issues — a future-epoch block could indicate a consensus bug, and logging alone (at `warn` level) may be missed in production. No metric is emitted, making it invisible to monitoring.

**Fix:** Add a metrics counter for dropped blocks by epoch mismatch, and consider returning an error for future-epoch blocks (which should never happen under correct consensus).

**Reference Code:**
```rust
// before (line 390-405)
// Check epoch validity
if block.block_meta.epoch < current_epoch {
    warn!(
        "set_ordered_blocks: ignoring block {} with old epoch {} (current epoch: {})",
        block.block_meta.block_number, block.block_meta.epoch, current_epoch
    );
    return Ok(());
}

if block.block_meta.epoch > current_epoch {
    warn!(
        "set_ordered_blocks: ignoring block {} with future epoch {} (current epoch: {})",
        block.block_meta.block_number, block.block_meta.epoch, current_epoch
    );
    return Ok(());
}

// after
if block.block_meta.epoch < current_epoch {
    warn!(
        "set_ordered_blocks: ignoring block {} with old epoch {} (current epoch: {})",
        block.block_meta.block_number, block.block_meta.epoch, current_epoch
    );
    metrics::counter!(
        "block_buffer_manager_dropped_blocks",
        &[("reason", "old_epoch")]
    )
    .increment(1);
    return Ok(());
}

if block.block_meta.epoch > current_epoch {
    // Future-epoch blocks should not arrive under correct consensus.
    // Treat as an error to surface the issue.
    let msg = format!(
        "set_ordered_blocks: block {} has future epoch {} (current epoch: {})",
        block.block_meta.block_number, block.block_meta.epoch, current_epoch
    );
    warn!("{}", msg);
    metrics::counter!(
        "block_buffer_manager_dropped_blocks",
        &[("reason", "future_epoch")]
    )
    .increment(1);
    return Err(anyhow::anyhow!("{msg}"));
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

---

## GSDK-020: In-Flight Blocks Discarded During Epoch Transition (MEDIUM)

**Problem:** `release_inflight_blocks` uses `retain()` to remove ALL blocks with `block_number > latest_epoch_change_block_number` regardless of their state (Ordered, Computed, Committed). Blocks in `Computed` state have already been executed — discarding them wastes the execution work and may leave the execution layer in an inconsistent state if it expects a response for those blocks.

**Fix:** Add a metric counter for discarded blocks by state, and log at `warn` level when `Computed` or `Committed` blocks are discarded. The execution layer already handles the missing block case via the timeout in `get_executed_res`, so no functional change is needed — but visibility is important.

**Reference Code:**
```rust
// before (line 958-964)
block_state_machine
    .blocks
    .retain(|key, _| key.block_number <= latest_epoch_change_block_number);

// after
let mut discarded_ordered = 0u64;
let mut discarded_computed = 0u64;
let mut discarded_committed = 0u64;

block_state_machine.blocks.retain(|key, state| {
    if key.block_number <= latest_epoch_change_block_number {
        return true;
    }
    match state {
        BlockState::Ordered { .. } => discarded_ordered += 1,
        BlockState::Computed { id, .. } => {
            warn!(
                "release_inflight_blocks: discarding Computed block {:?} num {}",
                id, key.block_number
            );
            discarded_computed += 1;
        }
        BlockState::Committed { id, .. } => {
            warn!(
                "release_inflight_blocks: discarding Committed block {:?} num {}",
                id, key.block_number
            );
            discarded_committed += 1;
        }
        BlockState::Historical { .. } => {}
    }
    false
});

if discarded_ordered + discarded_computed + discarded_committed > 0 {
    info!(
        "release_inflight_blocks: discarded {} ordered, {} computed, {} committed blocks \
         above epoch change block {}",
        discarded_ordered,
        discarded_computed,
        discarded_committed,
        latest_epoch_change_block_number
    );
    metrics::counter!(
        "block_buffer_manager_epoch_discarded_blocks",
        &[("state", "ordered")]
    )
    .increment(discarded_ordered);
    metrics::counter!(
        "block_buffer_manager_epoch_discarded_blocks",
        &[("state", "computed")]
    )
    .increment(discarded_computed);
    metrics::counter!(
        "block_buffer_manager_epoch_discarded_blocks",
        &[("state", "committed")]
    )
    .increment(discarded_committed);
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

---

## GSDK-021: TOCTOU Race in consume_epoch_change (MEDIUM)

**Problem:** In `consume_epoch_change` (line 329-333), `buffer_state` is set to `Ready` via atomic store *before* acquiring the `block_state_machine` lock. Between the atomic store and the lock acquisition, another task calling `is_epoch_change()` or `get_ordered_blocks()` could see `Ready` state and attempt to process blocks before the epoch change is fully consumed.

**Fix:** Reorder the operations: acquire the `block_state_machine` lock first, read the epoch, then set `buffer_state` to `Ready`.

**Reference Code:**
```rust
// before (line 329-333)
pub async fn consume_epoch_change(&self) -> u64 {
    self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
    let block_state_machine = self.block_state_machine.lock().await;
    block_state_machine.current_epoch
}

// after
pub async fn consume_epoch_change(&self) -> u64 {
    // GSDK-021: Acquire lock first to prevent TOCTOU race.
    // Other tasks checking is_epoch_change() will still see EpochChange
    // until we have fully read the new epoch and are ready to proceed.
    let block_state_machine = self.block_state_machine.lock().await;
    let epoch = block_state_machine.current_epoch;
    // Only now signal that the epoch change has been consumed
    self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
    epoch
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

---

## GSDK-022: Execution Failure Terminates Commit Vote Loop (MEDIUM)

**Problem:** In `start_commit_vote` (line 327-368), when `recv_compute_res()` returns an `Err`, the loop `break`s, permanently terminating the commit vote pipeline. This means no further execution results will ever be forwarded to the consensus layer, stalling the entire node. The error could be a transient channel issue (e.g., sender temporarily dropped during epoch transition).

**Fix:** Distinguish between fatal errors (channel permanently closed) and transient errors. For transient errors, log and continue the loop. For fatal errors (sender dropped), trigger a graceful shutdown rather than silently breaking.

**Reference Code:**
```rust
// before (bin/gravity_node/src/reth_cli.rs:327-368)
pub async fn start_commit_vote(&self) -> Result<(), String> {
    loop {
        let mut shutdown = self.shutdown.resubscribe();
        let execution_result = tokio::select! {
            res = self.recv_compute_res() => res,
            _ = shutdown.recv() => {
                info!("Shutdown signal received, stopping commit vote loop");
                break;
            }
        };

        let execution_result = match execution_result {
            Ok(res) => res,
            Err(e) => {
                warn!("recv_compute_res failed: {}. Stopping commit vote loop.", e);
                break;
            }
        };
        // ... process execution_result ...
    }
    Ok(())
}

// after
pub async fn start_commit_vote(&self) -> Result<(), String> {
    let mut consecutive_errors = 0u32;
    const MAX_CONSECUTIVE_ERRORS: u32 = 5;

    loop {
        let mut shutdown = self.shutdown.resubscribe();
        let execution_result = tokio::select! {
            res = self.recv_compute_res() => res,
            _ = shutdown.recv() => {
                info!("Shutdown signal received, stopping commit vote loop");
                break;
            }
        };

        let execution_result = match execution_result {
            Ok(res) => {
                consecutive_errors = 0; // Reset on success
                res
            }
            Err(e) => {
                consecutive_errors += 1;
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    // GSDK-022: Too many consecutive failures — channel is likely
                    // permanently broken. Trigger shutdown rather than silent stall.
                    error!(
                        "recv_compute_res failed {} consecutive times (last: {}). \
                         Triggering graceful shutdown.",
                        consecutive_errors, e
                    );
                    return Err(format!(
                        "Commit vote loop terminated after {} consecutive errors: {}",
                        consecutive_errors, e
                    ));
                }
                warn!(
                    "recv_compute_res failed (attempt {}/{}): {}. Retrying...",
                    consecutive_errors, MAX_CONSECUTIVE_ERRORS, e
                );
                // Brief delay before retry to avoid tight error loop
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };
        // ... process execution_result unchanged ...
    }
    Ok(())
}
```

**Files:**
- `bin/gravity_node/src/reth_cli.rs`

---

## GSDK-023: buffer_state AtomicU8 Checked Outside Lock — TOCTOU (LOW)

**Problem:** `buffer_state` (an `AtomicU8`) is checked via `is_ready()` and `is_epoch_change()` before acquiring the `block_state_machine` lock in several methods (`set_ordered_blocks` line 377, `get_ordered_blocks` line 474-479, `consume_epoch_change` line 329). The state can change between the atomic read and the lock acquisition, leading to TOCTOU races where the method proceeds based on stale state.

**Fix:** Document the acceptable race condition at each call site. The races are benign because: (1) `is_ready()` only gates a `Notify` wait, and re-checking after lock acquisition would be equivalent; (2) `is_epoch_change()` in `get_ordered_blocks` returns an error that the caller handles gracefully. Add a comment at each site and re-check inside the lock where it matters.

**Reference Code:**
```rust
// before (line 468-480 in get_ordered_blocks)
pub async fn get_ordered_blocks(
    &self,
    start_num: u64,
    max_size: Option<usize>,
    expected_epoch: u64,
) -> Result<Vec<(ExternalBlock, BlockId)>, anyhow::Error> {
    if !self.is_ready() {
        self.ready_notifier.notified().await;
    }

    if self.is_epoch_change() {
        return Err(anyhow::anyhow!("Buffer is in epoch change"));
    }

// after
pub async fn get_ordered_blocks(
    &self,
    start_num: u64,
    max_size: Option<usize>,
    expected_epoch: u64,
) -> Result<Vec<(ExternalBlock, BlockId)>, anyhow::Error> {
    // GSDK-023: is_ready() check outside lock is intentional — it only gates
    // the Notify wait. The actual epoch/state check happens inside the lock below.
    if !self.is_ready() {
        self.ready_notifier.notified().await;
    }

    // GSDK-023: is_epoch_change() is a fast-path exit. Even if the state changes
    // between this check and the lock acquisition, the epoch mismatch check
    // inside the lock (below) will catch it.
    if self.is_epoch_change() {
        return Err(anyhow::anyhow!("Buffer is in epoch change"));
    }

    // Check if expected_epoch matches current_epoch under the lock
    {
        let block_state_machine = self.block_state_machine.lock().await;
        let current_epoch = block_state_machine.current_epoch;
        if expected_epoch != current_epoch {
            // ...
        }
        // Re-check epoch change under lock for correctness
        if self.buffer_state.load(Ordering::SeqCst) == BufferState::EpochChange as u8 {
            return Err(anyhow::anyhow!("Buffer is in epoch change (re-checked under lock)"));
        }
    }
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

---

## GSDK-024: pop_txns Off-by-One in Gas Accounting (LOW)

**Problem:** In `pop_txns` (line 339-370), the iterator uses a `position()` call with a closure that returns `false` on the first item (when `total_gas_limit == 0`), then only adds `gas_limit` for subsequent items. This means the first transaction item's `gas_limit` is never added to `total_gas_limit`. The `count` variable is also never incremented for the first item. The net effect is that `pop_txns` always includes one extra item's worth of gas beyond the requested limit.

**Fix:** Initialize the gas accounting to include the first item, or restructure the loop to handle all items uniformly.

**Reference Code:**
```rust
// before (line 339-370)
pub async fn pop_txns(
    &self,
    max_size: usize,
    gas_limit: u64,
) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
    let mut txn_buffer = self.txn_buffer.txns.lock().await;
    let mut total_gas_limit = 0;
    let mut count = 0;
    let total_txn = txn_buffer.iter().map(|item| item.txns.len()).sum::<usize>();
    tracing::info!("pop_txns total_txn: {:?}", total_txn);
    let split_point = txn_buffer
        .iter()
        .position(|item| {
            if total_gas_limit == 0 {
                return false;
            }
            if total_gas_limit + item.gas_limit > gas_limit || count >= max_size {
                return true;
            }
            total_gas_limit += item.gas_limit;
            count += 1;
            false
        })
        .unwrap_or(txn_buffer.len());
    // ...
}

// after
pub async fn pop_txns(
    &self,
    max_size: usize,
    gas_limit: u64,
) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
    let mut txn_buffer = self.txn_buffer.txns.lock().await;
    let mut total_gas_limit = 0u64;
    let mut count = 0usize;
    let total_txn = txn_buffer.iter().map(|item| item.txns.len()).sum::<usize>();
    tracing::info!("pop_txns total_txn: {:?}", total_txn);

    // GSDK-024: Fixed off-by-one — account for every item's gas uniformly.
    // The split_point is the index of the first item that would exceed the limit.
    let split_point = txn_buffer
        .iter()
        .position(|item| {
            if total_gas_limit + item.gas_limit > gas_limit || count >= max_size {
                return true;
            }
            total_gas_limit += item.gas_limit;
            count += 1;
            false
        })
        .unwrap_or(txn_buffer.len());

    let valid_item = txn_buffer.drain(0..split_point).collect::<Vec<_>>();
    drop(txn_buffer);
    let mut result = Vec::new();
    for mut item in valid_item {
        result.extend(std::mem::take(&mut item.txns));
    }
    Ok(result)
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

---

## GSDK-026: Coinbase Address Hardcoded to Zero (LOW, PARTIAL)

**Problem:** In `get_coinbase_from_proposer_index` (line 136-161), when the proposer index is `None` or the proposer-to-reth-address map lookup fails, `Address::ZERO` is returned. Using the zero address as coinbase means block rewards and transaction fees go to an unrecoverable address. While the `warn!` log exists for the lookup failure case, the `proposer_index == None` case silently returns zero without any log.

**Fix:** Add a warning log for the `None` proposer_index case. Also add a metric counter for zero-address fallbacks to make monitoring easier. Ensure the proposer-reth map is populated early in epoch initialization.

**Reference Code:**
```rust
// before (bin/gravity_node/src/reth_cli.rs:134-161)
/// Get reth coinbase address from proposer's validator index
/// Returns the reth account address of the proposer if found, otherwise returns Address::ZERO
fn get_coinbase_from_proposer_index(proposer_index: Option<u64>) -> Address {
    let index = match proposer_index {
        Some(idx) => idx,
        None => return Address::ZERO,
    };

    // Get reth address from global map (built in epoch_manager when epoch starts)
    match get_reth_address_by_index(index) {
        Some(reth_addr_bytes) => {
            if reth_addr_bytes.len() == 20 {
                Address::from_slice(&reth_addr_bytes)
            } else {
                warn!(
                    "Reth address length {} is not 20 bytes for proposer index {}, using ZERO",
                    reth_addr_bytes.len(),
                    index
                );
                Address::ZERO
            }
        }
        None => {
            warn!("Failed to get reth coinbase for proposer index {}, using ZERO", index);
            Address::ZERO
        }
    }
}

// after
fn get_coinbase_from_proposer_index(proposer_index: Option<u64>) -> Address {
    let index = match proposer_index {
        Some(idx) => idx,
        None => {
            // GSDK-026: Log when proposer_index is absent — this means the block
            // metadata from consensus did not include a proposer.
            warn!(
                "Block has no proposer_index in metadata, using Address::ZERO as coinbase. \
                 This may indicate a consensus-layer issue."
            );
            metrics::counter!(
                "coinbase_zero_address_fallback",
                &[("reason", "no_proposer_index")]
            )
            .increment(1);
            return Address::ZERO;
        }
    };

    match get_reth_address_by_index(index) {
        Some(reth_addr_bytes) => {
            if reth_addr_bytes.len() == 20 {
                Address::from_slice(&reth_addr_bytes)
            } else {
                warn!(
                    "Reth address length {} is not 20 bytes for proposer index {}, using ZERO",
                    reth_addr_bytes.len(),
                    index
                );
                metrics::counter!(
                    "coinbase_zero_address_fallback",
                    &[("reason", "invalid_address_length")]
                )
                .increment(1);
                Address::ZERO
            }
        }
        None => {
            warn!("Failed to get reth coinbase for proposer index {}, using ZERO", index);
            metrics::counter!(
                "coinbase_zero_address_fallback",
                &[("reason", "proposer_not_in_map")]
            )
            .increment(1);
            Address::ZERO
        }
    }
}
```

**Files:**
- `bin/gravity_node/src/reth_cli.rs`

---

## GSDK-027: Validators May Temporarily Observe Different Epochs (LOW)

**Problem:** During epoch transitions, different validators may temporarily observe different `current_epoch` values. This happens because `release_inflight_blocks` (which updates `current_epoch`) is called asynchronously on each validator when they receive the epoch-change event from consensus. The temporal divergence window is bounded by network propagation time and processing delay.

**Fix:** No code change needed. This is expected behavior under BFT consensus. Add documentation to clarify that this temporal divergence is safe because:
1. The consensus layer (AptosBFT) ensures all validators eventually converge to the same epoch.
2. Blocks are only committed after 2/3+ quorum, so a validator cannot finalize a block from a "wrong" epoch.
3. The `BlockBufferManager` already handles epoch mismatches gracefully via `set_ordered_blocks` epoch checks and `get_ordered_blocks` epoch validation.

**Reference Code:**
```rust
// Add documentation to BlockBufferManager (no functional change)

// before (crates/block-buffer-manager/src/block_buffer_manager.rs:193)
pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    // ...
}

// after
/// Manages the lifecycle of blocks through the GCEI pipeline:
/// Ordered -> Computed -> Committed.
///
/// # Epoch Transition Behavior (GSDK-027)
///
/// During epoch transitions, different validators may temporarily observe
/// different `current_epoch` values. This is expected and safe because:
///
/// 1. AptosBFT consensus ensures all validators converge to the same epoch.
/// 2. Blocks require 2/3+ quorum to commit, preventing finalization from a
///    stale epoch.
/// 3. Epoch mismatches are handled gracefully: `set_ordered_blocks` drops
///    mismatched blocks, and `get_ordered_blocks` returns an error that the
///    caller retries.
///
/// The divergence window is bounded by network propagation + processing time,
/// typically under 1 second.
pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    // ...
}
```

**Files:**
- `crates/block-buffer-manager/src/block_buffer_manager.rs` (documentation only)
