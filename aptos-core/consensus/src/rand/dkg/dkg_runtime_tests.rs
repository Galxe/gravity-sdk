// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! DKG Runtime tests - tests for DKGManager, InnerState, and TranscriptAggregationState

use std::sync::Arc;
use gaptos::aptos_infallible::{RwLock, duration_since_epoch};
use std::time::{Duration, Instant};
use gaptos::aptos_crypto::bls12381::{PrivateKey, PublicKey, bls12381_keys};
use gaptos::aptos_crypto::Uniform;
use rand::thread_rng;
use gaptos::aptos_types::{
    account_address::AccountAddress,
    dkg::{
        DKGTrait, DKGSessionMetadata, DKGStartEvent, 
        dummy_dkg::{DummyDKG, DummyDKGTranscript},
        DKGTranscript, DKGTranscriptMetadata,
    },
    epoch_state::EpochState,
    on_chain_config::OnChainRandomnessConfig,
    validator_verifier::{ValidatorConsensusInfo, ValidatorConsensusInfoMoveStruct, ValidatorVerifier},
    validator_txn::ValidatorTransaction,
};
use gaptos::aptos_dkg_runtime::{
    agg_trx_producer::DummyAggTranscriptProducer,
    dkg_manager::{DKGManager, InnerState},
    network::{DummyRpcResponseSender, IncomingRpcRequest},
    types::{DKGMessage, DKGTranscriptRequest},
    TranscriptAggregationState,
};
use gaptos::aptos_reliable_broadcast::BroadcastStatus;
use aptos_consensus_types::common::Author;
use gaptos::aptos_validator_transaction_pool::{TransactionFilter, VTxnPoolState};
use bcs;

