# attack_liveness_and_leader_election

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 289944ms
- **Steps**: 1

## Report

# Red Team Attack Report: Consensus Liveness Security Audit — gravity-sdk

## Methodology

All findings from the implementation report were verified against source code by four independent exploration agents. Additional attack surfaces were discovered through targeted hunting for authorization gaps, TOCTOU races, state mutation failures, unbounded channels, and integer overflows. This report covers only **fatal logical vulnerabilities**.

---

## Critical Findings

### CRITICAL-1: Synchronous Persistence Blocking Creates Single-Point Consensus Kill Switch

**Location:** `gravity_state_computer.rs:86-130, 139-192`

Both `commit_blocks()` and `commit_ledger()` call `self.runtime.block_on(...)` then loop over `notifier.recv().await` for every block in the batch. The consensus thread **cannot process any events** — including timeouts — until all notifiers fire.

**Attack:**
- If `BlockBufferManager` hangs (disk I/O stall, downstream deadlock), the consensus thread is **permanently blocked** — the node cannot even emit timeout votes.
- An adversary degrading the storage layer of **f+1 validators** (not 2f+1) halts the entire chain: no QC forms (missing votes) and no TC forms (blocked nodes can't timeout-vote either).
- The `pre_commit_block` no-op (confirmed: bare `Ok(())`) removes the upstream Aptos durability checkpoint, funneling all durability through this single blocking path.

**Severity: Critical** — Violates the fundamental BFT assumption that consensus can make progress independently of execution/storage speed.

---

### CRITICAL-2: Unbounded Channel Enables Memory Exhaustion DoS

**Location:** `round_state.rs:167` — `delayed_qc_tx: UnboundedSender<DelayedQcMsg>`

The delayed QC channel is `futures_channel::mpsc::unbounded()`. Every vote triggering the delayed QC path enqueues a message with zero backpressure.

**Attack:**
- A Byzantine validator floods cryptographically valid votes triggering delayed QC processing.
- The consumer (round manager event loop, **priority 4 — lowest**) falls behind under load.
- The channel grows without bound, exhausting node memory.
- Timeout events share the same lowest-priority bucket, so the recovery mechanism is itself starved by the flood.

**Severity: Critical** — Remote memory exhaustion by any validator in the set, no rate limiting or capacity bound.

---

## High Findings

### HIGH-1: Environment Variable Parse Panic Kills Consensus Task

**Location:** `round_manager.rs:384`, `epoch_manager.rs:1479`, `consensusdb/include/reader.rs:43,47`

Four env vars use `std::env::var("X").map(|s| s.parse().unwrap()).unwrap_or(default)`. The `.unwrap()` is on `parse()` — if the var is **set but malformed** (e.g., `"20o"`), the node panics on first round.

**Affected vars:** `APTOS_PROPOSER_SLEEP_MS`, `ENABLE_QUORUM_STORE`, `FIXED_PROPOSER`

**Severity: High** — Trivial DoS if environment is attacker-influenced (container sidecar, config injection).

---

### HIGH-2: Three Silent Fallback Paths Degrade Leader Election to Predictable Selection

**Location:** `leader_reputation.rs:144, 154, 176`

Three code paths return `HashValue::zero()` as the root hash, collapsing the election seed to `SHA3-256(epoch || round)` — fully predictable:

| Path | Trigger | Log Level |
|------|---------|-----------|
| Empty event set | Event pruning | `warn!` |
| `get_accumulator_root_hash` failure | Transient DB error | `error!` |
| DB init failure | Startup race | `warn!` |

Only 1 of 3 paths logs at `error!`. The DAG adapter **permanently** uses `HashValue::zero()` (hardcoded). A validator who knows the deterministic election schedule can pre-compute and strategically time participation to maximize future election probability.

**Severity: High** — Silent, potentially persistent degradation of leader election unpredictability.

---

## Medium Findings

### MEDIUM-1: Simultaneous Vote Backpressure Creates Network-Wide Deadlock

**Location:** `block_store.rs:994-1004`

`vote_back_pressure()` is a purely local check (`ordered_round - commit_round > limit`). No cross-validator coordination. If gravity's synchronous commit path (CRITICAL-1) slows all validators simultaneously, all suppress votes → no QC → no commits → gap never shrinks → **permanent deadlock**. The 10ms polling loop (`BACK_PRESSURE_POLLING_INTERVAL_MS = 10`) further taxes CPU during overload.

**Severity: Medium** — Requires correlated slowdown, but CRITICAL-1 makes this much more likely than upstream Aptos.

---

### MEDIUM-2: TOCTOU in Proposal Generation — Leader Check Before Async Sleep

**Location:** `proposal_generator.rs:354-362`

The `last_round_generated` mutex is released immediately after the round-check. The function then enters a long async path including gravity's **200ms proposer sleep**. During this window, the round/epoch can advance. The proposal is still generated for the stale context. Safety rules are the backstop, but the TOCTOU window is 200ms+ wide.

**Severity: Medium**

---

### MEDIUM-3: Vote Inserted Before Validator-Set Membership Gate

**Location:** `round_manager.rs:1228-1277`, `round_state.rs:304-317`

`insert_vote()` mutates `pending_votes` before the verifier's signature check completes downstream. No early "is this author in the validator set?" rejection before state mutation. If the verifier is bypassed or buggy, the vote permanently occupies a slot.

**Severity: Medium**

---

### MEDIUM-4: `u32` Counter Overflow in Reputation Scoring

**Location:** `leader_reputation.rs:393-401, 518-519`

Vote/proposal counts are `u32` with unchecked `*count += 1`. The failure threshold uses `cur_failed_proposals * 100` — unchecked multiplication overflowing at ~42.9M. Counter wrap makes active validators appear inactive (weight 100→10) or failing validators appear healthy, directly manipulating leader election weights.

**Severity: Medium**

---

### MEDIUM-5: 200ms Proposer Sleep Consumes Critical Timeout Budget

**Location:** `round_manager.rs:382-386`

With the confirmed 2× multiplier in `setup_timeout`, a `base_ms` of 250 yields 500ms real timeout — 200ms (40%) consumed before any work begins. Under initial exponential backoff rounds, the proposer may timeout despite being alive, degrading liveness.

**Severity: Medium**

---

## Cascading Attack Chain

The most dangerous aspect is the **composability** of these findings:

```
Storage layer degradation (f+1 nodes)
  → CRITICAL-1: Consensus threads block on persist_notifier.recv()
  → Blocked nodes stop emitting votes AND timeout votes
  → MEDIUM-1: Remaining nodes hit vote backpressure
  → No QC forms, no TC forms
  → CRITICAL-2: Vote flood during recovery fills unbounded channel
  → Memory exhaustion on recovering nodes
  → Complete chain halt
```

This requires degrading only **f+1** validators (not 2f+1) because synchronous blocking prevents those nodes from participating in timeout certificate formation.

---

## Recommendations (Priority Order)

| Priority | Fix | Target |
|----------|-----|--------|
| **P0** | Make `commit_blocks`/`commit_ledger` non-blocking — bounded channel with timeout, allow TC participation during slow persistence | CRITICAL-1 |
| **P0** | Bound `delayed_qc_tx` channel — replace `UnboundedSender` with bounded; drop on overflow | CRITICAL-2 |
| **P1** | Replace `.unwrap()` on `parse()` with `.unwrap_or(default)` across all env var sites | HIGH-1 |
| **P1** | Promote all `HashValue::zero()` fallbacks to `error!` + metric counter; fix DAG adapter | HIGH-2 |
| **P2** | Add early validator-set membership check before `insert_vote` | MEDIUM-3 |
| **P2** | Use `checked_mul`/`saturating_add` for reputation counters | MEDIUM-4 |
| **P3** | Make proposer sleep proportional to `base_ms` or remove entirely | MEDIUM-5 |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Red Team Attack Report: Consensus Liveness Security Audit  | 289944ms |
