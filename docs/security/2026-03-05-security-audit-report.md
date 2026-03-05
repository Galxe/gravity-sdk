# Security Audit Report — gravity-sdk (Phase 2)

**Date:** 2026-03-05
**Scope:** BlockBufferManager state machine, GCEI protocol, epoch change protocol, RethCli coordinator bridge
**Repository:** https://github.com/Galxe/gravity-sdk
**Methodology:** Multi-agent parallel audit (6 specialist sub-agents + manager cross-review)
**Auditor:** Claude Opus 4.6
**Previous Audits:**
- [2026-02-23 Report](2026-02-23-security-audit-report.md) — 5 findings (GSDK-001 to GSDK-005), all fixed
- [2026-02-28 Report](2026-02-28-security-audit-report.md) — 11 findings (GSDK-006 to GSDK-016), all open

---

## Summary

| Severity | Findings | Status |
|----------|----------|--------|
| HIGH | 2 | Open |
| MEDIUM | 4 | Open |
| LOW | 5 | Open |
| **Total** | **11** | **All open** |

This report covers findings **not** addressed in the 2026-02-23 or 2026-02-28 audits. The focus is on the `BlockBufferManager` state machine, epoch transition protocol, and `RethCli` coordinator bridge.

---

## HIGH Severity (2)

### GSDK-017: Nested Mutex Holding in BlockBufferManager

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs`
**Locations:** Lines 580+626, 697+669, 926-927, 256+288
**Issue:** Two `tokio::sync::Mutex` fields — `block_state_machine` and `latest_epoch_change_block_number` — are frequently acquired together in a nested pattern. Multiple methods acquire Lock A (`block_state_machine`) first, then Lock B (`latest_epoch_change_block_number`) while Lock A is held.

Lock ordering is consistent (A before B in all cases), so no classic deadlock occurs. However:
1. Holding `block_state_machine` (the hot lock used by nearly every method) while waiting to acquire the second lock amplifies contention.
2. `latest_epoch_change_block_number` can be temporarily inconsistent with `current_epoch` inside `BlockStateMachine` — they are updated in different code paths with a temporal gap.

**Impact:** Under high contention (frequent epoch transitions + block processing), all block operations stall while the nested lock is held. The inconsistency window between `latest_epoch_change_block_number` and `current_epoch` could confuse callers.
**Recommendation:** Merge `latest_epoch_change_block_number` into `BlockStateMachine` struct (eliminating the second lock), or replace `Mutex<u64>` with `AtomicU64`.

### GSDK-018: Epoch Transition Stale Block Waste — No Cancellation Mechanism

**Files:** `crates/block-buffer-manager/src/block_buffer_manager.rs:943-944`, cross-repo with gravity-reth `lib.rs:374-393`
**Issue:** When `release_inflight_blocks()` is called during epoch transition, all blocks with `block_number > latest_epoch_change_block_number` are discarded via `retain()`. On the gravity-reth side, tasks spawned for these blocks (via `tokio::spawn`) have no cancellation mechanism — they must wait for the 2-second `execute_block_barrier` timeout before detecting epoch mismatch and returning.

If K blocks were in-flight with the old epoch, each wastes up to 2 seconds of task time (concurrent, so ~2 seconds wall-clock). During this window, these tasks hold their `ReceivedBlock` data (including full transaction lists) in memory.

**Impact:** 2-second latency penalty during epoch transitions. Wasted memory and compute resources for K in-flight blocks.
**Recommendation:** Add a `CancellationToken` per epoch to allow immediate cleanup of stale tasks. Proactively notify stale-epoch tasks when epoch transition is detected.

---

## MEDIUM Severity (4)

### GSDK-019: Silent Block Dropping on Epoch Mismatch

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:392-406`
**Issue:** Blocks from mismatched epochs (old or future) are silently dropped with `return Ok(())`. The caller believes the block was successfully queued. During epoch transitions, there is a window where consensus may order blocks with the new epoch before the buffer manager has transitioned — these blocks are silently discarded.

The system relies on the consensus layer re-sending these blocks after the epoch change is finalized, but this is an implicit assumption with no explicit retry mechanism in the buffer manager.

**Impact:** Blocks could be lost during the narrow epoch transition window. Recovery depends entirely on the consensus layer re-proposing them.
**Recommendation:** Return an `Err` variant for dropped blocks, or add a metric counter for silently dropped blocks to enable monitoring.

### GSDK-020: In-Flight Blocks Discarded During Epoch Transition

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:943-944`
**Issue:** `retain()` removes ALL blocks with `block_number > latest_epoch_change_block_number`, regardless of state (Ordered, Computed, or Committed). Computed blocks that were already executed by gravity-reth but not yet committed are discarded without notification to the execution layer.
**Impact:** Execution work for Computed blocks is wasted. The execution layer may hold stale references to discarded blocks until barriers timeout. This is intentional behavior for epoch transitions but documented here for completeness.

### GSDK-021: TOCTOU Race in consume_epoch_change

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:330-334`
**Issue:** `buffer_state` is set to `Ready` (line 331) **before** acquiring `block_state_machine` lock (line 332). This creates a window where another task could call `get_ordered_blocks()`, see `Ready` state, and proceed while the epoch hasn't been fully consumed.

