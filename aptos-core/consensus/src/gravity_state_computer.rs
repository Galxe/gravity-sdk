// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::state_computer::{ExecutionProxy, PipelineExecutionResult, StateComputeResultFut};
use crate::{
    error::StateSyncError,
    payload_manager::TPayloadManager,
    state_replication::{StateComputer, StateComputerCommitCallBackType},
    transaction_deduper::TransactionDeduper,
    transaction_shuffler::TransactionShuffler,
};
use anyhow::Result;
use aptos_consensus_types::{block::Block, pipelined_block::PipelinedBlock};
use aptos_crypto::HashValue;
use aptos_executor::block_executor::BlockExecutor;
use aptos_executor_types::{
    BlockExecutorTrait, ExecutorError, ExecutorResult, StateCheckpointOutput, StateComputeResult,
};
use aptos_mempool::MempoolClientRequest;
use aptos_types::block_executor::partitioner::ExecutableBlock;
use aptos_types::transaction::SignedTransaction;
use aptos_types::{
    block_executor::config::BlockExecutorConfigFromOnchain, epoch_state::EpochState,
    ledger_info::LedgerInfoWithSignatures, randomness::Randomness,
};
use futures::{FutureExt, SinkExt, StreamExt};
use futures_channel::mpsc::UnboundedReceiver;
use futures_channel::{mpsc, oneshot};
use std::time::Duration;
use std::{boxed::Box, sync::Arc};

pub struct ConsensusAdapterArgs {
    pub mempool_sender: mpsc::Sender<MempoolClientRequest>,
    pub pipeline_block_sender: mpsc::UnboundedSender<(
        HashValue,
        HashValue,
        Vec<SignedTransaction>,
        oneshot::Sender<HashValue>,
    )>,
    pub pipeline_block_receiver: Option<
        mpsc::UnboundedReceiver<(
            HashValue,
            HashValue,
            Vec<SignedTransaction>,
            oneshot::Sender<HashValue>,
        )>,
    >,
    pub committed_block_ids_sender: mpsc::UnboundedSender<(Vec<[u8; 32]>, oneshot::Sender<HashValue>)>,
    pub committed_block_ids_receiver:
        Option<mpsc::UnboundedReceiver<(Vec<[u8; 32]>, oneshot::Sender<HashValue>)>>,
}

impl ConsensusAdapterArgs {
    pub fn pipeline_block_receiver(
        &mut self,
    ) -> Option<
        UnboundedReceiver<(
            HashValue,
            HashValue,
            Vec<SignedTransaction>,
            oneshot::Sender<HashValue>,
        )>,
    > {
        self.pipeline_block_receiver.take()
    }
    pub fn new(mempool_sender: mpsc::Sender<MempoolClientRequest>) -> Self {
        let (pipeline_block_sender, pipeline_block_receiver) = mpsc::unbounded();
        let (committed_block_ids_sender, committed_block_ids_receiver) = mpsc::unbounded();
        Self {
            mempool_sender,
            pipeline_block_sender,
            pipeline_block_receiver: Some(pipeline_block_receiver),
            committed_block_ids_sender,
            committed_block_ids_receiver: Some(committed_block_ids_receiver),
        }
    }

    pub fn dummy() -> Self {
        let (mempool_sender, _mempool_receiver) = mpsc::channel(1);
        let (pipeline_block_sender, pipeline_block_receiver) = mpsc::unbounded();
        let (committed_block_ids_sender, committed_block_ids_receiver) = mpsc::unbounded();
        Self {
            mempool_sender,
            pipeline_block_sender,
            pipeline_block_receiver: Some(pipeline_block_receiver),
            committed_block_ids_sender,
            committed_block_ids_receiver: Some(committed_block_ids_receiver),
        }
    }
}

/// Basic communication with the Execution module;
/// implements StateComputer traits.
pub struct GravityExecutionProxy {
    pub aptos_state_computer: Arc<ExecutionProxy>,
    pipeline_block_sender: mpsc::UnboundedSender<(
        HashValue,
        HashValue,
        Vec<SignedTransaction>,
        oneshot::Sender<HashValue>,
    )>,
}

impl GravityExecutionProxy {
    pub fn new(aptos_state_computer: Arc<ExecutionProxy>, args: &ConsensusAdapterArgs) -> Self {
        Self {
            aptos_state_computer,
            pipeline_block_sender: args.pipeline_block_sender.clone(),
        }
    }
}

pub struct GravityBlockExecutor<V> {
    inner: BlockExecutor<V>,
    committed_blocks_sender: mpsc::UnboundedSender<(Vec<[u8; 32]>, oneshot::Sender<HashValue>)>,
}

