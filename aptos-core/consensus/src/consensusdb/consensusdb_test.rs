// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use self::schema::dag::NodeSchema;
use super::*;
use crate::dag::{CertifiedNode, Extensions, Node, Vote};
use aptos_consensus_types::{
    block::block_test_utils::certificate_for_genesis,
    common::{Author, Payload},
};
use gaptos::{
    aptos_crypto::bls12381::Signature, aptos_temppath::TempPath,
    aptos_types::aggregate_signature::AggregateSignature,
};
use std::{collections::HashMap, hash::Hash};

#[test]
fn test_put_get() {
    let tmp_dir = TempPath::new();
    let db = ConsensusDB::new(&tmp_dir, &PathBuf::new());

    let block = Block::make_genesis_block();
    let blocks = vec![block];

    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 0);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 0);

    let qcs = vec![certificate_for_genesis()];
    db.save_blocks_and_quorum_certificates(blocks.clone(), qcs.clone()).unwrap();

    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 1);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 1);

    let tc = vec![0u8, 1, 2];
    db.save_highest_2chain_timeout_certificate(tc.clone()).unwrap();

    let vote = vec![2u8, 1, 0];
    db.save_vote(vote.clone()).unwrap();

    // let (vote_1, tc_1, blocks_1, qc_1) = db.get_data().unwrap();
    // assert_eq!(blocks, blocks_1);
    // assert_eq!(qcs, qc_1);
    // assert_eq!(Some(tc), tc_1);
    // assert_eq!(Some(vote), vote_1);

    // db.delete_highest_2chain_timeout_certificate().unwrap();
    // db.delete_last_vote_msg().unwrap();
    // assert!(db
    //     .get_highest_2chain_timeout_certificate()
    //     .unwrap()
    //     .is_none());
    // assert!(db.get_last_vote().unwrap().is_none());
}

#[test]
fn test_delete_block_and_qc() {
    let tmp_dir = TempPath::new();
    let db = ConsensusDB::new(&tmp_dir, &PathBuf::new());

    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 0);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 0);

    let blocks = vec![Block::make_genesis_block()];
    let block_id = blocks[0].id();
    let epoch = blocks[0].epoch();

    let qcs = vec![certificate_for_genesis()];
    let qc_id = qcs[0].certified_block().id();

    db.save_blocks_and_quorum_certificates(blocks, qcs).unwrap();
    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 1);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 1);

    // Start to delete
    db.delete_blocks_and_quorum_certificates(vec![(epoch, block_id), (epoch, qc_id)]).unwrap();
    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 0);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 0);
}

fn test_dag_type<S: Schema<Key = K>, K: Eq + Hash>(key: S::Key, value: S::Value, db: &ConsensusDB) {
    db.put::<S>(&key, &value).unwrap();
    let mut from_db: HashMap<K, S::Value> = db.get_all::<S>().unwrap().into_iter().collect();
    assert_eq!(from_db.len(), 1);
    let value_from_db = from_db.remove(&key).unwrap();
    assert_eq!(value, value_from_db);
    db.delete::<S>(vec![key]).unwrap();
    assert_eq!(db.get_all::<S>().unwrap().len(), 0);
}

#[test]
fn test_dag() {
    let tmp_dir = TempPath::new();
    let db = ConsensusDB::new(&tmp_dir, &PathBuf::new());

    let node = Node::new(
        1,
        1,
        Author::random(),
        123,
        vec![],
        Payload::empty(false, true),
        vec![],
        Extensions::empty(),
    );
    test_dag_type::<NodeSchema, <NodeSchema as Schema>::Key>((), node.clone(), &db);

    let certified_node = CertifiedNode::new(node.clone(), AggregateSignature::empty());
    test_dag_type::<CertifiedNodeSchema, <CertifiedNodeSchema as Schema>::Key>(
        certified_node.digest(),
        certified_node,
        &db,
    );

    let vote = Vote::new(node.metadata().clone(), Signature::dummy_signature());
    test_dag_type::<DagVoteSchema, <DagVoteSchema as Schema>::Key>(node.id(), vote, &db);
}

#[test]
fn test_unwind_to_block() {
    use aptos_consensus_types::{
        block::block_test_utils::placeholder_certificate_for_block,
        common::Payload,
    };

    let tmp_dir = TempPath::new();
    let db = ConsensusDB::new(&tmp_dir, &PathBuf::new());

    let epoch = 1u64;
    let genesis = Block::make_genesis_block();
    let genesis_qc = certificate_for_genesis();

    // Save genesis block and its block_number mapping
    db.save_blocks_and_quorum_certificates(vec![genesis.clone()], vec![genesis_qc.clone()])
        .unwrap();
    db.save_block_numbers(vec![(epoch, 0, genesis.id())]).unwrap();

    // Create 5 blocks (block_number 1..=5) in epoch 1
    let signer = gaptos::aptos_types::validator_signer::ValidatorSigner::random(None);
    let mut parent_qc = genesis_qc;
    let mut blocks = Vec::new();
    let mut qcs = Vec::new();
    let mut block_numbers = Vec::new();

    for i in 1..=5u64 {
        let block = Block::new_proposal(
            Payload::empty(false, true),
            i,
            gaptos::aptos_infallible::duration_since_epoch().as_micros() as u64,
            parent_qc.clone(),
            &signer,
            Vec::new(),
        )
        .unwrap();
        block.set_block_number(i);
        block_numbers.push((epoch, i, block.id()));

        let qc = placeholder_certificate_for_block(
            &[signer.clone()],
            block.id(),
            i,
            if i == 1 { genesis.id() } else { blocks.last().map(|b: &Block| b.id()).unwrap() },
            i - 1,
        );

        blocks.push(block);
        qcs.push(qc.clone());
        parent_qc = qc;
    }

    db.save_blocks_and_quorum_certificates(blocks.clone(), qcs.clone()).unwrap();
    db.save_block_numbers(block_numbers).unwrap();

    // Save randomness for blocks 1..=5
    let randomness: Vec<(u64, Vec<u8>)> = (1..=5).map(|i| (i, vec![i as u8; 32])).collect();
    db.put_randomness(&randomness).unwrap();

    // Save vote and timeout cert
    db.save_vote(vec![1, 2, 3]).unwrap();
    db.save_highest_2chain_timeout_certificate(vec![4, 5, 6]).unwrap();

    // Verify initial state: genesis + 5 = 6 blocks
    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 6);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 6);

    // === Unwind to block 3 ===
    db.unwind_to_block(3).unwrap();

    // Blocks: genesis + 1,2,3 = 4 remaining
    assert_eq!(db.get_all::<BlockSchema>().unwrap().len(), 4);
    assert_eq!(db.get_all::<QCSchema>().unwrap().len(), 4);

    // BlockNumbers: 0,1,2,3 remaining; 4,5 deleted
    let bns: Vec<u64> = db.get_all::<BlockNumberSchema>().unwrap().into_iter().map(|(_, v)| v).collect();
    for i in 0..=3 { assert!(bns.contains(&i), "block_number {} should remain", i); }
    for i in 4..=5 { assert!(!bns.contains(&i), "block_number {} should be deleted", i); }

    // Randomness: 1,2,3 remain; 4,5 deleted
    for i in 1..=3 { assert!(db.get_randomness(i).unwrap().is_some()); }
    for i in 4..=5 { assert!(db.get_randomness(i).unwrap().is_none()); }

    // Vote and timeout cert cleared
    assert!(db.get_last_vote().unwrap().is_none());
    assert!(db.get_highest_2chain_timeout_certificate().unwrap().is_none());
}
