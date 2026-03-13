# attack_randomness_dkg_subsystem

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 490229ms
- **Steps**: 1

## Report

# Intention Attack Report: Randomness & DKG Implementation

## Red Team Adversarial Analysis

I compared the stated audit intention (comprehensive security audit of the randomness/DKG subsystem) against the actual implementation and found the following **fatal or high-impact logical vulnerabilities** that the initial audit either missed, downplayed, or mischaracterized.

---

## CRITICAL Findings

### 1. CertifiedAugData Skips Inner Delta Verification on Receipt — Quorum-Signed Malformed Deltas Are Applied Blindly

**Severity: CRITICAL**

**Location:** `aug_data_store.rs:108-119`, `types.rs:520-523`

**The Bug:** When a node receives a `CertifiedAugData` message from a peer, the verification path is:

```
RandMessage::CertifiedAugData → msg.verify() → CertifiedAugData::verify()
    → verifier.verify_multi_signatures(&self.aug_data, &self.signatures)  ← ONLY checks BLS aggregate sig
    → DOES NOT call AugmentedData::verify() / derive_apk()
```

Then in `add_certified_aug_data`, the delta is applied to the local `RandConfig` via `augment()` **without re-verifying the cryptographic well-formedness of the delta**.

**Attack Vector:** If a Byzantine quorum (>= threshold weight of colluding validators) signs a `CertifiedAugData` containing a **malformed or adversarially-crafted delta**, every honest node that receives it will:
1. Pass the aggregate signature check (the signatures are valid — the content is what's malicious)
2. Apply the poisoned delta to their local `RandConfig` via `add_certified_delta`
3. Produce incorrect randomness or crash during share generation

**Why the initial audit missed it:** The audit report (Finding 4, Risk: LOW) concluded "the API surface is purely observational" and did not trace the full `CertifiedAugData` verification chain to discover that `AugmentedData::verify()` is never called on the receiver path.

**Impact:** An adversary controlling threshold-weight of validators can poison the augmented public key state of all honest nodes, breaking the unpredictability guarantee of the entire randomness protocol for the epoch.

---

### 2. `process_reset` Does Not Clear In-Memory Share Map — Stale Shares Survive Round Resets

**Severity: CRITICAL**

**Location:** `rand_manager.rs:187-197`, `rand_store.rs`

**The Bug:** When `process_reset(ResetSignal::TargetRound(round))` is called (e.g., after state sync):

```rust
fn process_reset(&mut self, request: ResetRequest) {
    self.block_queue = BlockQueue::new();                          // blocks dropped ✓
    self.rand_store.lock().update_highest_known_round(target_round); // watermark updated
    // BUT: the in-memory shares HashMap inside RandStore is NOT cleared
}
```

The `rand_store`'s internal `HashMap<Round, RandItem>` retains shares from rounds **before the reset target**. After resetting to round N, if the node receives new block metadata for round M < N (which shouldn't happen but could during byzantine behavior or fork recovery), those stale shares could be matched against new metadata and aggregated.

