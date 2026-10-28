// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_storage::{block_store::sync_manager::NeedFetchResult, BlockReader},
    pending_votes::{PendingVotes, VoteReceptionResult},
    test_utils::{build_empty_tree, build_simple_tree, TreeInserter},
    util::mock_time_service::SimulatedTimeService,
};
use aptos_consensus_types::{
    block::{
        block_test_utils::{
            certificate_for_genesis, gen_test_certificate, placeholder_certificate_for_block,
            placeholder_ledger_info,
        },
        Block,
    },
    common::Payload,
    vote::Vote,
    vote_data::VoteData,
};
use futures_channel::mpsc::unbounded;
use gaptos::{
    aptos_config::config::QcAggregatorType,
    aptos_crypto::HashValue,
    aptos_types::{
        validator_signer::ValidatorSigner, validator_verifier::random_validator_verifier,
    },
};
use proptest::prelude::*;
use std::{cmp::min, sync::Arc};

#[tokio::test]
async fn test_highest_block_and_quorum_cert() {
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();
    assert_eq!(block_store.highest_certified_block().block(), &Block::make_genesis_block());
    assert_eq!(block_store.highest_quorum_cert().as_ref(), &certificate_for_genesis());

    let genesis = block_store.ordered_root();

    // Genesis block and quorum certificate is still the highest
    let block_round_1 = inserter.insert_block_with_qc(certificate_for_genesis(), &genesis, 1).await;
    assert_eq!(block_store.highest_certified_block().block(), &Block::make_genesis_block());
    assert_eq!(block_store.highest_quorum_cert().as_ref(), &certificate_for_genesis());

    // block_round_1 block and quorum certificate is now the highest
    let block_round_3 = inserter.insert_block(&block_round_1, 3, None).await;
    assert_eq!(block_store.highest_certified_block(), block_round_1);
    assert_eq!(
        block_store.highest_quorum_cert().as_ref(),
        block_store
            .get_block(block_round_3.id())
            .expect("block_round_1 should exist")
            .quorum_cert()
    );

    // block_round_1 block and quorum certificate is still the highest, since block_round_4
    // also builds on block_round_1
    let block_round_4 = inserter.insert_block(&block_round_1, 4, None).await;
    assert_eq!(block_store.highest_certified_block(), block_round_1);
    assert_eq!(
        block_store.highest_quorum_cert().as_ref(),
        block_store
            .get_block(block_round_4.id())
            .expect("block_round_1 should exist")
            .quorum_cert()
    );
}

#[tokio::test]
async fn test_qc_ancestry() {
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();
    let genesis = block_store.ordered_root();
    let block_a_1 = inserter.insert_block_with_qc(certificate_for_genesis(), &genesis, 1).await;
    let block_a_2 = inserter.insert_block(&block_a_1, 2, None).await;

    assert_eq!(block_store.get_block(genesis.quorum_cert().certified_block().id()), None);
    assert_eq!(
        block_store.get_block(block_a_1.quorum_cert().certified_block().id()),
        Some(genesis)
    );
    assert_eq!(
        block_store.get_block(block_a_2.quorum_cert().certified_block().id()),
        Some(block_a_1)
    );
}

