// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block::Block,
    common::{Payload, Round},
    order_vote_proposal::OrderVoteProposal,
    pipeline::commit_vote::CommitVote,
    pipeline_execution_result::PipelineExecutionResult,
    quorum_cert::QuorumCert,
    vote_proposal::VoteProposal,
};
use anyhow::Error;
use gaptos::aptos_crypto::hash::{HashValue, ACCUMULATOR_PLACEHOLDER_HASH};
use aptos_executor_types::{ExecutorResult, StateComputeResult};
use gaptos::aptos_infallible::Mutex;
use gaptos::aptos_logger::{error, warn};
use gaptos::aptos_types::{
    block_info::BlockInfo,
    contract_event::ContractEvent,
    ledger_info::LedgerInfoWithSignatures,
    randomness::Randomness,
    transaction::{
        signature_verified_transaction::SignatureVerifiedTransaction, SignedTransaction,
    },
    validator_txn::ValidatorTransaction,
};
use derivative::Derivative;
use futures::future::{BoxFuture, Shared};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::oneshot,
    task::{AbortHandle, JoinError},
};

#[derive(Clone, Debug)]
pub enum TaskError {
    JoinError(Arc<JoinError>),
    InternalError(Arc<Error>),
    PropagatedError(Box<TaskError>),
}

impl Display for TaskError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::JoinError(e) => write!(f, "JoinError: {}", e),
            TaskError::InternalError(e) => write!(f, "InternalError: {}", e),
            TaskError::PropagatedError(e) => write!(f, "PropagatedError: {}", e),
        }
    }
}

impl From<Error> for TaskError {
    fn from(value: Error) -> Self {
        Self::InternalError(Arc::new(value))
    }
}
pub type TaskResult<T> = Result<T, TaskError>;
pub type TaskFuture<T> = Shared<BoxFuture<'static, TaskResult<T>>>;

pub type PrepareResult = Arc<Vec<SignatureVerifiedTransaction>>;
pub type ExecuteResult = ();
pub type LedgerUpdateResult = (StateComputeResult, Option<u64>);
pub type PostLedgerUpdateResult = ();
pub type CommitVoteResult = CommitVote;
pub type PreCommitResult = StateComputeResult;
pub type PostPreCommitResult = ();
pub type CommitLedgerResult = Option<LedgerInfoWithSignatures>;
pub type PostCommitResult = ();

#[derive(Clone)]
pub struct PipelineFutures {
    pub prepare_fut: TaskFuture<PrepareResult>,
    pub execute_fut: TaskFuture<ExecuteResult>,
    pub ledger_update_fut: TaskFuture<LedgerUpdateResult>,
    pub post_ledger_update_fut: TaskFuture<PostLedgerUpdateResult>,
    pub commit_vote_fut: TaskFuture<CommitVoteResult>,
    pub pre_commit_fut: TaskFuture<PreCommitResult>,
    pub post_pre_commit_fut: TaskFuture<PostPreCommitResult>,
    pub commit_ledger_fut: TaskFuture<CommitLedgerResult>,
    pub post_commit_fut: TaskFuture<PostCommitResult>,
}

pub struct PipelineInputTx {
    pub rand_tx: Option<oneshot::Sender<Option<Randomness>>>,
    pub order_vote_tx: Option<oneshot::Sender<()>>,
    pub order_proof_tx: tokio::sync::broadcast::Sender<()>,
    pub commit_proof_tx: tokio::sync::broadcast::Sender<LedgerInfoWithSignatures>,
}

pub struct PipelineInputRx {
    pub rand_rx: oneshot::Receiver<Option<Randomness>>,
    pub order_vote_rx: oneshot::Receiver<()>,
    pub order_proof_rx: tokio::sync::broadcast::Receiver<()>,
    pub commit_proof_rx: tokio::sync::broadcast::Receiver<LedgerInfoWithSignatures>,
}

