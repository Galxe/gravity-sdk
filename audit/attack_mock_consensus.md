# attack_mock_consensus

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 141954ms
- **Steps**: 1

## Report

# Mock Consensus Security Audit — Attack Findings

---

## CRITICAL

### 1. Mock Consensus Activatable in Production via Environment Variable
**Location:** `bin/gravity_node/src/main.rs:270`

```rust
if std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap() {
```

**Vulnerability:** Any deployed production binary can be silently switched to mock consensus by setting `MOCK_CONSENSUS=true`. There is:
- No compile-time gate (`#[cfg(feature = "mock")]` or `#[cfg(debug_assertions)]`)
- No log warning emitted when mock mode activates
- No environment allowlist (e.g., rejecting activation when `ENV=production`)

**Impact:** An attacker with access to environment variables (container orchestration, CI/CD, compromised deploy config) can downgrade a production validator from BFT consensus to single-node mock consensus. This **eliminates all Byzantine fault tolerance** — the attacker becomes the sole block proposer with `proposer_index: 0`, can censor transactions, reorder them arbitrarily, and produce blocks at will. The rest of the network would not detect this if the node is a full node rather than a validator, but if it's a validator, it breaks the consensus quorum assumptions.

---

### 2. `commit_txns` Is a No-Op — Committed Transactions Are Never Removed from the Pool
**Location:** `mempool.rs:47`

```rust
pub fn commit_txns(&mut self, _txns: &[TxnId]) {}
```

**Vulnerability:** After a block is committed, the committed transactions are sent to the mempool via an unbounded channel (`mock.rs:263`), but `commit_txns` does nothing. The underlying `TxPool` is never notified that these transactions were committed. This means:

1. **Transaction replay in future blocks:** `best_txns()` will continue returning already-committed transactions. The only guard is the `next_sequence_numbers` nonce filter in `get_txns`, but this is **reset on every epoch change** (`reset_epoch` clears the map at `mempool.rs:23`). After an epoch boundary, all previously committed transactions become eligible for re-inclusion.

2. **Unbounded memory growth:** The `TxPool` accumulates transactions indefinitely since `remove_txns()` is never called. Over time, this causes memory exhaustion on long-running nodes.

3. **Unbounded channel as amplifier:** The `commit_txns_tx` channel (`mock.rs:197`) is unbounded. Even though the consumer is a no-op, the unbounded channel itself means the producer (main loop) can enqueue `Vec<TxnId>` faster than the consumer drains them under load, adding further memory pressure.

---

### 3. Non-Cryptographic Block ID with 75% Zero Bytes — Collision and Predictability
**Location:** `mock.rs:76-81`

```rust
let mut hasher = DefaultHasher::new();   // SipHash-1-3
txns.hash(&mut hasher);
attr.hash(&mut hasher);
let block_id = hasher.finish();          // 64-bit output
let mut bytes = [0u8; 32];
bytes[0..8].copy_from_slice(&block_id.to_be_bytes());
// bytes[8..32] = 0x00 — 24 zero bytes
```

**Vulnerability:** Three compounding issues:

- **64-bit collision space:** `BlockId` is a 256-bit type, but only 64 bits carry entropy. The birthday-bound collision probability reaches 50% at ~2³² blocks (~4 billion). More critically, `get_executed_res` (`block_buffer_manager.rs:574`) uses `assert_eq!(id, &block_id)` — a collision causes a **node panic**.
- **Deterministic within a process:** `DefaultHasher` (SipHash) uses keys that are fixed per compilation/process. An attacker who knows the binary can precompute block IDs for any given transaction set.
- **`bytes[8..32]` are always zero:** This is a distinguishing fingerprint. Any system inspecting `BlockId` values can trivially identify mock-consensus-produced blocks, and the zero-padding reduces effective entropy for any downstream use.

---

## HIGH

### 4. `std::sync::Mutex` Held Across `Condvar::wait()` Inside `tokio::spawn` — Thread Starvation
**Location:** `mock.rs:173-188`

```rust
// Inside a tokio::spawn'd async task:
let (lock, cvar) = executed_jam_wait.as_ref();
let mut executed_number = lock.lock().unwrap();
while (block_number - *executed_number) > get_max_executed_gap() {
    executed_number = cvar.wait(executed_number).unwrap();  // BLOCKS OS THREAD
}
```

**Vulnerability:** This uses `std::sync::Mutex` + `std::sync::Condvar` (blocking primitives) inside an async task running on the tokio runtime. When the execution engine falls behind by `MAX_EXECUTED_GAP` blocks, this **blocks the entire OS thread** that the tokio worker is using. Under the default tokio multi-threaded runtime, this occupies one of a limited number of worker threads. If multiple tasks hit this path or if the thread pool is small, this leads to:

- **Thread pool exhaustion:** Other async tasks (including the execution pipeline that would *resolve* the backpressure) can starve, creating a **deadlock-like livelock**.
- **Cascading latency:** All other tasks scheduled on the blocked worker thread experience unbounded delays.

The consumer side (`mock.rs:232-236`) also uses `std::sync::Mutex`, but only briefly, so it's less dangerous — however, if the consumer task happens to be scheduled on the same blocked thread, a true deadlock occurs.

---

### 5. `process_epoch_change` Uses `assert_eq!` — Production Panic on Non-Sequential Epoch
**Location:** `mock.rs:270`

```rust
assert_eq!(self.epoch.load(std::sync::atomic::Ordering::SeqCst), *epoch - 1);
```