// This test should be continuously extended to eventually become the
// single-page spec for the logic of our block storage.
proptest! {

    // #[tokio::test]
    // async fn test_block_store_insert(
    //     (private_keys, blocks) in block_test_utils::block_forest_and_its_keys(
    //         // quorum size
    //         10,
    //         // recursion depth
    //         50)
    // ){
    //     let authors: HashSet<Author> = private_keys.iter().map(
    //         // match the signer_strategy in validator_signer.rs
    //         |key| Author::from_bytes(&key.public_key().to_bytes()[0..32]).unwrap()
    //     ).collect();
    //     let runtime = consensus_runtime();
    //     let block_store = build_empty_tree().await;
    //     for block in blocks {
    //         if block.round() > 0 && authors.contains(&block.author().unwrap()) {
    //             let known_parent = block_store.block_exists(block.parent_id());
    //             let certified_parent = block.quorum_cert().certified_block().id() == block.parent_id();
    //             let verify_res = block.verify_well_formed();
    //             let res = timed_block_on(&runtime, block_store.insert_block(block.clone(), false));
    //             if !certified_parent {
    //                 prop_assert!(verify_res.is_err());
    //             } else if !known_parent {
    //                 // We cannot really bring blocks in this test because the block retrieval
    //                 // functionality invokes event processing, which is not setup here.
    //                 assert!(res.is_err());
    //             }
    //             else {
    //                 // The parent must be present if we get to this line.
    //                 let parent = block_store.get_block(block.parent_id()).unwrap();
    //                 if block.round() <= parent.round() {
    //                     prop_assert!(res.is_err());
    //                 } else {
    //                     let executed_block = res.unwrap();
    //                     prop_assert_eq!(executed_block.block(),
    //                          &block,
    //                         "expected ok on block: {:#?}, got {:#?}", block, executed_block.block());
    //                 }
    //             }
    //         }
    //     }
    // }
}

#[tokio::test]
async fn test_block_store_prune() {
    let (blocks, block_store) = build_simple_tree().await;
    // Attempt to prune genesis block (should be no-op)
    assert_eq!(block_store.prune_tree(blocks[0].id()).len(), 0);
    assert_eq!(block_store.len(), 7);
    assert_eq!(block_store.child_links(), block_store.len() - 1);
    assert_eq!(block_store.pruned_blocks_in_mem(), 0);

    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block A1
    assert_eq!(block_store.prune_tree(blocks[1].id()).len(), 4);
    assert_eq!(block_store.len(), 3);
    assert_eq!(block_store.child_links(), block_store.len() - 1);
    assert_eq!(block_store.pruned_blocks_in_mem(), 4);

    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block A2
    assert_eq!(block_store.prune_tree(blocks[2].id()).len(), 5);
    assert_eq!(block_store.len(), 2);
    assert_eq!(block_store.child_links(), block_store.len() - 1);
    assert_eq!(block_store.pruned_blocks_in_mem(), 5);

    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block A3
    assert_eq!(block_store.prune_tree(blocks[3].id()).len(), 6);
    assert_eq!(block_store.len(), 1);
    assert_eq!(block_store.child_links(), block_store.len() - 1);

    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block B1
    assert_eq!(block_store.prune_tree(blocks[4].id()).len(), 4);
    assert_eq!(block_store.len(), 3);
    assert_eq!(block_store.child_links(), block_store.len() - 1);

    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block B2
    assert_eq!(block_store.prune_tree(blocks[5].id()).len(), 6);
    assert_eq!(block_store.len(), 1);
    assert_eq!(block_store.child_links(), block_store.len() - 1);

    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block C1
    assert_eq!(block_store.prune_tree(blocks[6].id()).len(), 6);
    assert_eq!(block_store.len(), 1);
    assert_eq!(block_store.child_links(), block_store.len() - 1);

    // Prune the chain of Genesis -> B1 -> B2
    let (blocks, block_store) = build_simple_tree().await;
    // Prune up to block B1
    assert_eq!(block_store.prune_tree(blocks[4].id()).len(), 4);
    assert_eq!(block_store.len(), 3);
    assert_eq!(block_store.child_links(), block_store.len() - 1);
    // Prune up to block B2
    assert_eq!(block_store.prune_tree(blocks[5].id()).len(), 2);
    assert_eq!(block_store.len(), 1);
    assert_eq!(block_store.child_links(), block_store.len() - 1);
}

