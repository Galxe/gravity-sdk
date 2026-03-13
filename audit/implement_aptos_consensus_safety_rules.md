# implement_aptos_consensus_safety_rules

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 244945ms
- **Steps**: 1

## Report

# Safety Rules Engine — Implementation Analysis

## Entry Point & Crate Structure

**Crate:** `aptos-core/consensus/safety-rules/`
**Modification status:** No gravity-sdk-specific patches found. All files carry upstream Aptos copyright headers. The crate depends on `gaptos` (gravity-sdk's re-packaged Aptos libraries), so any type-level changes would reside there, not in this crate. `#![forbid(unsafe_code)]` is declared in `lib.rs`.

---

## Files/Contracts Involved

| File | Purpose |
|---|---|
| `src/safety_rules.rs` | Core `SafetyRules` struct, guard methods, `TSafetyRules` impl |
| `src/safety_rules_2chain.rs` | 2-chain protocol vote/timeout/order-vote guards |
| `src/persistent_safety_storage.rs` | `PersistentSafetyStorage` — typed KV wrapper for consensus state persistence |
| `src/t_safety_rules.rs` | `TSafetyRules` trait (public interface) |
| `src/consensus_state.rs` | `ConsensusState` — read-only monitoring snapshot (no keys) |
| `src/safety_rules_manager.rs` | `SafetyRulesManager` — factory, storage bootstrap, key rotation |
| `src/local_client.rs` | `LocalClient` — in-process `Arc<RwLock<SafetyRules>>` wrapper |
| `src/serializer.rs` | `SerializerClient`/`SerializerService` — JSON-over-wire IPC |
| `src/thread.rs` | `ThreadService` — dedicated-thread isolation |
| `src/process.rs` | `ProcessService` — separate-process isolation |
| `src/error.rs` | `Error` enum — all rejection reasons |
| `../consensus/src/metrics_safety_rules.rs` | `MetricsSafetyRules` — retry wrapper with metrics |
| `../consensus/src/epoch_manager.rs` | Creates `SafetyRulesManager`, calls `initialize()` on epoch changes |
| `../consensus/src/round_manager.rs` | Calls `sign_proposal`, `construct_and_sign_vote_two_chain`, `sign_timeout_with_qc` |

---

## Core Data Structures

### `SafetyRules` (safety_rules.rs:44)
```rust
pub struct SafetyRules {
    pub(crate) persistent_storage: PersistentSafetyStorage,
    pub(crate) validator_signer: Option<ValidatorSigner>,  // None until initialize()
    pub(crate) epoch_state: Option<EpochState>,
}
```

### `SafetyData` (from aptos-consensus-types)
```rust
pub struct SafetyData {
    pub epoch: u64,
    pub last_voted_round: u64,       // prevents double-voting
    pub preferred_round: u64,         // 2-chain head parent round (prevents equivocation)
    pub one_chain_round: u64,         // highest certified block round
    pub last_vote: Option<Vote>,      // idempotent re-delivery
    pub highest_timeout_round: u64,   // for order-vote guards
}
```

### `PersistentSafetyStorage` (persistent_safety_storage.rs:27)
```rust
pub struct PersistentSafetyStorage {
    enable_cached_safety_data: bool,
    cached_safety_data: Option<SafetyData>,  // write-through cache
    internal_store: Storage,                  // pluggable KV backend
}
```

Storage keys: `CONSENSUS_KEY`, `OWNER_ACCOUNT`, `SAFETY_DATA`, `WAYPOINT`.

---

## Execution Paths

### 1. Initialization (`guarded_initialize`, safety_rules.rs:236–316)

```
guarded_initialize(proof: &EpochChangeProof)
  ├─ Read current waypoint from persistent_storage
  ├─ proof.verify(&waypoint) — validate epoch change proof against trust anchor
  ├─ Extract next_epoch_state from final LedgerInfo
  ├─ If new_waypoint.version() > waypoint.version():
  │     └─ persistent_storage.set_waypoint(new_waypoint) — monotonic advance only
  ├─ Read current_epoch from safety_data
  ├─ Match current_epoch vs epoch_state.epoch:
  │     Greater → Err(WaypointOutOfDate)
  │     Less    → set_safety_data(SafetyData::new(epoch, 0, 0, 0, None, 0))
  │                *** ALL voting state reset to zero ***
  │     Equal   → no-op
  ├─ self.epoch_state = Some(epoch_state)
  └─ Key reconciliation:
        ├─ Look up expected public key from epoch verifier for this author
        ├─ If current signer already matches → Ok
        ├─ Else → consensus_sk_by_pk(expected_key):
        │     ├─ Try explicit key: "consensus_key_<pk_hex>"
        │     ├─ Fallback: default "consensus_key"
        │     ├─ Verify key.public_key() == expected pk
        │     └─ Set self.validator_signer = Some(ValidatorSigner::new(author, key))
        └─ On failure → self.validator_signer = None (disarms signing)
```

### 2. Voting (`guarded_construct_and_sign_vote_two_chain`, safety_rules_2chain.rs:54–96)

```
guarded_construct_and_sign_vote_two_chain(vote_proposal, timeout_cert)
  ├─ self.signer()? — fail if not initialized
  ├─ verify_proposal(vote_proposal):
  │     ├─ Read safety_data from storage
  │     ├─ verify_epoch(proposed_block.epoch(), &safety_data)
  │     ├─ verify_qc(proposed_block.quorum_cert()) — check QC signatures
  │     ├─ validate_signature(&epoch_state.verifier) — check proposal signature
  │     ├─ verify_well_formed()
  │     └─ gen_vote_data()
  ├─ If timeout_cert present → verify_tc(tc)
  ├─ Read safety_data from storage
  ├─ *** IDEMPOTENCY CHECK ***: if last_vote.round == proposed_block.round → return last_vote
  ├─ *** RULE 1 ***: verify_and_update_last_vote_round(round, &mut safety_data)
  │     └─ round <= last_voted_round → Err(IncorrectLastVotedRound)
  │     └─ safety_data.last_voted_round = round
  ├─ *** RULE 2 ***: safe_to_vote(block, timeout_cert)
  │     └─ round == qc_round + 1
  │        OR (round == tc_round + 1 AND qc_round >= hqc_round)
  │     └─ Else → Err(NotSafeToVote)
  ├─ observe_qc() — update one_chain_round, preferred_round
  ├─ construct_ledger_info_2chain() — 2-chain commit rule
  ├─ self.sign(&ledger_info)
  ├─ safety_data.last_vote = Some(vote)
  └─ persistent_storage.set_safety_data(safety_data) — persist atomically
```

### 3. Timeout Signing (`guarded_sign_timeout_with_qc`, safety_rules_2chain.rs:22–52)

```
guarded_sign_timeout_with_qc(timeout, timeout_cert)
  ├─ self.signer()?
  ├─ Read safety_data
  ├─ verify_epoch(timeout.epoch())
  ├─ timeout.verify(&verifier) — check timeout signatures
  ├─ If timeout_cert → verify_tc(tc)
  ├─ safe_to_timeout(timeout, tc, &safety_data):
  │     └─ (round == qc_round + 1 OR round == tc_round + 1)
  │        AND qc_round >= one_chain_round
  ├─ If timeout.round() < last_voted_round → Err
  ├─ If timeout.round() > last_voted_round → update last_voted_round
  ├─ update_highest_timeout_round(timeout, &mut safety_data)
  ├─ persistent_storage.set_safety_data(safety_data)
  └─ self.sign(&timeout.signing_format())
```

### 4. Proposal Signing (`guarded_sign_proposal`, safety_rules.rs:318–348)

```
guarded_sign_proposal(block_data)
  ├─ self.signer()?
  ├─ verify_author(block_data.author()) — must be this validator
  ├─ Read safety_data
  ├─ verify_epoch(block_data.epoch())
  ├─ If block_data.round() <= last_voted_round → Err(InvalidProposal)
  ├─ verify_qc(block_data.quorum_cert())
  ├─ verify_and_update_preferred_round(qc, &mut safety_data)
  │     *** Note: preferred_round NOT persisted here (comment: "save latency") ***
  └─ self.sign(block_data)
```

### 5. Commit Vote Signing (`guarded_sign_commit_vote`, safety_rules.rs:350–391)

```
guarded_sign_commit_vote(ledger_info_with_sigs, new_ledger_info)
  ├─ self.signer()?
  ├─ Verify old_ledger_info.commit_info().is_ordered_only()
  │   OR old == new commit_info (fast-forward sync path)
  ├─ Verify match_ordered_only(old, new)
  ├─ ledger_info.verify_signatures(&epoch_state.verifier) — 2f+1 check
  └─ self.sign(&new_ledger_info)
```

### 6. Order Vote (`guarded_construct_and_sign_order_vote`, safety_rules_2chain.rs:98–120)

```
guarded_construct_and_sign_order_vote(order_vote_proposal)
  ├─ self.signer()?
  ├─ verify_order_vote_proposal() — epoch, QC consistency
  ├─ Read safety_data
  ├─ observe_qc() — update one_chain_round, preferred_round
  ├─ safe_for_order_vote(block, &safety_data):
  │     └─ round > highest_timeout_round → Ok
  │     └─ Else → Err(NotSafeForOrderVote)
  ├─ self.sign(&ledger_info)
  └─ persistent_storage.set_safety_data(safety_data)
```

---

## Key Functions — Detailed Signatures and Behavior

### Double-Voting Prevention

| Function | Location | Behavior |
|---|---|---|
| `verify_and_update_last_vote_round(round, &mut safety_data)` | safety_rules.rs:191 | **Rejects if `round <= safety_data.last_voted_round`**. On success, sets `safety_data.last_voted_round = round`. This is the primary double-voting guard. |
| Idempotency check in `guarded_construct_and_sign_vote_two_chain` | safety_rules_2chain.rs:71–74 | If `last_vote.round == proposed_block.round`, returns the cached `last_vote` instead of re-signing — prevents conflicting votes on the same round while allowing retransmission. |

### Equivocation Prevention

| Function | Location | Behavior |
|---|---|---|
| `verify_and_update_preferred_round(qc, &mut safety_data)` | safety_rules.rs:156 | **Rejects if `qc.certified_block().round() < safety_data.preferred_round`**. Prevents voting for a fork with a weaker QC chain. |
| `safe_to_vote(block, maybe_tc)` | safety_rules_2chain.rs:146 | Enforces: `round == qc_round + 1` OR `(round == tc_round + 1 AND qc_round >= hqc_round)`. Prevents voting on rounds that don't properly extend the chain. |
| `safe_to_timeout(timeout, maybe_tc, safety_data)` | safety_rules_2chain.rs:125 | Enforces: `(round == qc_round + 1 OR round == tc_round + 1) AND qc_round >= one_chain_round`. |

### Private Key Handling

| Function | Location | Behavior |
|---|---|---|
| `signer(&self)` | safety_rules.rs:114 | Returns `&ValidatorSigner` or `Err(NotInitialized)`. Every signing path calls this first. |
| `sign<T>(&self, message)` | safety_rules.rs:106 | Delegates to `validator_signer.sign(message)`. Generic over `Serialize + CryptoHash`. |
| `consensus_sk_by_pk(pk)` | persistent_safety_storage.rs:101 | Looks up private key first by explicit key `"consensus_key_<pk_hex>"`, then fallback to default `"consensus_key"`. **Verifies `key.public_key() == pk` before returning.** |
| Key rotation in `storage()` | safety_rules_manager.rs:76–90 | Writes each historical consensus key under `"consensus_key_<pk_hex>"` to support offline validators catching up across key rotations. |

---

## State Changes

| Operation | What Changes | Persisted? |
|---|---|---|
| `guarded_initialize` (new epoch) | `SafetyData` reset to `(epoch, 0, 0, 0, None, 0)`, waypoint advanced, validator_signer set/cleared | Yes (safety_data + waypoint) |
| `guarded_construct_and_sign_vote_two_chain` | `last_voted_round` increased, `one_chain_round`/`preferred_round` potentially increased, `last_vote` set | Yes |
| `guarded_sign_timeout_with_qc` | `last_voted_round` potentially increased, `highest_timeout_round` potentially increased | Yes |
| `guarded_sign_proposal` | `preferred_round` potentially increased | **No** — comment: "we don't persist the updated preferred round to save latency (it'd be updated upon voting)" |
| `guarded_construct_and_sign_order_vote` | `one_chain_round`/`preferred_round` potentially increased | Yes |
| `guarded_sign_commit_vote` | None | No state mutation |

---

## Persistence Layer

### Storage Architecture

`PersistentSafetyStorage` wraps a pluggable `Storage` backend with a write-through cache:

- **Read path**: If `enable_cached_safety_data`, serves from `cached_safety_data`. Otherwise reads from `internal_store` on every call.
- **Write path**: Writes to `internal_store` first. On success, updates cache. **On failure, invalidates cache** (`cached_safety_data = None`) — conservative correctness.
- **Backend options**: `InMemoryStorage` (tests), `OnDiskStorage` (production default), `VaultStorage` (HSM-grade).
- **No WAL**: Crash safety depends entirely on the backend. `OnDiskStorage` performs synchronous filesystem writes with no atomic rename or fsync guarantees at this layer.

### Key Storage Schema

| Key | Value | Notes |
|---|---|---|
| `"safety_data"` | `SafetyData` (serialized) | Core BFT state — loss causes equivocation risk |
| `"consensus_key"` | `bls12381::PrivateKey` | Default consensus key |
| `"consensus_key_<pk_hex>"` | `bls12381::PrivateKey` | Epoch-specific rotated keys |
| `"owner_account"` | `Author` | Validator address |
| `"waypoint"` | `Waypoint` | Trust anchor — monotonically advancing |

---

## External Dependencies

| Dependency | Source | Used For |
|---|---|---|
| `gaptos::aptos_crypto::bls12381` | Re-packaged Aptos crypto | BLS signing primitives |
| `gaptos::aptos_types::ValidatorSigner` | Re-packaged Aptos types | Key holder, signing delegation |
| `gaptos::aptos_secure_storage::Storage` | Re-packaged Aptos secure storage | Pluggable KV backend |
| `gaptos::aptos_safety_rules::counters` | Re-packaged Aptos safety rules | Prometheus metrics counters |
| `aptos_consensus_types::safety_data::SafetyData` | Consensus types crate | BFT state struct |
| `aptos_consensus_types::timeout_2chain::*` | Consensus types crate | 2-chain timeout types |

---

## MetricsSafetyRules Retry Behavior

`MetricsSafetyRules` (in `consensus/src/metrics_safety_rules.rs`) wraps `Box<dyn TSafetyRules>` and provides:

1. **`monitor!("safety_rules", ...)` instrumentation** on every call.
2. **Auto-retry on transient errors**: catches `NotInitialized`, `IncorrectEpoch`, or `WaypointOutOfDate`, calls `perform_initialize()`, and retries once.
3. **`perform_initialize()`** loops calling `initialize(&proofs)` while the waypoint version keeps advancing — handles chunked epoch change proofs.
4. **`CommitSignerProvider` bridge**: exposes `sign_commit_vote` through `Mutex<MetricsSafetyRules>` for the pipeline signing phase.

---

## Service Deployment Modes

| Mode | Isolation | Description |
|---|---|---|
| `Local` | None (in-process) | `SafetyRules` behind `Arc<RwLock>` |
| `Thread` | Thread boundary | Dedicated OS thread |
| `Serializer` | Serialized IPC | JSON request/response over channel |
| `Process` | Process boundary | Separate OS process via TCP socket (strongest isolation) |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Safety Rules Engine — Implementation Analysis

## Entry Po | 244945ms |
