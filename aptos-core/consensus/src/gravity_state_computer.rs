// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::consensusdb::ConsensusDB;
use crate::payload_client::user::quorum_store_client::QuorumStoreClient;
use crate::state_computer::{ExecutionProxy, PipelineExecutionResult, StateComputeResultFut};
use crate::{
    error::StateSyncError,
    payload_manager::TPayloadManager,
    state_replication::{StateComputer, StateComputerCommitCallBackType},
    transaction_deduper::TransactionDeduper,
    transaction_shuffler::TransactionShuffler,
};
use anyhow::Result;
use api_types::account::{ExternalAccountAddress, ExternalChainId};
use api_types::u256_define::TxnHash;
use api_types::{
    u256_define::BlockId, ConsensusApi, ExecutionLayer, ExternalBlock, ExternalBlockMeta,
};
use aptos_consensus_types::{block::Block, pipelined_block::PipelinedBlock};
use aptos_crypto::HashValue;
use aptos_executor::block_executor::BlockExecutor;
use aptos_executor_types::{
    BlockExecutorTrait, ExecutorResult, StateCheckpointOutput, StateComputeResult,
};
use aptos_logger::info;
use aptos_mempool::core_mempool::transaction::VerifiedTxn;
use aptos_types::block_executor::partitioner::ExecutableBlock;
use aptos_types::{
    block_executor::config::BlockExecutorConfigFromOnchain, epoch_state::EpochState,
    ledger_info::LedgerInfoWithSignatures, randomness::Randomness,
};
use coex_bridge::{get_coex_bridge, Func};
use futures_channel::oneshot;
use once_cell::sync::OnceCell;
use std::time::Duration;
use std::{boxed::Box, sync::Arc};

pub struct ConsensusAdapterArgs {
    pub quorum_store_client: Option<Arc<QuorumStoreClient>>,
    pub execution_layer: Option<ExecutionLayer>,
    pub consensus_db: Option<Arc<ConsensusDB>>,
}

impl ConsensusAdapterArgs {
    pub fn new(execution_layer: ExecutionLayer, consensus_db: Arc<ConsensusDB>) -> Self {
        Self {
            quorum_store_client: None,
            execution_layer: Some(execution_layer),
            consensus_db: Some(consensus_db),
        }
    }

    pub fn set_quorum_store_client(&mut self, quorum_store_client: Option<Arc<QuorumStoreClient>>) {
        self.quorum_store_client = quorum_store_client;
    }

    pub fn dummy() -> Self {
        Self { quorum_store_client: None, execution_layer: None, consensus_db: None }
    }
}

/// Basic communication with the Execution module;
/// implements StateComputer traits.
pub struct GravityExecutionProxy {
    pub aptos_state_computer: Arc<ExecutionProxy>,
    consensus_engine: OnceCell<Arc<dyn ConsensusApi>>,
    inner_executor: Arc<GravityBlockExecutor>,
}

impl GravityExecutionProxy {
    pub fn new(
        aptos_state_computer: Arc<ExecutionProxy>,
        inner_executor: Arc<GravityBlockExecutor>,
    ) -> Self {
        Self { aptos_state_computer, consensus_engine: OnceCell::new(), inner_executor }
    }

    pub fn set_consensus_engine(&self, consensus_engine: Arc<dyn ConsensusApi>) {
        match self.consensus_engine.set(consensus_engine) {
            Ok(_) => {
                self.inner_executor.set_consensus_engine(
                    self.consensus_engine.get().expect("consensus engine").clone(),
                );
            }
            Err(_) => {
                panic!("failed to set consensus engine")
            }
        }
    }
}

pub struct GravityBlockExecutor {
    inner: BlockExecutor,
    consensus_engine: OnceCell<Arc<dyn ConsensusApi>>,
}

impl GravityBlockExecutor {
    pub(crate) fn new(inner: BlockExecutor) -> Self {
        Self { inner, consensus_engine: OnceCell::new() }
    }

