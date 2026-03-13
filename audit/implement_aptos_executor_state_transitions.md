# implement_aptos_executor_state_transitions

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 164890ms
- **Steps**: 1

## Report

# Implementation Analysis: Aptos Executor Layer in gravity-sdk

## Architecture Overview

Gravity-sdk replaces the upstream Aptos executor with a **dual-layer architecture**:

1. **`dependencies/aptos-executor/` and `dependencies/aptos-executor-types/`** — Minimal stub crates that define the `BlockExecutorTrait` interface and provide a no-op `BlockExecutor`. These exist so that the Aptos consensus layer compiles without the full upstream Aptos VM.

2. **`aptos-core/consensus/src/gravity_state_computer.rs`** — The `GravityBlockExecutor` wrapper that implements `BlockExecutorTrait` by delegating execution to the inner stub and **replacing the entire commit path** with gravity's `BlockBufferManager` pipeline.

Real EVM execution happens externally via Reth's `PipeExecLayerApi`, completely outside these crates.

---

## Files/Contracts Involved

| File | Description |
|---|---|
| `dependencies/aptos-executor/src/lib.rs` | No-op `BlockExecutor` stub |
| `dependencies/aptos-executor/src/mock_block_tree.rs` | In-memory block ID tracker |
| `dependencies/aptos-executor-types/src/lib.rs` | `BlockExecutorTrait` trait definition + `StateComputeResult` type |
| `aptos-core/consensus/src/gravity_state_computer.rs` | `GravityBlockExecutor` — the shim between consensus and execution |

---

## Key Functions

### `BlockExecutorTrait` (trait, `aptos-executor-types/src/lib.rs:67-111`)

| Method | Signature | Behavior |
|---|---|---|
| `committed_block_id` | `&self -> HashValue` | Returns last committed block ID |
| `reset` | `&self -> Result<()>` | Resets internal state |
| `execute_and_state_checkpoint` | `(&self, ExecutableBlock, HashValue, BlockExecutorConfigFromOnchain) -> ExecutorResult<()>` | Executes block and produces state checkpoint |
| `ledger_update` | `(&self, block_id, parent_block_id) -> ExecutorResult<StateComputeResult>` | Produces `StateComputeResult` with root hash |
| `commit_blocks` | `(&self, Vec<HashValue>, LedgerInfoWithSignatures) -> ExecutorResult<()>` | Default impl: calls `pre_commit_block` per block, then `commit_ledger` |
| `pre_commit_block` | `(&self, HashValue) -> ExecutorResult<()>` | Pre-commit hook |
| `commit_ledger` | `(&self, Vec<HashValue>, LedgerInfoWithSignatures, Vec<(u64, Vec<u8>)>) -> ExecutorResult<()>` | Final commit with randomness |

### Stub `BlockExecutor` (`dependencies/aptos-executor/src/lib.rs:21-83`)

- **`execute_and_state_checkpoint`** (line 41-48): Returns `Ok(())` immediately — **no actual transaction execution**.
- **`ledger_update`** (line 50-57): Returns `StateComputeResult::with_root_hash(block_id)` — the root hash is set to the **block ID itself** (identity mapping, not a real state hash).
- **`commit_blocks`** (line 59-66): Appends block IDs to `MockBlockTree.commited_blocks`. Does not write to any persistent storage.
- **`pre_commit_block`** (line 70-72): `todo!()` — **panics if called**.
- **`commit_ledger`** (line 74-81): `todo!()` — **panics if called**.

### `StateComputeResult` (`aptos-executor-types/src/lib.rs:113-192`)

- Wraps `gaptos::api_types::compute_res::ComputeRes` which carries a 32-byte `data` field (root hash), `txn_num`, `txn_status`, and `events`.
- **`version()`** (line 129-132): Hardcoded to return `0` with a TODO comment.
- **`transactions_to_commit()`** (line 171-187): Filters input transactions by `TxnStatus.is_discarded`, returning only non-discarded ones. Contains a commented-out assertion `assert_eq!(status_len, input_txns.len())` (line 178) with a TODO about unifying recovery and execution paths.
- **`root_hash()`** (line 154-156): Reads `HashValue` directly from `execution_output.data`.

### `GravityBlockExecutor` (`gravity_state_computer.rs:44-193`)

**Pass-through methods** (no modification):
- `committed_block_id()` → `self.inner.committed_block_id()`
- `reset()` → `self.inner.reset()`
- `execute_and_state_checkpoint()` → `self.inner.execute_and_state_checkpoint()`
- `ledger_update()` → `self.inner.ledger_update()`
- `finish()` → `self.inner.finish()`

**Intercepted methods:**

- **`pre_commit_block()`** (line 136-138): Returns `Ok(())` unconditionally — upstream pre-commit hook is **entirely bypassed**.

- **`commit_blocks()`** (line 86-130):
  1. Extracts `block_id` and `block_hash` from `LedgerInfoWithSignatures`
  2. Records transaction lifetime metrics
  3. **Asserts** `block_ids.last().unwrap().as_slice() == block_id.as_slice()` (line 98)
  4. Calls `self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false)` — persists only ledger info, **no transactions** (`None`)
  5. Routes block IDs into `BlockBufferManager::set_commit_blocks()` via async runtime
  6. Awaits `persist_notifiers` for each block — blocks until persistence is confirmed
  7. Only assigns `hash: Some(v)` to the **last block** (matching `block_id`); intermediate blocks get `hash: None`

- **`commit_ledger()`** (line 139-192):
  1. Increments `APTOS_COMMIT_BLOCKS` metric
  2. Asserts `!block_ids.is_empty()`
  3. **Persists randomness data** to `ConsensusDB` via `put_randomness()` (gravity-specific, not in upstream)
  4. Routes blocks into `BlockBufferManager::set_commit_blocks()` and awaits persistence
  5. Calls `save_transactions(None, Some(&ledger_info_with_sigs), false)` **after** persistence (ordering differs from `commit_blocks`)

