# attack_aptos_consensus_safety_rules

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 116226ms
- **Steps**: 1

## Report

# Safety Rules Engine — Red Team Attack Report

## Scope
Security audit of `aptos-core/consensus/safety-rules/` — the BFT-critical signing guard that prevents double-voting, equivocation, and private key misuse.

---

## Critical Findings

### CRITICAL-01: Proposal Signing Does Not Persist `preferred_round` — Equivocation Window After Crash

**Location:** `safety_rules.rs:318–348` (`guarded_sign_proposal`)

**Detail:** After signing a proposal, `preferred_round` is updated in-memory but **explicitly not persisted** (comment: *"we don't persist the updated preferred round to save latency"*). If the validator crashes after signing a proposal but before casting a vote (which does persist), the restarted node will reload a stale `preferred_round` from disk.

**Attack:** A Byzantine leader can:
1. Feed this validator a proposal that advances `preferred_round` to R.
2. Crash the validator (network partition, OOM, power loss).
3. After restart, `preferred_round` reverts to a value < R.
4. Feed the validator a conflicting proposal on a fork with a weaker QC that should have been rejected by the `preferred_round` guard.
5. The validator signs a proposal on a conflicting fork — an equivocation.

**Severity: CRITICAL** — This is an intentional correctness/performance tradeoff that creates a real equivocation window. The comment acknowledges it ("it'd be updated upon voting"), but the assumption that a vote always follows a proposal before any crash is not guaranteed.

---

### CRITICAL-02: No Crash-Safety Guarantee at the Persistence Layer — Double-Sign After Power Loss

**Location:** `persistent_safety_storage.rs:145–162` (`set_safety_data`)

**Detail:** The write path delegates entirely to `internal_store.set()`. No `fsync`, no atomic rename, no WAL, no CRC/integrity check exists at this layer. The crash-safety guarantee is wholly dependent on the `gaptos::aptos_secure_storage` backend, which is a vendored dependency **not auditable from this codebase**.

**Attack scenario:**
1. Validator votes on round R, `set_safety_data` returns `Ok`, `last_voted_round = R` is in the page cache.
2. Power loss before OS flushes to disk.
3. On restart, `last_voted_round` reloads as the prior value (< R).
4. Validator is presented with a conflicting block for round R and votes again — **double-sign**.

**Compounding factor:** There is no integrity check (CRC, hash, HMAC) on the serialized `SafetyData`. A partially written or corrupted value that is structurally valid (e.g., zeroed `last_voted_round`) will be silently accepted.

**Severity: CRITICAL** — In PoS systems, double-signing leads to slashing. The entire safety guarantee of this component rests on an unverified backend.

---

## High Findings

### HIGH-01: Idempotency Check Compares Round Only — Not Block Hash

**Location:** `safety_rules_2chain.rs:71–74`

**Detail:** The idempotency guard checks `last_vote.round == proposed_block.round` and returns the cached `last_vote`. It does **not** verify that the `proposed_block` (its hash/ID) matches the block the cached vote was cast for.

