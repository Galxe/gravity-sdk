# attack_txn_metrics_and_bench_binary

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 78714ms
- **Steps**: 1

## Report

# Intention Attack Report — Security Audit Findings

## Scope

- `crates/txn_metrics/src/lib.rs` — Transaction lifecycle metrics singleton
- `bin/bench/` — Benchmark harness (main.rs, txn.rs, kv.rs, stateful_mempool.rs)

---

## Critical Findings

### CRITICAL-1: DashMap Deadlock in `cleanup_old_entries` — Nested Iteration Causes Potential Livelock

**File:** `crates/txn_metrics/src/lib.rs:519-521`

```rust
self.txn_batch_id.retain(|_batch_id, txn_set| {
    txn_set.retain(|key| self.txn_initial_add_time.contains_key(key));
    !txn_set.is_empty()
});
```

The `retain()` closure on `txn_batch_id` calls `self.txn_initial_add_time.contains_key()` **while holding a shard lock on `txn_batch_id`**. DashMap's `retain` acquires shard locks sequentially. If another thread is simultaneously executing `record_committed()` — which calls `self.txn_batch_id.iter_mut()` (line 479) while also touching `txn_initial_add_time.remove()` (line 473) — you get a lock-ordering inversion between the two DashMaps. Under high contention, this can cause **deadlock or indefinite stall of the consensus commit path**.

The identical pattern exists at lines 528-531 with `txn_block_id` + `txn_initial_add_time`.

**Impact:** Consensus node hangs. The commit path (`record_committed`) and the cleanup path (`cleanup_old_entries`, triggered from `record_added`) race against each other on two different DashMap lock orderings. Since `record_committed` is called per-transaction on the critical commit path, a deadlock here halts block finalization.

---

### CRITICAL-2: `record_committed` Silently Drops Metrics for Untracked Transactions — Commits Without Audit Trail

**File:** `crates/txn_metrics/src/lib.rs:459-489`

```rust
pub fn record_committed(&self, sender: &AccountAddress, sequence_number: u64) {
    // ...
    let txn_key = (*sender, sequence_number);
    if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
        // observe metric
    }
    // Unconditionally removes from ALL maps regardless of whether metric was recorded
    self.txn_initial_add_time.remove(&txn_key);
    self.txn_hash_to_key.retain(|_hash, key| key != &txn_key);
    // ...
}
```

If `cleanup_old_entries` evicts a transaction from `txn_initial_add_time` (entries >60s old, line 502), a subsequent `record_committed` call for that transaction will:
1. **Not record the committed latency** (the `if let Some` fails silently)
2. **Still perform expensive O(n) cleanup** of `txn_hash_to_key`, `txn_batch_id`, and `txn_block_id`

This means **slow transactions (>60s) are systematically excluded from commit latency metrics**. The Prometheus histogram will show an artificially healthy latency distribution — the worst-case transactions are never counted. This is the **opposite of the stated intention**: the system was designed to track transaction lifecycle latency, but it specifically loses visibility on the transactions that matter most (the slow ones).

**Impact:** Operators relying on `aptos_txn_added_to_committed_time_seconds` will see a false-healthy latency picture. Actual P99/P100 degradation from slow transactions becomes invisible.

---

## High Findings

### HIGH-1: `record_added` Capacity Check is Racy — Concurrent Inserts Bypass Memory Limits

**File:** `crates/txn_metrics/src/lib.rs:213-222`

```rust
if self.txn_initial_add_time.len() >= MAX_TXN_INITIAL_ADD_TIME_CAPACITY {
    self.cleanup_old_entries();
}
// No re-check — insert proceeds unconditionally
self.txn_hash_to_key.insert(txn_hash, txn_key);
self.txn_initial_add_time.entry(txn_key).or_insert(now);
```

The capacity check and the insert are **not atomic**. Under high-throughput concurrent `record_added` calls (N validator threads processing mempool additions simultaneously):
1. Thread A checks `len() >= 100,000` → false (99,999 entries)
2. Threads B, C, D, ... all also check → false
3. All threads proceed to insert → map grows well beyond 100,000

Since `DashMap::len()` is also an approximation (it sums shard lengths non-atomically), the check can be stale even without race conditions. The 100,000 limit is effectively advisory, not enforced.

**Impact:** Under sustained load, `txn_initial_add_time` can grow unboundedly. Combined with Finding CRITICAL-1 (cleanup can deadlock), the fallback eviction path may never execute, making this a memory exhaustion vector.

### HIGH-2: `txn_hash_to_key` Grows Without Independent Bounds — Asymmetric Memory Exhaustion

**File:** `crates/txn_metrics/src/lib.rs:218`

`txn_hash_to_key` is inserted into at lines 218, 236, 368, and 396 — every time a transaction is seen in **any** lifecycle event. But it is only cleaned during `record_committed` (O(n) retain) or as a side-effect of `cleanup_old_entries` (also O(n) retain). There is **no independent capacity limit**.

An attacker can exploit this: submit many transaction variants with the **same `(sender, sequence_number)`** but different payloads. Each variant produces a different `committed_hash()` → a new entry in `txn_hash_to_key`. But `txn_initial_add_time` uses `entry().or_insert()` so it doesn't grow. The capacity check on `txn_initial_add_time` never triggers, and `txn_hash_to_key` grows without bound.

**Impact:** Controllable memory exhaustion on any node with `TXN_LIFE_ENABLED=true`. Each entry is `HashValue (32 bytes) + AccountAddress (32 bytes) + u64 (8 bytes) + DashMap overhead ≈ ~120 bytes`. 10M resubmissions ≈ 1.2 GB of leaked memory.