---

## State Changes

| Operation | What Gets Modified |
|---|---|
| `execute_and_state_checkpoint` | Nothing (stub returns `Ok(())`) |
| `ledger_update` | Nothing (returns synthetic `StateComputeResult`) |
| `commit_blocks` | `MockBlockTree.commited_blocks` (in-memory); `DbReaderWriter.writer` (ledger info only); `BlockBufferManager` (committed block entries) |
| `commit_ledger` | `ConsensusDB` (randomness data); `BlockBufferManager` (committed block entries); `DbReaderWriter.writer` (ledger info only) |
| `pre_commit_block` | Nothing (no-op) |

---

## Execution Path: Consensus → Execution → Commit

```
Consensus orders blocks
        │
        ▼
execute_and_state_checkpoint(block, parent_id, config)
  [GravityBlockExecutor delegates to inner stub → returns Ok(()) immediately]
  [No actual transaction execution occurs here]
        │
        ▼
ledger_update(block_id, parent_block_id)
  [Inner stub returns StateComputeResult with root_hash = block_id]
  [Root hash is the block ID itself, NOT a computed state root]
        │
        ▼
commit_blocks(block_ids, ledger_info_with_sigs)  ─OR─  commit_ledger(block_ids, LI, randomness)
  │
  ├─ Assert last block_id matches LedgerInfo.commit_info().id()
  ├─ [commit_ledger only] Persist randomness to ConsensusDB
  ├─ save_transactions(None, Some(&LI), false) — ledger info only, no txns
  ├─ BlockBufferManager.set_commit_blocks(block_hash_refs, epoch)
  └─ Await persist_notifiers (blocks until Reth confirms persistence)
```

The actual EVM execution happens in a separate pipeline:
```
BlockBufferManager.set_ordered_blocks()
        │
        ▼
RethCli::start_execution() → PipeExecLayerApi::push_ordered_block()
        │
        ▼
Reth EVM executes transactions
        │
        ▼
PipeExecLayerApi::pull_executed_block_hash() → execution result
        │
        ▼
RethCli::start_commit_vote() → BlockBufferManager.set_compute_res()
        │
        ▼
Consensus votes on StateComputeResult
        │
        ▼
RethCli::start_commit() → PipeExecLayerApi::commit_executed_block_hash()
```

---

## External Dependencies

| Dependency | Source | Usage |
|---|---|---|
| `gaptos` | Gravity's vendored Aptos re-export crate | `HashValue`, `LedgerInfoWithSignatures`, `ExecutableBlock`, `ComputeRes`, `EpochState`, runtimes |
| `block_buffer_manager` | Gravity crate | `get_block_buffer_manager()`, `BlockHashRef`, `set_commit_blocks()` |
| `txn_metrics` | Gravity crate | `TxnLifeTime::record_block_committed()` |
| `ConsensusDB` | `crate::consensusdb` | `put_randomness()` for randomness persistence |
| `DbReaderWriter` | Via `gaptos::aptos_storage_interface` | `writer.save_transactions()` for ledger info persistence |

---

## Notable Implementation Details

1. **`commit_blocks` vs `commit_ledger` ordering inconsistency**: In `commit_blocks()` (line 100-101), `save_transactions()` is called **before** `set_commit_blocks()`. In `commit_ledger()` (line 188-189), `save_transactions()` is called **after** `set_commit_blocks()` and after persistence confirmation. These two methods write the ledger info at different points relative to the BlockBufferManager persistence flow.

2. **`commit_blocks` error handling vs `commit_ledger`**: `commit_blocks()` uses `unwrap_or_else(|e| panic!(...))` on the `set_commit_blocks` result (line 123), while `commit_ledger()` uses a bare `.unwrap()` (line 184). Both panic on failure but with different diagnostic messages.

3. **`commit_ledger` randomness persistence error propagation**: `put_randomness()` failure is converted to an `anyhow::Error` and propagated via `?` (line 160), meaning randomness persistence failure aborts the commit. However, this happens **before** `set_commit_blocks()`, so a randomness write failure leaves no partial state in the BlockBufferManager.

4. **Block hash assignment**: In both `commit_blocks` and `commit_ledger`, only the block matching `block_id` (the last one) receives `hash: Some(v)`. All preceding blocks in the batch receive `hash: None` (lines 115 and 176). The hash value `v` is the same `block_hash` for all blocks — it's copied from the `LedgerInfoWithSignatures`.

5. **`version()` placeholder**: `StateComputeResult::version()` always returns `0` (line 130), with a TODO indicating this needs real implementation.

6. **Commented-out assertion**: `transactions_to_commit()` has `assert_eq!(status_len, input_txns.len())` commented out (line 178), with a TODO about unifying recovery and execution. This means there is no runtime validation that the transaction status vector length matches the input transaction count.

7. **`pre_commit_block` bypass**: The `GravityBlockExecutor` returns `Ok(())` for `pre_commit_block` (line 137), entirely skipping whatever the upstream Aptos executor would have done in this hook. The default `commit_blocks` trait implementation (line 93-98) calls `pre_commit_block` per block before `commit_ledger`, but `GravityBlockExecutor` overrides `commit_blocks` entirely and never calls the default implementation.

8. **The stub executor's `ledger_update` sets root_hash = block_id**: This means the `StateComputeResult` root hash used by consensus for voting is the block ID itself, not a computed Merkle root. The real state root comes from Reth's execution pipeline via a separate channel (`set_compute_res`).

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Implementation Analysis: Aptos Executor Layer in gravity-s | 164890ms |
