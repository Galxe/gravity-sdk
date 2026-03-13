# attack_error_handling_and_panic_safety

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 224812ms
- **Steps**: 1

## Report

# Intention Attack Report — gravity-sdk Security Audit

## Executive Summary

The gravity-sdk node contains **multiple fatal business logic vulnerabilities** that go far beyond the error-handling surface described in the implementation report. The system has broken invariants in state management, missing trust boundaries between consensus and execution, economic loss vectors, and several paths where the node enters an irrecoverable zombie state. Below are the findings, ordered by severity.

---

## CRITICAL Findings

### C1: Zombie Node — Silent Task Death with No Recovery

**Files:** `reth_coordinator/mod.rs:42-52`, `main.rs:300-303`

All three core loops (`start_execution`, `start_commit_vote`, `start_commit`) are spawned with discarded `JoinHandle`s and `.unwrap()`. When any loop dies:

- The other two loops **continue running in a degraded, inconsistent state**
- The main thread **hangs indefinitely** at `shutdown_rx.recv()` waiting for a Ctrl+C that may never come
- There is no health monitoring, no restart mechanism, no alerting

**Attack scenario:** An attacker who can trigger a single `panic!` in any of the 6+ panic paths in `set_commit_blocks` or the `assert_eq!` in `get_executed_res` permanently kills that pipeline stage. The node appears alive (responds to health checks, keeps its P2P connections) but **stops producing or committing blocks**, silently stalling the chain.

---

### C2: Committed Hash Never Verified Against Computed Hash — Block Forgery

**File:** `block_buffer_manager.rs` — `set_compute_res` vs `set_commit_blocks`

The hash stored during execution (`set_compute_res`) and the hash stored during commit (`set_commit_blocks`) are **two independent values that are never cross-checked**. A caller can commit a block with a different hash than what was computed, and the buffer manager silently accepts it.

**Furthermore**, if `start_commit_vote` is behind `start_commit`, the hash field is `Option<[u8; 32]>` and will be `None`. The code passes `None` directly to `pipe_api.commit_executed_block_hash` — **committing a block with no execution hash**.

**Impact:** This breaks the fundamental invariant that a committed block was actually executed with the claimed result.

---

### C3: Consensus `parent_id` Silently Overridden — Fork Acceptance

**File:** `block_buffer_manager.rs:478-484`

```rust
info!("set_ordered_blocks parent_id is not the same as actual_parent_id ... might be epoch change");
actual_parent_id  // silently uses local value, ignoring consensus
```

When the consensus layer provides a `parent_id` that doesn't match the local chain, the execution layer **silently overrides it** and continues. The `TODO(gravity_alex): assert epoch` confirms this logic is incomplete. This means the execution layer can accept a forked chain without any error, alert, or halt — **a fundamental trust boundary violation between consensus and execution**.

---

### C4: `chain_id` Hardcoded to `0` — Cross-Chain Replay Protection Broken

**File:** `mempool.rs:102, 115`

```rust
chain_id: ExternalChainId::new(0),
```

All `VerifiedTxn` objects are constructed with `chain_id: 0` regardless of the actual chain. A transaction signed for one gravity chain can be replayed on any other gravity chain. This defeats EIP-155 replay protection at the mempool layer.

---

### C5: `todo!()` in Production Consensus Path — Node Crash on First Error

**File:** `consensus_mempool_handler.rs:41, 112`

Two `todo!()` macros exist in **ungated production code**:

1. **Line 41:** If `notify_new_commit` ever returns an error, `todo!()` panics. The `Con2Mempool` task dies permanently. Committed transactions **are never evicted from the mempool again**, causing unbounded memory growth and potential re-proposal of already-committed transactions.

2. **Line 112:** A `SyncForDuration` consensus notification panics the handler. After this, commit acknowledgments stop flowing back to consensus, which can **stall consensus liveness** across all validators.

Both are denial-of-service vectors: any condition that triggers either path permanently degrades the node.

---

## HIGH Findings

### H1: Partial Batch Commit with No Rollback — Inconsistent State

**File:** `reth_cli.rs:472-491`

`start_commit` commits blocks in a batch loop. If the loop succeeds for blocks N and N+1 but fails on N+2, the `?` operator returns `Err`. The spawned task panics (see C1). Blocks N and N+1 are committed in the pipe layer, but the commit task is now dead. The execution and vote tasks continue running unaware. The buffer manager and reth pipe state are now permanently inconsistent — **there is no rollback of partial commits**.

---

### H2: Epoch Race — Old-Epoch Blocks Stamped with New Epoch

**Files:** `reth_cli.rs:337-343, 432, 448`

`current_epoch` is an `AtomicU64` written only by `start_execution` and read by `start_commit_vote` and `start_commit`. There is **no synchronization barrier** ensuring the other loops have finished processing old-epoch blocks before seeing the new epoch value. A block from epoch N still in-flight through `start_commit_vote` will be stamped with epoch N+1's value, violating epoch-boundary invariants.

---

### H3: Double Execution Risk — No In-Flight Guard

**File:** `block_buffer_manager.rs` — `get_ordered_blocks`

`get_ordered_blocks` returns blocks in `Ordered` state **without mutating state to mark them as dispatched**. If called twice (e.g., by a restarting executor or concurrent task), the same blocks are returned and executed again. The second `set_compute_res` call panics (block already in `Computed` state), crashing the execution task silently (see C1).

---

