# implement_shared_crates_and_dependencies

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 119682ms
- **Steps**: 1

## Report

# Shared Library Crates — Implementation Analysis

---

## 1. `crates/gravity-sdk/src/lib.rs` — SDK Public API Surface

### Files Involved
- `crates/gravity-sdk/src/lib.rs`

### Implementation
The SDK crate is a **pure re-export facade** consisting of 3 lines:

```rust
pub use api;
pub use block_buffer_manager;
pub use gaptos;
```

### Key Functions
None — no logic. The crate's sole purpose is to re-export three sub-crates (`api`, `block_buffer_manager`, `gaptos`) as the public API surface of `gravity-sdk`.

### State Changes
None.

### External Dependencies
- `api` crate
- `block_buffer_manager` crate
- `gaptos` crate

---

## 2. `crates/proposer-reth-map/src/lib.rs` — Proposer-to-Reth Address Mapping

### Files Involved
- `crates/proposer-reth-map/src/lib.rs`

### Implementation
A **global static `HashMap`** protected by an `InfallibleRwLock` (a `RwLock` that panics on poison), lazily initialized via `once_cell::Lazy`.

```rust
static PROPOSER_RETH_ADDRESS_MAP: Lazy<InfallibleRwLock<HashMap<u64, Vec<u8>>>>
```

### Key Functions

| Function | Signature | Behavior |
|---|---|---|
| `get_reth_address_by_index` | `(validator_index: u64) -> Option<Vec<u8>>` | Acquires a **read lock**, looks up the index, returns a `.cloned()` copy of the `Vec<u8>` address. |
| `update_proposer_reth_index_map` | `(validator_set: &ValidatorSet)` | Builds a **new** `HashMap` from scratch by iterating `validator_set.active_validators`, then acquires a **write lock** and replaces the entire map via `*map.write() = new_map`. |

### Execution Path — Read
1. Caller invokes `get_reth_address_by_index(index)`
2. `PROPOSER_RETH_ADDRESS_MAP.read()` acquires shared read lock
3. `.get(&validator_index).cloned()` returns `Option<Vec<u8>>`
4. Lock released on drop

### Execution Path — Write (Epoch Change)
1. Caller invokes `update_proposer_reth_index_map(validator_set)`
2. A new `HashMap` is built locally (no lock held during construction)
3. `PROPOSER_RETH_ADDRESS_MAP.write()` acquires exclusive write lock
4. Entire map is atomically swapped (old map dropped)
5. Lock released on drop

### State Changes
- Full replacement of the global `HashMap<u64, Vec<u8>>` on each epoch change.

### Concurrency Characteristics
- **Read path**: Multiple concurrent readers allowed (shared `RwLock` semantics).
- **Write path**: The new map is built entirely before acquiring the write lock, minimizing lock-hold duration. The swap is a single pointer-width assignment.
- **Atomicity boundary**: A reader calling `get_reth_address_by_index` during `update_proposer_reth_index_map` will see either the complete old map or the complete new map — never a partially updated state. However, there is **no cross-call atomicity** — two sequential reads (e.g., look up index A, then index B) could span an epoch boundary, returning values from different epochs.
- **`InfallibleRwLock`**: If a thread panics while holding the lock, subsequent lock attempts will also **panic** (intentional design — it treats poison as fatal rather than recoverable).

---

## 3. `crates/txn_metrics/src/lib.rs` — Transaction Lifecycle Metrics

### Files Involved
- `crates/txn_metrics/src/lib.rs`

### Implementation
A **singleton** (`OnceLock<TxnLifeTime>`) that tracks transaction timing through the pipeline stages. Gated by the `TXN_LIFE_ENABLED` environment variable (checked once at first call via `OnceLock`).

### Data Structures

```rust
struct TxnLifeTime {
    txn_initial_add_time: DashMap<(AccountAddress, u64), SystemTime>,  // (sender, seq_num) -> when added
    txn_hash_to_key: DashMap<HashValue, (AccountAddress, u64)>,        // tx_hash -> (sender, seq_num)
    txn_batch_id: DashMap<BatchId, HashSet<TxnKey>>,                   // batch -> set of tx keys
    txn_block_id: DashMap<HashValue, HashSet<TxnKey>>,                 // block -> set of tx keys
}
```

Capacity constants: `MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100,000`, `MAX_TXN_BATCH_ID_CAPACITY = 10,000`, `MAX_TXN_BLOCK_ID_CAPACITY = 1,000`.

