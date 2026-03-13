# attack_shared_crates_and_dependencies

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 137086ms
- **Steps**: 1

## Report

# Intention Attack Report: Shared Library Crates — Fatal Logic Vulnerabilities

---

## CRITICAL Severity

### 1. Potential Deadlock in `txn_metrics` Due to Lock Ordering Inversion
**File:** `crates/txn_metrics/src/lib.rs`

`cleanup_old_entries` and `record_committed` acquire DashMap shard locks in **opposite order**:

| Thread A (`record_committed`) | Thread B (`cleanup_old_entries`) |
|---|---|
| Write lock `txn_initial_add_time` shard M (via `.remove()`) | Write lock `txn_batch_id` shard N (via `.retain()`) |
| Then write lock `txn_batch_id` shard N (via `.iter_mut()`) | Then read lock `txn_initial_add_time` shard M (via `.contains_key()` inside retain closure) |

This is a **textbook ABBA deadlock**. If Thread A holds shard M of `txn_initial_add_time` and waits for shard N of `txn_batch_id`, while Thread B holds shard N of `txn_batch_id` and waits for shard M of `txn_initial_add_time`, both threads are permanently blocked. The same inversion exists for the `txn_block_id` ↔ `txn_initial_add_time` pair. Under high transaction throughput, this is a livelock/deadlock waiting to happen.

---

### 2. `todo!()` Panics in Production Consensus Pipeline
**File:** `crates/api/src/consensus_mempool_handler.rs`

Two `todo!()` macros exist in the consensus-to-mempool notification handler — the **critical path** for block commitment:

- **Line 41:** If `mempool_notification_sender.notify_new_commit()` returns an error, the handler calls `todo!()` → **unconditional panic**. Any transient mempool communication failure (backpressure, channel closed, timeout) crashes the node.
- **Line 112:** If a `SyncForDuration` consensus notification is received, the handler calls `todo!()` → **unconditional panic**. If this variant is ever emitted by the consensus layer, the node crashes immediately.

These are in an async task loop with no restart/supervisor logic — a panic here kills the consensus notification pipeline permanently for the running node.

---

### 3. Cascading `unwrap()` Panics on Event Notification Failure
**File:** `crates/api/src/consensus_mempool_handler.rs`

- **Line 85:** `event_subscription_service.notify_events(block_number, events).unwrap()` — panics if event notification fails.
- **Line 104:** `notify_initial_configs(...).unwrap()` — panics if initial config notification fails during `SyncToTarget`.

Both are on the consensus commit path. A single failed event subscriber (e.g., a slow downstream consumer or a dropped channel) will crash the entire node via panic propagation. There is no graceful degradation.

---

## HIGH Severity

### 4. Timestamp Unit Mismatch — Seconds Passed as Microseconds
**File:** `crates/api/src/consensus_mempool_handler.rs`, line 79

```rust
SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
//                                                   ^^^^^^^^^ seconds
```

This value is passed to `notify_new_commit` as the `block_timestamp_usecs` parameter, which expects **microseconds**. The timestamp is off by a factor of **1,000,000×**. Any downstream logic that uses this timestamp for:
- Transaction expiration (TTL checks)
- Replay protection windows
- Time-based ordering or deduplication

...will compute wildly incorrect durations. A transaction that should expire in 30 seconds would appear to expire in ~34 days. A TODO comment acknowledges the bug but it remains unfixed.

### 5. Race Condition in `txn_metrics`: Just-Inserted Transaction Can Lose Metrics
**File:** `crates/txn_metrics/src/lib.rs`

In `record_added`, the insert into `txn_hash_to_key` (step B) happens before the insert into `txn_initial_add_time` (step C). A concurrent `cleanup_old_entries` running between steps B and C will:
1. See the key in `txn_hash_to_key`
2. Check `txn_initial_add_time.contains_key()` → returns `false` (not yet inserted)
3. Remove the key from `txn_hash_to_key`

The key is now orphaned: it exists in `txn_initial_add_time` but has no mapping in `txn_hash_to_key`. All subsequent batch/block/proof metrics for this transaction are silently lost.

Similarly, `record_committed` does a non-atomic two-pass cleanup on `txn_batch_id` (first `iter_mut` to remove, then `retain` to drop empties). Between the passes, a concurrent `record_batch` can re-insert the committed key, leaving stale data permanently in the map.

### 6. Silent Error Swallowing on `SyncToTarget` Response
**File:** `crates/api/src/consensus_mempool_handler.rs`, lines 105–108

