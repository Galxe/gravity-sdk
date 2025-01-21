// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::consensusdb::ConsensusDB;
use crate::payload_client::user::quorum_store_client::QuorumStoreClient;
use anyhow::Result;
use api_types::ExecutionLayer;
use aptos_crypto::HashValue;
use aptos_executor::block_executor::BlockExecutor;
use aptos_executor_types::{
    BlockExecutorTrait, ExecutorResult, StateComputeResult,
};
use aptos_logger::info;
use aptos_types::block_executor::partitioner::ExecutableBlock;
use aptos_types::{
    block_executor::config::BlockExecutorConfigFromOnchain,
    ledger_info::LedgerInfoWithSignatures,
};
use coex_bridge::{get_coex_bridge, Func};
use std::sync::Arc;
use tokio::runtime::Runtime;

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

pub struct GravityBlockExecutor {
    inner: BlockExecutor,
    runtime: Runtime,
}

impl GravityBlockExecutor {
    pub(crate) fn new(inner: BlockExecutor) -> Self {
        Self { inner, runtime: aptos_runtimes::spawn_named_runtime("tmp".into(), None) }
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
    ) -> ExecutorResult<()> {
        self.inner.execute_and_state_checkpoint(block, parent_block_id, onchain_config)
    }

    fn ledger_update(
        &self,
        block_id: HashValue,
        parent_block_id: HashValue,
    ) -> ExecutorResult<StateComputeResult> {
        self.inner.ledger_update(block_id, parent_block_id)
    }

    fn commit_blocks(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        info!("commit blocks: {:?}", block_ids);
        if !block_ids.is_empty() {
            self.runtime.block_on(async move {
                let call = get_coex_bridge().borrow_func("commit_block_hash");
                match call {
                    Some(Func::CommittedBlockHash(call)) => {
                        info!("call commit_block_hash function");
                        for block_id in block_ids {
                            call.call(*block_id).await.unwrap();
                        }
                    }
                    _ => {
                        info!("no commit_block_hash function");
                    }
                }
            });
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
        self.inner.db.writer.commit_ledger(0, Some(&ledger_info_with_sigs), None);
        Ok(())
    }

    fn finish(&self) {
        self.inner.finish()
    }
    
    fn pre_commit_block(
        &self,
        block_id: HashValue,
    ) -> ExecutorResult<()> {
        todo!()
    }
    
    fn commit_ledger(&self, ledger_info_with_sigs: LedgerInfoWithSignatures) -> ExecutorResult<()> {
        todo!()
    }
}
