# implement_mock_consensus

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 140493ms
- **Steps**: 1

## Report

# Mock Consensus Implementation Analysis

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `bin/gravity_node/src/consensus/mock_consensus/mod.rs` | Module declaration — exports `mempool` and `mock` submodules |
| `bin/gravity_node/src/consensus/mock_consensus/mock.rs` | Core `MockConsensus` struct: block production loop, execution tracking, epoch handling (277 lines) |
| `bin/gravity_node/src/consensus/mock_consensus/mempool.rs` | `Mempool` wrapper over `TxPool` with nonce-based transaction filtering (49 lines) |

---

## Execution Path

```
TxPool (external trait object)
    │
    ▼
Mempool::get_txns()              ── nonce-filtered transaction selection
    │
    ▼
MockConsensus::construct_block() ── DefaultHasher-based BlockId, proposer_index=0
    │
    ▼
BlockBufferManager::set_ordered_blocks()
    │
    ▼
[std::sync::Condvar backpressure: MAX_EXECUTED_GAP]
    │
    ▼
BlockBufferManager::get_executed_res()  ── waits for execution engine
    │
    ▼
BlockBufferManager::set_commit_blocks() ── finalizes block commit
    │
    ▼
process_epoch_change()           ── handles NewEpoch events
    │
    ▼
commit_txns (unbounded mpsc)     ── forwards committed TxnIds to mempool
```

---

## Key Functions

### `mock.rs` — `MockConsensus`

| Function | Signature | Description |
|----------|-----------|-------------|
| `new` | `async fn new(pool: Box<dyn TxPool>) -> Self` | Initializes with a hardcoded genesis `BlockId` (32-byte constant), calls `get_block_buffer_manager().init(0, block_number_to_block_id, 1)`, starts at epoch 1 |
| `construct_block` | `fn(block_number: u64, txns: Vec<VerifiedTxn>, attr: ExternalPayloadAttr, epoch: u64) -> ExternalBlock` | Hashes `txns` + `attr` via `DefaultHasher`, writes the resulting `u64` into bytes `[0..8]` of a 32-byte array (bytes `[8..32]` are zeroed), returns `ExternalBlock` with `proposer_index: Some(0)` |
| `check_and_construct_block` | `async fn(&Mutex<Mempool>, u64, ExternalPayloadAttr, u64) -> ExternalBlock` | Polls mempool in a loop (10ms sleep between polls), returns block if: (a) time gap > 1 second, (b) no new txns but buffer non-empty, or (c) txn count exceeds `max_txn_num` |
| `run` | `async fn run(mut self)` | Main event loop — spawns ordering task, commit-txn forwarding task, then loops receiving executed block results and committing them |
| `process_epoch_change` | `fn(&mut self, events: &[GravityEvent], block_number: u64)` | Iterates events; on `GravityEvent::NewEpoch(epoch, _)`, asserts sequential epoch via `assert_eq!`, then atomically stores new epoch and epoch start block number |

### `mempool.rs` — `Mempool`

| Function | Signature | Description |
|----------|-----------|-------------|
| `new` | `fn(pool_txns: Box<dyn TxPool>) -> Self` | Wraps a `TxPool` with an empty `next_sequence_numbers` map |
| `reset_epoch` | `fn(&mut self)` | Clears `next_sequence_numbers` HashMap |
| `get_txns` | `fn(&mut self, block_txns: &mut Vec<VerifiedTxn>, max_block_size: usize) -> bool` | Clones `next_sequence_numbers`, creates a filter closure that accepts txns with `seq_num >= next_nonce`, calls `pool_txns.best_txns(Some(filter), max_block_size)`, inserts each returned txn's `(sender, nonce+1)` into tracking map, breaks when `block_txns.len() >= max_block_size` |
| `commit_txns` | `fn(&mut self, _txns: &[TxnId])` | **Empty method body** — committed transactions are accepted but no state is modified |

---

## Environment Variable Configuration

| Variable | Location | Cached? | Default | Purpose |
|----------|----------|---------|---------|---------|
| `MOCK_SET_ORDERED_INTERVAL_MS` | `mock.rs:28` | Yes (`OnceLock`) | `200` | Sleep duration (ms) between ordering successive blocks |
| `MOCK_MAX_BLOCK_SIZE` | `mock.rs:36` | **No** — re-read on every call | `7000` | Max transactions per block |
| `MAX_EXECUTED_GAP` | `mock.rs:45` | Yes (`OnceLock`) | `16` | Max allowed lag between ordered and executed block numbers before backpressure engages |