#[tokio::test]
async fn test_block_tree_gc() {
    // build a tree with 100 nodes, max_pruned_nodes_in_mem = 10
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();
    let genesis = block_store.ordered_root();
    let mut cur_node = block_store.get_block(genesis.id()).unwrap();
    let mut added_blocks = vec![];

    for round in 1..100 {
        if round == 1 {
            cur_node =
                inserter.insert_block_with_qc(certificate_for_genesis(), &cur_node, round).await;
        } else {
            cur_node = inserter.insert_block(&cur_node, round, None).await;
        }
        added_blocks.push(cur_node.clone());
    }

    for (i, block) in added_blocks.iter().enumerate() {
        assert_eq!(block_store.len(), 100 - i);
        assert_eq!(block_store.pruned_blocks_in_mem(), min(i, 10));
        block_store.prune_tree(block.id());
    }
}

#[tokio::test]
async fn test_path_from_root() {
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();
    let genesis = block_store.get_block(block_store.ordered_root().id()).unwrap();
    let b1 = inserter.insert_block_with_qc(certificate_for_genesis(), &genesis, 1).await;
    let b2 = inserter.insert_block(&b1, 2, None).await;
    let b3 = inserter.insert_block(&b2, 3, None).await;

    assert_eq!(block_store.path_from_ordered_root(b3.id()), Some(vec![b1, b2.clone(), b3.clone()]));
    assert_eq!(block_store.path_from_ordered_root(genesis.id()), Some(vec![]));

    block_store.prune_tree(b2.id());

    assert_eq!(block_store.path_from_ordered_root(b3.id()), Some(vec![b3.clone()]));
    assert_eq!(block_store.path_from_ordered_root(genesis.id()), None);
}

#[tokio::test]
async fn test_insert_vote() {
    ::gaptos::aptos_logger::Logger::init_for_testing();
    // Set up enough different authors to support different votes for the same block.
    let (signers, validator_verifier) = random_validator_verifier(11, Some(10), false);
    let my_signer = signers[10].clone();
    let mut inserter = TreeInserter::new(my_signer).await;
    let block_store = inserter.block_store();
    let genesis = block_store.ordered_root();
    let block = inserter.insert_block_with_qc(certificate_for_genesis(), &genesis, 1).await;
    let time_service = Arc::new(SimulatedTimeService::new());
    let (delayed_qc_tx, _) = unbounded();

    let mut pending_votes =
        PendingVotes::new(time_service, delayed_qc_tx, QcAggregatorType::NoDelay);

    assert!(block_store.get_quorum_cert_for_block(block.id()).is_none());
    for (i, voter) in signers.iter().enumerate().take(10).skip(1) {
        let vote = Vote::new(
            VoteData::new(
                block.block().gen_block_info(
                    block.compute_result().root_hash(),
                    block.compute_result().version(),
                    block.compute_result().epoch_state().clone(),
                ),
                block.quorum_cert().certified_block().clone(),
            ),
            voter.author(),
            placeholder_ledger_info(),
            voter,
        )
        .unwrap();
        let vote_res = pending_votes.insert_vote(&vote, &validator_verifier);

        // first vote of an author is accepted
        assert_eq!(vote_res, VoteReceptionResult::VoteAdded(i as u128));
        // filter out duplicates
        assert_eq!(
            pending_votes.insert_vote(&vote, &validator_verifier),
            VoteReceptionResult::DuplicateVote,
        );
        // qc is still not there
        assert!(block_store.get_quorum_cert_for_block(block.id()).is_none());
    }

    // Add the final vote to form a QC
    let final_voter = &signers[0];
    let vote = Vote::new(
        VoteData::new(
            block.block().gen_block_info(
                block.compute_result().root_hash(),
                block.compute_result().version(),
                block.compute_result().epoch_state().clone(),
            ),
            block.quorum_cert().certified_block().clone(),
        ),
        final_voter.author(),
        placeholder_ledger_info(),
        final_voter,
    )
    .unwrap();
    match pending_votes.insert_vote(&vote, &validator_verifier) {
        VoteReceptionResult::NewQuorumCertificate(qc) => {
            assert_eq!(qc.certified_block().id(), block.id());
            block_store.insert_single_quorum_cert(qc.as_ref().clone(), false).unwrap();
        }
        _ => {
            panic!("QC not formed!");
        }
    }

    let block_qc = block_store.get_quorum_cert_for_block(block.id()).unwrap();
    assert_eq!(block_qc.certified_block().id(), block.id());
}

