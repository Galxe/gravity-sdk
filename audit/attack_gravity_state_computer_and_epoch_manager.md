# attack_gravity_state_computer_and_epoch_manager

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 114508ms
- **Steps**: 1

## Report

# Intention Attack Report: Gravity Consensus-to-Execution Bridge

## Scope

Security audit of `gravity_state_computer.rs` and the consensus commit pipeline, focusing on state divergence between what consensus commits and what reth executes, crash recovery correctness, and epoch transition safety.

---

## Critical Vulnerabilities

### CRITICAL-1: Silent Discard of Persistence Failures Creates Phantom Commits

**Files:** `gravity_state_computer.rs:101`, `gravity_state_computer.rs:188-189`

```rust
let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info), false);
```

Both `commit_blocks` and `commit_ledger` discard the `Result` from `save_transactions` using `let _ =`. If the AptosDB write fails (disk full, RocksDB corruption, I/O error), the method returns `Ok(())` to the caller. Consensus believes the ledger info is persisted. It is not.

**Impact:** After a crash, the node cannot recover the finality proof for committed blocks. The consensus layer's view of "what is finalized" permanently diverges from what is on disk. This is an **irrecoverable state corruption** — the node will either be unable to restart or will fork from the network.

**The intention was:** Persist the finality proof reliably. The implementation achieves the **opposite** — it silently succeeds even when persistence fails.

---

### CRITICAL-2: Inconsistent State Mutation Ordering Between Commit Paths Enables State Divergence on Crash

**Files:** `gravity_state_computer.rs:100-127` vs `gravity_state_computer.rs:164-190`

The two commit paths have **reversed ordering** of critical state mutations:

| Step | `commit_blocks` (legacy) | `commit_ledger` (production) |
|------|-------------------------|------------------------------|
| 1 | `save_transactions` (AptosDB) | `set_commit_blocks` (notify reth) |
| 2 | `set_commit_blocks` (notify reth) | await `persist_notifiers` (reth confirms) |
| 3 | await `persist_notifiers` | `save_transactions` (AptosDB) |

In the **production path** (`commit_ledger`), reth finalizes blocks and confirms on-disk persistence **before** the AptosDB ledger info is written. A crash between steps 2 and 3 means:

- Reth has finalized and persisted the blocks (irreversible)
- AptosDB has **no record** of the finality proof
- On restart, consensus has no finality proof for blocks that reth considers final

Combined with CRITICAL-1 (the `save_transactions` error being discarded), even a non-crash failure at step 3 goes undetected.

**The intention was:** Ensure atomicity between reth finalization and ledger info persistence. The implementation creates a **crash window** where the two stores permanently diverge.

---

### CRITICAL-3: Persist Notification Failures Silently Ignored — Commit Proceeds Without Confirmation

**Files:** `gravity_state_computer.rs:124-126`, `gravity_state_computer.rs:185-187`

```rust
let _ = notifier.recv().await;
```

The `persist_notifiers` are oneshot channels that reth uses to confirm on-disk persistence. If the sender is dropped (reth panics, channel error), `recv()` returns `Err`. This error is silently discarded with `let _ =`.

**Impact:** The commit path proceeds as if reth confirmed persistence when it may not have. The node's AptosDB records a finality proof for blocks that reth never actually persisted. After restart, reth's state is behind the consensus layer's view — a **permanent fork**.

**The intention was:** Wait for reth to confirm persistence before proceeding. The implementation **ignores the confirmation result**, defeating the entire purpose of the two-phase commit pattern.

---

## High Vulnerabilities

### HIGH-1: Integer Underflow in Block Number Arithmetic

**Files:** `gravity_state_computer.rs:114`, `gravity_state_computer.rs:175`

```rust
block_num - (len - 1 - i) as u64
```

If `block_num < (len - 1)`, this underflows. In debug mode it panics; in release mode it wraps to `u64::MAX - delta`, producing a nonsensical block number that gets sent to reth via `BlockHashRef`. Reth would attempt to finalize a block at an astronomically high number.

**Precondition:** A batch of blocks is committed where the first block's expected number would be negative. This could happen during epoch boundaries or state sync edge cases where `block_num` is reset or is lower than expected relative to the batch size.

---

### HIGH-2: All BlockBufferManager Failures Are Hard Panics — No Graceful Degradation

**Files:** `gravity_state_computer.rs:123`, `gravity_state_computer.rs:184`

```rust
// commit_blocks path - at least has an error message
.unwrap_or_else(|e| panic!("Failed to push commit blocks {}", e))

// commit_ledger path - bare unwrap, no diagnostics
.unwrap()
```

