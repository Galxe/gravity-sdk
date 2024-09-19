// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{error::StateSyncError, payload_manager::TPayloadManager, state_replication::{StateComputer, StateComputerCommitCallBackType}, transaction_deduper::TransactionDeduper, transaction_shuffler::TransactionShuffler};
use anyhow::{Result};
use aptos_consensus_types::{block::Block, pipelined_block::PipelinedBlock};
use aptos_crypto::HashValue;
use aptos_executor_types::{BlockExecutorTrait, ExecutorResult, StateCheckpointOutput, StateComputeResult};
use aptos_types::{
    block_executor::config::BlockExecutorConfigFromOnchain,
    epoch_state::EpochState,
    ledger_info::LedgerInfoWithSignatures,
    randomness::Randomness,
};
use futures::{SinkExt, StreamExt};
use std::{boxed::Box, sync::Arc};
use std::time::Duration;
use futures_channel::{mpsc, oneshot};
use futures_channel::mpsc::UnboundedReceiver;
use futures_channel::oneshot::Sender;
use aptos_executor::block_executor::BlockExecutor;
use aptos_mempool::MempoolClientRequest;
use aptos_types::block_executor::partitioner::ExecutableBlock;
use crate::state_computer::{ExecutionProxy, PipelineExecutionResult, StateComputeResultFut};

pub struct ConsensusAdapterArgs {
    pub mempool_sender: mpsc::Sender<MempoolClientRequest>,
    pub pipeline_block_sender:
        mpsc::UnboundedSender<(HashValue, Block, oneshot::Sender<HashValue>)>,
    pub pipeline_block_receiver:
        Option<mpsc::UnboundedReceiver<(HashValue, Block, oneshot::Sender<HashValue>)>>,
    pub committed_blocks_sender: mpsc::Sender<Vec<HashValue>>,
    pub committed_blocks_receiver: mpsc::Receiver<Vec<HashValue>>,
}

impl ConsensusAdapterArgs {
    pub fn pipeline_block_receiver(&mut self) -> Option<UnboundedReceiver<(HashValue, Block, Sender<HashValue>)>> {
        self.pipeline_block_receiver.take()
    }
    pub fn new(mempool_sender: mpsc::Sender<MempoolClientRequest>) -> Self {
        let (pipeline_block_sender, pipeline_block_receiver) = mpsc::unbounded();
        let (committed_blocks_sender, committed_blocks_receiver) = mpsc::channel(1);
        Self {
            mempool_sender,
            pipeline_block_sender,
            pipeline_block_receiver: Some(pipeline_block_receiver),
            committed_blocks_sender,
            committed_blocks_receiver,
        }
    }
}

/// Basic communication with the Execution module;
/// implements StateComputer traits.
pub struct GravityExecutionProxy {
    pub aptos_state_computer: Arc<ExecutionProxy>,
    pipeline_block_sender:
        mpsc::UnboundedSender<(HashValue, Block, oneshot::Sender<HashValue>)>,
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
    committed_blocks_sender: mpsc::Sender<Vec<HashValue>>,
}

impl<V> GravityBlockExecutor<V> {
    pub(crate) fn new(inner: BlockExecutor<V>, committed_blocks_sender: mpsc::Sender<Vec<HashValue>>) -> Self {
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
        let (block_result_sender, block_result_receiver) = oneshot::channel();
        self.pipeline_block_sender.clone().send((parent_block_id, block.clone(), block_result_sender)).await.expect("what the fuck");

        Box::pin(async move {
            let res = block_result_receiver.await;
            let result = StateComputeResult::new_dummy_with_root_hash(res.unwrap());
            Ok(PipelineExecutionResult::new(vec![], result, Duration::ZERO))
        })
    }

    /// Send a successful commit. A future is fulfilled when the state is finalized.
    async fn commit(
        &self,
        blocks: &[Arc<PipelinedBlock>],
        finality_proof: LedgerInfoWithSignatures,
        callback: StateComputerCommitCallBackType,
    ) -> ExecutorResult<()> {
        self.aptos_state_computer.commit(blocks, finality_proof, callback).await
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
        self.aptos_state_computer.new_epoch(epoch_state, payload_manager, transaction_shuffler, block_executor_onchain_config, transaction_deduper, randomness_enabled)
    }

    // Clears the epoch-specific state. Only a sync_to call is expected before calling new_epoch
    // on the next epoch.
    fn end_epoch(&self) {
        self.aptos_state_computer.end_epoch()
    }
}

impl<V: Send + Sync> BlockExecutorTrait for GravityBlockExecutor<V>
{
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
        self.inner.execute_and_state_checkpoint(block, parent_block_id, onchain_config)
    }

    fn ledger_update(
        &self,
        block_id: HashValue,
        parent_block_id: HashValue,
        state_checkpoint_output: StateCheckpointOutput,
    ) -> ExecutorResult<StateComputeResult> {
        self.inner.ledger_update(block_id, parent_block_id, state_checkpoint_output)
    }

    fn commit_blocks(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        // let (sender, receiver) = mpsc::channel(1);
        tokio::runtime::Runtime::new().unwrap().block_on(
            self.committed_blocks_sender.clone().send(block_ids)
        ).map_err(|e| aptos_executor_types::ExecutorError::InternalError { error: "ken➗".parse().unwrap() })
    }

    fn finish(&self) {
        self.inner.finish()
    }
}