### Key Functions — Lifecycle Recording

| Function | What it records | Histogram emitted |
|---|---|---|
| `record_added(txn)` | First-seen timestamp for `(sender, seq_num)` | None (stores initial time) |
| `record_batch(batch_id, txns)` | Time from add → batch inclusion | `aptos_txn_added_to_batch_time_seconds` |
| `record_broadcast_batch(batch_id)` | Time from add → broadcast | `aptos_txn_added_to_broadcast_batch_time_seconds` |
| `record_before_persist(batch_id)` | Time from add → pre-persist | `aptos_txn_added_to_before_batch_persist_time_seconds` |
| `record_after_persist(batch_id)` | Time from add → post-persist | `aptos_txn_added_to_after_batch_persist_time_seconds` |
| `record_proof(batch_id)` | Time from add → proof generated | `aptos_txn_added_to_proof_time_seconds` |
| `record_block(payload, block_id)` | Time from add → block inclusion | `aptos_txn_added_to_block_time_seconds` |
| `record_executing(block_id)` | Time from add → execution start | `aptos_txn_added_to_executing_time_seconds` |
| `record_executed(block_id)` | Time from add → execution complete | `aptos_txn_added_to_executed_time_seconds` |
| `record_block_committed(block_id)` | Time from add → block committed | `aptos_txn_added_to_block_committed_time_seconds` |
| `record_committed(sender, seq_num)` | Time from add → final commit | `aptos_txn_added_to_committed_time_seconds` |

### `record_block` — Payload Dispatch

Handles 5 payload variants:
- **`DirectMempool(txns)`**: Iterates inline transactions, inserts hash→key mapping, observes block time, stores keys in `txn_block_id`.
- **`InQuorumStore(proof_with_data)`**: Delegates to `process_proof_with_data`, which looks up `txn_batch_id` for each proof's batch_id.
- **`InQuorumStoreWithLimit`**: Same as above, unwraps `.proof_with_data` field.
- **`QuorumStoreInlineHybrid(vec_payload, proof_with_data, _)`**: Processes proof data first, then processes inline transactions — both contribute to the same `txn_block_id` entry via `.entry().or_default().extend()`.
- **`OptQuorumStore`**: No-op (empty match arm).

### `record_committed` — Cleanup Path

1. Observes the final "added to committed" metric.
2. Removes the `(sender, seq_num)` from `txn_initial_add_time`.
3. Performs a **full scan** of `txn_hash_to_key` via `.retain()` to remove all matching entries.
4. Performs a **full scan** of `txn_batch_id` via `iter_mut()` → `.remove()` on each batch's `HashSet`, then retains non-empty batches.
5. Same full scan for `txn_block_id`.

### `cleanup_old_entries` — Capacity Control

Triggered when `txn_initial_add_time.len() >= 100,000`:
1. Collects all keys with age > 60 seconds.
2. Removes those keys from `txn_initial_add_time` and `txn_hash_to_key`.
3. If `txn_batch_id >= 10,000`: retains only batches referencing still-existing transactions.
4. If `txn_block_id >= 1,000`: same retention logic.

### Concurrency Characteristics
- All maps are `DashMap` (sharded concurrent maps) — no global mutex.
- Each `record_*` method independently acquires per-shard locks on the relevant `DashMap`.
- `record_committed` and `cleanup_old_entries` perform full iteration across all shards of multiple `DashMap`s simultaneously. `DashMap::retain` holds shard locks one at a time during iteration.
- **The `cleanup_old_entries` call within `record_added`** (line 214) can call `self.txn_hash_to_key.retain()` while `self.txn_initial_add_time` shard locks are temporarily acquired during the inner `contains_key` check — the DashMap design prevents deadlocks since these are different maps, but there is **no transactional consistency** across the multi-map cleanup.
- The `cleanup_old_entries` inside `txn_batch_id.retain` calls `self.txn_initial_add_time.contains_key(key)` — this acquires a read lock on `txn_initial_add_time` from within a write lock callback on `txn_batch_id`. Since these are different `DashMap` instances, no single-map deadlock occurs, but concurrent `record_added` inserting into `txn_initial_add_time` could race with the `contains_key` check, causing a just-inserted transaction to be cleaned up.

### Histogram Initialization
All 10 histograms use `OnceLock` + `register_histogram!` with `.unwrap()`. Each is initialized on first use. The `.unwrap()` will panic if the metric name is already registered in the global Prometheus registry by another component.

---

## 4. `crates/build-info/` — Build Information

