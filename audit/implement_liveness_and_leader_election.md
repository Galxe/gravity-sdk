# implement_liveness_and_leader_election

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 226633ms
- **Steps**: 1

## Report

# Consensus Liveness Security Audit: gravity-sdk

## Executive Summary

This audit covers the leader election, pacemaker/timeout, and round state machine in `aptos-core/consensus/src/liveness/` and `round_manager.rs`, with focus on gravity-sdk-specific modifications that could affect chain liveness or safety under Byzantine conditions.

---

## 1. Leader Election Algorithm

### Architecture

The election system is layered through a chain of wrappers:

```
UnequivocalProposerElection
  └─ CachedProposerElection
       └─ LeaderReputation  (production)
          RotatingProposer   (testing)
          RoundProposer      (testing)
```

### `LeaderReputation` — Weighted Election (`leader_reputation.rs`)

**Selection weight** = `reputation_weight × voting_power (stake)`

Three reputation tiers via `ProposerAndVoterHeuristic`:
| Tier | Weight | Condition |
|------|--------|-----------|
| Active | 100 | Proposed or voted within sliding window |
| Inactive | 10 | No recent participation |
| Failed | 1 | Failure rate > `failure_threshold_percent` |

**Election seed construction:**
```
use_root_hash=true:  SHA3-256(accumulator_root_hash ‖ epoch ‖ round)
use_root_hash=false: SHA3-256(epoch ‖ round)
```

The seed feeds into `choose_index()`, which performs weighted random selection over cumulative stake-weighted reputation scores using `next_in_range()` (SHA3-256 → u128 modular reduction).

### Stake-Grinding Resistance

**Finding 1 — `exclude_round` lag mechanism.** History is fetched at `round - exclude_round` rather than the current round, creating a deliberate gap between the state a validator can influence and the state used for election. This is the primary anti-grinding defense: a validator cannot immediately benefit from manipulating the most recent blocks because those blocks are excluded from the reputation window.

**Finding 2 — `use_root_hash` is critical for unpredictability.** When `use_root_hash = false`, the seed is `(epoch, round)` — fully predictable. When `true`, the accumulator root hash from the committed state is included, making the seed depend on the entire transaction history. However, in the **DAG anchor election adapter** (`leader_reputation_adapter.rs`), the root hash is **hardcoded to `HashValue::zero()`** with a TODO comment, meaning the DAG path effectively has a predictable seed despite `use_root_hash=true` being set.

**Finding 3 — DAG adapter chain health clamping.** `CommitHistory::get_voting_power_participation_ratio()` clamps any ratio below 0.67 to 1.0 with a `// TODO: fix this once leader reputation is fixed` comment. This suppresses chain health backpressure signals in the DAG path, meaning degraded participation won't trigger the expected block size reduction.

### Equivocation Protection (`unequivocal_proposer_election.rs`)

Tracks `(Round, HashValue)` in a mutex. Rejects:
- A proposal from a non-elected author
- A second distinct proposal from the same author in the same round
- Any proposal for a round earlier than the last seen

Logs `SecurityEvent::InvalidConsensusProposal` on rejection. The single-element tracking (only the most recent `(round, block_id)`) is sufficient because rounds are monotonically increasing.

---

## 2. Pacemaker / Timeout Logic

### No `pacemaker.rs` Exists

The traditional pacemaker is fully refactored into **`RoundState`** (`round_state.rs`).

### Exponential Backoff Formula

```
timeout = base_ms × exponent_base^min(round_index, max_exponent) × 2
```

Where `round_index` = rounds elapsed since `highest_ordered_round + 3`. The `+3` accounts for the 3-chain commit rule in HotStuff — within 3 rounds of the last commit, the base timeout is used (index=0).

**Finding 4 — Double multiplier in `setup_timeout`.** All callers pass `multiplier=1`, but internally `setup_deadline` is called with `multiplier * 2 = 2`. This means actual timeout duration is **2× the computed exponential interval**. This is intentional design (gives the network more time before triggering timeout votes) but means the configured `base_ms` does not directly represent the actual timeout — it represents half the actual timeout.

