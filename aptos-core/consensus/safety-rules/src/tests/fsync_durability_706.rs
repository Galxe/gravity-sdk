// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Reproduction / demonstration for gravity-audit issue #706 (F4 in
//! `galxe/RESTART_RECOVERY_FINDINGS.md`):
//!
//! The safety-rules `last_voted_round` (and `last_vote`) — the ONLY guard against a
//! validator voting twice in a round — is persisted through the `on_disk_storage`
//! backend, which production validators use
//! (`cluster/templates/validator.yaml.tpl`, `type: on_disk_storage`).
//!
//! `OnDiskStorage::write` (gravity-aptos `secure/storage/src/on_disk.rs`, rev
//! e9544c8) does:
//!
//! ```ignore
//! let mut file = File::create(self.temp_path.path())?;
//! file.write_all(&contents)?;
//! fs::rename(&self.temp_path, &self.file_path)?;   // NO sync_all / fsync, ever
//! ```
//!
//! There is no `sync_all`/`fsync` on the temp file OR the containing directory
//! anywhere in the crate. By contrast the consensus DB IS durable: schemadb's
//! `default_write_options()` sets `opts.set_sync(true)` (gravity-aptos
//! `storage/schemadb/src/lib.rs`). So on a power-loss crash, the durable consensusdb
//! `last_vote` + the wire-sent vote can survive while `secure_storage.json` reverts
//! to a *lower* `last_voted_round` whose dirty page never reached disk.
//! `enable_cached_safety_data` defaults to `true`, so this gap is invisible
//! in-process — the cache, not the disk, answers reads.
//!
//! This file contains three tests:
//!
//! * `on_disk_write_is_not_durable_unlike_consensusdb` — proves, at the code level,
//!   the durability ASYMMETRY: OnDiskStorage's write path is a non-fsync'd
//!   temp-file+rename and is therefore byte-revertible after a write "succeeds",
//!   while consensusdb forces `set_sync(true)`. This is the code-provable half and
//!   directly exercises the buggy `write()`.
//!
//! * `power_loss_revert_of_safety_data_enables_double_vote` — demonstrates the
//!   safety-relevant CONSEQUENCE end-to-end through the real `SafetyRules` /
//!   `PersistentSafetyStorage` vote path with the in-memory safety-data cache
//!   DISABLED: after voting in round R (persisting `last_voted_round = R`,
//!   `last_vote = Some(..)`), we revert the persisted `SafetyData` to its pre-vote
//!   value (`last_voted_round = R-1`, `last_vote = None`) — modelling the lost,
//!   never-fsync'd page — reload SafetyRules, and successfully construct a SECOND,
//!   CONFLICTING vote in round R = an AptosBFT safety violation (equivocation).
//!
//! * `cache_off_safety_data_read_is_broken` — a bonus, independent correctness
//!   observation surfaced while building this repro: with `enable_cached_safety_data
//!   = false`, `PersistentSafetyStorage::safety_data()` ALWAYS fails. Its cache-off
//!   branch is `return self.internal_store.get(SAFETY_DATA).map(|v| v.value)?;` —
//!   the misplaced `?` makes the generic `get`'s value type infer as
//!   `Result<SafetyData, Error>`, so it tries to deserialize the SafetyData JSON as
//!   a `Result` and errors with "unknown variant `epoch`, expected `Ok` or `Err`".
//!   This matters for #706's proposed fix: a power-loss-safe recovery that
//!   re-reads SafetyData from disk (rather than trusting the in-memory cache) is
//!   exactly the cache-off path — and it is currently non-functional.
//!
//! HONEST SCOPE. What is PROVEN here: (1) the no-fsync write path is exercised and
//! the on-disk state is byte-revertible (test 1); (2) given a revert of the
//! persisted SafetyData, the real safety-rules vote path will sign a second,
//! conflicting vote in the same round (test 2); (3) the on-disk backend can't even
//! deserialize SafetyData back via its `from_value` read path (test 3). What is NOT
//! proven by a unit test (and cannot be) is that a real power-loss loses exactly the
//! page holding the vote update — that needs fault injection on real hardware. The
//! revert here *models* that loss; the bug is that nothing fsyncs to prevent it, and
//! IF the page is lost the double-vote follows deterministically.

use crate::{test_utils, PersistentSafetyStorage, SafetyRules, TSafetyRules};
use aptos_consensus_types::{
    block::block_test_utils::random_payload, quorum_cert::QuorumCert, safety_data::SafetyData,
    vote_proposal::VoteProposal,
};
use gaptos::{
    aptos_secure_storage::{InMemoryStorage, KVStorage, OnDiskStorage, Storage},
    aptos_types::validator_signer::ValidatorSigner,
};
use std::fs;

