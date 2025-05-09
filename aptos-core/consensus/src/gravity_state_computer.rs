// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::consensusdb::ConsensusDB;
use crate::payload_client::user::quorum_store_client::QuorumStoreClient;
use anyhow::Result;
use gaptos::api_types::u256_define::BlockId;
use aptos_executor::block_executor::BlockExecutor;
use aptos_executor_types::{BlockExecutorTrait, ExecutorResult, StateComputeResult};
use block_buffer_manager::block_buffer_manager::BlockHashRef;
use block_buffer_manager::get_block_buffer_manager;
use coex_bridge::{get_coex_bridge, Func};
use gaptos::aptos_consensus::counters::{APTOS_COMMIT_BLOCKS, APTOS_EXECUTION_TXNS};
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::info;
use gaptos::aptos_types::block_executor::partitioner::ExecutableBlock;
use gaptos::aptos_types::{
    block_executor::config::BlockExecutorConfigFromOnchain, ledger_info::LedgerInfoWithSignatures,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct ConsensusAdapterArgs {
    pub quorum_store_client: Option<Arc<QuorumStoreClient>>,
    pub consensus_db: Option<Arc<ConsensusDB>>,
}

impl ConsensusAdapterArgs {
    pub fn new(consensus_db: Arc<ConsensusDB>) -> Self {
        Self { quorum_store_client: None, consensus_db: Some(consensus_db) }
    }

    pub fn set_quorum_store_client(&mut self, quorum_store_client: Option<Arc<QuorumStoreClient>>) {
        self.quorum_store_client = quorum_store_client;
    }

    pub fn dummy() -> Self {
        Self { quorum_store_client: None, consensus_db: None }
    }
}

pub struct GravityBlockExecutor {
    inner: BlockExecutor,
    runtime: Runtime,
}

impl GravityBlockExecutor {
    pub(crate) fn new(inner: BlockExecutor) -> Self {
        Self { inner, runtime: gaptos::aptos_runtimes::spawn_named_runtime("tmp".into(), None) }
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
            let (block_id, block_hash) = (
                ledger_info_with_sigs.ledger_info().commit_info().id(),
                ledger_info_with_sigs.ledger_info().block_hash(),
            );
            let block_num = ledger_info_with_sigs.ledger_info().block_number();
            assert!(block_ids.last().unwrap().as_slice() == block_id.as_slice());
            let len = block_ids.len();
            self.runtime.block_on(async move {
                get_block_buffer_manager()
                    .set_commit_blocks(
                        block_ids
                            .into_iter()
                            .enumerate()
                            .map(|(i, x)| {
                                let mut v = [0u8; 32];
                                v.copy_from_slice(block_hash.as_ref());
                                if x == block_id {
                                    BlockHashRef {
                                        block_id: BlockId::from_bytes(x.as_slice()),
                                        num: block_num + (i - len + 1) as u64,
                                        hash: Some(v),
                                    }
                                } else {
                                    // TODO: commit use block num, but here use block id, need to fix
                                    BlockHashRef {
                                        block_id: BlockId::from_bytes(x.as_slice()),
                                        num: block_num + (i - len + 1) as u64,
                                        hash: None,
                                    }
                                }
                            })
                            .collect(),
                    )
                    .await
                    .unwrap_or_else(|e| panic!("Failed to push commit blocks {}", e));
            });
        }
        self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
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
        let (block_id, block_hash) = (
            ledger_info_with_sigs.ledger_info().commit_info().id(),
            ledger_info_with_sigs.ledger_info().block_hash(),
        );
        let block_num = ledger_info_with_sigs.ledger_info().block_number();
        let len = block_ids.len();
        if !block_ids.is_empty() {
            self.runtime.block_on(async move {
                get_block_buffer_manager()
                    .set_commit_blocks(
                        block_ids
                            .into_iter()
                            .enumerate()
                            .map(|(i, x)| {
                                let mut v = [0u8; 32];
                                v.copy_from_slice(block_hash.as_ref());
                                if x == block_id {
                                    BlockHashRef {
                                        block_id: BlockId::from_bytes(x.as_slice()),
                                        num: block_num - (len - 1 - i) as u64,
                                        hash: Some(v),
                                    }
                                } else {
                                    BlockHashRef {
                                        block_id: BlockId::from_bytes(x.as_slice()),
                                        num: block_num - (len - 1 - i) as u64,
                                        hash: None,
                                    }
                                }
                            })
                            .collect(),
                    )
                    .await
                    .unwrap();
            });
        }
        self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
        Ok(())
    }
}
