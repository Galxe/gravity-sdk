# attack_aptos_consensus_types_and_voting

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 898637ms
- **Steps**: 1

## Report

# Security Audit Report: Consensus Types & Core Consensus

## Intention vs. Implementation Attack Surface Analysis

---

## CRITICAL Findings

### C1: `WrappedLedgerInfo::verify` Genesis Bypass Uses Different Field Than `QuorumCert::verify`

**File:** `wrapped_ledger_info.rs:91` vs `quorum_cert.rs:124`

`QuorumCert::verify` checks `self.certified_block().round() == 0` (the vote data's proposed block round), while `WrappedLedgerInfo::verify` checks `self.ledger_info().ledger_info().round() == 0` (the ledger info's round). The code itself contains a TODO acknowledging this inconsistency:

```rust
// TODO: Earlier, we were comparing self.certified_block().round() to 0. Now, we are
// comparing self.ledger_info().ledger_info().round() to 0. Is this okay?
```

**Impact:** These are semantically different fields. The `LedgerInfo` round and the certified block round can diverge (especially in pipelined execution where ordering and execution are decoupled). An attacker who can craft a `WrappedLedgerInfo` where `ledger_info.round() == 0` but the actual certified block is non-genesis could **bypass all signature verification**. The only guard is that `get_num_voters() == 0`, but the attacker controls the `AggregateSignature` bitmap — an empty signature set trivially passes this check. This could allow a forged genesis-like `WrappedLedgerInfo` to be accepted without any quorum proof.

**Severity: CRITICAL**

---

### C2: `create_merged_with_executed_state_without_checked` — Unchecked State Merge (Latent Bomb)

**File:** `quorum_cert.rs:159-164`

```rust
pub fn create_merged_with_executed_state_without_checked(
    &self, executed_ledger_info: LedgerInfoWithSignatures,
) -> QuorumCert {
    Self::new(self.vote_data.clone(), executed_ledger_info)
}
```

This method replaces the `signed_ledger_info` inside a `QuorumCert` with an arbitrary `LedgerInfoWithSignatures` **without any validation** that the new ledger info matches the original vote data. This breaks the fundamental binding between what was voted on and what is being committed.

**Current status:** Zero callers found — this is dead code. However, it is `pub` (public API), not `#[cfg(test)]`-gated, and not marked deprecated. Any future caller could silently bypass the `match_ordered_only` consistency check that the checked variant enforces, enabling a state commitment that was never actually voted on by the quorum.

**Severity: CRITICAL (latent — no current exploitation path, but public API surface)**

---

### C3: Non-Atomic Block + Block-Number Persistence Creates Crash-Recovery Inconsistency

**File:** `consensusdb/mod.rs:214-244`

`save_blocks_and_quorum_certificates` and `save_block_numbers` are **two separate RocksDB write batches**. A crash between the two writes leaves blocks persisted without their block-number mapping in `BlockNumberSchema`.

On recovery, `get_data()` builds a `block_number_to_block_id` map from `BlockNumberSchema` and uses it to find the root block. If the block-number mapping is missing, the root block cannot be located, causing the recovery logic to fall back to `(epoch, 0, HashValue::zero())` — **losing the actual recovery root**.

Additionally, `get_data()` line 157-158 does `.unwrap()` on the root block lookup:
```rust
let block = self.get::<BlockSchema>(&(epoch, block_number_to_block_id[&latest_block_number]))?.unwrap();
```

If `BlockNumberSchema` references a block that was pruned from `BlockSchema` (since `BlockNumberSchema` entries are **never deleted** — see H2 below), this panics the node on startup, rendering it unable to recover.

**Severity: CRITICAL**

---

## HIGH Findings

### H1: SafetyData Cache Can Diverge from Persistent Store

**File:** `persistent_safety_storage.rs:129-142`

When `enable_cached_safety_data = true`, `safety_data()` returns the in-memory cache without ever re-validating against the backing store. If the backing store is modified externally (snapshot restore, another process, manual intervention), the cache will serve **stale `last_voted_round`** values.

**Impact:** If the restored snapshot has a higher `last_voted_round` than the cache, the node could sign a vote for a round it already voted for — violating BFT safety guarantees (equivocation). If the restored snapshot has a lower `last_voted_round`, the node loses votes it already cast, which is less dangerous but still incorrect.

**Severity: HIGH**

---

### H2: `BlockNumberSchema` Entries Never Pruned on Block Deletion

**File:** `consensusdb/mod.rs:246-258`

`delete_blocks_and_quorum_certificates` removes entries from `BlockSchema` and `QCSchema` but **never removes** corresponding `BlockNumberSchema` entries. This causes:
1. Unbounded growth of the block-number index over the node's lifetime
2. Dangling references to non-existent blocks that can trigger panics during recovery (feeds into C3)

**Severity: HIGH**

---

### H3: `LedgerMetadataDb::write_schemas` Panics on DB Error

**File:** `consensusdb/ledger_db/ledger_metadata_db.rs:116`

```rust
Err(e) => panic!("{}", e),
```