Any transient failure in the BlockBufferManager (network hiccup to reth, temporary resource exhaustion) immediately crashes the node. There is no retry logic, no circuit breaker, no graceful degradation. The `commit_ledger` path doesn't even log the error before crashing.

**Impact:** A single transient reth failure takes down the validator. In a scenario where multiple validators experience the same reth issue simultaneously, this could halt consensus.

---

### HIGH-3: Only Last Block in Commit Batch Carries EVM Hash — Earlier Blocks Are Unverifiable

**Files:** `gravity_state_computer.rs:108-120`, `gravity_state_computer.rs:169-181`

```rust
if i == len - 1 {
    hash: Some(block_hash),  // only the last block
} else {
    hash: None,              // all earlier blocks
}
```

In a multi-block commit, only the final block carries `block_hash`. All preceding `BlockHashRef` entries have `hash: None`. This means:

- Reth cannot independently verify the identity of earlier blocks in the batch
- If block ordering within the batch is corrupted (e.g., by a bug in the `block_ids` vector construction), reth will finalize the wrong blocks at the wrong numbers with no way to detect the mismatch
- The EVM block hash linkage (parent → child) is broken for all but the last block

---

## Medium Vulnerabilities

### MEDIUM-1: `block_on` Inside Dedicated Runtime — Fragile Sync→Async Bridge

**Files:** `gravity_state_computer.rs:103`, `gravity_state_computer.rs:164`

The `GravityBlockExecutor` spawns a dedicated Tokio runtime named `"tmp"` and uses `self.runtime.block_on()` to bridge sync trait methods to async BlockBufferManager calls. If `commit_blocks` or `commit_ledger` is ever called from within an async context (e.g., via `spawn_blocking` from `ExecutionProxy::commit`), `block_on` on the **same** runtime would deadlock. The separate runtime mitigates this, but the pattern is fragile and the runtime name `"tmp"` provides no diagnostic value.

### MEDIUM-2: Randomness Persistence Succeeds But Commit Fails — Orphaned Randomness Data

**Files:** `gravity_state_computer.rs:158-161`

In `commit_ledger`, randomness is persisted to `ConsensusDB` **before** the BlockBufferManager call. If the subsequent `set_commit_blocks` panics (HIGH-2), the randomness data is persisted for a commit that never completed. On restart, `ConsensusDB` contains randomness entries for rounds that were never finalized — a consistency violation between the two RocksDB instances.

---

## Attack Scenario: The Silent Corruption Chain

Combining CRITICAL-1, CRITICAL-2, and CRITICAL-3 into a single attack narrative:

1. Consensus commits a batch of 3 blocks via `commit_ledger`
2. Randomness is persisted to ConsensusDB (succeeds)
3. `set_commit_blocks` notifies reth (succeeds)
4. Reth begins finalization but the sender side of one `persist_notifier` is dropped due to a reth internal error
5. `let _ = notifier.recv().await` — **error silently discarded** (CRITICAL-3)
6. `save_transactions` is called — **disk is full, write fails**
7. `let _ = self.inner.db.writer.save_transactions(...)` — **error silently discarded** (CRITICAL-1)
8. `commit_ledger` returns `Ok(())`
9. Consensus believes 3 blocks are committed and finalized
10. Node crashes (OOM from full disk, or any other reason)
11. On restart:
    - ConsensusDB has randomness for rounds that "committed"
    - AptosDB has **no finality proof** for those blocks
    - Reth may or may not have the blocks (notification was dropped)
    - **The node cannot recover to a consistent state**

---

## Summary

| ID | Finding | Severity |
|----|---------|----------|
| CRITICAL-1 | `save_transactions` result silently discarded — phantom commits | **Critical** |
| CRITICAL-2 | Inconsistent mutation ordering between commit paths — crash window | **Critical** |
| CRITICAL-3 | Persist notification failures silently ignored — false confirmation | **Critical** |
| HIGH-1 | Integer underflow in block number arithmetic | **High** |
| HIGH-2 | All BlockBufferManager failures are hard panics | **High** |
| HIGH-3 | Only last block in batch carries EVM hash — unverifiable intermediates | **High** |
| MEDIUM-1 | Fragile `block_on` sync→async bridge | **Medium** |
| MEDIUM-2 | Orphaned randomness data on commit failure | **Medium** |

The core design flaw is that `commit_ledger` attempts a **two-phase commit** across three independent storage systems (ConsensusDB, AptosDB, Reth) without any transactional guarantees or rollback mechanism. Each `let _ =` is a hole in the safety net. The system **cannot distinguish** "fully committed" from "partially committed" after a crash, making recovery inherently unsafe.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: Gravity Consensus-to-Execution Br | 114508ms |
