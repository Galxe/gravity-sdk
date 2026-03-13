# attack_mempool_transaction_pool

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 73432ms
- **Steps**: 1

## Report

# Security Audit Report: `bin/gravity_node/src/mempool.rs`

## Attack Surface Analysis — Fatal Logic Vulnerabilities

---

### CRITICAL-01: Unbounded `TxnCache` Memory Growth — Denial of Service

**Severity: Critical**
**Location:** `mempool.rs:162-163`, `mempool.rs:243-259`

**Finding:** The `txn_cache` (`DashMap<[u8;32], Arc<ValidPoolTransaction>>`) grows without bound. Every transaction yielded by `best_txns` is inserted (line 163), but `remove_txns` (line 243-259) **never removes entries from `txn_cache`** — it only removes from the underlying reth pool.

The only place `txn_cache` entries are removed is in `reth_cli.rs:198` during `push_ordered_block`, where entries are consumed via `self.txn_cache.remove(&key)`. However, this only removes transactions that are actually included in committed blocks. Transactions that are:
- Yielded by `best_txns` but never included in a block (e.g., dropped due to gas price, replaced by newer txns)
- Superseded by replacement transactions (same sender, same nonce, higher gas)
- From senders whose accounts are drained

...will **remain in the DashMap forever**, leaking memory proportional to total transaction throughput. On a high-throughput chain, this is a guaranteed OOM vector.

**Attack scenario:** An attacker submits thousands of transactions, then replaces them all with higher-gas-price replacements. The originals remain cached indefinitely. Repeated cycles exhaust node memory.

---

### CRITICAL-02: `add_external_txn` Returns `true` Before Pool Insertion — State Desynchronization

**Severity: Critical**
**Location:** `mempool.rs:206-241`

**Finding:** `add_external_txn` decodes and recovers the signer synchronously, then spawns an async task (line 223) to actually insert the transaction into the reth pool, and immediately returns `true` (line 234) **before the insertion completes or is even attempted**.

```rust
self.runtime.spawn(async move {
    let res = pool.add_external_transaction(pool_txn).await;
    if let Err(e) = res { tracing::error!(...); }
});
true  // returned before spawn completes
```

**Consequences:**
1. The caller (consensus/VFN forwarding) believes the transaction was accepted and may propagate this confirmation upstream, but the reth pool could reject it (duplicate nonce, insufficient balance, gas limit exceeded, pool full).
2. There is no backpressure mechanism — if the reth pool is congested or full, the fire-and-forget pattern silently drops transactions while reporting success.
3. In a Validator Full Node (VFN) forwarding scenario (confirmed by `e2e` test in recent commits), this means the forwarding node reports success to the user while the transaction may never enter the mempool.

---

### HIGH-01: Filtered Transactions Poison Nonce Tracking — Transaction Censorship

**Severity: High**
**Location:** `mempool.rs:141-158`

**Finding:** In `best_txns`, the nonce is recorded in `last_nonces` at line 151 **before** the filter is applied at lines 154-158. If a transaction passes the nonce-ordering check but is rejected by the filter, its nonce is still recorded:

```rust
// Line 151: nonce recorded BEFORE filter
last_nonces.insert(sender, nonce);

// Lines 154-158: filter applied AFTER nonce recording
if let Some(ref f) = filter {
    if !f((sender.clone(), nonce, hash)) {
        return None;  // filtered out, but nonce already advanced
    }
}
```

This means for sender `A` with nonces `[5, 6, 7]`: if nonce 6 is filtered out, `last_nonces[A] = 6`. Nonce 7 passes the check (`7 == 6 + 1`). But the block will contain nonces 5 and 7 **without 6**, which will fail execution on-chain since EVM enforces strict nonce sequencing.

**Impact:** The filter (used by consensus for deduplication) can create "impossible" transaction sequences that pass mempool validation but fail at execution, wasting block space. Worse: because `last_nonces` persists across calls within the TTL window, a single filtered transaction can permanently block all subsequent transactions from that sender until the cache expires.

---

### HIGH-02: `std::sync::Mutex` in Async Context — Potential Deadlock Under Load

**Severity: High**
**Location:** `mempool.rs:67`, `mempool.rs:126`

**Finding:** `cached_best` uses `std::sync::Mutex` (not `tokio::sync::Mutex`). The lock is acquired at line 126 and held during the entire iterator consumption chain including `.filter_map().take(limit).collect()` (lines 137-167).

The `filter_map` closure on each transaction performs:
- Hash computation (`pool_txn.hash()`)
- Address conversion
- Filter closure invocation
- EIP-2718 encoding
- DashMap insertion

If the reth `BestTransactions` iterator blocks or yields slowly (which it can under pool contention), this `std::sync::Mutex` blocks the OS thread. In an async context (tokio runtime), this blocks the entire tokio worker thread, potentially starving other async tasks including the consensus engine.

