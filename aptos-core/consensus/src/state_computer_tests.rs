#![allow(unused)]
// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::MempoolError, payload_manager::TPayloadManager,
    pipeline::pipeline_phase::CountedRequest, state_computer::ExecutionProxy,
    state_replication::StateComputer, test_utils::create_unsupported_signed_transaction,
    transaction_deduper::NoOpDeduper, transaction_filter::TransactionFilter,
    transaction_shuffler::NoOpShuffler, txn_notifier::TxnNotifier,
};

use aptos_consensus_types::{
    block::Block,
    block_data::BlockData,
    common::{Payload, RejectedTransactionSummary},
};
use aptos_executor_types::{BlockExecutorTrait, ExecutorError, ExecutorResult, StateComputeResult};
use gaptos::{
    aptos_config::config::transaction_filter_type::Filter,
    aptos_consensus_notifications::{ConsensusNotificationSender, Error},
    aptos_crypto::HashValue,
    aptos_infallible::Mutex,
    aptos_types::{
        block_executor::{config::BlockExecutorConfigFromOnchain, partitioner::ExecutableBlock},
        contract_event::ContractEvent,
        epoch_state::EpochState,
        ledger_info::LedgerInfoWithSignatures,
        transaction::{SignedTransaction, Transaction, TransactionStatus},
        validator_txn::ValidatorTransaction,
        validator_verifier::ValidatorVerifier,
    },
};
use std::{
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};
use tokio::runtime::Handle;

struct DummyStateSyncNotifier {
    invocations: Mutex<Vec<(Vec<Transaction>, Vec<ContractEvent>)>>,
}

impl DummyStateSyncNotifier {
    fn new() -> Self {
        Self { invocations: Mutex::new(vec![]) }
    }
}

#[async_trait::async_trait]
impl ConsensusNotificationSender for DummyStateSyncNotifier {
    async fn notify_new_commit(
        &self,
        transactions: Vec<Transaction>,
        subscribable_events: Vec<ContractEvent>,
        _block_number: u64,
    ) -> Result<(), Error> {
        self.invocations.lock().push((transactions, subscribable_events));
        Ok(())
    }

    async fn sync_to_target(&self, _target: LedgerInfoWithSignatures) -> Result<(), Error> {
        unreachable!()
    }

    async fn sync_for_duration(
        &self,
        _duration: Duration,
    ) -> Result<LedgerInfoWithSignatures, Error> {
        todo!()
    }
}

struct DummyTxnNotifier {}

#[async_trait::async_trait]
impl TxnNotifier for DummyTxnNotifier {
    async fn notify_failed_txn(
        &self,
        _rejected_txns: Vec<RejectedTransactionSummary>,
    ) -> anyhow::Result<(), MempoolError> {
        Ok(())
    }
}

struct DummyBlockExecutor {
    blocks_received: Mutex<Vec<ExecutableBlock>>,
}

enum TestPayloadResult {
    Transactions(Vec<SignedTransaction>),
    Missing(HashValue),
}

struct TestPayloadManager(TestPayloadResult);

#[async_trait::async_trait]
impl TPayloadManager for TestPayloadManager {
    fn notify_commit(&self, _block_timestamp: u64, _payloads: Vec<Payload>) {}

    fn prefetch_payload_data(&self, _payload: &Payload, _timestamp: u64) {}

    fn check_payload_availability(&self, _block: &Block) -> bool {
        matches!(&self.0, TestPayloadResult::Transactions(_))
    }

    async fn get_transactions(
        &self,
        _block: &Block,
    ) -> ExecutorResult<(Vec<SignedTransaction>, Option<u64>)> {
        match &self.0 {
            TestPayloadResult::Transactions(transactions) => Ok((transactions.clone(), None)),
            TestPayloadResult::Missing(digest) => Err(ExecutorError::DataNotFound(*digest)),
        }
    }
}

impl DummyBlockExecutor {
    fn new() -> Self {
        Self { blocks_received: Mutex::new(vec![]) }
    }
}

fn execution_proxy_with_payload_manager(
    executor: Arc<DummyBlockExecutor>,
    payload_manager: Arc<dyn TPayloadManager>,
) -> ExecutionProxy {
    let execution_proxy = ExecutionProxy::new(
        executor,
        Arc::new(DummyTxnNotifier {}),
        Arc::new(DummyStateSyncNotifier::new()),
        &Handle::current(),
        TransactionFilter::new(Filter::empty()),
        true,
    );
    execution_proxy.new_epoch(
        &EpochState::new(block_for_schedule_compute().epoch(), ValidatorVerifier::new(vec![])),
        payload_manager,
        Arc::new(NoOpShuffler {}),
        BlockExecutorConfigFromOnchain::new_no_block_limit(),
        Arc::new(NoOpDeduper {}),
        false,
    );
    execution_proxy
}