### H4: Coinbase Silently Zeroed — Validator Fee Burn

**File:** `reth_cli.rs:142-181`

Three separate failure paths in proposer-to-coinbase lookup all fall back to `Address::ZERO`:

```rust
warn!("Failed to get reth coinbase for proposer index {}, using ZERO.");
Address::ZERO
```

Block fees and MEV rewards are **sent to the zero address (burned)** instead of the correct validator. A malicious consensus layer that omits `proposer_index` from block metadata causes economic loss for every affected block.

---

### H5: TOCTOU on `is_epoch_change()` Check

**File:** `block_buffer_manager.rs:509-511`

`is_epoch_change()` reads an `AtomicU8` **without holding the mutex**, then the caller acquires the mutex afterward. Between the two operations, another task can trigger an epoch change. The existing `TODO` comments confirm this was a known concern only partially addressed.

---

### H6: `remove_committed_blocks` Is a No-Op — Memory Leak

**File:** `block_buffer_manager.rs:259-263`

```rust
block_state_machine.latest_finalized_block_number = std::cmp::max(
    block_state_machine.latest_finalized_block_number,
    latest_persist_block_num,  // same variable — max(x, x) = x
);
```

The cleanup function never advances its own watermark. The `blocks` map, `profile` map, and `block_number_to_block_id` map grow without bound. Under sustained load, this is a **deterministic OOM**.

---

### H7: Sensitive Credentials Logged

**File:** `relayer.rs:97, 224`

`RelayerConfig` containing RPC URLs (which may include embedded API keys like `https://eth-mainnet.alchemyapi.io/v2/SECRET_KEY`) is logged at `info` level on every startup. Oracle states are similarly dumped. These logs are typically shipped to external aggregators.

---

### H8: Malformed Transactions Silently Dropped — No Slashing Signal

**File:** `reth_cli.rs:210-222`

Transactions from consensus blocks that fail signature recovery are silently skipped with a `warn!` log. There is no counter, no metric, no signal to the consensus/slashing layer. A malicious proposer can fill blocks with garbage transactions to waste block space, and the only evidence is buried in log files with no alerting.

---

## MEDIUM Findings

| # | Finding | File | Impact |
|---|---------|------|--------|
| M1 | Integer underflow: `block_number - 1` when `block_number == 0` wraps to `u64::MAX` | `block_buffer_manager.rs:461` | Nonsensical block key lookup |
| M2 | `SystemTime::now()` used for block timestamp in mempool GC instead of block's consensus timestamp | `consensus_mempool_handler.rs:79` | TTL eviction incorrect under clock skew |
| M3 | `get_committed_blocks` destructively `take()`s `persist_notifier` — second call loses the ack channel | `block_buffer_manager.rs:879` | Epoch-change wait hangs indefinitely |
| M4 | Nonce ordering in mempool only tracks iterator-local state, not on-chain committed nonce | `mempool.rs:145-151` | Nonce gaps undetected, potential replay within mempool window |
| M5 | Epoch state (`next_epoch`, `latest_epoch_change_block_number`) mutated before block state transition; no rollback on partial failure | `block_buffer_manager.rs` `calculate_new_epoch_state` | Permanent epoch/block desync |
| M6 | `shutdown.resubscribe()` per loop iteration can miss shutdown signals under load | `reth_cli.rs:323,378,449` | Delayed shutdown, zombie loops |
| M7 | Unbounded `txn_buffer` with no size cap in `push_txns` | `block_buffer_manager.rs:332` | OOM under transaction flood |
| M8 | `run_reth` calls `process::exit(1)` on error, bypassing async cleanup and shutdown broadcast | `main.rs:153` | Unclean shutdown, data corruption risk |
| M9 | Mutex held across `.await` in `set_compute_res` | `block_buffer_manager.rs:724-726` | Lock starvation under contention |

---

## Architecture-Level Assessment

The systemic issue is a **failure propagation architecture that creates zombie states**:

```
start_execution()   ─Err──▶ .unwrap() ──▶ task panic (silent)
start_commit_vote() ─Err──▶ .unwrap() ──▶ task panic (silent)
start_commit()      ─Err──▶ .unwrap() ──▶ task panic (silent)

    ┌── No JoinHandle stored
    ├── No health monitoring
    ├── No restart mechanism
    └── Main thread blocks on shutdown_rx forever
         ═══▶ Zombie process that looks alive but is dead
```

Combined with the block buffer manager's state machine having **6 distinct panic paths** and **no in-flight execution guard**, any transient failure cascades into permanent, irrecoverable corruption. The design assumes infallible execution in a distributed system where failures are guaranteed.

---

## Top 5 Recommendations (by blast radius)

1. **Store JoinHandles, monitor tasks, and implement coordinated shutdown** — Any task death must trigger a full node restart or at minimum halt all three loops atomically.

2. **Verify committed hash against computed hash** — The commit path must assert that the block hash being committed matches what was computed during execution. This is the core safety invariant of any execute-then-commit blockchain.

3. **Replace `todo!()` in `consensus_mempool_handler.rs`** with proper error handling — These are live DoS vectors in production code.

4. **Fix `chain_id` from hardcoded `0`** to the actual chain ID — Cross-chain replay protection is currently non-functional.

5. **Add an `Executing` state to the block state machine** — Prevent double-dispatch of blocks for execution, which currently leads to a crash.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report — gravity-sdk Security Audit

## E | 224812ms |