#[tokio::test]
async fn test_illegal_timestamp() {
    let signer = ValidatorSigner::random(None);
    let block_store = build_empty_tree().await;
    let genesis = block_store.ordered_root();
    let block_with_illegal_timestamp = Block::new_proposal(
        Payload::empty(false, true),
        0,
        // This timestamp is illegal, it is the same as genesis
        genesis.timestamp_usecs(),
        certificate_for_genesis(),
        &signer,
        Vec::new(),
    )
    .unwrap();
    let result = block_store.insert_block(block_with_illegal_timestamp, false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_highest_qc() {
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();

    // build a tree of the following form
    // genesis <- a1 <- a2 <- a3
    let genesis = block_store.ordered_root();
    let a1 = inserter.insert_block_with_qc(certificate_for_genesis(), &genesis, 1).await;
    assert_eq!(block_store.highest_certified_block(), genesis);
    let a2 = inserter.insert_block(&a1, 2, None).await;
    assert_eq!(block_store.highest_certified_block(), a1);
    let _a3 = inserter.insert_block(&a2, 3, None).await;
    assert_eq!(block_store.highest_certified_block(), a2);
}

#[tokio::test]
async fn test_need_fetch_for_qc() {
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();

    // build a tree of the following form
    // genesis <- a1 <- a2 <- a3
    let genesis = block_store.ordered_root();
    let a1 = inserter.insert_block_with_qc(certificate_for_genesis(), &genesis, 1).await;
    let a2 = inserter.insert_block(&a1, 2, None).await;
    let a3 = inserter.insert_block(&a2, 3, None).await;
    block_store.prune_tree(a2.id());
    let need_fetch_qc = placeholder_certificate_for_block(
        &[inserter.signer().clone()],
        HashValue::zero(),
        a3.round() + 1,
        HashValue::zero(),
        a3.round(),
    );
    let too_old_qc = certificate_for_genesis();
    let can_insert_qc = placeholder_certificate_for_block(
        &[inserter.signer().clone()],
        a3.id(),
        a3.round(),
        a2.id(),
        a2.round(),
    );
    let duplicate_qc = block_store.get_quorum_cert_for_block(a2.id()).unwrap();
    assert_eq!(block_store.need_fetch_for_quorum_cert(&need_fetch_qc), NeedFetchResult::NeedFetch);
    assert_eq!(
        block_store.need_fetch_for_quorum_cert(&too_old_qc),
        NeedFetchResult::QCRoundBeforeRoot,
    );
    assert_eq!(
        block_store.need_fetch_for_quorum_cert(&can_insert_qc),
        NeedFetchResult::QCBlockExist,
    );
    assert_eq!(
        block_store.need_fetch_for_quorum_cert(duplicate_qc.as_ref()),
        NeedFetchResult::QCAlreadyExist,
    );
}

#[tokio::test]
async fn test_need_sync_for_ledger_info() {
    let mut inserter = TreeInserter::default().await;
    let block_store = inserter.block_store();

    let mut prev = block_store.ordered_root();
    for i in 1..=30 {
        prev = inserter.insert_block(&prev, i, None).await;
    }
    inserter
        .insert_block(&prev, 31, Some(prev.block().gen_block_info(HashValue::zero(), 1, None)))
        .await;
    assert_eq!(block_store.ordered_root().round(), 30);
    assert_eq!(block_store.commit_root().round(), 0);

    let create_ledger_info = |round: u64| {
        let future_block = inserter.create_block_with_qc(
            certificate_for_genesis(),
            1,
            round,
            Payload::empty(false, true),
            vec![],
        );
        gen_test_certificate(
            &[inserter.signer().clone()],
            future_block.gen_block_info(HashValue::zero(), 0, None),
            future_block.quorum_cert().parent_block().clone(),
            Some(future_block.gen_block_info(HashValue::zero(), 0, None)),
        )
        .ledger_info()
        .clone()
    };
    // it's larger and the block doesn't exist in the tree
    let ordered_round_too_far = block_store.ordered_root().round() + 1;
    let ordered_too_far = create_ledger_info(ordered_round_too_far);
    assert!(block_store.need_sync_for_ledger_info(&ordered_too_far));

    let committed_round_too_far =
        block_store.commit_root().round() + 30.max(block_store.vote_back_pressure_limit * 2) + 1;
    let committed_too_far = create_ledger_info(committed_round_too_far);
    assert!(block_store.need_sync_for_ledger_info(&committed_too_far));

    let round_not_too_far =
        block_store.commit_root().round() + block_store.vote_back_pressure_limit + 1;
    let not_too_far = create_ledger_info(round_not_too_far);
    assert!(!block_store.need_sync_for_ledger_info(&not_too_far));
}

#[tokio::test]
async fn issue845_recovery_includes_ordered_uncommitted_block() {
    use crate::{
        block_storage::{pending_blocks::PendingBlocks, ForwardEpochSyncService},
        consensusdb::{ConsensusDB, LedgerInfoSchema},
        payload_manager::DirectMempoolPayloadManager,
        persistent_liveness_storage::RootInfo,
        pipeline::execution_client::DummyExecutionClient,
        test_utils::EmptyStorage,
    };
    use block_buffer_manager::get_block_buffer_manager;
    use gaptos::{
        api_types::{
            compute_res::ComputeRes,
            on_chain_config::{
                dkg::DKGTranscript as ExecutionDKGTranscript,
                jwks::ProviderJWKs as ExecutionProviderJWKs,
            },
            u256_define::BlockId,
            ExtraDataType,
        },
        aptos_infallible::Mutex,
        aptos_types::{
            aggregate_signature::AggregateSignature,
            dkg::DKGTranscript,
            epoch_state::EpochState,
            jwks::{
                jwk::{JWKMoveStruct, JWK},
                unsupported::UnsupportedJWK,
                ProviderJWKs, QuorumCertifiedUpdate,
            },
            ledger_info::{LedgerInfo, LedgerInfoWithSignatures},
            validator_txn::ValidatorTransaction,
            validator_verifier::{ValidatorConsensusInfo, ValidatorVerifier},
        },
    };
    use std::{collections::HashMap, path::PathBuf, time::Duration};

    // A future activation leaves other tests' ordinary timestamps unaffected.
    // The simulated clock advances to this timestamp without a wall-clock wait.
    let activation_usecs =
        gaptos::aptos_infallible::duration_since_epoch().as_micros() as u64 + 240_000_000;
    crate::oracle_hardfork::init_oracle_signature_verification_time(Some(activation_usecs));

    let db_dir = tempfile::tempdir().unwrap();
    let consensus_db = Arc::new(ConsensusDB::new(db_dir.path(), &PathBuf::new()));
    let genesis = Block::make_genesis_block();
    genesis.set_block_number(0);
    let epoch = genesis.epoch();
    let signers = (1..=4).map(|seed| ValidatorSigner::random([seed; 32])).collect::<Vec<_>>();
    let other_epoch_signers =
        (11..=14).map(|seed| ValidatorSigner::random([seed; 32])).collect::<Vec<_>>();
    let verifier_for = |signers: &[ValidatorSigner]| {
        ValidatorVerifier::new(
            signers
                .iter()
                .map(|signer| ValidatorConsensusInfo::new(signer.author(), signer.public_key(), 1))
                .collect(),
        )
    };
    let epoch_state = EpochState::new(epoch, verifier_for(&signers));
    let other_epoch_state = EpochState::new(epoch, verifier_for(&other_epoch_signers));
    assert_eq!(epoch_state.verifier.quorum_voting_power(), 3);
    let aggregate = |update: &ProviderJWKs, signers: &[ValidatorSigner], state: &EpochState| {
        let partials = signers
            .iter()
            .take(3)
            .map(|signer| (signer.author(), signer.sign(update).unwrap()))
            .collect::<Vec<_>>();
        state
            .verifier
            .aggregate_signatures(partials.iter().map(|(author, signature)| (author, signature)))
            .unwrap()
    };
    let valid_update = ProviderJWKs {
        issuer: b"bridge".to_vec(),
        version: 7,
        jwks: vec![JWKMoveStruct::from(JWK::Unsupported(UnsupportedJWK {
            id: b"event-1".to_vec(),
            payload: b"payload-1".to_vec(),
        }))],
    };
    let invalid_update = ProviderJWKs { issuer: b"other-epoch".to_vec(), ..valid_update.clone() };
    let invalid_oracle = ValidatorTransaction::ObservedJWKUpdate(QuorumCertifiedUpdate {
        update: invalid_update.clone(),
        multi_sig: aggregate(&invalid_update, &other_epoch_signers, &other_epoch_state),
    });
    let dkg = ValidatorTransaction::DKGResult(DKGTranscript::new(
        epoch,
        signers[0].author(),
        vec![1, 2, 3],
    ));
    let valid_oracle = ValidatorTransaction::ObservedJWKUpdate(QuorumCertifiedUpdate {
        update: valid_update.clone(),
        multi_sig: aggregate(&valid_update, &signers, &epoch_state),
    });
    let qc = certificate_for_genesis();
    let proof = qc.clone().into_wrapped_ledger_info();
    let store = Arc::new(
        super::BlockStore::build(
            RootInfo(Box::new(genesis), qc, proof.clone(), proof),
            vec![],
            vec![],
            None,
            Arc::new(DummyExecutionClient),
            Arc::new(EmptyStorage::with_consensus_db(consensus_db.clone())),
            10,
            Arc::new(SimulatedTimeService::new()),
            10,
            Arc::new(DirectMempoolPayloadManager::new()),
            false,
            true,
            Arc::new(Mutex::new(PendingBlocks::new())),
            false,
            true,
            Arc::new(epoch_state.clone()),
            Arc::new(ForwardEpochSyncService::from_env()),
        )
        .await,
    );
    let mut inserter = TreeInserter::new_with_store(signers[0].clone(), store.clone());
    let committed = store.commit_root();
    let ordered = store
        .insert_block_with_qc(
            Block::new_proposal_ext(
                vec![invalid_oracle, dkg, valid_oracle],
                Payload::empty(false, true),
                1,
                activation_usecs,
                certificate_for_genesis(),
                inserter.signer(),
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    // Normal execution converts this block with the same epoch snapshot. Recovery
    // must submit byte-for-byte identical validator extra data to the external EL.
    let normal_extra_data = crate::state_computer::process_validator_transactions_util(
        ordered.block().validator_txns().map(|txns| &**txns),
        ordered.block(),
        &epoch_state,
    )
    .unwrap();
    let successor = inserter.insert_block(&ordered, 2, None).await;
    let competing = inserter.insert_block(&committed, 3, None).await;
    let proof = inserter
        .create_qc_for_block(&successor, Some(ordered.block_info()))
        .into_wrapped_ledger_info();

    // DummyExecutionClient accepts ordering but does not invoke the commit callback.
    store.send_for_execution(proof.clone(), false, None).await.unwrap();
    assert_eq!(store.ordered_root().id(), ordered.id());
    assert_eq!(store.commit_root().id(), committed.id());
    assert_eq!(store.path_from_ordered_root(ordered.id()), Some(vec![]));
    assert!(store.recovery_path_from_commit_root(committed.id()).unwrap().is_empty());
    assert_eq!(store.recovery_path_from_commit_root(ordered.id()).unwrap(), vec![ordered.clone()],);
    assert_eq!(
        store.recovery_path_from_commit_root(successor.id()).unwrap(),
        vec![ordered.clone(), successor.clone()],
    );
    assert!(store.recovery_path_from_commit_root(competing.id()).is_err());

    // The target is already the ordered root, but it still needs execution and commit.
    let block_number = ordered.block().block_number().unwrap();
    let block_hash = HashValue::new([0x5a; 32]);
    let persisted_li = LedgerInfoWithSignatures::new(
        LedgerInfo::new_with_block_info(
            ordered.block_info(),
            proof.ledger_info().ledger_info().consensus_data_hash(),
            block_hash,
            block_number,
        ),
        AggregateSignature::empty(),
    );
    consensus_db.put::<LedgerInfoSchema>(&block_number, &persisted_li).unwrap();
    let manager = get_block_buffer_manager().clone();
    manager
        .init(
            0,
            HashMap::from([(0, (committed.epoch(), BlockId(*committed.id())))]),
            ordered.epoch(),
        )
        .await
        .unwrap();
    let (_, external_block) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(store.send_for_execution(proof, true, None), async {
            let ordered_blocks =
                manager.get_ordered_blocks(block_number, Some(1), ordered.epoch()).await?;
            assert_eq!(ordered_blocks.len(), 1);
            assert_eq!(ordered_blocks[0].0.block_meta.block_id, BlockId(*ordered.id()));
            let external_block = ordered_blocks[0].0.clone();
            manager
                .set_compute_res(
                    BlockId(*ordered.id()),
                    *block_hash,
                    block_number,
                    ordered.epoch(),
                    Arc::new(None),
                    vec![],
                )
                .await?;
            Ok::<_, anyhow::Error>(external_block)
        })
    })
    .await
    .expect("recovery execution timed out")
    .unwrap();
    assert_eq!(external_block.block_meta.block_number, block_number);
    assert_eq!(external_block.block_meta.usecs, activation_usecs);
    assert_eq!(external_block.block_meta.epoch, epoch);
    assert_eq!(
        external_block.block_meta.block_hash,
        Some(ComputeRes::new(*block_hash, 0, vec![], vec![])),
    );
    assert!(external_block.block_meta.proposer_index.is_some());
    assert!(external_block.block_meta.failed_proposer_indices.is_empty());
    assert!(external_block.block_meta.randomness.is_none());
    assert!(!external_block.enable_randomness);
    assert!(external_block.txns.is_empty());
    assert_eq!(external_block.extra_data.len(), normal_extra_data.len());
    for (actual, normal) in external_block.extra_data.iter().zip(&normal_extra_data) {
        match (actual, normal) {
            (ExtraDataType::DKG(actual), ExtraDataType::DKG(normal)) |
            (ExtraDataType::JWK(actual), ExtraDataType::JWK(normal)) => {
                assert_eq!(actual, normal);
            }
            _ => panic!("recovery changed the validator extra-data type"),
        }
    }
    assert_eq!(external_block.extra_data.len(), 2);
    let ExtraDataType::DKG(dkg_bytes) = &external_block.extra_data[0] else {
        panic!("DKG result must remain ahead of the valid Oracle update");
    };
    let decoded_dkg: ExecutionDKGTranscript = bcs::from_bytes(dkg_bytes).unwrap();
    assert_eq!(decoded_dkg.metadata.epoch, epoch);
    assert_eq!(decoded_dkg.transcript_bytes, vec![1, 2, 3]);
    let ExtraDataType::JWK(bytes) = &external_block.extra_data[1] else {
        panic!("valid Oracle update must be retained after the DKG result");
    };
    let decoded: ExecutionProviderJWKs = bcs::from_bytes(bytes).unwrap();
    assert_eq!(decoded.issuer, valid_update.issuer);
    assert_eq!(decoded.version, valid_update.version);
    assert_eq!(decoded.jwks.len(), 1);
    assert_eq!(decoded.jwks[0].data.as_slice(), b"payload-1");
    assert_eq!(store.commit_root().id(), ordered.id());
    assert_eq!(store.ordered_root().id(), ordered.id());

    // A live commit may advance past this recovery target after the caller selected its QC.
    {
        let mut tree = store.inner.write();
        tree.update_ordered_root(successor.id());
        tree.update_commit_and_finalized_root(successor.id());
    }
    assert!(store.recovery_path_from_commit_root(ordered.id()).unwrap().is_empty());
}

#[tokio::test]
async fn recovery_rejects_block_from_a_different_epoch_before_external_execution() {
    use crate::{
        block_storage::{pending_blocks::PendingBlocks, ForwardEpochSyncService},
        consensusdb::{ConsensusDB, LedgerInfoSchema},
        payload_manager::DirectMempoolPayloadManager,
        persistent_liveness_storage::RootInfo,
        pipeline::execution_client::DummyExecutionClient,
        test_utils::EmptyStorage,
    };
    use gaptos::{
        aptos_infallible::Mutex,
        aptos_types::{
            aggregate_signature::AggregateSignature,
            epoch_state::EpochState,
            jwks::{ProviderJWKs, QuorumCertifiedUpdate},
            ledger_info::{LedgerInfo, LedgerInfoWithSignatures},
            validator_txn::ValidatorTransaction,
            validator_verifier::ValidatorVerifier,
        },
    };
    use std::path::PathBuf;

    let db_dir = tempfile::tempdir().unwrap();
    let consensus_db = Arc::new(ConsensusDB::new(db_dir.path(), &PathBuf::new()));
    let genesis = Block::make_genesis_block();
    genesis.set_block_number(0);
    let epoch = genesis.epoch();
    let qc = certificate_for_genesis();
    let proof = qc.clone().into_wrapped_ledger_info();
    let store = Arc::new(
        super::BlockStore::build(
            RootInfo(Box::new(genesis), qc, proof.clone(), proof),
            vec![],
            vec![],
            None,
            Arc::new(DummyExecutionClient),
            Arc::new(EmptyStorage::with_consensus_db(consensus_db.clone())),
            10,
            Arc::new(SimulatedTimeService::new()),
            10,
            Arc::new(DirectMempoolPayloadManager::new()),
            false,
            true,
            Arc::new(Mutex::new(PendingBlocks::new())),
            false,
            true,
            Arc::new(EpochState::new(epoch + 1, ValidatorVerifier::new(vec![]))),
            Arc::new(ForwardEpochSyncService::from_env()),
        )
        .await,
    );
    let inserter = TreeInserter::new_with_store(ValidatorSigner::random(None), store.clone());
    let committed = store.commit_root();
    let block = Block::new_proposal_ext(
        vec![ValidatorTransaction::ObservedJWKUpdate(QuorumCertifiedUpdate {
            update: ProviderJWKs { issuer: b"bridge".to_vec(), version: 1, jwks: vec![] },
            multi_sig: AggregateSignature::empty(),
        })],
        Payload::empty(false, true),
        1,
        1,
        certificate_for_genesis(),
        inserter.signer(),
        vec![],
    )
    .unwrap();
    let ordered = store.insert_block_with_qc(block).await.unwrap();
    let proof = inserter
        .create_qc_for_block(&ordered, Some(ordered.block_info()))
        .into_wrapped_ledger_info();
    let persisted_li = LedgerInfoWithSignatures::new(
        LedgerInfo::new_with_block_info(
            ordered.block_info(),
            proof.ledger_info().ledger_info().consensus_data_hash(),
            HashValue::new([0x6b; 32]),
            1,
        ),
        AggregateSignature::empty(),
    );
    consensus_db.put::<LedgerInfoSchema>(&1, &persisted_li).unwrap();

    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        store.send_for_execution(proof, true, None),
    )
    .await
    .expect("recovery did not reject the epoch mismatch promptly")
    .unwrap_err();
    assert!(error.to_string().contains("belongs to epoch"), "{error:?}");
    assert_eq!(store.commit_root().id(), committed.id());
    assert_eq!(store.ordered_root().id(), committed.id());
}