fn block_for_schedule_compute() -> Block {
    let block = Block::new_for_testing(
        HashValue::zero(),
        BlockData::dummy_with_validator_txns(vec![]),
        None,
    );
    block.set_block_number(1);
    block
}

impl BlockExecutorTrait for DummyBlockExecutor {
    fn committed_block_id(&self) -> HashValue {
        HashValue::zero()
    }

    fn reset(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn execute_and_state_checkpoint(
        &self,
        block: ExecutableBlock,
        _parent_block_id: HashValue,
        _onchain_config: BlockExecutorConfigFromOnchain,
    ) -> ExecutorResult<()> {
        self.blocks_received.lock().push(block);
        Ok(())
    }

    fn ledger_update(
        &self,
        _block_id: HashValue,
        _parent_block_id: HashValue,
    ) -> ExecutorResult<StateComputeResult> {
        Ok(StateComputeResult::new_dummy())
    }

    fn commit_blocks(
        &self,
        _block_ids: Vec<HashValue>,
        _ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        Ok(())
    }

    fn finish(&self) {}

    fn pre_commit_block(&self, block_id: HashValue) -> ExecutorResult<()> {
        Ok(())
    }

    fn commit_ledger(
        &self,
        block_ids: Vec<(HashValue, u64)>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
        randomness_data: Vec<(u64, Vec<u8>)>,
    ) -> ExecutorResult<()> {
        Ok(())
    }
}

#[tokio::test]
#[cfg(test)]
async fn schedule_compute_should_discover_validator_txns() {
    use crate::payload_manager::DirectMempoolPayloadManager;

    let executor = Arc::new(DummyBlockExecutor::new());

    let execution_policy = ExecutionProxy::new(
        executor.clone(),
        Arc::new(DummyTxnNotifier {}),
        Arc::new(DummyStateSyncNotifier::new()),
        &Handle::current(),
        TransactionFilter::new(Filter::empty()),
        true,
    );

    let validator_txn_0 = ValidatorTransaction::dummy(vec![0xFF; 99]);
    let validator_txn_1 = ValidatorTransaction::dummy(vec![0xFF; 999]);

    let block = Block::new_for_testing(
        HashValue::zero(),
        BlockData::dummy_with_validator_txns(vec![
            validator_txn_0.clone(),
            validator_txn_1.clone(),
        ]),
        None,
    );

    let epoch_state = EpochState::new(block.epoch(), ValidatorVerifier::new(vec![]));

    execution_policy.new_epoch(
        &epoch_state,
        Arc::new(DirectMempoolPayloadManager::new()),
        Arc::new(NoOpShuffler {}),
        BlockExecutorConfigFromOnchain::new_no_block_limit(),
        Arc::new(NoOpDeduper {}),
        false,
    );

    // Ensure the dummy executor has received the txns.
    let _ = execution_policy
        .schedule_compute(&block, HashValue::zero(), None, dummy_guard())
        .await
        .await;

    // Get the txns from the view of the dummy executor.
    let txns = executor.blocks_received.lock()[0].transactions.clone().into_txns();

    let supposed_validator_txn_0 = txns[1].expect_valid().try_as_validator_txn().unwrap();
    let supposed_validator_txn_1 = txns[2].expect_valid().try_as_validator_txn().unwrap();
    assert_eq!(&validator_txn_0, supposed_validator_txn_0);
    assert_eq!(&validator_txn_1, supposed_validator_txn_1);
}

#[tokio::test]
async fn schedule_compute_should_propagate_payload_fetch_errors() {
    let executor = Arc::new(DummyBlockExecutor::new());
    let missing_digest = HashValue::random();
    let execution_policy = execution_proxy_with_payload_manager(
        executor.clone(),
        Arc::new(TestPayloadManager(TestPayloadResult::Missing(missing_digest))),
    );
    let block = block_for_schedule_compute();

    let result = execution_policy
        .schedule_compute(&block, HashValue::zero(), None, dummy_guard())
        .await
        .await;

    assert!(matches!(
        result,
        Err(ExecutorError::DataNotFound(digest)) if digest == missing_digest
    ));
    assert!(executor.blocks_received.lock().is_empty());
}

#[tokio::test]
async fn schedule_compute_should_reject_missing_epoch_state() {
    let executor = Arc::new(DummyBlockExecutor::new());
    let execution_policy = ExecutionProxy::new(
        executor.clone(),
        Arc::new(DummyTxnNotifier {}),
        Arc::new(DummyStateSyncNotifier::new()),
        &Handle::current(),
        TransactionFilter::new(Filter::empty()),
        true,
    );

    let result = execution_policy
        .schedule_compute(&block_for_schedule_compute(), HashValue::zero(), None, dummy_guard())
        .await
        .await;

    assert!(matches!(
        result,
        Err(ExecutorError::InternalError { error })
            if error.contains("must be set within an epoch")
    ));
    assert!(executor.blocks_received.lock().is_empty());
}

#[tokio::test]
async fn schedule_compute_should_reject_unsupported_transaction_payloads() {
    let executor = Arc::new(DummyBlockExecutor::new());
    let execution_policy = execution_proxy_with_payload_manager(
        executor.clone(),
        Arc::new(TestPayloadManager(TestPayloadResult::Transactions(vec![
            create_unsupported_signed_transaction(1),
        ]))),
    );

    let result = execution_policy
        .schedule_compute(&block_for_schedule_compute(), HashValue::zero(), None, dummy_guard())
        .await
        .await;

    assert!(matches!(
        result,
        Err(ExecutorError::InternalError { error })
            if error.contains("Unsupported transaction payload type Script")
    ));
    assert!(executor.blocks_received.lock().is_empty());
}

#[tokio::test]
async fn commit_should_discover_validator_txns() {
    let state_sync_notifier = Arc::new(DummyStateSyncNotifier::new());

    let execution_policy = ExecutionProxy::new(
        Arc::new(DummyBlockExecutor::new()),
        Arc::new(DummyTxnNotifier {}),
        state_sync_notifier.clone(),
        &tokio::runtime::Handle::current(),
        TransactionFilter::new(Filter::empty()),
        true,
    );

    let validator_txn_0 = ValidatorTransaction::dummy(vec![0xFF; 99]);
    let validator_txn_1 = ValidatorTransaction::dummy(vec![0xFF; 999]);

    let block = Block::new_for_testing(
        HashValue::zero(),
        BlockData::dummy_with_validator_txns(vec![
            validator_txn_0.clone(),
            validator_txn_1.clone(),
        ]),
        None,
    );
    unimplemented!()
    // Eventually 3 txns: block metadata, validator txn 0, validator txn 1.
    // let state_compute_result = StateComputeResult::new_dummy_with_compute_status(vec![
    //         TransactionStatus::Keep(
    //             ExecutionStatus::Success
    //         );
    //         3
    //     ]);

    // let blocks = vec![Arc::new(PipelinedBlock::new(
    //     block,
    //     vec![],
    //     state_compute_result,
    // ))];
    // let epoch_state = EpochState::empty();

    // execution_policy.new_epoch(
    //     &epoch_state,
    //     Arc::new(DirectMempoolPayloadManager::new()),
    //     Arc::new(NoOpShuffler {}),
    //     BlockExecutorConfigFromOnchain::new_no_block_limit(),
    //     Arc::new(NoOpDeduper {}),
    //     false,
    // );

    // let (tx, rx) = oneshot::channel::<()>();

    // let callback = Box::new(
    //     move |_a: &[Arc<PipelinedBlock>], _b: LedgerInfoWithSignatures| {
    //         tx.send(()).unwrap();
    //     },
    // );

    // let _ = execution_policy
    //     .commit(
    //         blocks.as_slice(),
    //         LedgerInfoWithSignatures::new(LedgerInfo::dummy(), AggregateSignature::empty()),
    //         callback,
    //     )
    //     .await;

    // // Wait until state sync is notified.
    // let _ = rx.await;

    // // Get all txns that state sync was notified with.
    // let (txns, _) = state_sync_notifier.invocations.lock()[0].clone();

    // let supposed_validator_txn_0 = txns[1].try_as_validator_txn().unwrap();
    // let supposed_validator_txn_1 = txns[2].try_as_validator_txn().unwrap();
    // assert_eq!(&validator_txn_0, supposed_validator_txn_0);
    // assert_eq!(&validator_txn_1, supposed_validator_txn_1);
}

fn dummy_guard() -> CountedRequest<()> {
    CountedRequest::new((), Arc::new(AtomicU64::new(0)))
}

#[cfg(test)]
mod oracle_signature_tests {
    use super::*;
    use crate::state_computer::process_validator_transactions_util;
    use gaptos::{
        api_types::{
            on_chain_config::jwks::{JWKStruct, ProviderJWKs as ExecutionProviderJWKs},
            ExtraDataType,
        },
        aptos_types::{
            aggregate_signature::AggregateSignature,
            dkg::DKGTranscript,
            jwks::{
                jwk::{JWKMoveStruct, JWK},
                unsupported::UnsupportedJWK,
                ProviderJWKs, QuorumCertifiedUpdate,
            },
            validator_signer::ValidatorSigner,
            validator_verifier::ValidatorConsensusInfo,
        },
    };