### HIGH-3: Quadratic Complexity in Commit Path — O(n × m) Performance DoS

**File:** `crates/txn_metrics/src/lib.rs:476-488`

```rust
self.txn_hash_to_key.retain(|_hash, key| key != &txn_key);  // O(m)
for mut entry in self.txn_batch_id.iter_mut() {               // O(batches)
    entry.value_mut().remove(&txn_key);                        // O(1) per set
}
self.txn_batch_id.retain(|_batch_id, txn_set| !txn_set.is_empty()); // O(batches)
for mut entry in self.txn_block_id.iter_mut() {               // O(blocks)
    entry.value_mut().remove(&txn_key);
}
self.txn_block_id.retain(|_block_id, txn_set| !txn_set.is_empty()); // O(blocks)
```

`record_committed` is called **per committed transaction**. Each call iterates the **entire** `txn_hash_to_key` map. If a block commits 1,000 transactions and `txn_hash_to_key` has 100,000 entries, this is **100 million iterations per block** just for this one map. The batch and block map iterations add further linear overhead.

**Impact:** On a validator under sustained load, the commit path latency grows quadratically with map size. Since metrics collection runs on the consensus critical path (not async/background), this directly degrades block finalization throughput. An attacker inflating `txn_hash_to_key` (via HIGH-2) amplifies this into a **consensus-level performance DoS**.

---

## Medium Findings

### MEDIUM-1: `into_verified()` Constructs "Verified" Transactions Without Any Verification

**File:** `bin/bench/src/txn.rs:42-52`

```rust
pub fn into_verified(self) -> VerifiedTxn {
    let bytes = self.to_bytes();
    let hash = simple_hash::hash_to_fixed_array(&bytes);
    VerifiedTxn::new(bytes, self.account, self.sequence_number, ExternalChainId::new(0), TxnHash::new(hash))
}
```

This constructs a `VerifiedTxn` (a type whose name implies cryptographic verification has occurred) with:
- **No signature check**
- **Hardcoded `ExternalChainId::new(0)`** — not the bench chain ID (1337) or any real chain
- **Hash computed from JSON serialization** — not from the canonical transaction encoding

The `VerifiedTxn::new()` constructor evidently does not enforce verification at the type level, meaning **any code path that trusts `VerifiedTxn` as "already verified" can be bypassed** by constructing one through this pattern.

**Risk:** If `VerifiedTxn::new()` is accessible outside test code (it's a public constructor in `gaptos`), any crate in the dependency tree can forge "verified" transactions. The type provides no verification guarantee — it's a naming lie that could propagate into production.

### MEDIUM-2: Unauthenticated HTTP Endpoint Controlling Consensus Behavior

**File:** `bin/bench/src/main.rs:139-143`

```rust
let route = warp::path!("ProduceTxn")
    .and(warp::query::<ProduceTxnQuery>())
    .and_then(handle_produce_txn);
warp::serve(route).run(([0, 0, 0, 0], port.expect("No port"))).await;
```

Binds to `0.0.0.0` with no authentication. The endpoint toggles transaction production on/off for a node running the **real `ConsensusEngine`**. Any network-reachable actor can disable transaction production, effectively taking the node offline from a consensus participation perspective.

### MEDIUM-3: `pending_txns` Hardcodes `account_seq_num: 1` — Sequence Number Tracking is Broken

**File:** `bin/bench/src/stateful_mempool.rs:70`

```rust
txns.push(VerifiedTxnWithAccountSeqNum { txn, account_seq_num: 1 })
```

Every transaction returned from `pending_txns()` reports sequence number 1 regardless of actual state. If this mempool were connected (the `ExecutionChannel` impl is commented out but structurally present), it would cause the consensus layer to believe all accounts are at sequence 1, breaking replay protection and nonce ordering entirely.

---

## Summary

| ID | Severity | Component | Finding |
|---|---|---|---|
| CRITICAL-1 | **Critical** | txn_metrics | DashMap lock-ordering inversion between `cleanup_old_entries` and `record_committed` — potential deadlock on consensus commit path |
| CRITICAL-2 | **Critical** | txn_metrics | Transactions >60s silently dropped from commit metrics — creates false-healthy latency picture (opposite of stated intention) |
| HIGH-1 | **High** | txn_metrics | Non-atomic capacity check allows unbounded growth under concurrent inserts |
| HIGH-2 | **High** | txn_metrics | `txn_hash_to_key` has no capacity limit — controllable memory exhaustion via transaction resubmission |
| HIGH-3 | **High** | txn_metrics | O(n×m) quadratic complexity in `record_committed` — consensus-path performance DoS |
| MEDIUM-1 | **Medium** | bin/bench | `VerifiedTxn::new()` constructs "verified" transactions without verification — type-level safety violation |
| MEDIUM-2 | **Medium** | bin/bench | Unauthenticated `0.0.0.0` HTTP endpoint controls consensus transaction production |
| MEDIUM-3 | **Medium** | bin/bench | Hardcoded `account_seq_num: 1` breaks sequence number tracking |

**Key Insight:** The two Critical findings are especially dangerous in combination. CRITICAL-2 means the metrics system is *designed* to silently discard the most operationally important data (slow transactions), while CRITICAL-1 means the cleanup mechanism that causes this data loss can also deadlock the consensus commit path. The metrics system, intended as a passive observer, becomes an active threat to node liveness and observability correctness.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report — Security Audit Findings

## Scop | 78714ms |