#[tokio::test]
async fn test_dkg_state_transition() {
    // Setup a validator set of 4 validators.
    let num_validators = 4;
    let epoch = 999;
    let addrs: Vec<AccountAddress> = (0..num_validators)
        .map(|_| AccountAddress::random())
        .collect();
    let mut rng = thread_rng();
    let private_keys: Vec<Arc<PrivateKey>> = (0..num_validators)
        .map(|_| Arc::new(PrivateKey::generate(&mut rng)))
        .collect();
    let public_keys: Vec<PublicKey> = (0..num_validators)
        .map(|i| PublicKey::from(&*private_keys[i]))
        .collect();
    let voting_powers = [1, 1, 1, 1];
    let validator_infos: Vec<ValidatorConsensusInfo> = (0..num_validators)
        .map(|i| ValidatorConsensusInfo::new(addrs[i], public_keys[i].clone(), voting_powers[i]))
        .collect();
    let validator_consensus_info_move_structs = validator_infos
        .iter()
        .map(|info| ValidatorConsensusInfoMoveStruct::from(info.clone()))
        .collect::<Vec<_>>();
    let validator_verifier = ValidatorVerifier::new(validator_infos.clone());
    let epoch_state = EpochState {
        epoch,
        verifier: Arc::new(validator_verifier),
    };
    let vtxn_pool_handle = VTxnPoolState::default();
    let agg_node_producer = DummyAggTranscriptProducer {};
    let mut dkg_manager: DKGManager<DummyDKG> = DKGManager::new(
        private_keys[0].clone(),
        0,
        addrs[0],
        Arc::new(epoch_state.clone()),
        Arc::new(agg_node_producer),
        vtxn_pool_handle.clone(),
    );

    // Initial state should be `NotStarted`.
    assert!(matches!(&dkg_manager.state, InnerState::NotStarted));

    let rpc_response_collector = Arc::new(RwLock::new(vec![]));

    // In state `NotStarted`, DKGManager should reply to RPC request with errors.
    let rpc_node_request = new_rpc_node_request(999, addrs[3], rpc_response_collector.clone());
    let handle_result = dkg_manager.process_peer_rpc_msg(rpc_node_request).await;
    assert!(handle_result.is_ok());
    let last_invocations = std::mem::take(&mut *rpc_response_collector.write());
    assert!(last_invocations.len() == 1 && last_invocations[0].is_err());
    assert!(matches!(&dkg_manager.state, InnerState::NotStarted));

    // In state `NotStarted`, DKGManager should accept `DKGStartEvent`:
    // it should record start time, compute its own node, and enter state `InProgress`.
    let start_time_1 = Duration::from_secs(1700000000);
    let event = DKGStartEvent {
        session_metadata: DKGSessionMetadata {
            dealer_epoch: 999,
            randomness_config: OnChainRandomnessConfig::default_enabled().into(),
            dealer_validator_set: validator_consensus_info_move_structs.clone(),
            target_validator_set: validator_consensus_info_move_structs.clone(),
        },
        start_time_us: start_time_1.as_micros() as u64,
    };
    let handle_result = dkg_manager.process_dkg_start_event(event.clone()).await;
    assert!(handle_result.is_ok());
    assert!(
        matches!(&dkg_manager.state, InnerState::InProgress { start_time, my_transcript, .. } if *start_time == start_time_1 && my_transcript.metadata == DKGTranscriptMetadata{ epoch: 999, author: addrs[0]})
    );

    // 2nd `DKGStartEvent` should be rejected.
    let handle_result = dkg_manager.process_dkg_start_event(event).await;
    println!("{:?}", handle_result);
    assert!(handle_result.is_err());

    // In state `InProgress`, DKGManager should respond to `DKGNodeRequest` with its own node.
    let rpc_node_request = new_rpc_node_request(999, addrs[3], rpc_response_collector.clone());
    let handle_result = dkg_manager.process_peer_rpc_msg(rpc_node_request).await;
    assert!(handle_result.is_ok());
    let last_responses = std::mem::take(&mut *rpc_response_collector.write())
        .into_iter()
        .map(anyhow::Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        vec![DKGMessage::TranscriptResponse(
            dkg_manager.state.my_node_cloned()
        )],
        last_responses
    );
    assert!(matches!(&dkg_manager.state, InnerState::InProgress { .. }));

    // In state `InProgress`, DKGManager should accept `DKGAggNode`:
    // it should update validator txn pool, and enter state `Finished`.
    let agg_trx = <DummyDKG as DKGTrait>::Transcript::default();
    let handle_result = dkg_manager
        .process_aggregated_transcript(agg_trx.clone())
        .await;
    assert!(handle_result.is_ok());
    let available_vtxns = vtxn_pool_handle.pull(
        Instant::now() + Duration::from_secs(10),
        999,
        2048,
        TransactionFilter::no_op(),
    );
    assert_eq!(
        vec![ValidatorTransaction::DKGResult(DKGTranscript {
            metadata: DKGTranscriptMetadata {
                epoch: 999,
                author: addrs[0],
            },
            transcript_bytes: bcs::to_bytes(&agg_trx).unwrap(),
        })],
        available_vtxns
    );
    assert!(matches!(&dkg_manager.state, InnerState::Finished { .. }));

    // In state `Finished`, DKGManager should still respond to `DKGNodeRequest` with its own node.
    let rpc_node_request = new_rpc_node_request(999, addrs[3], rpc_response_collector.clone());
    let handle_result = dkg_manager.process_peer_rpc_msg(rpc_node_request).await;
    assert!(handle_result.is_ok());
    let last_responses = std::mem::take(&mut *rpc_response_collector.write())
        .into_iter()
        .map(anyhow::Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        vec![DKGMessage::TranscriptResponse(
            dkg_manager.state.my_node_cloned()
        )],
        last_responses
    );
    assert!(matches!(&dkg_manager.state, InnerState::Finished { .. }));
}

