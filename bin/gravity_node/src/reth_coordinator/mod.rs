pub mod queue;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::reth_cli::{convert_account, RethCli};
use block_buffer_manager::get_block_buffer_manager;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use alloy_primitives::B256;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, Sleep};
use tracing::{debug, info};

pub struct RethCoordinator {
    reth_cli: Arc<RethCli>,
    execution_args_tx: Arc<Mutex<Option<oneshot::Sender<ExecutionArgs>>>>,
}

impl RethCoordinator {
    pub fn new(
        reth_cli: RethCli,
        latest_block_number: u64,
        execution_args_tx: oneshot::Sender<ExecutionArgs>,
    ) -> Self {
        Self {
            reth_cli: Arc::new(reth_cli),
            execution_args_tx: Arc::new(Mutex::new(Some(execution_args_tx))),
        }
    }

    pub async fn send_execution_args(&self) {
        let mut guard = self.execution_args_tx.lock().await;
        let execution_args_tx = guard.take();
        if let Some(execution_args_tx) = execution_args_tx {
            let block_number_to_block_id = get_block_buffer_manager()
                .block_number_to_block_id().await
                .into_iter()
                .map(|(block_number, block_id)| (block_number, B256::new(block_id.bytes())))
                .collect();
            info!("send_execution_args block_number_to_block_id: {:?}", block_number_to_block_id);
            let execution_args = ExecutionArgs { block_number_to_block_id };
            execution_args_tx.send(execution_args).unwrap();
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