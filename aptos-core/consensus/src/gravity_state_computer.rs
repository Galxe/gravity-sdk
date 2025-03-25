// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::consensusdb::ConsensusDB;
use crate::counters::{APTOS_COMMIT_BLOCKS, APTOS_EXECUTION_TXNS};
use crate::payload_client::user::quorum_store_client::QuorumStoreClient;
use anyhow::Result;
use api_types::u256_define::BlockId;
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
use block_buffer_manager::get_block_buffer_manager;
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
        if !block_ids.is_empty() {
            let (block_id, block_hash) = (ledger_info_with_sigs.ledger_info().commit_info().id(), ledger_info_with_sigs.ledger_info().block_hash());
            let block_num = ledger_info_with_sigs.ledger_info().block_number();
            self.runtime.block_on(async move {
                get_block_buffer_manager()
                    .push_commit_blocks(block_ids.into_iter()
                    .map(|x| 
                        {
                            let mut v = [0u8; 32];
                            v.copy_from_slice(block_hash.as_ref());
                            if x == block_id {
                                (BlockId::from_bytes(x.as_slice()), Some(v), Some(block_num))
                            } else {
                                (BlockId::from_bytes(x.as_slice()), None, None)
                            }
                        }
                    ).collect())
                    .await
                    .unwrap_or_else(|e| panic!("Failed to push commit blocks {}", e));
            });
        }
        self.inner.db.writer.commit_ledger(0, Some(&ledger_info_with_sigs), None);
        Ok(())
    }

    fn finish(&self) {
        self.inner.finish()
    }
    
    fn pre_commit_block(&self, block_id: HashValue) -> ExecutorResult<()> {
        Ok(())
    }
    fn commit_ledger(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        APTOS_COMMIT_BLOCKS.inc_by(block_ids.len() as u64);
        info!("commit blocks: {:?}", block_ids);
        let (block_id, block_hash) = (ledger_info_with_sigs.ledger_info().commit_info().id(), ledger_info_with_sigs.ledger_info().block_hash());
        let block_num = ledger_info_with_sigs.ledger_info().block_number();
        if !block_ids.is_empty() {
            self.runtime.block_on(async move {
                get_block_buffer_manager().push_commit_blocks(block_ids.into_iter()
                .map(|x| {
                    let mut v = [0u8; 32];
                    v.copy_from_slice(block_hash.as_ref());
                    if x == block_id {
                        (BlockId::from_bytes(x.as_slice()), Some(v), Some(block_num))
                    } else {
                        (BlockId::from_bytes(x.as_slice()), None, None)
                    }
                })
                .collect())
                .await.unwrap()
                ;
            });
        }
        self.inner.db.writer.commit_ledger(0, Some(&ledger_info_with_sigs), None);
        Ok(())
    }
}