While `std::sync::Mutex` is acceptable for very short critical sections, this lock is held for an iteration of up to `limit` items with non-trivial per-item work, making it a scalability bottleneck and potential contributor to consensus timeouts under high transaction volumes.

---

### HIGH-03: `CachedBest` TTL Allows Stale/Duplicate Transaction Delivery

**Severity: High**
**Location:** `mempool.rs:59-61`, `mempool.rs:127-133`, `mempool.rs:136-169`

**Finding:** The `CachedBest` iterator is reused across multiple calls within the TTL window (default 1 second). The `last_nonces` map persists across these calls. This creates two problems:

1. **Stale iterator state:** Within the TTL window, the underlying reth pool may have changed (new transactions added, old ones removed, replacements). The cached `BestTransactions` iterator does not reflect these changes, potentially yielding transactions that have already been removed from the pool or missing higher-priority replacements.

2. **Cross-call nonce coupling:** If call 1 yields sender A's nonce 5, and call 2 (within TTL) checks sender A, it requires nonce 6. If nonce 6 was added between calls but the cached iterator was already past that position, nonce 6 is skipped. This can cause valid transactions to be systematically excluded until cache expiry.

3. **Environment variable manipulation:** `MEMPOOL_CACHE_TTL_MS` is read via `OnceLock` (line 29-37). Setting it to `0` makes the cache expire every call (performance issue), and setting it to `u64::MAX` effectively freezes the iterator permanently (liveness issue). No validation bounds.

---

### MEDIUM-01: `get_broadcast_txns` Has No Transaction Limit — Amplification Attack

**Severity: Medium**
**Location:** `mempool.rs:180-204`

**Finding:** Unlike `best_txns` which has a `limit` parameter, `get_broadcast_txns` calls `self.pool.all_transactions().all()` with no limit, collects everything into a `Vec`, and returns it. On a FullNode (`enable_broadcast == true`), this dumps the entire mempool contents.

If the reth pool contains thousands of transactions, each call:
- Allocates a `Vec` of all transactions
- EIP-2718 encodes every single one
- Performs address conversion for each

This can be triggered repeatedly by the caller (presumably the broadcast/gossip subsystem), creating a CPU and memory amplification vector. An attacker flooding the mempool with cheap transactions forces expensive serialization work on every broadcast cycle.

---

### MEDIUM-02: Hardcoded `chain_id: 0` in `VerifiedTxn` — Chain Replay Risk

**Severity: Medium**
**Location:** `mempool.rs:102`, `mempool.rs:115`

**Finding:** Both `to_verified_txn` (line 102) and `to_verified_txn_from_recovered_txn` (line 115) hardcode `chain_id: ExternalChainId::new(0)` regardless of the actual chain ID. The real chain ID is known and used elsewhere (e.g., `reth_cli.rs:104-107`).

If downstream consumers of `VerifiedTxn` rely on `chain_id` for replay protection validation or cross-chain checks, the hardcoded zero defeats that protection. Transactions signed for one chain could be treated as valid on another if only the `VerifiedTxn.chain_id` field is checked.

---

### MEDIUM-03: `tokio::runtime::Runtime::new().unwrap()` — Unguarded Panic in Constructor

**Severity: Medium**
**Location:** `mempool.rs:78`

**Finding:** `Mempool::new` creates a dedicated tokio runtime with `Runtime::new().unwrap()`. If runtime creation fails (e.g., OS thread/fd limits exhausted), this panics and crashes the entire node. This is constructed in the main startup path (main.rs:256), so a transient resource shortage at startup causes a hard crash with no recovery.

Additionally, creating a **separate** tokio runtime means `add_external_txn` spawns tasks on a different runtime than the main application. If the main runtime shuts down, the Mempool's runtime may still have in-flight tasks, creating an unclean shutdown path. Conversely, the Mempool runtime is never explicitly shut down.

---

### Summary

| ID | Severity | Finding | Impact |
|----|----------|---------|--------|
| CRITICAL-01 | Critical | `txn_cache` grows unbounded, never cleaned for non-included transactions | OOM / node crash |
| CRITICAL-02 | Critical | `add_external_txn` returns `true` before async pool insertion | Silent transaction loss, false confirmations |
| HIGH-01 | High | Filtered transactions poison nonce tracking | Invalid transaction sequences, sender censorship |
| HIGH-02 | High | `std::sync::Mutex` held during full iteration in async context | Thread starvation, consensus timeouts |
| HIGH-03 | High | Stale cached iterator + unvalidated TTL env var | Missed transactions, potential liveness freeze |
| MEDIUM-01 | Medium | `get_broadcast_txns` has no limit on returned transactions | CPU/memory amplification |
| MEDIUM-02 | Medium | Hardcoded `chain_id: 0` in all `VerifiedTxn` output | Bypassed replay protection |
| MEDIUM-03 | Medium | Unguarded panic on runtime creation + orphaned runtime on shutdown | Hard crash, unclean shutdown |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Security Audit Report: `bin/gravity_node/src/mempool.rs`

 | 73432ms |
