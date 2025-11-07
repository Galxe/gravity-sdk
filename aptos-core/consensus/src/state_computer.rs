// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::block_storage::tracing::{observe_block, BlockStage};
use crate::counters::update_counters_for_compute_res;
use crate::pipeline::pipeline_builder::PipelineBuilder;
use crate::{
    block_preparer::BlockPreparer,
    error::StateSyncError,
    execution_pipeline::ExecutionPipeline,
    monitor,
    payload_manager::TPayloadManager,
    pipeline::pipeline_phase::CountedRequest,
    state_replication::{StateComputer, StateComputerCommitCallBackType},
    transaction_deduper::TransactionDeduper,
    transaction_filter::TransactionFilter,
    transaction_shuffler::TransactionShuffler,
    txn_notifier::TxnNotifier,
};
use anyhow::Result;
use gaptos::api_types::account::{ExternalAccountAddress, ExternalChainId};
use gaptos::api_types::u256_define::{BlockId, Random, TxnHash};
use gaptos::api_types::{ExternalBlock, ExternalBlockMeta, ExtraDataType};
use aptos_consensus_types::common::RejectedTransactionSummary;
use gaptos::aptos_consensus_notifications::ConsensusNotificationSender;
use aptos_consensus_types::{
    block::Block, common::Round, pipeline_execution_result::PipelineExecutionResult,
    pipelined_block::PipelinedBlock,
};
use gaptos::aptos_crypto::HashValue;
use aptos_executor_types::{BlockExecutorTrait, ExecutorResult, StateComputeResult};
use gaptos::aptos_infallible::RwLock;
use gaptos::aptos_logger::prelude::*;
use aptos_mempool::core_mempool::transaction::VerifiedTxn;

use gaptos::aptos_types::jwks::jwk::JWK;
use gaptos::aptos_types::jwks::{self, ProviderJWKs};
use gaptos::aptos_types::transaction::SignedTransaction;
use gaptos::aptos_types::validator_signer::ValidatorSigner;
use gaptos::aptos_types::validator_txn::ValidatorTransaction;
use gaptos::aptos_types::{
    account_address::AccountAddress, block_executor::config::BlockExecutorConfigFromOnchain,
    contract_event::ContractEvent, epoch_state::EpochState, ledger_info::LedgerInfoWithSignatures,
    randomness::Randomness, transaction::Transaction, vm_status::{DiscardedVMStatus, StatusCode}
};
use block_buffer_manager::get_block_buffer_manager;
use fail::fail_point;
use futures::{future::BoxFuture, SinkExt, StreamExt};
use std::iter::once;
use std::{boxed::Box, sync::Arc, time::Duration};
use tokio::sync::Mutex as AsyncMutex;
use gaptos::aptos_consensus::counters as counters;
use counters::APTOS_EXECUTION_TXNS;

pub type StateComputeResultFut = BoxFuture<'static, ExecutorResult<PipelineExecutionResult>>;

type NotificationType = (
    Box<dyn FnOnce() + Send + Sync>,
    Vec<Transaction>,
    Vec<ContractEvent>, // Subscribable events, e.g. NewEpochEvent, DKGStartEvent
    u64, // block number
);

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
struct LogicalTime {
    epoch: u64,
    round: Round,
}

impl LogicalTime {
    pub fn new(epoch: u64, round: Round) -> Self {
        Self { epoch, round }
    }
}

#[derive(Clone)]
struct MutableState {
    validators: Arc<[AccountAddress]>,
    payload_manager: Arc<dyn TPayloadManager>,
    transaction_shuffler: Arc<dyn TransactionShuffler>,
    block_executor_onchain_config: BlockExecutorConfigFromOnchain,
    transaction_deduper: Arc<dyn TransactionDeduper>,
    is_randomness_enabled: bool,
}

/// Basic communication with the Execution module;
/// implements StateComputer traits.
pub struct ExecutionProxy {
    executor: Arc<dyn BlockExecutorTrait>,
    txn_notifier: Arc<dyn TxnNotifier>,
    state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
    async_state_sync_notifier: gaptos::aptos_channels::Sender<NotificationType>,
    write_mutex: AsyncMutex<LogicalTime>,
    transaction_filter: Arc<TransactionFilter>,
    execution_pipeline: ExecutionPipeline,
    state: RwLock<Option<MutableState>>,
}

