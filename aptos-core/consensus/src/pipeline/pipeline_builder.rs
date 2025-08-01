// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_preparer::BlockPreparer,
    block_storage::tracing::{observe_block, BlockStage},
    counters::{update_counters_for_block, update_counters_for_compute_res, update_counters_for_compute_result},
    execution_pipeline::SIG_VERIFY_POOL,
    monitor,
    payload_manager::TPayloadManager,
    txn_notifier::TxnNotifier,
};
use gaptos::aptos_consensus::counters as counters;
use anyhow::anyhow;
use gaptos::api_types::{account::{ExternalAccountAddress, ExternalChainId}, u256_define::{BlockId, Random, TxnHash}, ExternalBlock, ExternalBlockMeta};
use gaptos::aptos_consensus_notifications::ConsensusNotificationSender;
use aptos_consensus_types::{
    block::Block,
    common::{RejectedTransactionSummary, Round},
    pipeline::commit_vote::CommitVote,
    pipelined_block::{
        CommitLedgerResult, CommitVoteResult, ExecuteResult, LedgerUpdateResult, PipelineFutures,
        PipelineInputRx, PipelineInputTx, PostCommitResult, PostLedgerUpdateResult,
        PostPreCommitResult, PreCommitResult, PrepareResult, TaskError, TaskFuture, TaskResult,
    },
};
use gaptos::aptos_crypto::HashValue;
use aptos_executor_types::{StateComputeResult, BlockExecutorTrait};
use gaptos::aptos_experimental_runtimes::thread_manager::optimal_min_len;
use gaptos::aptos_logger::{error, info, warn};
use aptos_mempool::core_mempool::transaction::VerifiedTxn;
use gaptos::aptos_types::{
    block_executor::config::BlockExecutorConfigFromOnchain,
    block_metadata_ext::BlockMetadataExt,
    ledger_info::{LedgerInfo, LedgerInfoWithSignatures},
    randomness::Randomness,
    transaction::{
        signature_verified_transaction::SignatureVerifiedTransaction, SignedTransaction, Transaction
    },
    validator_signer::ValidatorSigner, vm_status::{DiscardedVMStatus, StatusCode},
};
use block_buffer_manager::get_block_buffer_manager;
use futures::FutureExt;
use itertools::Itertools;
use gaptos::move_core_types::account_address::AccountAddress;
use rayon::prelude::*;
use std::{
    collections::HashMap, future::Future, sync::Arc, time::{Duration, Instant}
};
use tokio::{select, sync::{oneshot, Mutex}, task::AbortHandle};

/// The pipeline builder is responsible for constructing the pipeline structure for a block.
/// Each phase is represented as a shared future, takes in other futures as pre-condition.
/// Future returns a TaskResult<T>, which error can be either a user error or task error (e.g. cancellation).
///
/// Currently, the critical path is the following, more details can be found in the comments of each phase.
/// prepare -> execute -> ledger update -> pre-commit -> commit ledger
///    rand ->
///                         order proof ->
///                                      commit proof ->
#[derive(Clone)]
pub struct PipelineBuilder {
    block_preparer: Arc<BlockPreparer>,
    executor: Arc<dyn BlockExecutorTrait>,
    validators: Arc<[AccountAddress]>,
    block_executor_onchain_config: BlockExecutorConfigFromOnchain,
    is_randomness_enabled: bool,
    signer: Arc<ValidatorSigner>,
    state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
    payload_manager: Arc<dyn TPayloadManager>,
    txn_notifier: Arc<dyn TxnNotifier>,
    block_metadata: Arc<Mutex<HashMap<BlockId, ExternalBlockMeta>>>,
}

fn spawn_shared_fut<
    T: Send + Clone + 'static,
    F: Future<Output = TaskResult<T>> + Send + 'static,
>(
    f: F,
    abort_handles: &mut Vec<AbortHandle>,
) -> TaskFuture<T> {
    let join_handle = tokio::spawn(f);
    abort_handles.push(join_handle.abort_handle());
    async move {
        match join_handle.await {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(e)) => Err(TaskError::PropagatedError(Box::new(e))),
            Err(e) => Err(TaskError::JoinError(Arc::new(e))),
        }
    }
    .boxed()
    .shared()
}

