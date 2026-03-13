# attack_aptos_executor_state_transitions

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 243015ms
- **Steps**: 1

## Report

# Intention Attack Report: Aptos Executor Layer in gravity-sdk

## Intent vs. Implementation Summary

**User Intent:** Ensure state transition execution safety, WriteSet application correctness, and consistency between consensus-agreed state and executed state.

**Implementation Reality:** The executor layer is a hollow shim — no transaction execution occurs in it. Real execution is delegated to an external Reth/EVM pipeline via `BlockBufferManager`. This architectural choice introduces several critical gaps where consensus-agreed state can diverge from executed state.

---

## Critical Severity

### C1. `save_transactions` Error Silently Discarded

**File:** `gravity_state_computer.rs:100-101` and `gravity_state_computer.rs:188-189`

```rust
let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
```

Both `commit_blocks` and `commit_ledger` use `let _ =` to discard the `Result`. The investigation confirmed `save_transactions` is **not a no-op** — it writes `LedgerInfoWithSignatures` to the metadata DB and updates the in-memory latest ledger info cache. If this write fails (disk full, I/O error), the error is silently swallowed and the function returns `Ok(())`.

**Impact:** Consensus believes the ledger info was persisted when it was not. On restart, the node's ledger info is behind, causing it to disagree with peers about the latest committed state. This is an **irrecoverable consensus divergence** that cannot be detected at runtime.

**Fix:** Replace `let _ =` with `?` or explicit error handling.

---

### C2. `commit_blocks` vs `commit_ledger` — Ordering Inconsistency Creates Crash-Recovery Split-Brain

**File:** `gravity_state_computer.rs:86-130` vs `gravity_state_computer.rs:139-192`

| Step | `commit_blocks` (line 100) | `commit_ledger` (line 188) |
|------|---------------------------|---------------------------|
| 1st | `save_transactions` (ledger metadata) | `set_commit_blocks` + await persistence |
| 2nd | `set_commit_blocks` + await persistence | `save_transactions` (ledger metadata) |

If the node crashes between steps 1 and 2 in `commit_blocks`, the ledger metadata is persisted but BlockBufferManager never committed the block. On restart: consensus believes committed, execution has no record. **Irrecoverable split-brain.**

**Mitigating factor:** Investigation confirmed `commit_blocks` is dead code in the live pipeline — only `commit_ledger` is called from `ExecutionProxy::commit` and `PipelineBuilder`. However, it remains a valid, non-deprecated `BlockExecutorTrait` method that compiles and could be invoked by future code.

**Fix:** Delete or mark `commit_blocks` override as `unreachable!()`, or fix the ordering to match `commit_ledger`.

---

### C3. No Validation of `block_hash` in BlockBufferManager Commit Path

**File:** `gravity_state_computer.rs:110-115` and `block_buffer_manager.rs`

The `block_hash` from `LedgerInfoWithSignatures` is passed into `BlockHashRef` and stored by `set_commit_blocks` **without cross-checking** against the execution result hash stored in `BlockState::Computed`. Additionally, only the **last block** in a batch receives a hash — all intermediate blocks get `hash: None`:

```rust
hash: if x == block_id { Some(v) } else { None },
```

**Impact:** A corrupted or tampered `LedgerInfoWithSignatures` could inject a wrong hash that propagates to the execution layer as the "committed" hash. Intermediate blocks have no hash integrity whatsoever, so the execution layer cannot verify them.

---

## High Severity

### H1. `persist_notifier.take()` Is Destructive — Retry Causes Permanent Deadlock

**File:** `block_buffer_manager.rs` (set_commit_blocks / get_committed_blocks)

The `persist_notifier` is a one-shot `mpsc::Sender` consumed via `.take()` when `get_committed_blocks` retrieves a committed block. If `get_committed_blocks` is called twice for the same epoch-change block (e.g., retry after transient error):
1. Second call returns `persist_notifier: None`
2. No `Sender` exists to signal the `Receiver`
3. Consensus side blocks **forever** on `notifier.recv().await`

**Impact:** Any retry on the execution side permanently deadlocks the consensus commit pipeline. The node becomes unresponsive with no visible error or timeout.

---

### H2. `set_compute_res` Panics on Duplicate Calls — No Idempotency