impl<V> GravityBlockExecutor<V> {
    pub(crate) fn new(
        inner: BlockExecutor<V>,
        committed_blocks_sender: mpsc::UnboundedSender<(Vec<[u8; 32]>, oneshot::Sender<HashValue>)>,
    ) -> Self {
        Self {
            inner,
            committed_blocks_sender,
        }
    }
}

#[async_trait::async_trait]
impl StateComputer for GravityExecutionProxy {
    async fn schedule_compute(
        &self,
        // The block to be executed.
        block: &Block,
        // The parent block id.
        parent_block_id: HashValue,
        randomness: Option<Randomness>,
    ) -> StateComputeResultFut {
        let txns = self.aptos_state_computer.get_block_txns(block).await;
        let empty_block = txns.is_empty();

        let block_id = block.id();
        let (block_result_sender, block_result_receiver) = oneshot::channel();
        // We would export the empty block detail to the outside GCEI caller
        if empty_block {
            let compute_res_bytes = [0u8; 32];
            block_result_sender
                .send(HashValue::new(compute_res_bytes))
                .expect("send failed");
        } else {
            self.pipeline_block_sender
                .clone()
                .send((parent_block_id, block.id(), txns.clone(), block_result_sender))
                .await
                .expect("what happened");
        }
        Box::pin(async move {
            let res = block_result_receiver.await;
            println!("block id {:?} received result", block_id);
            let result = StateComputeResult::new_dummy_with_root_hash(res.unwrap());
            Ok(PipelineExecutionResult::new(txns, result, Duration::ZERO))
        })
    }

    /// Send a successful commit. A future is fulfilled when the state is finalized.
    async fn commit(
        &self,
        blocks: &[Arc<PipelinedBlock>],
        finality_proof: LedgerInfoWithSignatures,
        callback: StateComputerCommitCallBackType,
    ) -> ExecutorResult<()> {
        self.aptos_state_computer
            .commit(blocks, finality_proof, callback)
            .await
    }

    /// Synchronize to a commit that not present locally.
    async fn sync_to(&self, target: LedgerInfoWithSignatures) -> Result<(), StateSyncError> {
        self.aptos_state_computer.sync_to(target).await
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
        self.aptos_state_computer.new_epoch(
            epoch_state,
            payload_manager,
            transaction_shuffler,
            block_executor_onchain_config,
            transaction_deduper,
            randomness_enabled,
        )
    }

    // Clears the epoch-specific state. Only a sync_to call is expected before calling new_epoch
    // on the next epoch.
    fn end_epoch(&self) {
        self.aptos_state_computer.end_epoch()
    }
}

impl<V: Send + Sync> BlockExecutorTrait for GravityBlockExecutor<V> {
    fn committed_block_id(&self) -> HashValue {
        self.inner.committed_block_id()
    }

    fn reset(&self) -> Result<()> {
        self.inner.reset()
    }

    fn execute_and_state_checkpoint(
        &self,
        block: ExecutableBlock,
        parent_block_id: HashValue,
        onchain_config: BlockExecutorConfigFromOnchain,
    ) -> ExecutorResult<StateCheckpointOutput> {
        self.inner
            .execute_and_state_checkpoint(block, parent_block_id, onchain_config)
    }

    fn ledger_update(
        &self,
        block_id: HashValue,
        parent_block_id: HashValue,
        state_checkpoint_output: StateCheckpointOutput,
    ) -> ExecutorResult<StateComputeResult> {
        self.inner
            .ledger_update(block_id, parent_block_id, state_checkpoint_output)
    }

    fn commit_blocks(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        if !block_ids.is_empty() {
            let (send, receiver) = oneshot::channel::<HashValue>();
            let r = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on({
                    let encode_ids = block_ids
                        .iter()
                        .map(|id| {
                            let mut bytes = [0; 32];
                            bytes.copy_from_slice(id.as_slice());
                            bytes
                        })
                        .collect();

                    self.committed_blocks_sender
                        .clone()
                        .send((encode_ids, send))
                })
                .map_err(|e| aptos_executor_types::ExecutorError::InternalError {
                    error: "send commit ids failed".parse().unwrap(),
                });
            if let Err(e) = r {
                return Err(e);
            }
            let max_committed_block_id = block_ids.last().unwrap();
            let r = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move { receiver.await })
                .map_err(|e| aptos_executor_types::ExecutorError::InternalError {
                    error: "receive commit successful id failed".parse().unwrap(),
                });
            if let Err(e) = r {
                return Err(e);
            }
            let persistent_id = r.unwrap();
            if persistent_id != *max_committed_block_id {
                panic!("Persisten id not match");
            }
        }
        self.inner
            .db
            .writer
            .commit_ledger(0, Some(&ledger_info_with_sigs), None);
        Ok(())
    }

    fn finish(&self) {
        self.inner.finish()
    }
}