impl ExecutionProxy {
    async fn get_block_txns(&self, block: &Block) -> Vec<SignedTransaction> {
        let MutableState {
            validators,
            payload_manager,
            transaction_shuffler,
            block_executor_onchain_config,
            transaction_deduper,
            is_randomness_enabled,
        } = self.state.read().as_ref().cloned().expect("must be set within an epoch");
        let mut txns = vec![];
        match payload_manager.get_transactions(block).await {
            Ok((transactions, _)) => {
                txns.extend(transactions);
            }
            Err(e) => {
                warn!("failed to get transactions from block {:?}, error {:?}", block, e);
            }
        }
        txns
    }
    pub fn new(
        executor: Arc<dyn BlockExecutorTrait>,
        txn_notifier: Arc<dyn TxnNotifier>,
        state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
        handle: &tokio::runtime::Handle,
        txn_filter: TransactionFilter,
        enable_pre_commit: bool,
    ) -> Self {
        let (tx, mut rx) =
            gaptos::aptos_channels::new::<NotificationType>(10, &counters::PENDING_STATE_SYNC_NOTIFICATION);
        let notifier = state_sync_notifier.clone();
        handle.spawn(async move {
            while let Some((callback, txns, subscribable_events, block_number)) = rx.next().await {
                if let Err(e) = monitor!(
                    "notify_state_sync",
                    notifier.notify_new_commit(txns, subscribable_events, block_number).await
                ) {
                    error!(error = ?e, "Failed to notify state synchronizer");
                }

                callback();
            }
        });
        let execution_pipeline =
            ExecutionPipeline::spawn(executor.clone(), handle, enable_pre_commit);
        Self {
            executor,
            txn_notifier,
            state_sync_notifier,
            async_state_sync_notifier: tx,
            write_mutex: AsyncMutex::new(LogicalTime::new(0, 0)),
            transaction_filter: Arc::new(txn_filter),
            execution_pipeline,
            state: RwLock::new(None),
        }
    }

    fn transactions_to_commit(
        &self,
        executed_block: &PipelinedBlock,
        validators: &[AccountAddress],
        randomness_enabled: bool,
    ) -> Vec<Transaction> {
        // reconfiguration suffix don't execute
        if executed_block.is_reconfiguration_suffix() {
            return vec![];
        }

        let user_txns = executed_block.input_transactions().clone();
        let validator_txns = executed_block.validator_txns().cloned().unwrap_or_default();
        let metadata = if randomness_enabled {
            executed_block
                .block()
                .new_metadata_with_randomness(validators, executed_block.randomness().cloned())
        } else {
            executed_block.block().new_block_metadata(validators).into()
        };

        // let input_txns = Block::combine_to_input_transactions(validator_txns, user_txns, metadata);

        // TODO(gravity_byteyue): We manually skipped the validator txns and metadata here.
        // we might need to re-add them back
        // Adds StateCheckpoint/BlockEpilogue transaction if needed.
        once(Transaction::from(metadata))
            .chain(user_txns.into_iter().map(Transaction::UserTransaction))
            .collect()
        // executed_block
        //     .compute_result()
        //     .transactions_to_commit(user_txns.into_iter().map(Transaction::UserTransaction).collect())
        // input_txns
    }

    pub fn pipeline_builder(&self, commit_signer: Arc<ValidatorSigner>) -> PipelineBuilder {
        let MutableState {
            validators,
            payload_manager,
            transaction_shuffler,
            block_executor_onchain_config,
            transaction_deduper,
            is_randomness_enabled,
        } = self.state.read().as_ref().cloned().expect("must be set within an epoch");

        let block_preparer = Arc::new(BlockPreparer::new(
            payload_manager.clone(),
            self.transaction_filter.clone(),
            transaction_deduper.clone(),
            transaction_shuffler.clone(),
        ));
        PipelineBuilder::new(
            block_preparer,
            self.executor.clone(),
            validators,
            block_executor_onchain_config,
            is_randomness_enabled,
            commit_signer,
            self.state_sync_notifier.clone(),
            payload_manager,
            self.txn_notifier.clone(),
        )
    }

