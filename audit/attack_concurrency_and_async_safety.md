# attack_concurrency_and_async_safety

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 271673ms
- **Steps**: 1

## Report

# Intention Attack Report — gravity-sdk Security Audit

## Attack Surface: Cross-Cutting Concurrency & Business Logic

---

## CRITICAL Severity

### C1: Unauthenticated Remote Fail-Point Injection (`/set_failpoint`)

**File:** `crates/api/src/https/mod.rs:107-116`

The `/set_failpoint` and `/mem_prof` endpoints are bound to `http_routes` with **zero authentication, zero TLS requirement, and zero IP allowlisting**. Any network-reachable attacker can POST arbitrary fail-points into a running validator node.

**Business Logic Flaw:** Fail-point injection is a *complete remote control primitive*. An attacker can:
- Halt consensus by injecting a fail-point on the block proposal path
- Corrupt execution state by triggering panics at chosen code locations
- Cause targeted denial-of-service at will

The `https_routes` group gets the `ensure_https` middleware, but these two endpoints were explicitly placed on the unprotected `http_routes` group. This is not a missing feature — it's an architectural misplacement that turns a debug tool into a weaponizable backdoor.

**Verdict:** Remote code-control without authentication. This is the single highest-priority fix.

---

### C2: `TxnCache` Unbounded Memory Growth — Exploitable OOM

**File:** `bin/gravity_node/src/mempool.rs:163` (insert), `bin/gravity_node/src/reth_cli.rs:198` (remove)

**The invariant violation:** Entries are inserted on every `best_txns()` call (per transaction yielded) but are *only* removed when a transaction is included in an executed block. There is no size limit, no TTL, no eviction, no background cleanup.

**Attack scenario:**
1. Attacker submits thousands of valid-gas-price transactions from many sender addresses.
2. Consensus calls `best_txns()` repeatedly during block proposals — each call inserts all candidate transactions into `TxnCache`.
3. If attacker's transactions are never selected for inclusion (e.g., they are outbid, or the attacker replaces them with new nonces in the reth pool), the reth pool internally evicts them, but **`TxnCache` never learns about the eviction**.
4. The DashMap grows monotonically. Each entry holds an `Arc<ValidPoolTransaction<EthPooledTransaction>>` — a full decoded transaction with signature and sender cached.
5. Node OOMs.

**Compounding factor:** `TxPool::remove_txns()` at line 243 removes transactions from the reth pool but **never calls `txn_cache.remove()`**. This is a second orphaning pathway for cache entries.

**Contrast:** The `txn_metrics` DashMaps in `crates/txn_metrics/src/lib.rs` have explicit `MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100_000` and age-based cleanup. The `TxnCache` has neither — the mitigation pattern was applied elsewhere but missed here.

---

### C3: Pipeline Reset Drain Has No Timeout — Permanent Node Freeze

**File:** `aptos-core/consensus/src/pipeline/buffer_manager.rs:444-446`

```rust
while self.ongoing_tasks.load(Ordering::SeqCst) > 0 {
    tokio::time::sleep(Duration::from_millis(10)).await;
}
```

During an epoch change, `reset()` busy-polls until all in-flight `CountedRequest` guards are dropped. **There is no timeout.** If any execution/signing/persisting task hangs, this loop spins forever.

**Business Logic Flaw — the chain of consequences:**
1. `BufferManager::start()` select loop is suspended (it `await`s `process_reset_request` inline at line 825).
2. No new ordered blocks are processed, no commit votes are aggregated.
3. The reset ack is never sent → epoch-change handler blocks indefinitely.
4. **The node is permanently frozen.** It cannot recover without a restart.
5. Blocks arriving during the drain (after the `try_next()` flush at line 441) are silently dropped with no retry mechanism.

**This converts any single hung pipeline task into a permanent liveness failure for the entire node.**

---

## HIGH Severity

### H1: Silent Consensus Death via Fire-and-Forget `tokio::spawn`

**~22 of 28 spawn sites** drop the `JoinHandle`. There is no `catch_unwind`, no `panic::set_hook`, no `JoinSet`, and no watchdog anywhere in the codebase.

**Most dangerous spawn sites:**

| Spawn Site | What Dies Silently |
|---|---|
| `main.rs:273` — `mock.run()` | Entire block production loop. Node runs but produces nothing. |
| `consensus_api.rs:334` — `consensus_mempool_handler.start()` | All consensus→mempool commit notifications. Mempool never GCs committed txns — memory grows, stale transactions are re-proposed. |
| `mempool.rs:223` — `pool.add_external_transaction()` | Transaction ingestion. Caller receives `true` (success) even when the spawned task panics. |