fn spawn_ready_fut<T: Send + Clone + 'static>(f: T) -> TaskFuture<T> {
    async move { Ok(f) }.boxed().shared()
}

async fn wait_and_log_error<T, F: Future<Output = TaskResult<T>>>(f: F, msg: String) {
    if let Err(TaskError::InternalError(e)) = f.await {
        warn!("{} failed: {}", msg, e);
    }
}

struct Tracker {
    name: &'static str,
    block_id: HashValue,
    epoch: u64,
    round: Round,
}

impl Tracker {
    pub fn new(name: &'static str, block: &Block) -> Self {
        let ret = Self {
            name,
            block_id: block.id(),
            epoch: block.epoch(),
            round: block.round(),
        };
        ret.log_start();
        ret
    }

    fn log_start(&self) {
        info!(
            "[Pipeline] Block {} {} {} enters {}",
            self.block_id, self.epoch, self.round, self.name
        );
    }

    fn log_end(&self) {
        info!(
            "[Pipeline] Block {} {} {} finishes {}",
            self.block_id, self.epoch, self.round, self.name
        );
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.log_end();
    }
}

// TODO: add counters for each phase
impl PipelineBuilder {
    pub fn new(
        block_preparer: Arc<BlockPreparer>,
        executor: Arc<dyn BlockExecutorTrait>,
        validators: Arc<[AccountAddress]>,
        block_executor_onchain_config: BlockExecutorConfigFromOnchain,
        is_randomness_enabled: bool,
        signer: Arc<ValidatorSigner>,
        state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
        payload_manager: Arc<dyn TPayloadManager>,
        txn_notifier: Arc<dyn TxnNotifier>,
    ) -> Self {
        Self {
            block_preparer,
            executor,
            validators,
            block_executor_onchain_config,
            is_randomness_enabled,
            signer,
            state_sync_notifier,
            payload_manager,
            txn_notifier,
            block_metadata: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn channel() -> (PipelineInputTx, PipelineInputRx) {
        let (rand_tx, rand_rx) = oneshot::channel();
        let (order_vote_tx, order_vote_rx) = oneshot::channel();
        let (order_proof_tx, order_proof_rx) = tokio::sync::broadcast::channel(1);
        let (commit_proof_tx, commit_proof_rx) = tokio::sync::broadcast::channel(1);
        (
            PipelineInputTx {
                rand_tx: Some(rand_tx),
                order_vote_tx: Some(order_vote_tx),
                order_proof_tx,
                commit_proof_tx,
            },
            PipelineInputRx {
                rand_rx,
                order_vote_rx,
                order_proof_rx,
                commit_proof_rx,
            },
        )
    }

    pub fn build_root(
        &self,
        compute_result: StateComputeResult,
        commit_proof: LedgerInfoWithSignatures,
    ) -> PipelineFutures {
        let prepare_fut = spawn_ready_fut(Arc::new(vec![]));
        let execute_fut = spawn_ready_fut(());
        let ledger_update_fut = spawn_ready_fut((compute_result.clone(), None));
        let commit_vote_fut = spawn_ready_fut(CommitVote::new_with_signature(
            self.signer.author(),
            commit_proof.ledger_info().clone(),
            self.signer
                .sign(commit_proof.ledger_info())
                .expect("Signing should succeed"),
        ));
        let pre_commit_fut = spawn_ready_fut(compute_result);
        let commit_ledger_fut = spawn_ready_fut(Some(commit_proof));
        let post_ledger_update_fut = spawn_ready_fut(());
        let post_pre_commit_fut = spawn_ready_fut(());
        let post_commit_fut = spawn_ready_fut(());
        PipelineFutures {
            prepare_fut,
            execute_fut,
            ledger_update_fut,
            post_ledger_update_fut,
            commit_vote_fut,
            pre_commit_fut,
            post_pre_commit_fut,
            commit_ledger_fut,
            post_commit_fut,
        }
    }

    pub fn build(
        &self,
        parent: &PipelineFutures,
        block: Arc<Block>,
        block_store_callback: Box<dyn FnOnce(LedgerInfoWithSignatures) + Send + Sync>,
    ) -> (PipelineFutures, PipelineInputTx, Vec<AbortHandle>) {
        let (tx, rx) = Self::channel();
        let PipelineInputRx {
            rand_rx,
            order_vote_rx,
            order_proof_rx,
            commit_proof_rx,
        } = rx;

        let mut abort_handles = vec![];

        let prepare_fut = spawn_shared_fut(
            Self::prepare(self.block_preparer.clone(), block.clone()),
            &mut abort_handles,
        );
        let execute_fut = spawn_shared_fut(
            Self::execute(
                prepare_fut.clone(),
                parent.execute_fut.clone(),
                rand_rx,
                self.executor.clone(),
                block.clone(),
                self.is_randomness_enabled,
                self.validators.clone(),
                self.block_executor_onchain_config.clone(),
            ),
            &mut abort_handles,
        );
        let ledger_update_fut = spawn_shared_fut(
            Self::ledger_update(
                execute_fut.clone(),
                parent.ledger_update_fut.clone(),
                self.executor.clone(),
                block.clone(),
            ),
            &mut abort_handles,
        );
        let commit_vote_fut = spawn_shared_fut(
            Self::sign_commit_vote(
                ledger_update_fut.clone(),
                order_vote_rx,
                order_proof_rx.resubscribe(),
                commit_proof_rx.resubscribe(),
                self.signer.clone(),
                block.clone(),
            ),
            &mut abort_handles,
        );
        let pre_commit_fut = spawn_shared_fut(
            Self::pre_commit(
                ledger_update_fut.clone(),
                parent.pre_commit_fut.clone(),
                order_proof_rx,
                commit_proof_rx.resubscribe(),
                self.executor.clone(),
                block.clone(),
            ),
            &mut abort_handles,
        );
        let commit_ledger_fut = spawn_shared_fut(
            Self::commit_ledger(
                pre_commit_fut.clone(),
                commit_proof_rx,
                parent.commit_ledger_fut.clone(),
                self.executor.clone(),
                block.clone(),
            ),
            &mut abort_handles,
        );

        let post_ledger_update_fut = spawn_shared_fut(
            Self::post_ledger_update(
                prepare_fut.clone(),
                ledger_update_fut.clone(),
                self.txn_notifier.clone(),
                block.clone(),
            ),
            &mut abort_handles,
        );
        let post_pre_commit_fut = spawn_shared_fut(
            Self::post_pre_commit(
                pre_commit_fut.clone(),
                parent.post_pre_commit_fut.clone(),
                self.state_sync_notifier.clone(),
                self.payload_manager.clone(),
                block.clone(),
            ),
            &mut abort_handles,
        );
        let post_commit_fut = spawn_shared_fut(
            Self::post_commit_ledger(
                pre_commit_fut.clone(),
                commit_ledger_fut.clone(),
                parent.post_commit_fut.clone(),
                block_store_callback,
                block.clone(),
            ),
            &mut abort_handles,
        );
        let all_fut = PipelineFutures {
            prepare_fut,
            execute_fut,
            ledger_update_fut,
            post_ledger_update_fut,
            commit_vote_fut,
            pre_commit_fut,
            post_pre_commit_fut,
            commit_ledger_fut,
            post_commit_fut,
        };
        tokio::spawn(Self::monitor(
            block.epoch(),
            block.round(),
            block.id(),
            all_fut.clone(),
        ));
        (all_fut, tx, abort_handles)
    }

    /// Precondition: Block is inserted into block tree (all ancestors are available)
    /// What it does: Wait for all data becomes available and verify transaction signatures
    async fn prepare(preparer: Arc<BlockPreparer>, block: Arc<Block>) -> TaskResult<PrepareResult> {
        let _tracker = Tracker::new("prepare", &block);
        // the loop can only be abort by the caller
        let input_txns = loop {
            match preparer.prepare_block(&block).await {
                Ok(input_txns) => break input_txns,
                Err(e) => {
                    warn!(
                        "[BlockPreparer] failed to prepare block {}, retrying: {}",
                        block.id(),
                        e
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                },
            }
        };
        let sig_verification_start = Instant::now();
        let sig_verified_txns: Vec<SignatureVerifiedTransaction> = SIG_VERIFY_POOL.install(|| {
            let num_txns = input_txns.len();
            input_txns
                .into_par_iter()
                .with_min_len(optimal_min_len(num_txns, 32))
                .map(|t| Transaction::UserTransaction(t).into())
                .collect::<Vec<_>>()
        });
        counters::PREPARE_BLOCK_SIG_VERIFICATION_TIME
            .observe_duration(sig_verification_start.elapsed());
        Ok(Arc::new(sig_verified_txns))
    }

    /// Precondition: 1. prepare finishes, 2. parent block's phase finishes 3. randomness is available
    /// What it does: Execute all transactions in block executor
    async fn execute(
        prepare_phase: TaskFuture<PrepareResult>,
        parent_block_execute_phase: TaskFuture<ExecuteResult>,
        randomness_rx: oneshot::Receiver<Option<Randomness>>,
        executor: Arc<dyn BlockExecutorTrait>,
        block: Arc<Block>,
        is_randomness_enabled: bool,
        validator: Arc<[AccountAddress]>,
        onchain_execution_config: BlockExecutorConfigFromOnchain,
    ) -> TaskResult<ExecuteResult> {
        parent_block_execute_phase.await?;
        let user_txns = prepare_phase.await?;
        let maybe_rand = randomness_rx
            .await
            .map_err(|_| anyhow!("randomness tx cancelled"))?;

        let _tracker = Tracker::new("execute", &block);
        // let metadata_txn = if is_randomness_enabled {
        //     block.new_metadata_with_randomness(&validator, maybe_rand)
        // } else {
        //     block.new_block_metadata(&validator).into()
        // };
        let metadata_txn : BlockMetadataExt = block.new_block_metadata(&validator).into();
        let txns = [
            vec![SignatureVerifiedTransaction::from(Transaction::from(
                metadata_txn,
            ))],
            block
                .validator_txns()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(Transaction::ValidatorTransaction)
                .map(SignatureVerifiedTransaction::from)
                .collect(),
            user_txns.as_ref().clone(),
        ]
        .concat();
        // tokio::task::spawn_blocking(move || {
        //     executor
        //         .execute_and_state_checkpoint(
        //             (block.id(), txns).into(),
        //             block.parent_id(),
        //             onchain_execution_config,
        //         )
        //         .map_err(anyhow::Error::from)
        // })
        // .await
        // .expect("spawn blocking failed")?;
        let stxns =
            txns.into_iter().map(|txn| {
                match txn.into_inner() {
                    Transaction::UserTransaction(sign_txn) => sign_txn,
                    _ => panic!("Shouldn't be other txn"),
                }
            }).collect::<Vec<_>>();
        let vtxns =
            stxns.iter().map(|txn| Into::<VerifiedTxn>::into(&txn.clone())).collect::<Vec<_>>();
        let real_txns = vtxns
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
        let meta_data = ExternalBlockMeta {
            block_id: BlockId(*block.id()),
            block_number: block.block_number().unwrap_or_else(|| panic!("No block number")),
            usecs: block.timestamp_usecs(),
            epoch: block.epoch(),
            randomness: maybe_rand.map(|r| Random::from_bytes(r.randomness())),
            block_hash: None,
        };
        get_block_buffer_manager()
            .set_ordered_blocks(BlockId::from_bytes(block.parent_id().as_slice()), ExternalBlock { block_meta: meta_data, txns: real_txns })
            .await
            .unwrap_or_else(|e| panic!("Failed to push ordered blocks {}", e));
        Ok(())
    }

    /// Precondition: 1. execute finishes, 2. parent block's phase finishes
    /// What it does: Generate state compute result from the execution, it's split from execution for more parallelism
    /// It carries block timestamp from epoch-ending block to all suffix block
    async fn ledger_update(
        execute_phase: TaskFuture<ExecuteResult>,
        parent_block_ledger_update_phase: TaskFuture<LedgerUpdateResult>,
        executor: Arc<dyn BlockExecutorTrait>,
        block: Arc<Block>,
    ) -> TaskResult<LedgerUpdateResult> {
        let (_, prev_epoch_end_timestamp) = parent_block_ledger_update_phase.await?;
        execute_phase.await?;
        let _tracker = Tracker::new("ledger_update", &block);
        let block_id = block.id();
        let block_number = block.block_number();
        let timestamp = block.timestamp_usecs();
        let hash = get_block_buffer_manager()
            .get_executed_res(BlockId::from_bytes(block_id.as_slice()), block_number.unwrap())
            .await
            .unwrap_or_else(|e| panic!("Failed to get executed result {}", e));
        let hash = hash.execution_output;
        update_counters_for_compute_res(&hash);
        let result = StateComputeResult::new(hash, None, None);
        observe_block(timestamp, BlockStage::EXECUTED);
        // FIXME(gravity_byteyue): fixme
        // let epoch_end_timestamp =
        //     if result.has_reconfiguration() && !result.compute_status_for_input_txns().is_empty() {
        //         Some(timestamp)
        //     } else {
        //         prev_epoch_end_timestamp
        //     };
        let epoch_end_timestamp = None;
        Ok((result, epoch_end_timestamp))
    }

    /// Precondition: ledger update finishes
    /// What it does: For now this is mainly to notify mempool about failed transactions
    /// This is off critical path
    async fn post_ledger_update(
        prepare_fut: TaskFuture<PrepareResult>,
        ledger_update: TaskFuture<LedgerUpdateResult>,
        mempool_notifier: Arc<dyn TxnNotifier>,
        block: Arc<Block>,
    ) -> TaskResult<PostLedgerUpdateResult> {
        let user_txns = prepare_fut.await?;
        let (compute_result, _) = ledger_update.await?;

        let _tracker = Tracker::new("post_ledger_update", &block);
        let txn_status = compute_result.txn_status();
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
                if let Err(e) = mempool_notifier.notify_failed_txn(rejected_txns).await {
                    error!(error = ?e, "Failed to notify mempool of rejected txns");
                }
            }
            None => {}
        }
        // let compute_status = compute_result.compute_status_for_input_txns();
        // // the length of compute_status is user_txns.len() + num_vtxns + 1 due to having blockmetadata
        // if user_txns.len() >= compute_status.len() {
        //     // reconfiguration suffix blocks don't have any transactions
        //     // otherwise, this is an error
        //     if !compute_status.is_empty() {
        //         error!(
        //                 "Expected compute_status length and actual compute_status length mismatch! user_txns len: {}, compute_status len: {}, has_reconfiguration: {}",
        //                 user_txns.len(),
        //                 compute_status.len(),
        //                 compute_result.has_reconfiguration(),
        //             );
        //     }
        // } else {
        //     let user_txn_status = &compute_status[compute_status.len() - user_txns.len()..];
        //     // todo: avoid clone
        //     let txns: Vec<SignedTransaction> = user_txns
        //         .iter()
        //         .flat_map(|txn| txn.get_transaction().map(|t| t.try_as_signed_user_txn()))
        //         .flatten()
        //         .cloned()
        //         .collect();

        //     // notify mempool about failed transaction
        //     if let Err(e) = mempool_notifier
        //         .notify_failed_txn(&txns, user_txn_status)
        //         .await
        //     {
        //         error!(
        //             error = ?e, "Failed to notify mempool of rejected txns",
        //         );
        //     }
        // }
        Ok(())
    }

    /// Precondition: 1. ledger update finishes, 2. order vote or order proof or commit proof is received
    /// What it does: Sign the commit vote with execution result, it needs to update the timestamp for reconfig suffix blocks
    async fn sign_commit_vote(
        ledger_update_phase: TaskFuture<LedgerUpdateResult>,
        order_vote_rx: oneshot::Receiver<()>,
        mut order_proof_rx: tokio::sync::broadcast::Receiver<()>,
        mut commit_proof_rx: tokio::sync::broadcast::Receiver<LedgerInfoWithSignatures>,
        signer: Arc<ValidatorSigner>,
        block: Arc<Block>,
    ) -> TaskResult<CommitVoteResult> {
        let (compute_result, epoch_end_timestamp) = ledger_update_phase.await?;
        // either order_vote_rx or order_proof_rx can trigger the next phase
        select! {
            Ok(_) = order_vote_rx => {
            }
            Ok(_) = order_proof_rx.recv() => {
            }
            Ok(_) = commit_proof_rx.recv() => {
            }
            else => {
                return Err(anyhow!("all receivers dropped"))?;
            }
        }

        let _tracker = Tracker::new("sign_commit_vote", &block);
        let mut block_info = block.gen_block_info(
            compute_result.root_hash(),
            0, // TODO(gravity_byteyue): fix this
            compute_result.epoch_state().clone(),
        );
        if let Some(timestamp) = epoch_end_timestamp {
            info!(
                "[Pipeline] update block timestamp from {} to epoch end timestamp {}",
                block_info.timestamp_usecs(),
                timestamp
            );
            block_info.change_timestamp(timestamp);
        }
        let ledger_info = LedgerInfo::new(block_info, HashValue::zero());
        info!("[Pipeline] Signed ledger info {ledger_info}");
        let signature = signer.sign(&ledger_info).expect("Signing should succeed");
        Ok(CommitVote::new_with_signature(
            signer.author(),
            ledger_info,
            signature,
        ))
    }

    /// Precondition: 1. ledger update finishes, 2. parent block's phase finishes 2. order proof is received
    /// What it does: pre-write result to storage even commit proof is not yet available
    /// For epoch ending block, wait until commit proof is available
    async fn pre_commit(
        ledger_update_phase: TaskFuture<LedgerUpdateResult>,
        // TODO bound parent_commit_ledger too
        parent_block_pre_commit_phase: TaskFuture<PreCommitResult>,
        mut order_proof_rx: tokio::sync::broadcast::Receiver<()>,
        mut commit_proof_rx: tokio::sync::broadcast::Receiver<LedgerInfoWithSignatures>,
        executor: Arc<dyn BlockExecutorTrait>,
        block: Arc<Block>,
    ) -> TaskResult<PreCommitResult> {
        let (compute_result, _) = ledger_update_phase.await?;
        parent_block_pre_commit_phase.await?;

        order_proof_rx
            .recv()
            .await
            .map_err(|_| anyhow!("order proof tx cancelled"))?;

        if compute_result.has_reconfiguration() {
            commit_proof_rx
                .recv()
                .await
                .map_err(|_| anyhow!("commit proof tx cancelled"))?;
        }

        let _tracker = Tracker::new("pre_commit", &block);
        tokio::task::spawn_blocking(move || {
            executor
                .pre_commit_block(block.id())
                .map_err(anyhow::Error::from)
        })
        .await
        .expect("spawn blocking failed")?;
        Ok(compute_result)
    }

    /// Precondition: 1. pre-commit finishes, 2. parent block's phase finishes
    /// What it does: Notify state synchronizer and payload manager about committed transactions
    /// This is off critical path
    async fn post_pre_commit(
        pre_commit: TaskFuture<PreCommitResult>,
        parent_post_pre_commit: TaskFuture<PostCommitResult>,
        state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
        payload_manager: Arc<dyn TPayloadManager>,
        block: Arc<Block>,
    ) -> TaskResult<PostPreCommitResult> {
        let compute_result = pre_commit.await?;
        parent_post_pre_commit.await?;

        let _tracker = Tracker::new("post_pre_commit", &block);
        let payload = block.payload().cloned();
        let timestamp = block.timestamp_usecs();
        let _timer = counters::OP_COUNTERS.timer("pre_commit_notify");

        // let txns = compute_result.transactions_to_commit().to_vec();
        let (txns, _) = payload_manager.get_transactions(&block).await.unwrap_or_default();
        let txns = txns.into_iter().map(|t| { Transaction::UserTransaction(t) }).collect_vec();
        let commit_txns = compute_result.transactions_to_commit(txns);
        // let subscribable_events = compute_result.subscribable_events().to_vec();
        let subscribable_events = vec![];
        if let Err(e) = monitor!(
            "notify_state_sync",
            state_sync_notifier
                .notify_new_commit(commit_txns, subscribable_events, block.block_number().unwrap_or_else(|| panic!("No block number")))
                .await
        ) {
            error!(error = ?e, "Failed to notify state synchronizer");
        }

        let payload_vec = payload.into_iter().collect();
        payload_manager.notify_commit(timestamp, payload_vec);
        Ok(())
    }

    /// Precondition: 1. pre-commit finishes, 2. parent block's phase finishes 3. commit proof is available
    /// What it does: Commit the ledger info to storage, this makes the data visible for clients
    async fn commit_ledger(
        pre_commit_fut: TaskFuture<PreCommitResult>,
        mut commit_proof_rx: tokio::sync::broadcast::Receiver<LedgerInfoWithSignatures>,
        parent_block_commit_phase: TaskFuture<CommitLedgerResult>,
        executor: Arc<dyn BlockExecutorTrait>,
        block: Arc<Block>,
    ) -> TaskResult<CommitLedgerResult> {
        parent_block_commit_phase.await?;
        pre_commit_fut.await?;
        let ledger_info_with_sigs = commit_proof_rx
            .recv()
            .await
            .map_err(|_| anyhow!("commit rx cancelled"))?;

        // it's committed as prefix
        if ledger_info_with_sigs.commit_info().id() != block.id() {
            return Ok(None);
        }

        let _tracker = Tracker::new("commit_ledger", &block);
        let mut block_ids = vec![];
        block_ids.push(block.id());
        let ledger_info_with_sigs_clone = ledger_info_with_sigs.clone();
        tokio::task::spawn_blocking(move || {
            executor
                .commit_ledger(block_ids, ledger_info_with_sigs_clone)
                .map_err(anyhow::Error::from)
        })
        .await
        .expect("spawn blocking failed")?;
        Ok(Some(ledger_info_with_sigs))
    }

    /// Precondition: 1. commit ledger finishes, 2. parent block's phase finishes
    /// What it does: Update counters for the block, and notify block tree about the commit
    async fn post_commit_ledger(
        pre_commit_fut: TaskFuture<PreCommitResult>,
        commit_ledger_fut: TaskFuture<CommitLedgerResult>,
        parent_post_commit: TaskFuture<PostCommitResult>,
        block_store_callback: Box<dyn FnOnce(LedgerInfoWithSignatures) + Send + Sync>,
        block: Arc<Block>,
    ) -> TaskResult<PostCommitResult> {
        parent_post_commit.await?;
        let maybe_ledger_info_with_sigs = commit_ledger_fut.await?;
        let compute_result = pre_commit_fut.await?;

        let _tracker = Tracker::new("post_commit_ledger", &block);
        update_counters_for_block(&block);
        update_counters_for_compute_result(&compute_result);

        if let Some(ledger_info_with_sigs) = maybe_ledger_info_with_sigs {
            block_store_callback(ledger_info_with_sigs);
        }
        Ok(())
    }

    async fn monitor(epoch: u64, round: Round, block_id: HashValue, all_futs: PipelineFutures) {
        let PipelineFutures {
            prepare_fut,
            execute_fut,
            ledger_update_fut,
            post_ledger_update_fut: _,
            commit_vote_fut: _,
            pre_commit_fut,
            post_pre_commit_fut: _,
            commit_ledger_fut,
            post_commit_fut: _,
        } = all_futs;
        wait_and_log_error(prepare_fut, format!("{epoch} {round} {block_id} prepare")).await;
        wait_and_log_error(execute_fut, format!("{epoch} {round} {block_id} execute")).await;
        wait_and_log_error(
            ledger_update_fut,
            format!("{epoch} {round} {block_id} ledger update"),
        )
        .await;
        wait_and_log_error(
            pre_commit_fut,
            format!("{epoch} {round} {block_id} pre commit"),
        )
        .await;
        wait_and_log_error(
            commit_ledger_fut,
            format!("{epoch} {round} {block_id} commit ledger"),
        )
        .await;
    }
}