### Files Involved
- `crates/build-info/build.rs` — build script
- `crates/build-info/src/lib.rs` — runtime library

### Build Script (`build.rs`)
1. Propagates `CARGO_CFG_TOKIO_UNSTABLE` as `USING_TOKIO_UNSTABLE` rustc-env variable.
2. Sets `cargo:rerun-if-changed` on `build.rs` and `../../.git/HEAD` (if it exists).
3. Invokes `shadow_rs::ShadowBuilder` with `deny_const(Default::default())` to generate compile-time git/build metadata.

### Runtime Library (`lib.rs`)

**`get_build_information() -> BTreeMap<String, String>`**:
1. Calls `shadow!(build)` to access shadow_rs compile-time constants.
2. Populates a `BTreeMap` with 12 keys:
   - `build_branch` — git branch at build time
   - `build_cargo_version` — cargo version
   - `build_clean_checkout` — whether git working tree was clean
   - `build_commit_hash` — git commit SHA
   - `build_tag` — git tag
   - `build_time` — build timestamp
   - `build_os` — build OS
   - `build_rust_channel` — stable/nightly/beta
   - `build_rust_version` — rustc version
   - `build_is_release_build` — debug vs release
   - `build_profile_name` — derived from `OUT_DIR` path (3rd-from-last path segment)
   - `build_using_tokio_unstable` — whether tokio unstable features enabled
3. **Runtime environment override**: checks `GIT_SHA`, `GIT_BRANCH`, `GIT_TAG`, `BUILD_DATE` environment variables and overwrites the corresponding entries if present.

**`build_information!()` macro**: Calls `get_build_information()` then inserts `build_pkg_version` from `CARGO_PKG_VERSION`.

**`get_git_hash() -> String`**: Returns `GIT_SHA` env var if set, otherwise falls back to the shadow_rs `COMMIT_HASH`.

### Information Exposed
The crate exposes: git branch, commit hash, tag, build time, OS, cargo version, rust version/channel, profile name, clean checkout status, tokio unstable flag, and package version. All values are compile-time constants with optional runtime env-var overrides (for Docker builds).

---

## 5. `crates/api/src/config_storage.rs` — ConfigStorageWrapper

### Files Involved
- `crates/api/src/config_storage.rs` — wrapper implementation
- `crates/api/src/consensus_api.rs` — sets `GLOBAL_CONFIG_STORAGE` (line 136)
- `crates/api/src/https/consensus.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 285)
- `crates/api/src/https/dkg.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 111)
- `bin/gravity_node/src/relayer.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 109)
- `aptos-core/consensus/src/consensusdb/include/reader.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 26)

### Implementation

**`ConfigStorageWrapper`** wraps an `Arc<dyn ConfigStorage>` and implements the `ConfigStorage` trait:

```rust
fn fetch_config_bytes(&self, config_name: OnChainConfig, block_number: BlockNumber) -> Option<OnChainConfigResType>
```

**Execution Path**:
1. **Always** prints to stdout via `println!` and logs via `info!` — both include the `config_name` and `block_number`.
2. Checks `config_name` against an allowlist of 8 variants:
   - `Epoch`, `ValidatorSet`, `JWKConsensusConfig`, `ObservedJWKs`, `RandomnessConfig`, `OracleState`, `DKGState`, `ConsensusConfig`
3. If matched → delegates to the inner `self.config_storage.fetch_config_bytes(config_name, block_number)`.
4. If not matched (`_` wildcard) → returns `None`.

### `GLOBAL_CONFIG_STORAGE` Usage Pattern
- **Type**: Defined externally (in `gaptos::api_types::config_storage`), likely an `OnceLock` or similar static.
- **Writer**: `consensus_api.rs:136` — calls `GLOBAL_CONFIG_STORAGE.set(config)` (one-time initialization).
- **Readers**: 4 call sites across `consensus.rs`, `dkg.rs`, `relayer.rs`, and `consensusdb/reader.rs` — all access via `.get()` which returns `Option`.
- **OnceLock semantics**: The `.set()` succeeds only once; subsequent `.set()` calls return `Err`. Readers use `.get()` and handle the `None` (uninitialized) case, with explicit error logging in consensus.rs (line 326) and dkg.rs (line 178).

### State Changes
None — purely a read-through filter/proxy.

---

## 6. `crates/api/src/consensus_mempool_handler.rs` — Consensus-to-Mempool Notification Pipeline

