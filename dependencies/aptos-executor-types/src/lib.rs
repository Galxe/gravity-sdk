// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use api_types::compute_res::{ComputeRes, TxnStatus};
use aptos_crypto::hash::{HashValue, ACCUMULATOR_PLACEHOLDER_HASH};
use aptos_types::{block_executor::config::BlockExecutorConfigFromOnchain, transaction::Transaction};
use aptos_types::block_executor::partitioner::ExecutableBlock;
use aptos_types::epoch_state::EpochState;
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use aptos_types::transaction::block_epilogue::BlockEndInfo;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, sync::Arc};
use thiserror::Error;

#[derive(Debug, Deserialize, Error, PartialEq, Eq, Serialize)]
/// Different reasons for proposal rejection
pub enum ExecutorError {
    #[error("Cannot find speculation result for block id {0}")]
    BlockNotFound(HashValue),

    #[error("Cannot get data for batch id {0}")]
    DataNotFound(HashValue),

    #[error(
        "Bad num_txns_to_commit. first version {}, num to commit: {}, target version: {}",
        first_version,
        to_commit,
        target_version
    )]
    BadNumTxnsToCommit { first_version: Version, to_commit: usize, target_version: Version },

    #[error("Internal error: {:?}", error)]
    InternalError { error: String },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Received Empty Blocks")]
    EmptyBlocks,

    #[error("Could Not Get Data")]
    CouldNotGetData,
}

pub type Version = u64;

impl ExecutorError {
    pub fn internal_err<E: Display>(e: E) -> Self {
        Self::InternalError { error: format!("{}", e) }
    }
}

pub type ExecutorResult<T> = Result<T, ExecutorError>;

#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")] // cannot use tag = "type" as nested enums cannot work, and bcs doesn't support it
pub enum BlockGasLimitType {
    NoLimit,
    Limit(u64),
}

pub trait BlockExecutorTrait: Send + Sync {
    /// Get the latest committed block id
    fn committed_block_id(&self) -> HashValue;

    /// Reset the internal state including cache with newly fetched latest committed block from storage.
    fn reset(&self) -> anyhow::Result<()>;

    /// Executes a block and returns the state checkpoint output.
    fn execute_and_state_checkpoint(
        &self,
        block: ExecutableBlock,
        parent_block_id: HashValue,
        onchain_config: BlockExecutorConfigFromOnchain,
    ) -> ExecutorResult<()>;

    fn ledger_update(
        &self,
        block_id: HashValue,
        parent_block_id: HashValue,
    ) -> ExecutorResult<StateComputeResult>;

    #[cfg(any(test, feature = "fuzzing"))]
    fn commit_blocks(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        for block_id in &block_ids {
            self.pre_commit_block(block_id.clone())?;
        }
        self.commit_ledger(block_ids, ledger_info_with_sigs)
    }

    fn pre_commit_block(&self, block_id: HashValue) -> ExecutorResult<()>;

    fn commit_ledger(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()>;

    /// Finishes the block executor by releasing memory held by inner data structures(SMT).
    fn finish(&self);
}

#[derive(Debug, Default, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct StateComputeResult {
    execution_output: ComputeRes,
    epoch_state: Option<EpochState>,
    block_end_info: Option<BlockEndInfo>,
}

impl StateComputeResult {
    pub fn new(
        execution_output: ComputeRes,
        epoch_state: Option<EpochState>,
        block_end_info: Option<BlockEndInfo>,
    ) -> Self {
        Self { execution_output, epoch_state, block_end_info }
    }

    pub fn version(&self) -> Version {
        // TODO(gravity_byteyue): this is a placeholder, we should return the real version
        Version::from(0u8)
    }

    pub fn new_dummy() -> Self {
        StateComputeResult::with_root_hash(*ACCUMULATOR_PLACEHOLDER_HASH)
    }

    /// generate a new dummy state compute result with a given root hash.
    /// this function is used in RandomComputeResultStateComputer to assert that the compute
    /// function is really called.
    pub fn with_root_hash(root_hash: HashValue) -> Self {
        Self { execution_output: ComputeRes {
            data: root_hash.to_vec().try_into().unwrap(),
            txn_num: 0,
            txn_status: Arc::new(None),
        }, epoch_state: None, block_end_info: None }
    }

    pub fn root_hash(&self) -> HashValue {
        HashValue::new(self.execution_output.data)
    }

    pub fn epoch_state(&self) -> &Option<EpochState> {
        &self.epoch_state
    }

    pub fn has_reconfiguration(&self) -> bool {
        self.epoch_state.is_some()
    }

    pub fn txn_status(&self) -> Arc<Option<Vec<TxnStatus>>> {
        self.execution_output.txn_status.clone()
    }

    /// This function only returns the user transactions for the mempool to do gc
    pub fn transactions_to_commit(&self, input_txns: Vec<Transaction>) -> Vec<Transaction> {
        let txn_status = self.execution_output.txn_status.clone();
        let status_len = match txn_status.as_ref() {
            Some(status) => status.len(),
            None => return input_txns,
        };
        // TODO(gravity_byteyue): how to unify recover and execution here
        // assert_eq!(status_len, input_txns.len());
        let status = txn_status.as_ref().as_ref().unwrap();
        // for the corresponding status, if it is discarded, then remove the txn from the input_txns
        input_txns.into_iter().zip(status.iter()).filter(|(_, status)| !status.is_discarded).map(|(txn, _)| txn).collect()
    }
}

impl From<anyhow::Error> for ExecutorError {
    fn from(error: anyhow::Error) -> Self {
        Self::InternalError { error: format!("{}", error) }
    }
}

pub mod state_checkpoint_output {
    use std::marker::PhantomData;

    #[derive(Default)]
    pub struct StateCheckpointOutput {}

    pub struct BlockExecutorInner<V> {
        phantom: PhantomData<V>,
    }
}

#[cfg(test)]
mod tests {}