/// A representation of a block that has been added to the execution pipeline. It might either be in ordered
/// or in executed state. In the ordered state, the block is waiting to be executed. In the executed state,
/// the block has been executed and the output is available.
#[derive(Derivative, Clone)]
#[derivative(Eq, PartialEq)]
pub struct PipelinedBlock {
    /// Block data that cannot be regenerated.
    block: Block,
    /// Input transactions in the order of execution
    input_transactions: Vec<SignedTransaction>,
    /// The state_compute_result is calculated for all the pending blocks prior to insertion to
    /// the tree. The execution results are not persisted: they're recalculated again for the
    /// pending blocks upon restart.
    state_compute_result: StateComputeResult,
    randomness: OnceCell<Randomness>,
    pipeline_insertion_time: OnceCell<Instant>,
    execution_summary: Arc<OnceCell<ExecutionSummary>>,
    #[derivative(PartialEq = "ignore")]
    pre_commit_fut: Arc<Mutex<Option<BoxFuture<'static, ExecutorResult<()>>>>>,
    // pipeline related fields
    #[derivative(PartialEq = "ignore")]
    pipeline_futures: Arc<Mutex<Option<PipelineFutures>>>,
    #[derivative(PartialEq = "ignore")]
    pipeline_tx: Option<Arc<Mutex<PipelineInputTx>>>,
    #[derivative(PartialEq = "ignore")]
    pipeline_abort_handle: Option<Vec<AbortHandle>>,
}

impl Serialize for PipelinedBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename = "PipelineBlock")]
        struct SerializedBlock<'a> {
            block: &'a Block,
            input_transactions: &'a Vec<SignedTransaction>,
            state_compute_result: &'a StateComputeResult,
            randomness: Option<&'a Randomness>,
        }

        let serialized = SerializedBlock {
            block: &self.block,
            input_transactions: &self.input_transactions,
            state_compute_result: &self.state_compute_result,
            randomness: self.randomness.get(),
        };
        serialized.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PipelinedBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "PipelineBlock")]
        struct SerializedBlock {
            block: Block,
            input_transactions: Vec<SignedTransaction>,
            state_compute_result: StateComputeResult,
            randomness: Option<Randomness>,
        }

        let SerializedBlock {
            block,
            input_transactions,
            state_compute_result,
            randomness,
        } = SerializedBlock::deserialize(deserializer)?;

        let block = PipelinedBlock::new(block, input_transactions, StateComputeResult::new_dummy());
        if let Some(r) = randomness {
            block.set_randomness(r);
        }
        Ok(block)
    }
}

impl PipelinedBlock {
    pub fn set_compute_result(&mut self, compute_result: StateComputeResult) {
        self.state_compute_result = compute_result;
    }

    pub fn set_execution_result(
        mut self,
        pipeline_execution_result: PipelineExecutionResult,
    ) -> Self {
        let PipelineExecutionResult {
            input_txns,
            result,
            execution_time,
            pre_commit_fut,
        } = pipeline_execution_result;

        self.state_compute_result = result;
        self.input_transactions = input_txns;
        self.pre_commit_fut = Arc::new(Mutex::new(Some(pre_commit_fut)));

        let mut to_commit = 0;
        let to_retry = 0;

        to_commit += self.block.payload().map_or(0, |payload| {
            payload.len() as u64
        });

        let execution_summary = ExecutionSummary {
            payload_len: self
                .block
                .payload()
                .map_or(0, |payload| payload.len_for_execution()),
            to_commit,
            to_retry,
            execution_time,
            root_hash: self.state_compute_result.root_hash(),
        };

        // We might be retrying execution, so it might have already been set.
        // Because we use this for statistics, it's ok that we drop the newer value.
        let previous = self.execution_summary.get_or_init(|| execution_summary.clone());

        if previous.root_hash == execution_summary.root_hash
            || previous.root_hash == *ACCUMULATOR_PLACEHOLDER_HASH
        {
            warn!(
                "Skipping re-inserting execution result, from {:?} to {:?}",
                previous, execution_summary
            );
        } else {
            error!(
                "Re-inserting execution result with different root hash: from {:?} to {:?}",
                previous, execution_summary
            );
        }


        self
    }

    #[cfg(any(test, feature = "fuzzing"))]
    pub fn mark_successful_pre_commit_for_test(&self) {
        *self.pre_commit_fut.lock() = Some(Box::pin(async { Ok(()) }));
    }

    pub fn set_randomness(&self, randomness: Randomness) {
        assert!(self.randomness.set(randomness.clone()).is_ok());
    }

    pub fn set_insertion_time(&self) {
        assert!(self.pipeline_insertion_time.set(Instant::now()).is_ok());
    }

    pub fn take_pre_commit_fut(&self) -> BoxFuture<'static, ExecutorResult<()>> {
        self.pre_commit_fut
            .lock()
            .take()
            .expect("pre_commit_result_rx missing.")
    }
}

impl Debug for PipelinedBlock {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Display for PipelinedBlock {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.block())
    }
}