    /// Process validator transactions (DKG and JWK) and extract extra data for execution
    fn process_validator_transactions(
        &self,
        validator_txns: Option<&[ValidatorTransaction]>,
        block: &Block,
    ) -> Vec<ExtraDataType> {
        let mut extra_data = Vec::new();
        
        if let Some(validator_txns) = validator_txns {
            extra_data = validator_txns
                .iter()
                .map(|txn| self.process_single_validator_transaction(txn, block))
                .collect();
        }
        
        extra_data
    }

    /// Process a single validator transaction (DKG or JWK) and return the serialized data
    fn process_single_validator_transaction(
        &self,
        txn: &ValidatorTransaction,
        block: &Block,
    ) -> ExtraDataType {
        match txn {
            ValidatorTransaction::DKGResult(transcript) => {
                let transcript = gaptos::api_types::on_chain_config::dkg::DKGTranscript {
                    metadata: gaptos::api_types::on_chain_config::dkg::DKGTranscriptMetadata {
                        epoch: transcript.metadata.epoch,
                        author: ExternalAccountAddress::new(transcript.metadata.author.into_bytes()),
                    },
                    transcript_bytes: transcript.transcript_bytes.clone(),
                };
                ExtraDataType::DKG(bcs::to_bytes(&transcript).unwrap())
            }
            ValidatorTransaction::ObservedJWKUpdate(jwks::QuorumCertifiedUpdate {
                update,
                multi_sig,
            }) => {
                ExtraDataType::JWK(self.process_jwk_update(&update, block))
            }
        }
    }

    /// Process JWK update and convert to gaptos format
    fn process_jwk_update(
        &self,
        update: &ProviderJWKs,
        block: &Block,
    ) -> Vec<u8> {
        use gaptos::api_types::on_chain_config::jwks::{JWKStruct, ProviderJWKs};
        // TODO(Gravity): Check the signature here instead of execution layer
        info!(
            "jwk txn block number {} , {:?}",
            block.block_number().unwrap_or_else(|| panic!("No block number")),
            update.version
        );

        let gaptos_provider_jwk = ProviderJWKs {
            issuer: update.issuer.clone(),
            version: update.version,
            jwks: update
                .jwks
                .iter()
                .map(|jwk| {
                    let aptos_jwk = JWK::try_from(jwk).unwrap();
                    match aptos_jwk {
                        JWK::RSA(rsa_jwk) => JWKStruct {
                            type_name: "0".to_string(),
                            data: serde_json::to_vec(&rsa_jwk).unwrap(),
                        },
                        JWK::Unsupported(unsupported_jwk) => {
                            JWKStruct {
                                type_name: "1".to_string(),
                                data: unsupported_jwk.payload,
                            }
                        }
                    }
                })
                .collect(),
        };

        bcs::to_bytes(&gaptos_provider_jwk).unwrap()
    }
}

/// Public utility function to process validator transactions (JWK and DKG)
pub fn process_validator_transactions_util(
    validator_txns: Option<&[ValidatorTransaction]>,
    block: &Block,
) -> Vec<ExtraDataType> {
    let mut extra_data = Vec::new();
    
    if let Some(validator_txns) = validator_txns {
        extra_data = validator_txns
            .iter()
            .map(|txn| process_single_validator_transaction_util(txn, block))
            .collect();
    }
    
    extra_data
}

/// Public utility function to process a single validator transaction
pub fn process_single_validator_transaction_util(
    txn: &ValidatorTransaction,
    block: &Block,
) -> ExtraDataType {
    match txn {
        ValidatorTransaction::DKGResult(transcript) => {
            let transcript = gaptos::api_types::on_chain_config::dkg::DKGTranscript {
                metadata: gaptos::api_types::on_chain_config::dkg::DKGTranscriptMetadata {
                    epoch: transcript.metadata.epoch,
                    author: ExternalAccountAddress::new(transcript.metadata.author.into_bytes()),
                },
                transcript_bytes: transcript.transcript_bytes.clone(),
            };
            ExtraDataType::DKG(bcs::to_bytes(&transcript).unwrap())
        }
        ValidatorTransaction::ObservedJWKUpdate(jwks::QuorumCertifiedUpdate {
            update,
            multi_sig,
        }) => {
            ExtraDataType::JWK(process_jwk_update_util(&update, block))
        }
    }
}