### Files Involved
- `crates/api/src/consensus_mempool_handler.rs`

### Data Structures

**`MempoolNotificationHandler<M: MempoolNotificationSender>`**:
- Holds `mempool_notification_sender: M`
- Clone-able

**`ConsensusToMempoolHandler<M: MempoolNotificationSender>`**:
- `mempool_notification_handler: MempoolNotificationHandler<M>`
- `consensus_notification_listener: ConsensusNotificationListener`
- `event_subscription_service: Arc<Mutex<EventSubscriptionService>>` (tokio Mutex)

### Key Functions

**`MempoolNotificationHandler::notify_mempool_of_committed_transactions`**
- Signature: `(&mut self, committed_transactions: Vec<Transaction>, block_timestamp_usecs: u64) -> anyhow::Result<()>`
- Calls `self.mempool_notification_sender.notify_new_commit(committed_transactions, block_timestamp_usecs).await`
- On error: **calls `todo!()`** — this will panic at runtime if the mempool notification fails.

**`ConsensusToMempoolHandler::handle_consensus_commit_notification`**
- Signature: `(&mut self, consensus_commit_notification: ConsensusCommitNotification) -> anyhow::Result<()>`
- Execution path:
  1. Extracts committed transactions via `.get_transactions().clone()`
  2. Calls `notify_mempool_of_committed_transactions` with transactions and **`SystemTime::now()`** converted to seconds (not microseconds, despite the parameter name `block_timestamp_usecs`) — there is a TODO comment acknowledging this needs modification.
  3. Extracts `block_number` and `subscribable_events` from the notification.
  4. Acquires the `event_subscription_service` tokio mutex lock.
  5. Calls `event_subscription_service.notify_events(block_number, events).unwrap()` — panics on error.
  6. Responds to consensus via `respond_to_commit_notification(notification, Ok(()))`.

**`ConsensusToMempoolHandler::handle_consensus_notification`**
- Dispatches on `ConsensusNotification` variant:
  - **`NotifyCommit`** → delegates to `handle_consensus_commit_notification`
  - **`SyncToTarget`** → acquires `event_subscription_service` lock, calls `notify_initial_configs(block_number).unwrap()`, responds `Ok(())` to consensus. The `map_err` result on `respond_to_sync_target_notification` is bound to `let _` (silently discarded).
  - **`SyncForDuration`** → **calls `todo!()`** — will panic if this variant is received.
- On any `Err` from the dispatched handler: logs a warning via `warn!`.

**`ConsensusToMempoolHandler::start`**
- Runs an infinite loop using `futures::select!`.
- Single arm: awaits `consensus_notification_listener.select_next_some()`, then calls `handle_consensus_notification`.
- A progress check interval arm is commented out.

### Execution Path — Full Commit Notification Flow
```
ConsensusNotificationListener (channel)
  → start() loop via futures::select!
    → handle_consensus_notification(NotifyCommit)
      → handle_consensus_commit_notification
        → mempool_notification_sender.notify_new_commit(txns, timestamp)
        → event_subscription_service.lock().notify_events(block_number, events)
        → respond_to_commit_notification(notification, Ok(()))
```

### External Dependencies
- `gaptos::aptos_consensus_notifications` — `ConsensusCommitNotification`, `ConsensusNotification`, `ConsensusNotificationListener`
- `gaptos::aptos_event_notifications` — `EventNotificationSender`, `EventSubscriptionService`
- `gaptos::aptos_mempool_notifications` — `MempoolNotificationSender`
- `tokio::sync::Mutex` — used for `EventSubscriptionService`

### State Changes
- Mempool state: modified indirectly via `notify_new_commit` (removes committed transactions from mempool).
- Event subscriptions: `notify_events` and `notify_initial_configs` propagate events to downstream subscribers.
- Consensus acknowledgment: `respond_to_commit_notification` / `respond_to_sync_target_notification` send ack back through the consensus channel.

### Notable Implementation Details
- Two `todo!()` calls exist that will **panic** at runtime: one in `notify_mempool_of_committed_transactions` error path (line 41), one in `SyncForDuration` handler (line 112).
- The `block_timestamp_usecs` parameter receives seconds (`.as_secs()`) not microseconds — the TODO on line 75 acknowledges this mismatch.
- `notify_events().unwrap()` on line 85 will panic if event notification fails.
- The `SyncToTarget` handler discards the error from `respond_to_sync_target_notification` via `let _`.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Shared Library Crates — Implementation Analysis

---

## 1 | 119682ms |