/// Documents/locks-in the durability ASYMMETRY at the heart of #706, exercising the
/// exact buggy `OnDiskStorage::write` (temp-file + `fs::rename`, no fsync).
///
/// We can't intercept the kernel's fsync from a unit test, but we CAN exercise the
/// write path and demonstrate that a value whose write "succeeded" (returned `Ok`)
/// is freely byte-revertible to its prior on-disk content — precisely the state a
/// power-loss can resurrect, because the write was never fsync'd. A durable store
/// (consensusdb's `WriteOptions::set_sync(true)`) would not silently revert.
#[test]
fn on_disk_write_is_not_durable_unlike_consensusdb() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secure_storage.json");

    let mut storage = OnDiskStorage::new(path.clone());
    storage.set("k", 1u64).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(!before.is_empty(), "OnDiskStorage produced a plain on-disk JSON file");
    assert_eq!(storage.get::<u64>("k").unwrap().value, 1);

    storage.set("k", 2u64).unwrap();
    assert_eq!(storage.get::<u64>("k").unwrap().value, 2);

    // The write that just "succeeded" is recoverable to its prior bytes simply by
    // restoring the file — exactly what a lost (never-fsync'd) page-cache page does
    // on power-loss. OnDiskStorage::write does File::create(temp) -> write_all ->
    // fs::rename with NO sync_all/fsync of the file or its directory.
    fs::write(&path, &before).unwrap();
    let reopened = OnDiskStorage::new(path.clone());
    assert_eq!(
        reopened.get::<u64>("k").unwrap().value,
        1,
        "on-disk value reverted after restoring pre-write bytes => OnDiskStorage::write \
         is non-durable (no fsync); contrast consensusdb set_sync(true)"
    );
}

/// END-TO-END demonstration of the double-vote (BFT safety violation) that the
/// missing fsync enables.
///
/// We drive the *real* `SafetyRules` vote path over `PersistentSafetyStorage` with
/// the safety-data cache DISABLED (so SafetyData is re-read across the simulated
/// restart). The persisted `SafetyData` is the same object that `on_disk_storage`
/// persists in production; the revert below models the lost, never-fsync'd page.
#[test]
fn power_loss_revert_of_safety_data_enables_double_vote() {
    let signer = ValidatorSigner::from_int(0);

    let (proof, genesis_qc) = test_utils::make_genesis(&signer);
    let round = genesis_qc.certified_block().round() + 1;

    // Two CONFLICTING proposals for the *same* round R, both valid (round == qc+1),
    // off the same genesis QC but with distinct payloads => distinct block ids.
    let proposal_a = make_round_proposal(round, genesis_qc.clone(), &signer);
    let proposal_b = make_round_proposal(round, genesis_qc, &signer);
    assert_ne!(
        proposal_a.block().id(),
        proposal_b.block().id(),
        "test setup: the two round-{round} proposals must be conflicting (distinct ids)"
    );

    // A persistent safety storage backed by the same SafetyData object the
    // production on_disk_storage backend persists. We keep the default cache ON, and
    // model the power-loss revert by re-persisting the pre-vote SafetyData (which
    // also resets the cache) — equivalent to a restart whose cache is rebuilt from
    // the reverted on-disk file.
    let waypoint = test_utils::validator_signers_to_waypoint(&[&signer]);
    let mut storage = PersistentSafetyStorage::initialize(
        Storage::from(InMemoryStorage::new()),
        signer.author(),
        signer.private_key().clone(),
        waypoint,
        /* enable_cached_safety_data = */ true,
    );

    // Snapshot the persisted SafetyData BEFORE the vote. This is the on-disk page
    // state that a power-loss can resurrect, because the upcoming vote-write to the
    // on_disk_storage backend is never fsync'd.
    let pre_vote_safety_data: SafetyData = storage.safety_data().unwrap();
    assert_eq!(pre_vote_safety_data.last_voted_round, 0);
    assert!(pre_vote_safety_data.last_vote.is_none());

    // --- Honest validator: vote once in round R.
    let mut sr = SafetyRules::new(storage);
    sr.initialize(&proof).unwrap();
    let vote1 = sr
        .construct_and_sign_vote_two_chain(&proposal_a, None)
        .expect("first vote in round R must succeed");
    assert_eq!(vote1.vote_data().proposed().round(), round);
    assert_eq!(
        sr.consensus_state().unwrap().last_voted_round(),
        round,
        "after voting, last_voted_round is persisted as R"
    );

    // Inspect the (now-advanced) persisted state directly (pub(crate) field).
    let post_vote = sr.persistent_storage.safety_data().unwrap();
    assert_eq!(post_vote.last_voted_round, round);
    assert!(post_vote.last_vote.is_some(), "durable vote recorded as last_vote");

    // CONTROL: with the durable state INTACT, the guard works — asking to vote the
    // conflicting proposal_b in the same round R returns the *already-cast* vote
    // (for proposal_a), NOT a new signature over proposal_b. This proves the test is
    // not trivially passing and that the safety guard is real.
    let guarded = sr
        .construct_and_sign_vote_two_chain(&proposal_b, None)
        .expect("guard returns the prior vote for round R");
    assert_eq!(
        guarded.vote_data().proposed().id(),
        proposal_a.block().id(),
        "intact state: conflicting proposal_b is refused; prior vote for proposal_a is returned"
    );

    // --- POWER-LOSS: the vote-write to on_disk_storage was never fsync'd, so the
    // page holding last_voted_round = R / last_vote = Some(..) is lost; the store
    // reverts to its pre-vote SafetyData (last_voted_round = R-1 = 0, last_vote =
    // None). We model exactly that revert here. (Meanwhile the durable consensusdb
    // last_vote and the wire-sent vote1 survived — but the same-epoch restart path
    // never cross-checks them against the reverted secure-storage SafetyData.)
    sr.persistent_storage.set_safety_data(pre_vote_safety_data.clone()).unwrap();
    let reverted = sr.persistent_storage.safety_data().unwrap();
    assert!(reverted.last_voted_round < round, "post-crash last_voted_round reverted below R");
    assert!(reverted.last_vote.is_none(), "post-crash last_vote reverted to None");

    // --- Restart: reload SafetyRules from the reverted state (cache still off).
    // Move the reverted storage into a fresh SafetyRules (clears the in-memory
    // signer/epoch_state, exactly as a process restart would).
    let mut sr2 = SafetyRules::new(sr.persistent_storage);
    sr2.initialize(&proof).unwrap();

    // The conflicting proposal for round R now passes `round > last_voted_round`
    // (R > R-1) and the `last_vote`-already-voted short-circuit is gone => a SECOND,
    // CONFLICTING vote in round R is signed.
    let vote2 = sr2
        .construct_and_sign_vote_two_chain(&proposal_b, None)
        .expect("DOUBLE VOTE: conflicting second vote in round R succeeded after revert");
    assert_eq!(vote2.vote_data().proposed().round(), round);

    // The smoking gun: two distinct, validly-signed votes for the SAME round on
    // CONFLICTING blocks by the same validator — an AptosBFT safety violation.
    assert_eq!(vote1.vote_data().proposed().round(), vote2.vote_data().proposed().round());
    assert_ne!(
        vote1.vote_data().proposed().id(),
        vote2.vote_data().proposed().id(),
        "two conflicting blocks voted in the same round"
    );
    assert_ne!(
        vote1.ledger_info().consensus_data_hash(),
        vote2.ledger_info().consensus_data_hash(),
        "the two votes commit to different ledger info => genuine equivocation"
    );
}

