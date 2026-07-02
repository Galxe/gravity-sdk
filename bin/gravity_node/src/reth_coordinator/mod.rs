use std::{sync::Arc, time::Duration};

use crate::reth_cli::{RethCli, RethEthCall};
use alloy_primitives::B256;
use block_buffer_manager::get_block_buffer_manager;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use tokio::{
    sync::{broadcast, oneshot, Mutex},
    task::{JoinError, JoinHandle},
};
use tracing::info;

const COORDINATOR_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

pub struct RethCoordinator<EthApi: RethEthCall> {
    reth_cli: Arc<RethCli<EthApi>>,
    execution_args_tx: Arc<Mutex<Option<oneshot::Sender<ExecutionArgs>>>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl<EthApi: RethEthCall> RethCoordinator<EthApi> {
    pub fn new(
        reth_cli: Arc<RethCli<EthApi>>,
        _latest_block_number: u64,
        execution_args_tx: oneshot::Sender<ExecutionArgs>,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            reth_cli,
            execution_args_tx: Arc::new(Mutex::new(Some(execution_args_tx))),
            shutdown_tx,
        }
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

    pub async fn run(&self) -> Result<(), String> {
        let reth_cli1 = self.reth_cli.clone();
        let mut h1 = tokio::spawn(async move { reth_cli1.start_execution().await });

        let reth_cli2 = self.reth_cli.clone();
        let mut h2 = tokio::spawn(async move { reth_cli2.start_commit_vote().await });

        let reth_cli3 = self.reth_cli.clone();
        let mut h3 = tokio::spawn(async move { reth_cli3.start_commit().await });

        let mut shutdown_rx = self.shutdown_tx.subscribe();
        tokio::select! {
            res = &mut h1 => {
                let result = Self::task_result("start_execution", res);
                self.signal_shutdown();
                Self::wait_for_task("start_commit_vote", h2).await;
                Self::wait_for_task("start_commit", h3).await;
                result
            }
            res = &mut h2 => {
                let result = Self::task_result("start_commit_vote", res);
                self.signal_shutdown();
                Self::wait_for_task("start_execution", h1).await;
                Self::wait_for_task("start_commit", h3).await;
                result
            }
            res = &mut h3 => {
                let result = Self::task_result("start_commit", res);
                self.signal_shutdown();
                Self::wait_for_task("start_execution", h1).await;
                Self::wait_for_task("start_commit_vote", h2).await;
                result
            }
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping reth coordinator tasks");
                self.signal_shutdown();
                Self::wait_for_task("start_execution", h1).await;
                Self::wait_for_task("start_commit_vote", h2).await;
                Self::wait_for_task("start_commit", h3).await;
                Ok(())
            }
        }
    }

    fn task_result(
        task_name: &'static str,
        result: Result<Result<(), String>, JoinError>,
    ) -> Result<(), String> {
        match result {
            Ok(Ok(())) => {
                info!("{task_name} task stopped");
                Ok(())
            }
            Ok(Err(err)) => {
                tracing::error!("{task_name} task failed: {err}");
                Err(format!("{task_name} task failed: {err}"))
            }
            Err(err) => {
                tracing::error!("{task_name} task join failed: {err}");
                Err(format!("{task_name} task join failed: {err}"))
            }
        }
    }

    async fn wait_for_task(task_name: &'static str, mut handle: JoinHandle<Result<(), String>>) {
        match tokio::time::timeout(COORDINATOR_SHUTDOWN_TIMEOUT, &mut handle).await {
            Ok(result) => {
                if let Err(err) = Self::task_result(task_name, result) {
                    tracing::warn!("reth coordinator observed task shutdown error: {err}");
                }
            }
            Err(_) => {
                tracing::warn!(
                    "{task_name} task did not stop within {:?}; aborting",
                    COORDINATOR_SHUTDOWN_TIMEOUT
                );
                handle.abort();
            }
        }
    }

    fn signal_shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}