```rust
let _ = self.consensus_notification_listener
    .respond_to_sync_target_notification(sync_notification, Ok(()))
    .map_err(|e| anyhow::anyhow!(e));
```

The error is converted via `map_err` and then **immediately discarded** via `let _`. If the response fails to reach the consensus layer, consensus believes the sync target was never acknowledged. This can cause consensus to stall, retry indefinitely, or make incorrect assumptions about synchronization state — with zero log output to diagnose the failure.

### 7. `InfallibleRwLock` Poison Propagation Is Fatal and Unrecoverable
**File:** `crates/proposer-reth-map/src/lib.rs`

The `PROPOSER_RETH_ADDRESS_MAP` uses `InfallibleRwLock`, which panics on lock poisoning rather than returning a `Result`. If any thread panics while holding the write lock (e.g., during `update_proposer_reth_index_map` if the `ValidatorSet` iteration hits a panic), **every subsequent call** to `get_reth_address_by_index` from any thread will also panic. Since this map is queried on the block proposal path, a single poison event permanently disables block production for the node with no recovery path short of a restart.

---

## MEDIUM Severity

### 8. Unbounded Capacity Growth in `txn_metrics`
**File:** `crates/txn_metrics/src/lib.rs`

The capacity limits (`MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100,000`, etc.) are **advisory, not enforced**:
- The check-then-act pattern is non-atomic — concurrent `record_added` calls all pass the `>=` check simultaneously and all insert.
- `cleanup_old_entries` only removes entries older than 60s. If all 100,000 entries are fresh, zero are evicted and the map grows unbounded.
- `txn_block_id` has **no capacity check at insert time** in `record_block` — only the indirect cleanup from `record_added`/`record_batch` protects it.

Under sustained high throughput, memory consumption grows without bound.

### 9. Build Metadata Spoofing via Runtime Environment Variables
**File:** `crates/build-info/src/lib.rs`

`GIT_SHA`, `GIT_BRANCH`, `GIT_TAG`, and `BUILD_DATE` are runtime overrides (checked via `std::env::var` at call time, not compile time). Any process that can set environment variables — including a compromised container entrypoint, sidecar, or co-tenant — can inject arbitrary values into version reporting. If downstream systems use `build_commit_hash` for audit trails, deployment verification, or trust decisions, that trust is trivially undermined with no log trace.

### 10. TOCTOU in Proposer-Reth Address Lookups Across Epoch Boundaries
**File:** `crates/proposer-reth-map/src/lib.rs`

Two sequential calls to `get_reth_address_by_index` can return values from different epochs because each call independently acquires and releases the read lock. Any block production code that looks up multiple validator addresses in sequence (e.g., "get proposer A's address, then get proposer B's address") may operate on an inconsistent view of the validator set. The API provides no epoch token or snapshot mechanism to prevent this.

### 11. Debug `println!` in Production Config Storage Path
**File:** `crates/api/src/config_storage.rs`, line 23

An unconditional `println!` logs every `fetch_config_bytes` call to stdout, duplicating the structured `info!()` log on line 25. While the values printed (enum variant name + block number) are not currently secrets, stdout output in production is often captured into unstructured logs with different retention/access policies than the structured logging system. The code comment on line 134 of `consensus_api.rs` ("It seems stupid, refactor when debugging finished") confirms this is an unfinished debug artifact.

---

## Summary

| # | Vulnerability | Severity | Impact |
|---|---|---|---|
| 1 | DashMap lock ordering deadlock in `txn_metrics` | **Critical** | Node freeze under load |
| 2 | `todo!()` panics on consensus commit path | **Critical** | Node crash on any mempool error or `SyncForDuration` |
| 3 | `unwrap()` panics on event notification failure | **Critical** | Node crash from single failed subscriber |
| 4 | Timestamp off by 10⁶× (seconds vs microseconds) | **High** | Broken TTL, expiration, and time-based logic |
| 5 | Non-atomic multi-map operations lose metrics | **High** | Silent metric data loss, orphaned map entries |
| 6 | `SyncToTarget` response error silently discarded | **High** | Consensus stall with no diagnostic output |
| 7 | `InfallibleRwLock` poison cascades fatally | **High** | Permanent block production failure |
| 8 | Unbounded capacity growth in metrics maps | **Medium** | Memory exhaustion under sustained load |
| 9 | Runtime env var spoofing of build metadata | **Medium** | Audit trail / version trust undermined |
| 10 | TOCTOU on epoch-boundary validator lookups | **Medium** | Inconsistent validator set reads |
| 11 | Debug `println!` in production config path | **Medium** | Unstructured information leakage |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: Shared Library Crates — Fatal Log | 137086ms |