**Attack:** If the cached `last_vote` is for block A at round R, and a Byzantine leader presents block B (different content, same round R), the guard returns the vote for block A. This is actually **safe** (it doesn't create a conflicting vote), but it means the idempotency check silently papers over a Byzantine leader equivocating. The validator will never detect or report that it received conflicting proposals for the same round from the same leader. This is a missed detection opportunity rather than a direct exploit, but in combination with other bugs it could mask attacks.

**Severity: HIGH** — Not directly exploitable for equivocation, but suppresses Byzantine fault detection.

### HIGH-02: Epoch Transition Resets ALL Safety State to Zero

**Location:** `safety_rules.rs:236–316` (`guarded_initialize`)

**Detail:** When `current_epoch < new_epoch`, all of `SafetyData` is reset:
```
SafetyData::new(epoch, 0, 0, 0, None, 0)
```
This sets `last_voted_round = 0`, `preferred_round = 0`, `one_chain_round = 0`, `highest_timeout_round = 0`.

**Attack:** If a validator processes an epoch change proof but the network has not yet fully transitioned, and old-epoch messages are still in-flight, the zeroed state could potentially interact with stale messages. The `verify_epoch` check should prevent this in the normal case, but any bug or bypass in epoch verification during the transition window would have catastrophic impact since all guards are effectively disabled (all thresholds are 0).

**Severity: HIGH** — The epoch check is the single point of defense during transition. If it has any edge-case bypass, all safety invariants are void.

### HIGH-03: Prometheus Counters Updated Before Durable Write

**Location:** `persistent_safety_storage.rs:145–152`

**Detail:** `counters::set_state(LAST_VOTED_ROUND, ...)` is called **before** `internal_store.set()`. If the write fails, Prometheus reports a `last_voted_round` that was never persisted.

**Attack:** An operator monitoring dashboards sees `last_voted_round = R` while the actual persisted value is R-1. During incident response, operators may believe the validator is in a safe state when it is not, leading to incorrect manual interventions (e.g., restarting the node thinking it's safe, when it's actually vulnerable to double-signing round R).

**Severity: HIGH** — Misleads operational monitoring for the most safety-critical metric in the system.

---

## Medium Findings

### MEDIUM-01: `consensus_sk_by_pk` Fallback to Default Key

**Location:** `persistent_safety_storage.rs:101`

**Detail:** Key lookup first tries `"consensus_key_<pk_hex>"`, then falls back to the default `"consensus_key"`. The fallback verifies `key.public_key() == expected_pk`, which is correct. However, this means the default key slot is read on every cache miss for a rotated key, creating a timing side-channel that reveals whether a key rotation has occurred.

**Severity: MEDIUM** — Information leak, not directly exploitable for signing.

### MEDIUM-02: `LocalClient` Uses `Arc<RwLock>` — No Process Isolation

**Location:** `local_client.rs`

**Detail:** In `Local` deployment mode, `SafetyRules` lives behind `Arc<RwLock<SafetyRules>>` in the same process as consensus. Any memory corruption, buffer overflow, or RCE in the consensus module gives direct access to the signing key in memory. The `Thread` and `Process` modes exist but are not the default.

**Severity: MEDIUM** — Reduces defense-in-depth. The `#![forbid(unsafe_code)]` mitigates but does not eliminate this risk (dependencies may contain unsafe code).

### MEDIUM-03: Write-Through Cache Has No Cross-Process Guard

**Location:** `persistent_safety_storage.rs:27`

**Detail:** The `cached_safety_data` is process-local with no file lock or fencing token. If two processes (e.g., misconfigured deployment, zombie process) share the same storage file, they will have independent caches and can independently advance `last_voted_round`, resulting in conflicting votes.

**Severity: MEDIUM** — Requires operational misconfiguration, but the system provides no defense against it (no PID file, no flock, no fencing).

---

## Summary

| ID | Severity | Finding | Exploitable? |
|---|---|---|---|
| CRITICAL-01 | **Critical** | `preferred_round` not persisted after proposal signing — equivocation after crash | Yes, by Byzantine leader + crash |
| CRITICAL-02 | **Critical** | No verified crash-safety (fsync/atomic write) — double-sign after power loss | Yes, by power loss timing |
| HIGH-01 | High | Idempotency check on round only, not block hash — suppresses Byzantine detection | Detection gap |
| HIGH-02 | High | Epoch transition zeros all safety state — single-point-of-failure on epoch check | Conditional |
| HIGH-03 | High | Prometheus counters updated before durable write — misleads operators | Operational risk |
| MEDIUM-01 | Medium | Key lookup fallback leaks rotation timing | Information leak |
| MEDIUM-02 | Medium | Default `Local` mode has no process isolation for signing keys | Reduces defense-in-depth |
| MEDIUM-03 | Medium | No cross-process fencing on storage file | Requires misconfig |

**Bottom line:** The two critical findings (CRITICAL-01 and CRITICAL-02) represent real double-signing/equivocation vectors that are exploitable under crash conditions. For a BFT validator where slashing penalties exist, these are not theoretical — they are the exact class of bug that causes fund loss. The `preferred_round` persistence skip (CRITICAL-01) is the most concerning because it is an **intentional design decision** that trades correctness for latency, documented in a code comment.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Safety Rules Engine — Red Team Attack Report

## Scope
Sec | 116226ms |