/// Public utility function to process JWK update
pub fn process_jwk_update_util(
    update: &ProviderJWKs,
    block: &Block,
) -> Vec<u8> {
    use gaptos::api_types::on_chain_config::jwks::{JWKStruct, ProviderJWKs};
    // TODO(Gravity): Check the signature here instead of execution layer
    info!(
        "jwk txn block number {} , {:?}",
        block.block_number().unwrap_or_else(|| panic!("No block number")),
        update.version
    );

    let gaptos_provider_jwk = ProviderJWKs {
        issuer: update.issuer.clone(),
        version: update.version,
        jwks: update
            .jwks
            .iter()
            .map(|jwk| {
                let aptos_jwk = JWK::try_from(jwk).unwrap();
                match aptos_jwk {
                    JWK::RSA(rsa_jwk) => JWKStruct {
                        type_name: "0".to_string(),
                        data: serde_json::to_vec(&rsa_jwk).unwrap(),
                    },
                    JWK::Unsupported(unsupported_jwk) => {
                        JWKStruct {
                            type_name: "1".to_string(),
                            data: unsupported_jwk.payload,
                        }
                    }
                }
            })
            .collect(),
    };

    bcs::to_bytes(&gaptos_provider_jwk).unwrap()
}

#[async_trait::async_trait]
impl StateComputer for ExecutionProxy {
    async fn schedule_compute(
        &self,
        // The block to be executed.
        block: &Block,
        // The parent block id.
        parent_block_id: HashValue,
        randomness: Option<Randomness>,
        lifetime_guard: CountedRequest<()>,
    ) -> StateComputeResultFut {
        assert!(block.block_number().is_some());
        let txns = self.get_block_txns(block).await;
        let validator_txns = block.validator_txns();
        let extra_data = process_validator_transactions_util(validator_txns.map(|v| &**v), block);
        
        let meta_data = ExternalBlockMeta {
            block_id: BlockId(*block.id()),
            block_number: block.block_number().unwrap_or_else(|| panic!("No block number")),
            usecs: block.timestamp_usecs(),
            epoch: block.epoch(),
            randomness: randomness.map(|r| Random::from_bytes(r.randomness())),
            block_hash: None,
            proposer: block.author().map(|author| ExternalAccountAddress::new(author.into_bytes())),
        };

        // We would export the empty block detail to the outside GCEI caller
        let vtxns =
            txns.iter().map(|txn| Into::<VerifiedTxn>::into(&txn.clone())).collect::<Vec<_>>();
        let real_txns: Vec<gaptos::api_types::VerifiedTxn> = vtxns
            .into_iter()
            .map(|txn| {
                gaptos::api_types::VerifiedTxn::new(
                    txn.bytes().to_vec(),
                    ExternalAccountAddress::new(txn.sender().into_bytes()),
                    txn.sequence_number(),
                    ExternalChainId::new(txn.chain_id().id()),
                    TxnHash::from_bytes(&txn.get_hash().to_vec()),
                )
            })
            .collect();
        APTOS_EXECUTION_TXNS.observe(real_txns.len() as f64);
        let txn_notifier = self.txn_notifier.clone();
        let block_id_hashvalue = block.id();
        let enable_randomness = self.state.read().as_ref().unwrap().is_randomness_enabled;
        Box::pin(async move {
            let block_id = meta_data.block_id;
            let block_timestamp = meta_data.usecs;
            txn_metrics::TxnLifeTime::get_txn_life_time().record_executing(block_id_hashvalue.clone());
            get_block_buffer_manager()
                .set_ordered_blocks(BlockId::from_bytes(parent_block_id.as_slice()), ExternalBlock {
                    block_meta: meta_data.clone(),
                    txns: real_txns,
                    extra_data,
                    enable_randomness: enable_randomness,
                })
                .await.unwrap_or_else(|e| panic!("Failed to push ordered blocks {}", e));
            let u_ts = meta_data.usecs;
            let compute_result = get_block_buffer_manager()
                .get_executed_res(block_id, meta_data.block_number)
                .await?;
            txn_metrics::TxnLifeTime::get_txn_life_time().record_executed(block_id_hashvalue);
            update_counters_for_compute_res(&compute_result.execution_output);
           
            observe_block(u_ts, BlockStage::EXECUTED);
            let txn_status = compute_result.execution_output.txn_status.clone();
            match (*txn_status).as_ref() {
                Some(txn_status) => {
                    let rejected_txns = txn_status.iter().filter(|txn| txn.is_discarded).map(|discard_txn_info| {
                        RejectedTransactionSummary {
                            sender: discard_txn_info.sender.into(),
                            sequence_number: discard_txn_info.nonce,
                            hash: HashValue::new(discard_txn_info.txn_hash),
                            reason: DiscardedVMStatus::from(StatusCode::SEQUENCE_NONCE_INVALID),
                        }
                    }).collect::<Vec<_>>();
                    if let Err(e) = txn_notifier.notify_failed_txn(rejected_txns).await {
                        error!(error = ?e, "Failed to notify mempool of rejected txns");
                    }
                }
                None => {}
            }

            let pre_commit_fut: BoxFuture<'static, ExecutorResult<()>> =
                    {
                        Box::pin(async move {
                            Ok(())
                        })
                    };
            Ok(PipelineExecutionResult::new(txns, compute_result, Duration::ZERO, pre_commit_fut))
        })
    }