**Finding 5 — Timeout re-arms on the same round, does NOT advance.** `process_local_timeout()` re-arms the timer for the same round and returns `true`. Round advancement only happens when a `TwoChainTimeoutCertificate` is formed from a quorum of timeout votes and processed via `process_certificates()`. This is correct BFT behavior — a single node's timeout cannot unilaterally advance the round.

### Timeout Certificate (TC) Formation Flow

```
1. Timer fires → process_local_timeout(R) → returns true
2. Round manager generates nil block, signs timeout vote
3. Broadcasts timeout vote to all validators
4. PendingVotes collects timeout votes
5. Quorum reached → TwoChainTimeoutCertificate formed
6. TC broadcast via SyncInfo
7. process_certificates(SyncInfo{TC}) → new_round = TC.round + 1
8. NewRoundEvent emitted with reason=Timeout
```

**Finding 6 — Non-validator bypass.** Gravity-SDK adds an `is_validator: bool` field to `RoundState`. Non-validators:
- Never schedule timeouts
- Return `None` from `process_certificates` even when round advances
- Short-circuit `process_local_timeout` immediately

This correctly prevents observer/VFN nodes from participating in liveness signaling, but means a misconfigured validator running as non-validator would silently stop participating in consensus without any alert beyond log messages.

---

## 3. Round State Machine Transitions

### Round Advancement (`process_certificates`)

```rust
new_round = max(QC.round, TC.round) + 1
if new_round > current_round → advance
if new_round <= current_round → ignore (stale)
```

**Finding 7 — No stuck-round protection beyond exponential backoff.** If a validator cannot form a QC (network partition) or TC (< 2f+1 nodes timing out), the round will keep re-arming timeouts with exponential backoff until `max_exponent` is reached, then plateau. There is no circuit-breaker or epoch-change trigger from pure round staleness. Liveness depends entirely on eventually reaching a quorum for either QC or TC.

### Vote Back-Pressure

```rust
fn sync_only(&self) -> bool {
    self.local_config.sync_only || self.block_store.vote_back_pressure()
}
```

When `vote_back_pressure()` returns true, the node:
- Stops voting on proposals
- Only broadcasts `SyncInfo`
- Proposals encountering backpressure are re-queued via `resend_verified_proposal_to_self()` with 10ms polling

