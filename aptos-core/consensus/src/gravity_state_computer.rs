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
use gaptos::aptos_consensus::counters::{APTOS_COMMIT_BLOCKS, APTOS_EXECUTION_TXNS};
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::{info, debug};
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
    consensus_db: Arc<ConsensusDB>,
    runtime: Runtime,
}

impl GravityBlockExecutor {
    pub(crate) fn new(inner: BlockExecutor, consensus_db: Arc<ConsensusDB>) -> Self {
        Self { inner, consensus_db, runtime: gaptos::aptos_runtimes::spawn_named_runtime("tmp".into(), None) }
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
            txn_metrics::TxnLifeTime::get_txn_life_time().record_block_committed(block_id.clone());
            let block_num = ledger_info_with_sigs.ledger_info().block_number();
            assert!(block_ids.last().unwrap().as_slice() == block_id.as_slice());
            let len = block_ids.len();
            let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
            let epoch = ledger_info_with_sigs.ledger_info().epoch();
            self.runtime.block_on(async move {
                let mut persist_notifiers = get_block_buffer_manager()
                    .set_commit_blocks(
                        block_ids
                            .into_iter()
                            .enumerate()
                            .map(|(i, x)| {
                                let mut v = [0u8; 32];
                                v.copy_from_slice(block_hash.as_ref());
                                BlockHashRef {
                                    block_id: BlockId::from_bytes(x.as_slice()),
                                    num: block_num - (len - 1 - i) as u64,
                                    hash: if x == block_id { Some(v) } else { None },
                                    persist_notifier: None,
                                }
                            })
                            .collect(),
                        epoch,
                    )
                    .await
                    .unwrap_or_else(|e| panic!("Failed to push commit blocks {}", e));
                for notifier in persist_notifiers.iter_mut() {
                    let _ = notifier.recv().await;
                }
            });
        }
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
        randomness_data: Vec<(u64, Vec<u8>)>,
    ) -> ExecutorResult<()> {
        APTOS_COMMIT_BLOCKS.inc_by(block_ids.len() as u64);
        info!("commit blocks: {:?}", block_ids);
        let (block_id, block_hash) = (
            ledger_info_with_sigs.ledger_info().commit_info().id(),
            ledger_info_with_sigs.ledger_info().block_hash(),
        );
        let block_num = ledger_info_with_sigs.ledger_info().block_number();
        let len = block_ids.len();
        assert!(!block_ids.is_empty(), "commit_ledger block_ids is empty");
        let epoch = ledger_info_with_sigs.ledger_info().epoch();
        
        // Persist randomness data
        if !randomness_data.is_empty() {
            self.consensus_db.put_randomness(&randomness_data)
                .map_err(|e| anyhow::anyhow!("Failed to persist randomness: {:?}", e))?;
            info!("Persisted randomness data: {:?}", randomness_data);
        }
        
        self.runtime.block_on(async move {
            let mut persist_notifiers = get_block_buffer_manager()
                .set_commit_blocks(
                    block_ids
                        .into_iter()
                        .enumerate()
                        .map(|(i, x)| {
                            let mut v = [0u8; 32];
                            v.copy_from_slice(block_hash.as_ref());
                            BlockHashRef {
                                block_id: BlockId::from_bytes(x.as_slice()),
                                num: block_num - (len - 1 - i) as u64,
                                hash: if x == block_id { Some(v) } else { None },
                                persist_notifier: None,
                            }
                        })
                        .collect(),
                    epoch,
                )
                .await
                .unwrap();
            for notifier in persist_notifiers.iter_mut() {
                let _ = notifier.recv().await;
            }
            let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
        });
        Ok(())
    }
}