    /// Send a successful commit. A future is fulfilled when the state is finalized.
    async fn commit(
        &self,
        blocks: &[Arc<PipelinedBlock>],
        finality_proof: LedgerInfoWithSignatures,
        callback: StateComputerCommitCallBackType,
    ) -> ExecutorResult<()> {
        let block_id_num = blocks.iter().map(|block| (block.id(), block.block().block_number())).collect::<Vec<_>>();
        info!(
            "Received a commit request for blocks {:?} at round {}",
            block_id_num,
            finality_proof.ledger_info().round()
        );
        let mut latest_logical_time = self.write_mutex.lock().await;
        let mut txns = Vec::new();
        let mut subscribable_txn_events = Vec::new();
        let mut payloads = Vec::new();
        let logical_time = LogicalTime::new(
            finality_proof.ledger_info().epoch(),
            finality_proof.ledger_info().round(),
        );
        let block_timestamp = finality_proof.commit_info().timestamp_usecs();

        let MutableState { payload_manager, validators, is_randomness_enabled, .. } =
            self.state.read().as_ref().cloned().expect("must be set within an epoch");
        let mut committed_block_ids = vec![];
        // TODO(gravity_lightman): The take_pre_commit_fut will cause a coredump. 
        // The reason has not been found yet, 
        // but this asynchronous function is an empty one and can be skipped
        // let mut pre_commit_futs = Vec::with_capacity(blocks.len());
        let mut block_ids = vec![];
        for block in blocks {
            if let Some(payload) = block.block().payload() {
                payloads.push(payload.clone());
            }
            committed_block_ids.push(block.id());

            let commit_transactions =
                self.transactions_to_commit(block, &validators, is_randomness_enabled);
            if !commit_transactions.is_empty() {
                txns.extend(commit_transactions);
            }
            subscribable_txn_events.extend(block.subscribable_events());
            // pre_commit_futs.push(block.take_pre_commit_fut());
            block_ids.push(block.id());
        }

        // wait until all blocks are committed
        // for pre_commit_fut in pre_commit_futs {
        //     pre_commit_fut.await?
        // }

        let executor = self.executor.clone();
        let proof = finality_proof.clone();

        monitor!(
            "commit_block",
            tokio::task::spawn_blocking(move || {
                executor.commit_ledger(block_ids, proof).expect("Failed to commit blocks");
            })
            .await
        )
        .expect("spawn_blocking failed");

        let blocks = blocks.to_vec();
        let block_number = blocks.last().unwrap().block().block_number().unwrap_or_else(|| panic!("No block number"));
        let wrapped_callback = move || {
            payload_manager.notify_commit(block_timestamp, payloads);
            callback(&blocks, finality_proof);
        };
        self.async_state_sync_notifier
            .clone()
            .send((Box::new(wrapped_callback), txns, subscribable_txn_events, block_number))
            .await
            .expect("Failed to send async state sync notification");
        // tokio::time::sleep(Duration::from_millis(1)).await;
        *latest_logical_time = logical_time;
        Ok(())
    }

