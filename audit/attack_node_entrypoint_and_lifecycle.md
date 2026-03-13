# attack_node_entrypoint_and_lifecycle

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 71845ms
- **Steps**: 1

## Report

# Intention Attack Report: `gravity_node` Security Audit

## Critical Findings

### 1. **CRITICAL — Shutdown Signal Loss via `resubscribe()` Creates Potential Infinite Hang**

**Files:** `reth_cli.rs:323`, `reth_cli.rs:378`, `reth_cli.rs:449`

All three core loops (`start_execution`, `start_commit_vote`, `start_commit`) call `self.shutdown.resubscribe()` **on every loop iteration**, creating a fresh receiver each time. A fresh receiver created via `resubscribe()` only sees **future** messages — it will not see a shutdown signal that was already sent.

**Attack scenario:** If the shutdown signal is sent during the window between the `resubscribe()` call and the `tokio::select!` branch registration, or while the loop is between iterations (e.g., processing a block), the new receiver **misses the signal entirely**. The loop then blocks indefinitely on `get_ordered_blocks()`, `recv_compute_res()`, or `get_committed_blocks()`.

The only backstop is that the tokio runtime is eventually dropped when `main()` exits (line 305), which cancels tasks. However, this relies on the main `shutdown_rx` at line 302-303 receiving the signal — creating a single point of failure. If for any reason the main `block_on` doesn't exit cleanly, the node hangs forever.

**Fix:** Clone the receiver once and reuse it, or use a `tokio::sync::watch` / `CancellationToken` instead of broadcast for shutdown signaling.

---

### 2. **CRITICAL — Spawned Task Panics Crash the Node Silently with No Recovery**

**File:** `reth_coordinator/mod.rs:42-52`

All three critical loops are spawned with `tokio::spawn` followed by `.unwrap()`:

```rust
tokio::spawn(async move {
    reth_cli.start_execution().await.unwrap();
});
```

If any of these tasks returns `Err(...)`, the `.unwrap()` panics **inside a spawned task**. Tokio's default behavior for spawned task panics is to silently abort that task (the JoinHandle is dropped, so the panic is swallowed). The other loops continue running, creating a **split-brain state**:

- If `start_execution` panics: blocks are ordered but never executed; the commit vote loop starves waiting for execution results.
- If `start_commit_vote` panics: blocks execute but compute results are never propagated to the block buffer manager; committed blocks stall.
- If `start_commit` panics: blocks execute and vote but are **never committed to persistent storage**. The node appears operational but **no state is being finalized**. On restart, all uncommitted blocks are lost.

**Severity amplifier:** There is no health check, watchdog, or cross-task liveness monitoring. The node can run in this degraded state indefinitely without any operator visibility.

---

### 3. **HIGH — `MOCK_CONSENSUS` Environment Variable Bypasses Real Consensus in Production**

**File:** `main.rs:270`

```rust
if std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap()
```

Setting `MOCK_CONSENSUS=true` replaces the entire consensus engine with `MockConsensus`. There is **no guard** against this being set in a production environment — no warning log, no config file cross-check, no compile-time feature gate.

**Attack scenario:** An attacker with access to the process environment (container escape, CI pipeline injection, shared hosting misconfiguration) sets `MOCK_CONSENSUS=true`. The node now runs with mock consensus, meaning it:
- Accepts blocks without real validator signatures
- Bypasses all BFT safety guarantees
- Potentially allows arbitrary block injection

This is a **single environment variable away from disabling all consensus safety**.

---

### 4. **HIGH — `std::env::set_var` After Thread Spawning = Undefined Behavior**

**File:** `main.rs:72-74`

```rust
if std::env::var_os("RUST_BACKTRACE").is_none() {
    std::env::set_var("RUST_BACKTRACE", "1");
}
```

At this point in execution, at least two threads have already been spawned:
1. The pprof profiler thread (line 225-226, runs before `run_reth` at line 253)
2. The signal handler thread (line 235)

`std::env::set_var` is **not thread-safe** per POSIX. Concurrent reads of the environment (which any thread can do implicitly, e.g., via libc locale lookups, DNS resolution, or tracing initialization) while `set_var` modifies it is **undefined behavior**. This is UB in Rust ≥1.66 editions and has caused real-world segfaults in production systems.

---

### 5. **HIGH — `Address::ZERO` Coinbase Fallback Silently Misdirects Block Rewards**

**File:** `reth_cli.rs:142-181`

When the proposer index is missing or not found in the map, the coinbase falls back to `Address::ZERO`:

```rust
return Address::ZERO;
```