Currently safe because `consume_epoch_change` and `get_ordered_blocks` are called sequentially in the same async task (`reth_cli.rs`). But the API is a footgun for future multi-consumer usage.

**Recommendation:** Reorder: acquire lock first, then set `buffer_state`.

### GSDK-022: Execution Failure Terminates Commit Vote Loop

**File:** `bin/gravity_node/src/reth_cli.rs:300-306`
**Issue:** If `recv_compute_res` returns an error (channel closed), the commit vote loop terminates via `break`. Since this loop forwards execution results to the buffer manager, its termination means no further blocks transition from Ordered to Computed. The consensus pipeline stalls. Same pattern in `start_execution` (line 247) and `start_commit` (line 344).
**Impact:** A single channel failure terminates the entire pipeline with no recovery mechanism.

---

## LOW Severity (5)

### GSDK-023: buffer_state AtomicU8 Checked Outside Lock — TOCTOU

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:322-331,378,475-480`
**Issue:** `buffer_state` (AtomicU8) is checked at the start of operations (`is_ready()`, `is_epoch_change()`) before acquiring `block_state_machine` lock. Between the check and lock acquisition, another task could change `buffer_state`.
**Impact:** Low — the actual block operations are guarded by the lock, providing internal consistency. The `is_ready()` transition is irreversible (`Uninitialized` → `Ready`), so no TOCTOU for that check.

### GSDK-024: pop_txns Off-by-One in Gas Accounting

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:340-371`
**Issue:** The first transaction's `gas_limit` is never added to `total_gas_limit`. When `total_gas_limit == 0`, the closure returns `false` without incrementing the counter. Subsequent items' gas is checked without accounting for the first item, potentially including more transactions than the gas limit allows.
**Impact:** Low — actual gas limit enforcement occurs during EVM execution. But batch sizes may be unexpectedly large.
**Recommendation:** Remove the special case for `total_gas_limit == 0`.

### GSDK-025: Two-Phase Epoch Update Creates Observation Window

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:674-676`
**Issue:** Between `set_compute_res` setting `next_epoch` and `release_inflight_blocks` applying it to `current_epoch`, the system is in a liminal state. `next_epoch` is set but `current_epoch` still reflects the old epoch.
**Impact:** By design — prevents premature block rejection. The lock on `block_state_machine` makes the update atomic with the block's Computed state.

### GSDK-026: Coinbase Address Hardcoded to Zero

**File:** `bin/gravity_node/src/reth_cli.rs:189`
**Issue:** `coinbase: Address::ZERO` — the block beneficiary is always the zero address. Block rewards (if any) go to the zero address. The code contains `// TODO(gravity_jan): add reth coinbase`.
**Impact:** Fee distribution is non-functional. Miners/validators receive no block rewards at the execution layer.

### GSDK-027: Validators May Temporarily Observe Different Epochs

**Location:** Cross-repository
**Issue:** During epoch transitions, validators process the epoch-change block at different wall-clock times. Some may be in `EpochChange` state while others are in `Ready`. This is handled by the AptosBFT consensus protocol which coordinates epoch transitions through its own reset mechanism.
**Impact:** No correctness risk — consensus ensures all validators agree on the epoch boundary. Temporal divergence is expected.

---

## Cross-Reference to Prior Audits

| Prior Finding | Status | Relation to New Findings |
|---|---|---|
| GSDK-006 (Panic unwrap in reth_cli.rs) | Open | Partially overlaps with GSDK-022 (loop termination on error) |
| GSDK-007 (Relayer panic on missing cached result) | Open | Different code path from GSDK-017 (nested mutex) |

---

## Architectural Recommendations

### R-01: Merge latest_epoch_change_block_number into BlockStateMachine (GSDK-017)

Since every access to `latest_epoch_change_block_number` already holds `block_state_machine`, the separate mutex provides no benefit. Merge it as a field of `BlockStateMachine`.

### R-02: Add Epoch Cancellation Tokens (GSDK-018)

Implement `tokio_util::sync::CancellationToken` per epoch. When an epoch transition occurs, cancel the token for the old epoch. All in-flight block tasks check this token instead of waiting for barrier timeouts.

### R-03: Return Distinguishable Result for Dropped Blocks (GSDK-019)

Change `set_ordered_blocks` to return `Err(BlockDropped)` instead of `Ok(())` for silently dropped blocks. Add monitoring metrics.

### R-04: Fix pop_txns Gas Accounting (GSDK-024)

Remove the `total_gas_limit == 0` special case:
```rust
.position(|item| {
    if total_gas_limit + item.gas_limit > gas_limit || count >= max_size {
        return true;
    }
    total_gas_limit += item.gas_limit;
    count += 1;
    false
})
```

---

*Report generated by Claude Opus 4.6 multi-agent audit framework.*