    /// Synchronize to a commit that not present locally.
    async fn sync_to(&self, target: LedgerInfoWithSignatures) -> Result<(), StateSyncError> {
        let mut latest_logical_time = self.write_mutex.lock().await;
        let logical_time =
            LogicalTime::new(target.ledger_info().epoch(), target.ledger_info().round());
        let block_timestamp = target.commit_info().timestamp_usecs();

        // Before the state synchronization, we have to call finish() to free the in-memory SMT
        // held by BlockExecutor to prevent memory leak.
        self.executor.finish();

        // The pipeline phase already committed beyond the target block timestamp, just return.
        if *latest_logical_time >= logical_time {
            warn!(
                "State sync target {:?} is lower than already committed logical time {:?}",
                logical_time, *latest_logical_time
            );
            return Ok(());
        }

        // This is to update QuorumStore with the latest known commit in the system,
        // so it can set batches expiration accordingly.
        // Might be none if called in the recovery path, or between epoch stop and start.
        if let Some(inner) = self.state.read().as_ref() {
            inner.payload_manager.notify_commit(block_timestamp, Vec::new());
        }

        fail_point!("consensus::sync_to", |_| {
            Err(anyhow::anyhow!("Injected error in sync_to").into())
        });
        // Here to start to do state synchronization where ChunkExecutor inside will
        // process chunks and commit to Storage. However, after block execution and
        // commitments, the sync state of ChunkExecutor may be not up to date so
        // it is required to reset the cache of ChunkExecutor in State Sync
        // when requested to sync.
        let res = monitor!("sync_to", self.state_sync_notifier.sync_to_target(target).await);
        *latest_logical_time = logical_time;

        // Similarly, after the state synchronization, we have to reset the cache
        // of BlockExecutor to guarantee the latest committed state is up to date.
        self.executor.reset()?;

        res.map_err(|error| {
            let anyhow_error: anyhow::Error = error.into();
            anyhow_error.into()
        })
    }

    fn new_epoch(
        &self,
        epoch_state: &EpochState,
        payload_manager: Arc<dyn TPayloadManager>,
        transaction_shuffler: Arc<dyn TransactionShuffler>,
        block_executor_onchain_config: BlockExecutorConfigFromOnchain,
        transaction_deduper: Arc<dyn TransactionDeduper>,
        randomness_enabled: bool,
    ) {
        *self.state.write() = Some(MutableState {
            validators: epoch_state
                .verifier
                .get_ordered_account_addresses_iter()
                .collect::<Vec<_>>()
                .into(),
            payload_manager,
            transaction_shuffler,
            block_executor_onchain_config,
            transaction_deduper,
            is_randomness_enabled: randomness_enabled,
        });
    }

    // Clears the epoch-specific state. Only a sync_to call is expected before calling new_epoch
    // on the next epoch.
    fn end_epoch(&self) {
        self.state.write().take();
    }
}