impl PipelinedBlock {
    pub fn new(
        block: Block,
        input_transactions: Vec<SignedTransaction>,
        state_compute_result: StateComputeResult,
    ) -> Self {
        Self {
            block,
            input_transactions,
            state_compute_result,
            randomness: OnceCell::new(),
            pipeline_insertion_time: OnceCell::new(),
            execution_summary: Arc::new(OnceCell::new()),
            pre_commit_fut: Arc::new(Mutex::new(None)),
            pipeline_futures: Arc::new(Mutex::new(None)),
            pipeline_tx: None,
            pipeline_abort_handle: None,
        }
    }

    pub fn new_ordered(block: Block) -> Self {
        Self::new(block, vec![], StateComputeResult::new_dummy())
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn id(&self) -> HashValue {
        self.block().id()
    }

    pub fn input_transactions(&self) -> &Vec<SignedTransaction> {
        &self.input_transactions
    }

    pub fn epoch(&self) -> u64 {
        self.block.epoch()
    }

    pub fn payload(&self) -> Option<&Payload> {
        self.block().payload()
    }

    pub fn parent_id(&self) -> HashValue {
        self.block.parent_id()
    }

    pub fn quorum_cert(&self) -> &QuorumCert {
        self.block().quorum_cert()
    }

    pub fn round(&self) -> Round {
        self.block().round()
    }

    pub fn validator_txns(&self) -> Option<&Vec<ValidatorTransaction>> {
        self.block().validator_txns()
    }

    pub fn timestamp_usecs(&self) -> u64 {
        self.block().timestamp_usecs()
    }

    pub fn compute_result(&self) -> &StateComputeResult {
        &self.state_compute_result
    }

    pub fn randomness(&self) -> Option<&Randomness> {
        self.randomness.get()
    }

    pub fn has_randomness(&self) -> bool {
        self.randomness.get().is_some()
    }

    pub fn block_info(&self) -> BlockInfo {
        let version = if let Some(block_number) = self.block().block_number() {
            block_number
        } else {
            0
        };
        self.block().gen_block_info(
            self.compute_result().root_hash(),
            version,
            self.compute_result().epoch_state().clone(),
        )
    }

    pub fn vote_proposal(&self) -> VoteProposal {
        VoteProposal::new(
            self.block.clone(),
            self.compute_result().epoch_state().clone(),
            true,
        )
    }

    pub fn order_vote_proposal(&self, quorum_cert: Arc<QuorumCert>) -> OrderVoteProposal {
        OrderVoteProposal::new(self.block.clone(), self.block_info(), quorum_cert)
    }

    pub fn subscribable_events(&self) -> Vec<ContractEvent> {
        // reconfiguration suffix don't count, the state compute result is carried over from parents
        // TODO(gravity_byteyue): we should have a better way to identify reconfiguration suffix
        // reconfiguration event之后的block是不应该被执行的. 在gravity-sdk的模型中暂时不会有这个问题 但是可能需要仔细考虑
        self.state_compute_result.events()
    }

    /// The block is suffix of a reconfiguration block if the state result carries over the epoch state
    /// from parent but has no transaction.
    pub fn is_reconfiguration_suffix(&self) -> bool {
        self.state_compute_result.has_reconfiguration()
    }

    pub fn elapsed_in_pipeline(&self) -> Option<Duration> {
        self.pipeline_insertion_time.get().map(|t| t.elapsed())
    }

    pub fn get_execution_summary(&self) -> Option<ExecutionSummary> {
        self.execution_summary.get().cloned()
    }

    pub fn pipeline_fut(&self) -> Option<PipelineFutures> {
        self.pipeline_futures.lock().clone()
    }

    pub fn set_pipeline_fut(&mut self, pipeline_futures: PipelineFutures) {
        *self.pipeline_futures.lock() = Some(pipeline_futures);
    }

    pub fn set_pipeline_tx(&mut self, pipeline_tx: PipelineInputTx) {
        self.pipeline_tx = Some(Arc::new(Mutex::new(pipeline_tx)));
    }

    pub fn set_pipeline_abort_handles(&mut self, abort_handles: Vec<AbortHandle>) {
        self.pipeline_abort_handle = Some(abort_handles);
    }

    pub fn pipeline_tx(&self) -> Option<&Arc<Mutex<PipelineInputTx>>> {
        self.pipeline_tx.as_ref()
    }

    pub fn abort_pipeline(&self) -> Option<PipelineFutures> {
        if let Some(abort_handles) = &self.pipeline_abort_handle {
            for handle in abort_handles {
                handle.abort();
            }
        }
        self.pipeline_futures.lock().take()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExecutionSummary {
    pub payload_len: u64,
    pub to_commit: u64,
    pub to_retry: u64,
    pub execution_time: Duration,
    pub root_hash: HashValue,
}