**Business Logic Flaw:** The node enters an **undetectable degraded state**. The tokio runtime stays alive, the process appears healthy, but critical subsystems are silently dead. There are no health checks, no restart mechanisms, and no metrics that surface the failure.

### H2: `add_external_txn` Returns `true` Regardless of Pool Outcome

**File:** `bin/gravity_node/src/mempool.rs:206-241`

The method spawns pool insertion as fire-and-forget and returns `true` immediately. The actual pool result (duplicate, underpriced, pool full, invalid) is logged and discarded.

**Business Logic Flaw — false success signal:**
- VFN transaction forwarding (added in commit `48135e64`) trusts this return value. A VFN that forwards a transaction and receives `true` believes submission succeeded, but the pool may have rejected it.
- No backpressure is ever communicated to callers. If the pool is full, callers keep submitting and keep receiving `true`.
- The `bool` return creates a **contract violation** where the documented semantics ("was the transaction added?") diverge from the actual semantics ("was the transaction decodable?").

---

## MEDIUM Severity

### M1: `start_commit_num` Cursor Advanced Before Persistence Confirmation

**File:** `bin/gravity_node/src/reth_cli.rs:470-496`

`start_commit_num = last_block.num + 1` is set **before** `wait_for_block_persistence` is called. If persistence fails after the cursor advances, the in-memory cursor has already skipped past blocks that were never persisted. On restart, those blocks are silently skipped — **committed state is lost without any record of the loss**.

### M2: Epoch Race in `start_commit_vote`

**File:** `bin/gravity_node/src/reth_cli.rs:432-436`

`current_epoch` is snapshot-loaded and passed to `set_compute_res`. The epoch can be updated concurrently by `start_execution` (line 341) in a separate task. A block's compute result could be attributed to the wrong epoch in the buffer manager.

### M3: `ensure_https` Middleware Checks URI Scheme, Not Transport

**File:** `crates/api/src/https/mod.rs:32-37`

The middleware checks `req.uri().scheme_str()` — this is the scheme the *client* puts in the request, not the actual transport layer. Behind a TLS-terminating proxy (standard in Kubernetes), plain HTTP requests with an `https` scheme pass this check. The middleware provides a false sense of transport security.

### M4: `cached_best` Blocking Mutex Held During Full Pool Iteration

**File:** `bin/gravity_node/src/mempool.rs:126-176`

A `std::sync::Mutex` (not `tokio::sync::Mutex`) is held while iterating the entire transaction pool, running filter closures, encoding transactions, and inserting into `txn_cache`. An attacker who floods the pool with thousands of transactions makes each `best_txns()` call block the consensus proposer's executor thread for tens of milliseconds, starving other async tasks.

### M5: Unbounded `get_all` Table Scan on Unauthenticated Endpoints

**File:** `crates/api/src/https/consensus.rs:117-136`

`/consensus/ledger_info/:epoch` and `/consensus/validator_count/:epoch` load the **entire** epoch-by-block-number table into memory on every request. These endpoints are unauthenticated with no rate limiting — a trivial DoS vector.

### M6: `GLOBAL_CONFIG_STORAGE.get().unwrap()` — Fragile Init Ordering

**File:** `bin/gravity_node/src/relayer.rs:110`

This `.unwrap()` panics with no diagnostic message if called before `ConsensusEngine::init()` completes. The safety relies entirely on implicit call-site ordering — no compile-time or runtime guard exists. Any refactor that changes task spawn order causes an immediate node panic.

---

## Consolidated Remediation Priority

| Priority | Finding | Fix |
|---|---|---|
| **P0** | C1: Unauthenticated `/set_failpoint` | Move to `https_routes`, add auth, or remove from non-debug builds |
| **P0** | C2: Unbounded `TxnCache` | Add capacity cap + eviction on reth pool removal events |
| **P0** | C3: Reset drain no timeout | Add bounded timeout with forced cancellation and error escalation |
| **P1** | H1: Fire-and-forget spawns | Store `JoinHandle`s, add panic hooks, implement critical-task watchdog |
| **P1** | H2: False `true` return | Await pool result before returning, or change return type to signal async |
| **P1** | M1: Cursor before persistence | Move `start_commit_num` advance to after persistence confirmation |
| **P2** | M2: Epoch race | Atomic read-and-use or pass epoch through the compute result channel |
| **P2** | M3: `ensure_https` bypass | Enforce TLS at bind/socket level, not URI scheme |
| **P2** | M4: Blocking mutex | Use `tokio::sync::Mutex` or `spawn_blocking` |
| **P2** | M5: Unbounded scan | Index-based lookup + rate limiting on endpoints |
| **P2** | M6: OnceLock ordering | Replace `.unwrap()` with `.get().expect("descriptive message")` or return `Result` |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report — gravity-sdk Security Audit

## A | 271673ms |