**Vulnerability:** If a `GravityEvent::NewEpoch` event arrives with a non-sequential epoch number (e.g., due to a bug in the execution engine, an event replay, or epoch skipping during recovery), the node **panics unconditionally**. This is:

- **Unrecoverable:** The node crashes and, on restart, may hit the same state and crash again (crash loop).
- **Externally triggerable:** If the execution engine or event source can be influenced to emit an out-of-sequence epoch event, this becomes a remote denial-of-service vector against the consensus node.
- **Missing in `#[cfg(debug_assertions)]`:** `assert_eq!` compiles into release builds. This is not a debug-only check.

---

### 6. TOCTOU Race on Epoch Check in Ordering Task
**Location:** `mock.rs:142-152`

```rust
let mut current_epoch = epoch.load(std::sync::atomic::Ordering::SeqCst);
loop {
    if current_epoch != epoch.load(std::sync::atomic::Ordering::SeqCst) {
        current_epoch = epoch.load(std::sync::atomic::Ordering::SeqCst);
        // ... reset mempool, reload block_number ...
    }
    block_number += 1;
    // ... construct block with current_epoch ...
```

**Vulnerability:** The epoch is checked via two separate atomic loads (lines 144–145). Between the comparison and the reload, the epoch could change again (e.g., two rapid epoch changes). More critically, the epoch is checked at the **top of the loop**, but `check_and_construct_block` (which can take >1 second due to polling) runs with the **stale** `current_epoch`. If an epoch change occurs during block construction:

- The block is constructed with the old epoch number
- `set_ordered_blocks` may silently drop it (old-epoch block) or the execution engine processes it under the wrong epoch
- The `block_number` may be wrong because `epoch_start_block_number` was updated by the main loop concurrently

This is a classic TOCTOU: the epoch is checked, then used non-atomically across an `await` boundary.

---

## MEDIUM

### 7. `Mempool::reset_epoch` Discards Nonce Tracking — Post-Epoch Transaction Replay
**Location:** `mempool.rs:22-24`, called from `mock.rs:148`

```rust
pub fn reset_epoch(&mut self) {
    self.next_sequence_numbers.clear();
}
```

**Vulnerability:** On epoch change, the entire nonce tracking map is cleared. Since `commit_txns` is a no-op (finding #2), the underlying `TxPool` still contains all previously seen transactions. After the reset, `get_txns` will accept transactions starting from nonce 0 for every account, meaning **every uncommitted transaction in the pool becomes eligible for re-inclusion**, including transactions that were already committed in the previous epoch. This can lead to duplicate transaction execution if the execution engine doesn't independently enforce nonce ordering.

### 8. `MOCK_MAX_BLOCK_SIZE` Re-Read on Every Block — Runtime Manipulation
**Location:** `mock.rs:35-40`

```rust
fn get_max_txn_num() -> usize {
    std::env::var("MOCK_MAX_BLOCK_SIZE")
        .unwrap_or_else(|_| "7000".to_string())
        .parse()
        .unwrap_or(7000)
}
```

**Vulnerability:** Unlike `MOCK_SET_ORDERED_INTERVAL_MS` and `MAX_EXECUTED_GAP` (which use `OnceLock`), `MOCK_MAX_BLOCK_SIZE` is re-read from the environment on **every block construction**. An attacker who can modify the process environment at runtime (e.g., via `/proc/<pid>/environ` manipulation on Linux, or a co-located process) can:
- Set it to `0` → blocks are produced with zero transactions, halting chain progress
- Set it to `usize::MAX` → a single block attempts to include all pool transactions, causing memory spikes and execution timeouts
- Oscillate the value to create unpredictable block sizes

### 9. Arithmetic Underflow in Backpressure Check
**Location:** `mock.rs:176`

```rust
let large_gap = block_number - *executed_number;
```

**Vulnerability:** If `executed_number` somehow exceeds `block_number` (e.g., due to the epoch TOCTOU race where `block_number` is reset but `executed_number` retains the old value), this unsigned subtraction **wraps to `u64::MAX`**, causing the backpressure `while` loop to spin indefinitely (since `u64::MAX > MAX_EXECUTED_GAP`). The `Condvar::wait` would never be satisfied because no execution result will produce a block number large enough to close the gap. This results in a **permanent thread hang**.

---

## Summary

| # | Severity | Finding | Root Cause |
|---|----------|---------|------------|
| 1 | **Critical** | Mock consensus activatable in production | No compile-time or runtime guard on `MOCK_CONSENSUS` env var |
| 2 | **Critical** | `commit_txns` is a no-op — txns never removed from pool | Empty method body; `TxPool::remove_txns` never called |
| 3 | **Critical** | Non-cryptographic 64-bit Block ID in 256-bit field | `DefaultHasher` (SipHash) + 24 zero-padded bytes |
| 4 | **High** | `std::sync::Mutex` blocks tokio worker threads | Blocking sync primitives in async context |
| 5 | **High** | `assert_eq!` panics node on non-sequential epoch | Debug assertion in release code path |
| 6 | **High** | TOCTOU race on epoch transitions | Non-atomic epoch check + async block construction |
| 7 | **Medium** | Nonce tracking reset enables post-epoch tx replay | `reset_epoch` clears map while pool retains txns |
| 8 | **Medium** | `MOCK_MAX_BLOCK_SIZE` mutable at runtime | No `OnceLock` caching; re-read per block |
| 9 | **Medium** | Unsigned underflow in backpressure arithmetic | `block_number - executed_number` with no underflow guard |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Mock Consensus Security Audit — Attack Findings

---

## C | 141954ms |