#[tokio::test]
async fn test_transcript_aggregation_state() {
    let num_validators = 5;
    let epoch = 999;
    let addrs: Vec<AccountAddress> = (0..num_validators)
        .map(|_| AccountAddress::random())
        .collect();
    let vfn_addr = AccountAddress::random();
    let mut rng = thread_rng();
    let private_keys: Vec<bls12381_keys::PrivateKey> = (0..num_validators)
        .map(|_| bls12381_keys::PrivateKey::generate(&mut rng))
        .collect();
    let public_keys: Vec<bls12381_keys::PublicKey> = (0..num_validators)
        .map(|i| bls12381_keys::PublicKey::from(&private_keys[i]))
        .collect();
    let voting_powers = [1, 1, 1, 6, 6]; // total voting power: 15, default threshold: 11
    let validator_infos: Vec<ValidatorConsensusInfo> = (0..num_validators)
        .map(|i| ValidatorConsensusInfo::new(addrs[i], public_keys[i].clone(), voting_powers[i]))
        .collect();
    let validator_consensus_info_move_structs = validator_infos
        .iter()
        .map(|info| ValidatorConsensusInfoMoveStruct::from(info.clone()))
        .collect::<Vec<_>>();
    let validator_verifier = ValidatorVerifier::new(validator_infos.clone());
    let pub_params = DummyDKG::new_public_params(&DKGSessionMetadata {
        dealer_epoch: 999,
        randomness_config: OnChainRandomnessConfig::default_enabled().into(),
        dealer_validator_set: validator_consensus_info_move_structs.clone(),
        target_validator_set: validator_consensus_info_move_structs.clone(),
    });
    let epoch_state = Arc::new(EpochState::new(epoch, validator_verifier));
    let trx_agg_state = Arc::new(TranscriptAggregationState::<DummyDKG>::new(
        duration_since_epoch(),
        addrs[0],
        pub_params,
        epoch_state,
    ));
    let good_transcript = DummyDKGTranscript::default();
    let good_trx_bytes = bcs::to_bytes(&good_transcript).unwrap();

    // Node with incorrect epoch should be rejected.
    let result = trx_agg_state.add(addrs[0], DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 998,
            author: addrs[0],
        },
        transcript_bytes: good_trx_bytes.clone(),
    });
    assert!(result.is_err());

    // Node authored by X but sent by Y should be rejected.
    let result = trx_agg_state.add(addrs[1], DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 999,
            author: addrs[0],
        },
        transcript_bytes: good_trx_bytes.clone(),
    });
    assert!(result.is_err());

    // Node authored by non-active-validator should be rejected.
    let result = trx_agg_state.add(vfn_addr, DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 999,
            author: vfn_addr,
        },
        transcript_bytes: good_trx_bytes.clone(),
    });
    assert!(result.is_err());

    // Node with invalid transcript should be rejected.
    let result = trx_agg_state.add(addrs[2], DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 999,
            author: addrs[2],
        },
        transcript_bytes: vec![],
    });
    assert!(result.is_err());

    // Good node should be accepted.
    let result = trx_agg_state.add(addrs[3], DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 999,
            author: addrs[3],
        },
        transcript_bytes: good_trx_bytes.clone(),
    });
    assert!(matches!(result, Ok(None)));

    // Node from contributed author should be ignored.
    let result = trx_agg_state.add(addrs[3], DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 999,
            author: addrs[3],
        },
        transcript_bytes: good_trx_bytes.clone(),
    });
    assert!(matches!(result, Ok(None)));

    // Aggregated trx should be returned if after adding a node, the threshold is exceeded.
    let result = trx_agg_state.add(addrs[4], DKGTranscript {
        metadata: DKGTranscriptMetadata {
            epoch: 999,
            author: addrs[4],
        },
        transcript_bytes: good_trx_bytes.clone(),
    });
    assert!(matches!(result, Ok(Some(_))));
}

fn new_rpc_node_request(
    epoch: u64,
    sender: AccountAddress,
    response_collector: Arc<RwLock<Vec<anyhow::Result<DKGMessage>>>>,
) -> IncomingRpcRequest {
    IncomingRpcRequest {
        msg: DKGMessage::TranscriptRequest(DKGTranscriptRequest::new(epoch)),
        sender,
        response_sender: Box::new(DummyRpcResponseSender::new(response_collector)),
    }
}