Any RocksDB write failure (disk full, corruption, I/O error) causes an **unrecoverable panic** instead of returning an error. This prevents graceful degradation, retry logic, or clean shutdown. In a consensus system, a panic during ledger metadata write can leave the DB in a partially-committed state that cascades into recovery failures.

**Severity: HIGH**

---

### H4: `DAGNode` Schema Uses `()` Key — Only One Node Storable

**File:** `consensusdb/schema/dag/mod.rs:23`

`NodeSchema` is defined with key type `()`, meaning `encode_key` produces an empty `Vec<u8>`. Every write to the `"node"` column family overwrites the single entry. If DAG consensus depends on persisting multiple nodes, this is a silent data loss bug.

**Severity: HIGH (if DAG consensus is enabled; otherwise inert)**

---

## MEDIUM Findings

### M1: `VoteData::verify` Allows `proposed.version() == 0` to Bypass Version Ordering

**File:** `vote_data.rs:77-78`

```rust
self.proposed.version() == 0 || self.parent.version() <= self.proposed.version(),
```

The comment explains this is for "decoupled execution" where versions are dummy values. However, a malicious proposer could set `proposed.version() = 0` on a non-decoupled-execution block to bypass the version monotonicity check, potentially causing state confusion downstream if version ordering is assumed elsewhere.

**Severity: MEDIUM**

---

### M2: `get_max_epoch` Silently Defaults to Epoch 1 on Iterator Error

**File:** `consensusdb/mod.rs:390-396`

```rust
let max_epoch = match iter.next() {
    Some(Ok(((epoch, _), _))) => epoch,
    _ => 1,  // silent fallback
};
```

Any RocksDB iterator error (corruption, missing CF) is swallowed and treated as "epoch 1". This masks critical storage failures and could cause the node to operate in the wrong epoch, reading/writing blocks under incorrect epoch keys.

**Severity: MEDIUM**

---

### M3: `get_state_proof` Double-Unwrap Without Error Propagation

**File:** `consensusdb/include/reader.rs:83`

```rust
self.get::<LedgerInfoSchema>(&block_number).unwrap().unwrap()
```

A missing or corrupted `LedgerInfoSchema` entry panics the node during state proof serving rather than returning an error. This is a denial-of-service vector if an RPC client can trigger `get_state_proof` for a block number where the ledger info was not yet persisted.

**Severity: MEDIUM**

---

### M4: `commit_callback` Panics on Merge Mismatch Instead of Graceful Error Handling

**File:** `block_storage/block_tree.rs:550`

```rust
.create_merged_with_executed_state(commit_decision)
    .expect("Inconsistent commit proof and evaluation decision, cannot commit block");
```

All three call sites of `create_merged_with_executed_state` use `.expect()` (panic on error) rather than propagating the error. In the live consensus commit path (`commit_callback`), a mismatch between ordered and executed state — even due to a transient bug — takes down the entire node. The checked variant correctly detects inconsistency, but the caller converts the graceful error into a crash.

**Severity: MEDIUM**

---

### M5: Leader Election Predictability When `use_root_hash = false`

**File:** `liveness/leader_reputation.rs`

When `use_root_hash = false`, the leader selection seed is only `epoch || round` — fully deterministic and predictable by any observer. An adversary who knows the validator set and stake weights can predict all future leaders and target them for DoS before their round arrives.

**Severity: MEDIUM** (configuration-dependent)

---

## Summary

| ID | Severity | Finding | Status |
|---|---|---|---|
| C1 | **Critical** | `WrappedLedgerInfo::verify` genesis bypass uses wrong round field, may skip all signature verification | Active |
| C2 | **Critical** | Public unchecked merge method can forge QuorumCerts binding wrong execution state | Latent (dead code, but public) |
| C3 | **Critical** | Non-atomic block/block-number persist + unwrap-on-recovery = crash loop or lost root | Active |
| H1 | High | SafetyData cache divergence enables potential equivocation after snapshot restore | Active |
| H2 | High | BlockNumberSchema never pruned — unbounded growth + dangling refs | Active |
| H3 | High | LedgerMetadataDb panics on write error, no graceful degradation | Active |
| H4 | High | DAGNode schema uses `()` key — silent single-entry overwrite | Active if DAG enabled |
| M1 | Medium | `version == 0` bypass in VoteData verification | Active |
| M2 | Medium | Silent epoch=1 fallback masks DB errors | Active |
| M3 | Medium | Double-unwrap in state proof serving = DoS-triggerable panic | Active |
| M4 | Medium | All merge callers panic on mismatch instead of error propagation | Active |
| M5 | Medium | Predictable leader election seed without root hash | Configuration-dependent |

---

**Recommendation priorities:**
1. **Immediately** resolve C1 — validate whether `ledger_info().round()` vs `certified_block().round()` is intentional, and if so, add defensive checks to prevent forged genesis bypass.
2. **Gate C2** behind `#[cfg(test)]` or delete the unchecked merge variant entirely.
3. **Merge C3** block + block-number writes into a single atomic `SchemaBatch`, and add `BlockNumberSchema` cleanup to the deletion path.
4. Audit all `.expect()`/`.unwrap()` calls in the consensus DB and commit pipeline for graceful error handling.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Security Audit Report: Consensus Types & Core Consensus

# | 258608ms |