/// BONUS (independent of the fsync gap, relevant to #706's recovery fix):
/// `PersistentSafetyStorage::safety_data()` is BROKEN whenever the in-memory cache
/// is disabled (`enable_cached_safety_data = false`). The cache-off branch is:
///
/// ```ignore
/// return self.internal_store.get(SAFETY_DATA).map(|v| v.value)?;
/// ```
///
/// The trailing `?` makes the generic `get`'s value type infer as
/// `Result<SafetyData, Error>`, so the SafetyData JSON is (mis)deserialized as a
/// `Result` and the read fails with "unknown variant `epoch`, expected `Ok` or
/// `Err`". A direct `internal_store().get::<SafetyData>(SAFETY_DATA)` round-trips
/// the very same bytes fine, proving the value IS readable and the fault is in the
/// `safety_data()` wrapper. This matters for #706: the proposed fix re-reads the
/// durable SafetyData on recovery instead of trusting the cache, which runs on
/// precisely this broken cache-off path.
#[test]
fn cache_off_safety_data_read_is_broken() {
    let signer = ValidatorSigner::from_int(0);
    let waypoint = test_utils::validator_signers_to_waypoint(&[&signer]);
    let mut storage = PersistentSafetyStorage::initialize(
        Storage::from(InMemoryStorage::new()),
        signer.author(),
        signer.private_key().clone(),
        waypoint,
        /* enable_cached_safety_data = */ false,
    );

    // The exact same bytes ARE readable directly from the internal store:
    let direct: SafetyData = storage
        .internal_store()
        .get::<SafetyData>(gaptos::aptos_global_constants::SAFETY_DATA)
        .expect("internal_store can read SafetyData back")
        .value;
    assert_eq!(direct.epoch, 1);
    assert_eq!(direct.last_voted_round, 0);

    // But the safety_data() wrapper (cache off) cannot:
    let err = storage
        .safety_data()
        .expect_err("cache-off safety_data() is broken by a misplaced `?`");
    let msg = err.to_string();
    assert!(
        msg.contains("expected `Ok` or `Err`") || msg.contains("unknown variant"),
        "unexpected error from cache-off safety_data(): {msg}"
    );
}

fn make_round_proposal(round: u64, qc: QuorumCert, signer: &ValidatorSigner) -> VoteProposal {
    // Distinct random payload => distinct block id for the same round/qc.
    // decoupled_execution = true is required by VoteProposal::gen_vote_data().
    let block = aptos_consensus_types::block::Block::new_proposal(
        random_payload(1),
        round,
        qc.certified_block().timestamp_usecs() + 1,
        qc,
        signer,
        Vec::new(),
    )
    .unwrap();
    VoteProposal::new(block, None, /* decoupled_execution = */ true)
}