**File:** `block_buffer_manager.rs:703-756`

If the execution layer retries `set_compute_res` for a block already in `Computed` state, the function **panics** — it only handles `Ordered` state. A transient network issue causing a re-submission crashes the node.

**Impact:** Node crash during mid-commit, requiring restart and potential state inconsistency.

---

### H3. TOCTOU Race in Epoch-Change Detection

**File:** `block_buffer_manager.rs` (is_epoch_change / get_ordered_blocks)

`is_epoch_change()` reads `AtomicU8` **without** holding the `block_state_machine` lock. In `get_ordered_blocks`, the pattern is:

```
if self.is_epoch_change() { ... }  // no lock
let guard = self.block_state_machine.lock().await;  // lock acquired later
```

Between these two operations, another task can change the epoch state. The codebase itself acknowledges this: `consume_epoch_change` comments say "Acquire lock first to prevent TOCTOU race" — but `get_ordered_blocks` does not follow this discipline.

**Impact:** Under concurrent epoch transitions, blocks could be processed in the wrong epoch context.

---

### H4. `StateComputeResult::version()` Hardcoded to `0`

**File:** `aptos-executor-types/src/lib.rs:129-132`

```rust
pub fn version(&self) -> Version {
    // TODO(gravity_byteyue): this is a placeholder
    Version::from(0u8)
}
```

All blocks report version 0. If any Aptos consensus internals use this for ordering, deduplication, or pruning, all blocks appear identical. No callers found in gravity-sdk itself, but upstream Aptos framework code may call it.

---

## Medium Severity

### M1. `transactions_to_commit()` Silently Truncates on Length Mismatch

**File:** `aptos-executor-types/src/lib.rs:170-187`

The assertion `assert_eq!(status_len, input_txns.len())` is **commented out**. When `txn_status` is shorter than `input_txns`, `zip` stops early — transactions at the end are **silently dropped** without being reported as committed or discarded. Mempool GC never cleans them up, causing a mempool leak.

---

### M2. Block Number Arithmetic Could Underflow

**File:** `gravity_state_computer.rs:114` and `gravity_state_computer.rs:175`

```rust
num: block_num - (len - 1 - i) as u64,
```

If `block_num < (len - 1)`, this wraps to `u64::MAX - delta` in release mode (Rust unsigned subtraction wraps). The corrupt block number propagates into BlockBufferManager lookups.

---

### M3. Randomness Data Logged at Debug Level

**File:** `gravity_state_computer.rs:161`

```rust
debug!("Persisted randomness data: {:?}", randomness_data);
```

Raw randomness bytes are logged in full. If used for leader election or cryptographic commitments, this exposes values to anyone with log access.

---

## Summary

| ID | Finding | Severity | Live? |
|----|---------|----------|-------|
| C1 | `save_transactions` error silently discarded (`let _ =`) | **Critical** | Yes |
| C2 | `commit_blocks` vs `commit_ledger` ordering — crash-recovery split-brain | **Critical** | Latent |
| C3 | No `block_hash` validation; intermediate blocks have `None` hash | **Critical** | Yes |
| H1 | `persist_notifier.take()` — retry causes permanent deadlock | **High** | Yes |
| H2 | `set_compute_res` panics on duplicate calls | **High** | Yes |
| H3 | TOCTOU race in epoch-change detection | **High** | Yes |
| H4 | `StateComputeResult::version()` hardcoded to `0` | **High** | TBD |
| M1 | `transactions_to_commit()` silently truncates on mismatch | **Medium** | Yes |
| M2 | Block number arithmetic underflow | **Medium** | Yes |
| M3 | Randomness data logged at debug level | **Medium** | Yes |

## Recommended Priority Actions

1. **C1** — Replace `let _ =` with proper error propagation on `save_transactions`. One-line fix, critical impact.
2. **C3** — Cross-check `block_hash` in `set_commit_blocks` against `BlockState::Computed` hash.
3. **H1** — Make `persist_notifier` retrieval idempotent or add timeout to `recv().await`.
4. **H2** — Make `set_compute_res` idempotent for duplicate calls with matching block_id/hash.
5. **C2** — Delete or deprecate the `commit_blocks` override since it is dead code with dangerous ordering.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: Aptos Executor Layer in gravity-s | 243015ms |