    fn block_with_validator_txns(txns: Vec<ValidatorTransaction>) -> Block {
        let block = Block::new_for_testing(
            HashValue::zero(),
            BlockData::dummy_with_validator_txns(txns),
            None,
        );
        block.set_block_number(1);
        block
    }

    fn validator_set(block: &Block) -> (Vec<ValidatorSigner>, EpochState) {
        let signers = (1..=4).map(|seed| ValidatorSigner::random([seed; 32])).collect::<Vec<_>>();
        let verifier = ValidatorVerifier::new(
            signers
                .iter()
                .map(|signer| ValidatorConsensusInfo::new(signer.author(), signer.public_key(), 1))
                .collect(),
        );
        assert_eq!(verifier.quorum_voting_power(), 3);
        (signers, EpochState::new(block.epoch(), verifier))
    }

    fn oracle_update() -> ProviderJWKs {
        ProviderJWKs {
            issuer: b"bridge".to_vec(),
            version: 7,
            jwks: [
                (b"event-1".to_vec(), b"payload-1".to_vec()),
                (b"event-2".to_vec(), b"payload-2".to_vec()),
            ]
            .into_iter()
            .map(|(id, payload)| {
                JWKMoveStruct::from(JWK::Unsupported(UnsupportedJWK { id, payload }))
            })
            .collect(),
        }
    }