#[tokio::test]
async fn test_commit_sync_race() {
    use crate::{
        error::MempoolError, payload_manager::DirectMempoolPayloadManager,
        transaction_deduper::create_transaction_deduper,
        transaction_shuffler::create_transaction_shuffler,
    };

    use gaptos::aptos_config::config::transaction_filter_type::Filter;
    use gaptos::aptos_consensus_notifications::Error;
    
    use gaptos::aptos_infallible::Mutex;
    use gaptos::aptos_types::{
        aggregate_signature::AggregateSignature,
        block_executor::partitioner::ExecutableBlock,
        block_info::BlockInfo,
        ledger_info::LedgerInfo,
        on_chain_config::{TransactionDeduperType, TransactionShufflerType},
        transaction::{SignedTransaction, TransactionStatus},
    };

    struct RecordedCommit {
        time: Mutex<LogicalTime>,
    }

    impl BlockExecutorTrait for RecordedCommit {
        fn committed_block_id(&self) -> HashValue {
            HashValue::zero()
        }

        fn reset(&self) -> Result<()> {
            Ok(())
        }

        // fn execute_block(
        //     &self,
        //     _block: ExecutableBlock,
        //     _parent_block_id: HashValue,
        //     _onchain_config: BlockExecutorConfigFromOnchain,
        // ) -> ExecutorResult<StateComputeResult> {
        //     Ok(StateComputeResult::new_dummy())
        // }

        fn execute_and_state_checkpoint(
            &self,
            _block: ExecutableBlock,
            _parent_block_id: HashValue,
            _onchain_config: BlockExecutorConfigFromOnchain,
        ) -> ExecutorResult<()> {
            todo!()
        }

        fn ledger_update(
            &self,
            _block_id: HashValue,
            _parent_block_id: HashValue,
        ) -> ExecutorResult<StateComputeResult> {
            todo!()
        }

        fn pre_commit_block(&self, _block_id: HashValue) -> ExecutorResult<()> {
            todo!()
        }

        fn commit_ledger(
            &self,
            block_ids: Vec<HashValue>,
            ledger_info_with_sigs: LedgerInfoWithSignatures,
        ) -> ExecutorResult<()> {
            *self.time.lock() = LogicalTime::new(
                ledger_info_with_sigs.ledger_info().epoch(),
                ledger_info_with_sigs.ledger_info().round(),
            );
            Ok(())
        }

        fn finish(&self) {}
    }

    #[async_trait::async_trait]
    impl TxnNotifier for RecordedCommit {
        async fn notify_failed_txn(
            &self,
            _rejected_txns: Vec<RejectedTransactionSummary>,
        ) -> Result<(), MempoolError> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl ConsensusNotificationSender for RecordedCommit {
        async fn notify_new_commit(
            &self,
            _transactions: Vec<Transaction>,
            _subscribable_events: Vec<ContractEvent>,
            _block_number: u64,
        ) -> std::result::Result<(), Error> {
            Ok(())
        }

        async fn sync_to_target(
            &self,
            target: LedgerInfoWithSignatures,
        ) -> std::result::Result<(), Error> {
            let logical_time =
                LogicalTime::new(target.ledger_info().epoch(), target.ledger_info().round());
            if logical_time <= *self.time.lock() {
                return Err(Error::NotificationError("Decreasing logical time".to_string()));
            }
            *self.time.lock() = logical_time;
            Ok(())
        }

        async fn sync_for_duration(
            &self,
            _duration: Duration,
        ) -> Result<LedgerInfoWithSignatures, Error> {
            todo!()
        }
    }

    let callback = Box::new(move |_a: &[Arc<PipelinedBlock>], _b: LedgerInfoWithSignatures| {});
    let recorded_commit = Arc::new(RecordedCommit { time: Mutex::new(LogicalTime::new(0, 0)) });
    let generate_li = |epoch, round| {
        LedgerInfoWithSignatures::new(
            LedgerInfo::new(BlockInfo::random_with_epoch(epoch, round), HashValue::zero()),
            AggregateSignature::empty(),
        )
    };
    let executor = ExecutionProxy::new(
        recorded_commit.clone(),
        recorded_commit.clone(),
        recorded_commit.clone(),
        &tokio::runtime::Handle::current(),
        TransactionFilter::new(Filter::empty()),
        true,
    );

    executor.new_epoch(
        &EpochState::empty(),
        Arc::new(DirectMempoolPayloadManager {}),
        create_transaction_shuffler(TransactionShufflerType::NoShuffling),
        BlockExecutorConfigFromOnchain::new_no_block_limit(),
        create_transaction_deduper(TransactionDeduperType::NoDedup),
        false,
    );
    executor.commit(&[], generate_li(1, 1), callback.clone()).await.unwrap();
    executor.commit(&[], generate_li(1, 10), callback).await.unwrap();
    assert!(executor.sync_to(generate_li(1, 8)).await.is_ok());
    assert_eq!(*recorded_commit.time.lock(), LogicalTime::new(1, 10));
    assert!(executor.sync_to(generate_li(2, 8)).await.is_ok());
    assert_eq!(*recorded_commit.time.lock(), LogicalTime::new(2, 8));
}