**Attack Vector:** During a state sync event, a Byzantine peer could have pre-seeded shares for future rounds. After the reset, if those rounds are re-proposed with different block content, the stale shares (generated against different `RandMetadata`) survive in the store. The `retain()` pruning only fires when `add_metadata` is called — but if the metadata happens to match (same epoch+round, different block hash that isn't checked at insertion), stale shares could contribute to aggregation with mismatched randomness inputs.

**Impact:** Potential randomness bias or aggregation failure after state sync, as shares computed against one block could be aggregated with metadata for a different block at the same round.

---

## HIGH Findings

### 3. Secret Key Shares Stored in Plaintext RocksDB Without Encryption or Zeroize

**Severity: HIGH**

**Location:** `storage/db.rs`, `storage/schema.rs`, `types.rs`

**The Bug (chain of failures):**

1. **No encryption-at-rest:** `RandDb` opens RocksDB with `Options::default()` — no encryption. The `KeyPairSchema` column family stores raw `(epoch, Vec<u8>)` key bytes via BCS serialization directly to disk.

2. **No `Zeroize` on drop:** The secret key travels as `Vec<u8>` through `save_key_pair_bytes` / `get_key_pair_bytes`. Standard `Vec<u8>` does not zero memory on deallocation — the key material persists in freed heap pages.

3. **`Arc<RandKeys>` amplifies the problem:** `RandConfig` clones distribute the same `Arc<RandKeys>` to `RandStore`, `AugDataStore`, `ShareAggregateState`, and the `verification_task`. The secret key remains live in memory until **all** of these components are dropped — which only happens at epoch end.

4. **No process isolation:** Any process with read access to `<db_root>/rand_db/` can extract the secret key share, reconstruct the augmented secret key, and generate valid randomness shares for any round in the epoch.

**Impact:** Full compromise of a node's randomness contribution for the current (and potentially future) epochs. Combined with compromising threshold-many nodes, an attacker can predict or bias all randomness.

---

### 4. Equivocation in AugData Ceremony Detected But Not Punished — Free Grinding Attack

**Severity: HIGH**

**Location:** `aug_data_store.rs:93-106`, `rand_manager.rs:445-448`

**The Bug:** When a validator sends two different `AugData` payloads (equivocation), the receiving node:

```rust
pub fn add_aug_data(&mut self, data: AugData<D>) -> anyhow::Result<AugDataSignature> {
    if let Some(existing_data) = self.data.get(data.author()) {
        ensure!(existing_data == &data, "[AugDataStore] equivocate data from {}", data.author());
        // Returns Err(...) — but ONLY locally
    }
    // ...
}
```

The caller logs the error and moves on:
```rust
Err(e) => error!("[RandManager] Failed to add aug data: {}", e),
```

**Attack Vector:** A Byzantine validator can send **different `AugData` (different deltas) to different subsets of honest validators**. Each honest node only sees one version (no equivocation from its perspective), signs it, and returns a signature. The attacker collects signatures for multiple delta values, selects the one that produces the most favorable augmented public key, and broadcasts only that `CertifiedAugData`.

This is a **grinding attack on the augmentation ceremony**. The attacker gets multiple "rolls" at choosing their delta, breaking the one-shot commitment property that bias resistance depends on.

**Why the initial audit missed it:** The audit noted equivocation detection exists (Finding 6.4, Risk: INFORMATIONAL) but did not analyze the **split-brain equivocation vector** where different nodes see different data.

**Impact:** A single Byzantine validator can bias their augmented public key contribution, weakening the bias resistance of the randomness protocol proportional to the number of honest nodes they can partition.

---

### 5. `RequestShare` Messages Have Zero Verification — Amplification DoS

**Severity: HIGH**

**Location:** `network_messages.rs` (line ~47)

**The Bug:** The `RequestShare` variant's `verify()` implementation:

```rust
RandMessage::RequestShare(_) => Ok(()),  // no verification whatsoever
```

Any peer (even one not in the validator set) that can send authenticated RPC messages can spam `RequestShare` for arbitrary rounds. The recipient will:
1. Look up shares in `RandStore`
2. Re-serialize and transmit share data back
3. Consume CPU for share lookup and network bandwidth for response

**Attack Vector:** An attacker floods a validator with `RequestShare` messages for rounds [0, FUTURE_ROUNDS_TO_ACCEPT]. Each request triggers a store lookup and potential share retransmission. With `FUTURE_ROUNDS_TO_ACCEPT = 200`, the amplification is bounded but the CPU and bandwidth cost is real.

**Impact:** Network-layer DoS against the randomness subsystem. Could delay share aggregation enough to stall block finalization.

---

## MEDIUM Findings

### 6. Threshold Is `floor(total_weight / 2)`, Not Strict Majority

**Severity: MEDIUM**

**Location:** `types.rs:635-637`, test harness at line 399

**The Bug:**
```rust
let half_total_weights = weights.clone().into_iter().sum::<usize>() / 2;
let weighted_config = WeightedConfig::new(half_total_weights, weights).unwrap();
```

And the comparison:
```rust
if self.total_weight < rand_config.threshold() {  // strict less-than
```

This means aggregation triggers when `total_weight >= floor(total_weight / 2)` — exactly 50% of weight suffices, not 50%+1. For even total weights, this creates a scenario where two disjoint sets of validators, each holding exactly half the weight, could independently produce valid randomness — breaking uniqueness.

**Impact:** If the validator set weight distribution allows a 50/50 split, two conflicting randomness values could be produced for the same round. This is unlikely in practice with odd total weights but is a protocol-level correctness bug.

---

### 7. DKG Integration Tests Disabled — Silent Regression Window

**Severity: MEDIUM**

**Location:** `rand/dkg/mod.rs`

**The Bug:** All 8 DKG test modules are `#[cfg(test)]` **and** additionally disabled with a comment: *"DKG runtime tests are temporarily disabled due to API changes in gaptos"*. Combined with the `#![allow(dead_code)]` on the top-level `mod.rs`, this means:

- No automated validation that `gaptos` PVSS/WVUF APIs produce correct outputs
- No detection of serialization format changes between Aptos upstream and Gravity's fork
- The `WeightedConfig` threshold semantics (Finding 6) could have changed in `gaptos` without being caught

**Impact:** Any behavioral regression in `gaptos` cryptographic primitives would silently propagate to production.

---

## Summary: Intention vs. Implementation Gap

| Audit Intention | What Was Found | Verdict |
|---|---|---|
| DKG correctness | Core DKG is in `gaptos` (out of scope), local tests disabled | **Gap: untestable** |
| Bias resistance | AugData equivocation enables grinding (Finding 4) | **BROKEN for Byzantine validators** |
| Private key handling | Plaintext on disk, no zeroize, Arc-amplified lifetime | **HIGH risk** |
| DKG HTTP endpoint interaction | Correctly read-only | **OK** |
| Gravity-specific modifications | Disabled tests + modified deps = uncovered regression surface | **Gap** |
| Epoch transition integration | `process_reset` doesn't clear share map (Finding 2) | **CRITICAL state leak** |

**Bottom Line:** The initial audit rated overall risk as LOW-MEDIUM. This adversarial analysis elevates the assessment to **HIGH**, with two CRITICAL findings (CertifiedAugData verification bypass, stale shares surviving reset) and three HIGH findings (plaintext key storage, equivocation grinding, RequestShare DoS) that represent exploitable logical vulnerabilities in the randomness protocol's trust model.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: Randomness & DKG Implementation

 | 180212ms |