    pub fn set_consensus_engine(&self, consensus_engine: Arc<dyn ConsensusApi>) {
        match self.consensus_engine.set(consensus_engine) {
            Ok(_) => {}
            Err(_) => {
                panic!("failed to set consensus engine")
            }
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
        assert!(block.block_number().is_some());
        let txns = self.aptos_state_computer.get_block_txns(block).await;
        let meta_data = ExternalBlockMeta {
            block_id: BlockId(*block.id()),
            block_number: block.block_number().unwrap_or_else(|| { panic!("No block number") }),
            usecs: block.timestamp_usecs(),
        };
        let id = HashValue::from(block.id());

        // We would export the empty block detail to the outside GCEI caller
        let vtxns: Vec<VerifiedTxn> =
            txns.iter().map(|txn| Into::<VerifiedTxn>::into(&txn.clone())).collect();
        let real_txns = vtxns
            .into_iter()
            .map(|txn| {
                api_types::VerifiedTxn::new(
                    txn.bytes().to_vec(),
                    ExternalAccountAddress::new(txn.sender().into_bytes()),
                    txn.sequence_number(),
                    ExternalChainId::new(txn.chain_id().id()),
                    TxnHash::from_bytes(&txn.get_hash().to_vec()),
                )
            })
            .collect();
        let external_block = ExternalBlock { block_meta: meta_data.clone(), txns: real_txns };
        self.consensus_engine
            .get()
            .expect("ConsensusEngine")
            .send_ordered_block(*parent_block_id, external_block)
            .await;

        let engine = Some(self.consensus_engine.clone());
        Box::pin(async move {
            let result = StateComputeResult::with_root_hash(HashValue::new(
                engine
                    .expect("No consensus api")
                    .get()
                    .expect("consensus engine")
                    .recv_executed_block_hash(meta_data)
                    .await
                    .bytes(),
            ));
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
        let call = get_coex_bridge().take_func("test_info");
        match call {
            Some(Func::TestInfo(call)) => {
                info!("call test_info function");
                call.call("commit in aptos".to_string()).unwrap();
            }
            _ => {
                info!("no test_info function");
            }
        }
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

impl BlockExecutorTrait for GravityBlockExecutor {
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
        info!("commit blocks: {:?}", block_ids);
        if !block_ids.is_empty() {
            let (send, receiver) = oneshot::channel::<HashValue>();
            // todo(gravity_byteyue): don't spawn runtime each time
            let runtime = aptos_runtimes::spawn_named_runtime("tmp".into(), None);
            let _ = runtime.block_on(async move {
                for block_id in block_ids {
                    self.consensus_engine
                        .get()
                        .expect("consensus engine")
                        .commit_block_hash(*block_id)
                        .await;
                }
            });
            // if let Err(e) = r {
            //     return Err(e);
            // }
            // let last_block = block_ids.last();
            // let max_committed_block_id;
            // match last_block {
            //     Some(id) => {
            //         max_committed_block_id = id;
            //     }
            //     None => {
            //         return Err(ExecutorError::InternalError { error: format!("empty blocks") })
            //     }
            // }
            // let r = runtime.block_on(async move { receiver.await }).map_err(|e| {
            //     aptos_executor_types::ExecutorError::InternalError {
            //         error: "receive commit successful id failed".parse().unwrap(),
            //     }
            // });
            // match r {
            //     Ok(persistent_id) => {
            //         if persistent_id != *max_committed_block_id {
            //             panic!("Persisten id not match");
            //         }
            //     }
            //     Err(e) => return Err(e),
            // }
        }
        // TODO(gravity_lightman): handle the following logic
        // self.inner
        //     .db
        //     .writer
        //     .commit_ledger(0, Some(&ledger_info_with_sigs), None);
        Ok(())
    }

    fn finish(&self) {
        self.inner.finish()
    }
}