All three use the pattern: `std::env::var(...).unwrap_or_else(|_| default).parse().unwrap_or(default)`. Invalid values silently fall back to defaults.

---

## State Changes

| Operation | What Changes |
|-----------|-------------|
| `MockConsensus::new` | Initializes `BlockBufferManager` global with genesis block at position 0, epoch 1 |
| `Mempool::get_txns` | Mutates `next_sequence_numbers`: inserts `(sender, nonce+1)` for each txn yielded |
| `Mempool::reset_epoch` | Clears entire `next_sequence_numbers` map |
| `set_ordered_blocks` | Writes ordered block into `BlockBufferManager` (external) |
| `set_commit_blocks` | Writes committed block with execution hash into `BlockBufferManager` (external) |
| Condvar consumer (`mock.rs:232-236`) | Updates `executed_jam_wait` mutex value to latest `block_number`, calls `notify_all()` |
| `process_epoch_change` | Atomically stores new `epoch` and `epoch_start_block_number` |
| `Mempool::commit_txns` | **No-op** — method body is empty |

---

## Concurrency Architecture

### Channels
- **`block_meta_tx/rx`**: bounded `tokio::sync::mpsc::channel(8)` — ordering task sends `ExternalBlockMeta`, main loop receives
- **`commit_txns_tx/rx`**: unbounded `tokio::sync::mpsc::unbounded_channel` — main loop sends committed `Vec<TxnId>`, dedicated task forwards to `Mempool::commit_txns` (which is a no-op)

### Backpressure (Condvar)
- **Field**: `executed_jam_wait: Arc<(Mutex<u64>, Condvar)>`
- **Producer** (ordering task, `mock.rs:174-188`): After ordering a block, acquires mutex, enters `while (block_number - *executed_number) > MAX_EXECUTED_GAP` spin-wait on `cvar.wait()`
- **Consumer** (main loop, `mock.rs:232-236`): After receiving execution result, acquires mutex, sets `*executed_number = block_number`, calls `cvar.notify_all()`
- The `std::sync::Mutex` is held across a `Condvar::wait()` call inside a `tokio::spawn`'d async task — this blocks the OS thread

### Epoch Transitions
- Ordering task checks `epoch` atomic on each iteration; on change, calls `release_inflight_blocks()`, resets mempool, reloads `epoch_start_block_number`
- Main loop calls `process_epoch_change` after each committed block

---

## External Dependencies

| Dependency | Usage |
|------------|-------|
| `block_buffer_manager::BlockBufferManager` (via `get_block_buffer_manager()`) | Global singleton for block ordering, execution result retrieval, and commit finalization |
| `block_buffer_manager::TxPool` (trait) | Transaction pool interface injected into `Mempool` |
| `gaptos::api_types` | Type definitions: `BlockId`, `ExternalBlock`, `ExternalBlockMeta`, `ExternalPayloadAttr`, `VerifiedTxn`, `GravityEvent`, `ExternalAccountAddress`, `TxnHash` |
| `std::hash::DefaultHasher` | Non-cryptographic `SipHash`-based hasher used for `BlockId` generation |
| `tokio` | Async runtime: `tokio::sync::Mutex`, `tokio::sync::mpsc`, `tokio::time::sleep`, `tokio::spawn` |

---

## Assertions and Panics

| Location | Type | Condition | Effect |
|----------|------|-----------|--------|
| `mock.rs:270` | `assert_eq!` | `current_epoch != new_epoch - 1` | Panics if epoch transition is non-sequential |
| `mock.rs:225` | `panic!` | `get_executed_res` returns error not containing `"get_executed_res timeout"` | Panics on non-timeout execution errors; timeouts are retried in a loop |

---

## Block ID Generation Detail

`construct_block` (`mock.rs:76-88`) produces a `BlockId` as follows:
1. Creates `DefaultHasher::new()` (Rust's `SipHash-1-3`, deterministic within a process but seed may vary across builds/versions)
2. Hashes the transaction list and payload attributes
3. Calls `hasher.finish()` → `u64`
4. Copies 8 bytes (big-endian) into `bytes[0..8]` of a 32-byte array
5. `bytes[8..32]` remain `0x00`
6. Wraps as `BlockId(bytes)`

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Mock Consensus Implementation Analysis

## Files/Contracts | 140493ms |