    fn signature_from(
        update: &ProviderJWKs,
        signers: &[ValidatorSigner],
        epoch_state: &EpochState,
        count: usize,
    ) -> AggregateSignature {
        let partials = signers
            .iter()
            .take(count)
            .map(|signer| (signer.author(), signer.sign(update).unwrap()))
            .collect::<Vec<_>>();
        epoch_state
            .verifier
            .aggregate_signatures(partials.iter().map(|(author, signature)| (author, signature)))
            .unwrap()
    }

    fn oracle_txn(update: ProviderJWKs, multi_sig: AggregateSignature) -> ValidatorTransaction {
        ValidatorTransaction::ObservedJWKUpdate(QuorumCertifiedUpdate { update, multi_sig })
    }

    fn convert(
        txns: Vec<ValidatorTransaction>,
        epoch_state: &EpochState,
    ) -> anyhow::Result<Vec<ExtraDataType>> {
        let block = block_with_validator_txns(txns);
        process_validator_transactions_util(
            block.validator_txns().map(|txns| &**txns),
            &block,
            epoch_state,
        )
    }

    #[test]
    fn oracle_certificate_covers_the_entire_original_update() {
        let block = block_with_validator_txns(vec![]);
        let (signers, epoch_state) = validator_set(&block);
        let update = oracle_update();
        let quorum_signature = signature_from(&update, &signers, &epoch_state, 3);

        let valid =
            convert(vec![oracle_txn(update.clone(), quorum_signature.clone())], &epoch_state)
                .unwrap();
        assert_eq!(valid.len(), 1);
        let ExtraDataType::JWK(bytes) = &valid[0] else {
            panic!("valid Oracle update must produce JWK extra data");
        };
        let decoded: ExecutionProviderJWKs = bcs::from_bytes(bytes).unwrap();
        assert_eq!(decoded.issuer, update.issuer);
        assert_eq!(decoded.version, update.version);
        assert_eq!(decoded.jwks.len(), 2);
        assert_eq!(decoded.jwks[0].data.as_slice(), b"payload-1");
        assert_eq!(decoded.jwks[1].data.as_slice(), b"payload-2");
        // A valid certificate must keep the external executor's original wire format.
        let expected = ExecutionProviderJWKs {
            issuer: update.issuer.clone(),
            version: update.version,
            jwks: vec![
                JWKStruct {
                    type_name: "0x1::jwks::Unsupported_JWK".to_string(),
                    data: b"payload-1".to_vec(),
                },
                JWKStruct {
                    type_name: "0x1::jwks::Unsupported_JWK".to_string(),
                    data: b"payload-2".to_vec(),
                },
            ],
        };
        assert_eq!(bytes, &bcs::to_bytes(&expected).unwrap());

        let invalid_certificates = [
            AggregateSignature::empty(),
            signature_from(&update, &signers, &epoch_state, 2),
            AggregateSignature::new((*quorum_signature.get_signers_bitvec()).clone(), None),
            signature_from(
                &ProviderJWKs { version: update.version + 1, ..update.clone() },
                &signers,
                &epoch_state,
                3,
            ),
        ];
        for multi_sig in invalid_certificates {
            assert!(convert(vec![oracle_txn(update.clone(), multi_sig)], &epoch_state)
                .unwrap()
                .is_empty());
        }

        let mut invalid_bitmap = (*quorum_signature.get_signers_bitvec()).clone();
        invalid_bitmap.set(4); // Four validators have indices 0 through 3.
        let malformed_certificate =
            AggregateSignature::new(invalid_bitmap, (*quorum_signature.sig()).clone());
        assert!(convert(vec![oracle_txn(update.clone(), malformed_certificate)], &epoch_state,)
            .unwrap()
            .is_empty());

        let mut modified_updates = vec![
            ProviderJWKs { issuer: b"price-feed".to_vec(), ..update.clone() },
            ProviderJWKs { version: update.version + 1, ..update.clone() },
        ];
        let mut changed_id = update.clone();
        changed_id.jwks[1] = JWKMoveStruct::from(JWK::Unsupported(UnsupportedJWK {
            id: b"changed-id".to_vec(),
            payload: b"payload-2".to_vec(),
        }));
        modified_updates.push(changed_id);
        let mut changed_payload = update.clone();
        changed_payload.jwks[1] = JWKMoveStruct::from(JWK::Unsupported(UnsupportedJWK {
            id: b"event-2".to_vec(),
            payload: b"changed-payload".to_vec(),
        }));
        modified_updates.push(changed_payload);
        for modified in modified_updates {
            assert!(convert(vec![oracle_txn(modified, quorum_signature.clone())], &epoch_state,)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn invalid_oracle_certificate_drops_only_its_update() {
        let block = block_with_validator_txns(vec![]);
        let (signers, epoch_state) = validator_set(&block);
        let valid_update = oracle_update();
        let valid_signature = signature_from(&valid_update, &signers, &epoch_state, 3);
        let invalid_update = ProviderJWKs { issuer: b"price-feed".to_vec(), ..oracle_update() };
        let invalid = oracle_txn(invalid_update, AggregateSignature::empty());
        let dkg = ValidatorTransaction::DKGResult(DKGTranscript::new(
            block.epoch(),
            signers[0].author(),
            vec![1, 2, 3],
        ));
        let valid = oracle_txn(valid_update.clone(), valid_signature);

        let converted = convert(vec![invalid, dkg, valid], &epoch_state).unwrap();
        assert_eq!(converted.len(), 2);
        assert!(matches!(&converted[0], ExtraDataType::DKG(_)));
        let ExtraDataType::JWK(bytes) = &converted[1] else {
            panic!("the valid Oracle update must remain after the DKG result");
        };
        let decoded: ExecutionProviderJWKs = bcs::from_bytes(bytes).unwrap();
        assert_eq!(decoded.issuer, valid_update.issuer);
        assert_eq!(decoded.jwks.len(), 2);
    }

    #[test]
    fn oracle_conversion_rejects_epoch_mismatch() {
        let block = block_with_validator_txns(vec![]);
        let (signers, matching_epoch_state) = validator_set(&block);
        let wrong_epoch_state = EpochState {
            epoch: matching_epoch_state.epoch + 1,
            verifier: matching_epoch_state.verifier.clone(),
        };
        let update = oracle_update();
        let signature = signature_from(&update, &signers, &matching_epoch_state, 3);

        let error = convert(vec![oracle_txn(update, signature)], &wrong_epoch_state).unwrap_err();
        assert!(error.to_string().contains("belongs to epoch"));
    }
}
