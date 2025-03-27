pub mod queue;
pub mod state;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::reth_cli::{convert_account, RethCli};
use api_types::compute_res::{ComputeRes, TxnStatus};
use api_types::u256_define::TxnHash;
use api_types::RecoveryApi;
use api_types::{
    u256_define::BlockId, ExecError, ExecTxn, ExecutionChannel, ExternalBlock, ExternalBlockMeta,
    ExternalPayloadAttr, VerifiedTxn, VerifiedTxnWithAccountSeqNum,
};
use async_trait::async_trait;
use block_buffer_manager::get_block_buffer_manager;
use greth::reth::revm::db::components::block_hash;
use greth::reth_pipe_exec_layer_ext_v2::{ExecutionArgs, ExecutionResult};
use alloy_primitives::B256;
use state::State;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, Sleep};
use tracing::{debug, info};

pub struct RethCoordinator {
    reth_cli: Arc<RethCli>,
    state: Arc<Mutex<State>>,
    execution_args_tx: Arc<Mutex<Option<oneshot::Sender<ExecutionArgs>>>>,
}

impl RethCoordinator {
    pub fn new(
        reth_cli: RethCli,
        latest_block_number: u64,
        execution_args_tx: oneshot::Sender<ExecutionArgs>,
    ) -> Self {
        let state = State::new(latest_block_number);
        Self {
            reth_cli: Arc::new(reth_cli),
            state: Arc::new(Mutex::new(state)),
            execution_args_tx: Arc::new(Mutex::new(Some(execution_args_tx))),
        }
    }

    pub async fn run(&self) {
        let reth_cli = self.reth_cli.clone();
        tokio::spawn(async move {
            reth_cli.start_mempool().await.unwrap();
        });
        let reth_cli = self.reth_cli.clone();
        tokio::spawn(async move {
            reth_cli.start_execution().await.unwrap();
        });
        let reth_cli = self.reth_cli.clone();
        tokio::spawn(async move {
            reth_cli.start_commit_vote().await.unwrap();
        });
        let reth_cli = self.reth_cli.clone();
        tokio::spawn(async move {
            reth_cli.start_commit().await.unwrap();
        });
    }
}

#[async_trait]
impl ExecutionChannel for RethCoordinator {
    async fn send_user_txn(&self, _bytes: ExecTxn) -> Result<TxnHash, ExecError> {
        panic!("Reth Coordinator does not support add_txn");
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        panic!("Reth Coordinator does not support recv_unbroadcasted_txn");
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
        panic!("Reth Coordinator does not support check_block_txns");
    }

    async fn send_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        panic!("Reth Coordinator does not support send_pending_txns");
    }

    async fn recv_ordered_block(
        &self,
        parent_id: BlockId,
        mut ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        panic!("Reth Coordinator does not support recv_ordered_block");
    }

    async fn send_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        panic!("Reth Coordinator does not support send_executed_block_hash");
    }

    async fn recv_committed_block_info(&self, block_id: BlockId) -> Result<(), ExecError> {
        panic!("Reth Coordinator does not support recv_committed_block_info")
    }
}

#[async_trait]
impl RecoveryApi for RethCoordinator {
    async fn register_execution_args(&self, args: api_types::ExecutionArgs) {
        let mut guard = self.execution_args_tx.lock().await;
        let execution_args_tx = guard.take();
        if let Some(execution_args_tx) = execution_args_tx {
            let block_number_to_block_id = args
                .block_number_to_block_id
                .into_iter()
                .map(|(block_number, block_id)| (block_number, B256::new(*block_id)))
                .collect();
            let execution_args = ExecutionArgs { block_number_to_block_id };
            execution_args_tx.send(execution_args).unwrap();
        }
    }

    async fn latest_block_number(&self) -> u64 {
        self.reth_cli.latest_block_number().await
    }

    async fn finalized_block_number(&self) -> u64 {
        self.reth_cli.finalized_block_number().await
    }

    async fn recover_ordered_block(
        &self,
        parent_id: BlockId,
        block: ExternalBlock,
    ) -> Result<(), ExecError> {
        let block_id = block.block_meta.block_id.clone();
        let origin_block_hash = block.block_meta.block_hash.clone();
        let mut block_hash;
        let block_number = block.block_meta.block_number;
        match self.recv_ordered_block(parent_id, block).await {
            Err(ExecError::DuplicateExecError) => {
                loop {
                    let state = self.state.lock().await;
                    if let Some(block_hash_) = state.get_block_result(block_id) {
                        block_hash = block_hash_;
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            },
            Err(err) => return Err(err),
            Ok(()) => {
                block_hash = B256::from_slice(&get_block_buffer_manager().get_executed_res(
                    BlockId::from_bytes(&block_id.0),
                    block_number
                ).await.unwrap_or_else(|_| {
                    panic!("Failed to get executed result for block {:?}", block_id);
                }).data);
                // self.reth_cli.recv_compute_res(reth_block_id).await.unwrap().into();
            }
        }
        if let Some(origin_block_hash) = origin_block_hash {
            let origin_block_hash = B256::new(origin_block_hash.data);
            assert_eq!(origin_block_hash, block_hash.block_hash);
        }
        self.state.lock().await.insert_new_block(block_id, block_hash);
        Ok(())
    }
}
