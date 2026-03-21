use std::sync::Arc;

use crate::reth_cli::{RethCli, RethEthCall};
use alloy_primitives::B256;
use block_buffer_manager::get_block_buffer_manager;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use tokio::sync::{oneshot, Mutex};
use tracing::info;

pub struct RethCoordinator<EthApi: RethEthCall> {
    reth_cli: Arc<RethCli<EthApi>>,
    execution_args_tx: Arc<Mutex<Option<oneshot::Sender<ExecutionArgs>>>>,
}

impl<EthApi: RethEthCall> RethCoordinator<EthApi> {
    pub fn new(
        reth_cli: Arc<RethCli<EthApi>>,
        _latest_block_number: u64,
        execution_args_tx: oneshot::Sender<ExecutionArgs>,
    ) -> Self {
        Self { reth_cli, execution_args_tx: Arc::new(Mutex::new(Some(execution_args_tx))) }
    }

    pub async fn send_execution_args(&self) {
        let mut guard = self.execution_args_tx.lock().await;
        let execution_args_tx = guard.take();
        if let Some(execution_args_tx) = execution_args_tx {
            let block_number_to_block_id = get_block_buffer_manager()
                .block_number_to_block_id()
                .await
                .into_iter()
                .map(|(block_number, block_id)| (block_number, B256::new(block_id.bytes())))
                .collect();
            info!("send_execution_args block_number_to_block_id: {:?}", block_number_to_block_id);
            let execution_args = ExecutionArgs { block_number_to_block_id };
            execution_args_tx
                .send(execution_args)
                .expect("Failed to send execution args: reth receiver may have been dropped");
        }
    }

    pub async fn run(&self) {
        let reth_cli1 = self.reth_cli.clone();
        let h1 = tokio::spawn(async move { reth_cli1.start_execution().await });

        let reth_cli2 = self.reth_cli.clone();
        let h2 = tokio::spawn(async move { reth_cli2.start_commit_vote().await });

        let reth_cli3 = self.reth_cli.clone();
        let h3 = tokio::spawn(async move { reth_cli3.start_commit().await });

        tokio::spawn(async move {
            tokio::select! {
                res = h1 => tracing::error!("start_execution task exited unexpectedly: {:?}", res),
                res = h2 => tracing::error!("start_commit_vote task exited unexpectedly: {:?}", res),
                res = h3 => tracing::error!("start_commit task exited unexpectedly: {:?}", res),
            };
            std::process::exit(1);
        });
    }
}