This means block rewards (base fees, priority fees) are **sent to the zero address** — effectively burned. This happens silently with only a `warn!` log.

**Attack scenario:** If an attacker can influence the `proposer_reth_map` (e.g., by corrupting the epoch manager data or submitting a malformed validator set update), they can cause an entire epoch's worth of block rewards to be burned. Since the block still executes successfully, this corruption is **not caught by consensus** — the execution result hash includes the zero-address coinbase, and all honest nodes would agree on the same wrong coinbase.

**Impact:** Permanent, irrecoverable loss of block rewards with no on-chain indication of error.

---

### 6. **HIGH — TxnCache Unbounded Growth / Memory Exhaustion**

**Files:** `mempool.rs:163`, `reth_cli.rs:198`

The `TxnCache` (`DashMap<[u8; 32], Arc<ValidPoolTransaction>>`) grows on every call to `best_txns()` (line 163: `txn_cache.insert(tx_hash, pool_txn)`), but entries are only removed in `push_ordered_block` when a matching transaction is found in an ordered block (line 198: `self.txn_cache.remove(&key)`).

**Attack scenario:** If transactions enter the mempool cache via `best_txns()` but are never included in an ordered block (e.g., they expire, are replaced, or are filtered by consensus), their cache entries are **never evicted**. Over time, this causes unbounded memory growth. An attacker can flood the mempool with valid-but-low-priority transactions that are cached but never committed, eventually causing OOM.

There is no max-size limit, no TTL eviction, and no periodic cleanup on the `TxnCache` itself (the `CachedBest` TTL only controls the iterator, not the cache).

---

### 7. **MEDIUM — Profiler Writes to CWD with Debug-Formatted Filenames**

**File:** `main.rs:202`

```rust
let proto_path = format!("profile_{count}_proto_{formatted_time:?}.pb");
```

The `:?` (Debug) format on a `String` produces filenames with **escaped quotes**: `profile_0_proto_"042".pb`. This creates files with literal quote characters in the name, which:
- Can break shell scripts, monitoring tools, and log parsers
- May cause path traversal issues on certain filesystems
- Makes cleanup automation fragile

Additionally, `formatted_time` only captures milliseconds (`time.millisecond()`, range 0–999), not the full timestamp. Files from different profiling sessions at the same millisecond-of-second will **silently overwrite** each other, causing profile data loss.

---

### 8. **MEDIUM — Broadcast Channel Capacity of 1 with Fire-and-Forget Send**

**File:** `main.rs:231, 248`

```rust
let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
// ...
let _ = shutdown_tx_clone.send(());
```

The return value of `send()` is discarded with `let _ =`. If all receivers have been dropped (e.g., due to a panic in `run_reth` or the coordinator), the shutdown signal is silently lost. Combined with finding #1 (resubscribe races), this creates a scenario where a **second signal** (e.g., operator sends SIGTERM twice) cannot be delivered because the channel is already full and receivers have lagged.

---

### 9. **MEDIUM — `assert_eq!` in `start_commit` Panics the Commit Loop on Block ID Mismatch**

**File:** `reth_cli.rs:469`

```rust
assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id);
```

This assertion runs in the commit loop. If there is **any** block ID mismatch between the pipe API and the block buffer manager (e.g., due to a race condition during epoch change, or a corrupted block buffer), this `assert_eq!` panics, killing the commit task. Per finding #2, this panic is silently swallowed, and the node enters a state where blocks are executed but never committed.

A blockchain node should **never** use `assert_eq!` on data derived from external/consensus sources in a production loop. This should return an error that triggers a controlled shutdown or recovery.

---

## Summary

| # | Severity | Finding |
|---|----------|---------|
| 1 | **Critical** | Shutdown signal loss via `resubscribe()` — potential infinite hang |
| 2 | **Critical** | Spawned task panics silently swallowed — split-brain node state |
| 3 | **High** | `MOCK_CONSENSUS` env var disables all consensus safety with no guard |
| 4 | **High** | `std::env::set_var` after thread spawn — undefined behavior |
| 5 | **High** | `Address::ZERO` coinbase fallback burns block rewards irrecoverably |
| 6 | **High** | `TxnCache` unbounded growth — OOM via mempool flooding |
| 7 | **Medium** | Profiler writes with debug-formatted filenames + timestamp collision |
| 8 | **Medium** | Broadcast send result silently discarded — shutdown signal can be lost |
| 9 | **Medium** | `assert_eq!` in commit loop — consensus data mismatch kills commit silently |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: `gravity_node` Security Audit

## | 71844ms |