**Finding 8 — Backpressure-induced liveness risk.** If enough validators simultaneously enter vote backpressure (execution pipeline overloaded), the network could lose the ability to form QCs. The timeout path would still function (nil blocks don't require execution), but chain progress would halt until backpressure clears. The 10ms polling interval (`BACK_PRESSURE_POLLING_INTERVAL_MS`) in `resend_verified_proposal_to_self` creates a tight loop that could exacerbate CPU pressure during overload.

---

## 4. Gravity-SDK Modifications to Upstream Liveness

### Critical Modification: Proposer Sleep (`round_manager.rs:382-386`)

```rust
tokio::time::sleep(Duration::from_millis(
    std::env::var("APTOS_PROPOSER_SLEEP_MS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(200),
)).await;
```

**Finding 9 — 200ms mandatory proposer delay.** Every `process_new_round_event` sleeps for 200ms (configurable via env var) before generating a proposal. This is not present in upstream Aptos.

- **Liveness impact:** Adds 200ms of latency to every round, reducing theoretical maximum throughput.
- **Safety concern:** The env var uses `.unwrap()` on parse — a malformed value will **panic the consensus task**, halting the node. This is a denial-of-service vector if the environment is attacker-controlled.
- **Interaction with timeouts:** The 200ms sleep eats into the round timeout budget. If `base_ms` is configured too low (< ~500ms accounting for the 2× multiplier), the proposer might not have enough time to generate and broadcast a proposal before the round times out.

### Critical Modification: `GravityBlockExecutor` (`gravity_state_computer.rs`)

```
commit path: BlockBufferManager.set_commit_blocks() → persist_notifier.recv()
pre_commit: no-op (returns Ok(()))
```

**Finding 10 — Synchronous persistence wait in commit path.** `commit_blocks()` calls `persist_notifier.recv()`, blocking the consensus commit path until gravity's persistence layer confirms. If the persistence layer hangs or is slow:
- The consensus thread blocks
- No new rounds can be processed
- The node will eventually timeout and be perceived as failed by peers
- Reputation penalty accumulates (failed_weight=1), reducing future election probability

This creates a tight coupling between gravity's persistence layer and consensus liveness that does not exist in upstream Aptos.

**Finding 11 — `pre_commit_block` is a no-op.** Standard Aptos uses pre-commit as a durability checkpoint. Gravity skips this entirely, pushing all durability to the `BlockBufferManager` path. If `BlockBufferManager` fails between execution and commit, blocks may need to be re-executed.

### Epoch Change Handling

```rust
VerifiedEvent::EpochChange(epoch) => {
    if !self.wait_change_epoch_flag {
        self.block_store.fast_forward_sync_by_epoch(..., epoch).await?;
        self.wait_change_epoch_flag = true;
    }
}
```

**Finding 12 — One-shot epoch change flag.** `wait_change_epoch_flag` is set to `true` after the first epoch change and never reset. If a second `EpochChange` event arrives in the same `RoundManager` lifecycle, it will be silently ignored. This is likely intentional (epoch change triggers a full restart), but if the first `fast_forward_sync_by_epoch` partially fails and returns `Ok`, the node would be stuck in a state where it believes epoch change succeeded but hasn't fully synchronized.

### Event Loop Priority Bias (`round_manager.rs:1483+`)

```
Priority 1: Shutdown
Priority 2: Delayed QC
Priority 3: Buffered proposals (batch-drained, sorted by round)
Priority 4: Payload-ready callbacks
Priority 5: Votes, OrderVotes, SyncInfo, LocalTimeout, EpochChange
```

**Finding 13 — Timeout events at lowest priority.** `LocalTimeout` events share the lowest priority bucket with votes and sync messages. Under heavy proposal load, timeout processing could be delayed, extending the effective timeout beyond the configured value. However, since the timer is managed by `TimeService` (separate from the event loop), the timeout *message* is enqueued at the correct time — only processing is delayed.

---

## 5. Summary of Findings

| # | Area | Severity | Description |
|---|------|----------|-------------|
| 1 | Leader Election | Info | `exclude_round` lag prevents immediate stake-grinding |
| 2 | Leader Election | **Medium** | DAG adapter uses `HashValue::zero()` as root hash, making election seed predictable in DAG mode |
| 3 | Leader Election | **Low** | DAG chain health ratio clamped below 0.67 → backpressure suppressed |
| 4 | Timeout | Info | Actual timeout is 2× configured `base_ms` |
| 5 | Timeout | Info | Timeout re-arms same round; TC required to advance (correct BFT) |
| 6 | Timeout | **Low** | Misconfigured validator as non-validator silently stops participating |
| 7 | Round State | Info | No stuck-round circuit breaker beyond exponential backoff plateau |
| 8 | Liveness | **Medium** | Simultaneous vote backpressure across validators can halt chain progress; 10ms polling loop may worsen CPU pressure |
| 9 | Gravity-SDK | **Medium** | 200ms proposer sleep: adds latency, `.unwrap()` on env parse can panic consensus, eats into timeout budget |
| 10 | Gravity-SDK | **High** | Synchronous `persist_notifier.recv()` in commit path — persistence layer hang blocks consensus entirely |
| 11 | Gravity-SDK | **Low** | `pre_commit_block` no-op removes upstream durability checkpoint |
| 12 | Gravity-SDK | **Low** | One-shot `wait_change_epoch_flag` — partial epoch sync failure leaves node stuck |
| 13 | Round Manager | **Low** | Timeout processing at lowest event loop priority — can be delayed under high proposal load |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Consensus Liveness Security Audit: gravity-sdk

## Executi | 226633ms |
