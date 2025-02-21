pub mod queue;
pub mod state;
use std::collections::HashMap;
use std::sync::Arc;

use crate::reth_cli::RethCli;
use api_types::compute_res::ComputeRes;
use api_types::u256_define::TxnHash;
use api_types::{
    u256_define::BlockId, ExecError, ExecTxn, ExecutionChannel,
    ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn,
    VerifiedTxnWithAccountSeqNum,
};
use api_types::{ExecutionBlocks, RecoveryApi, RecoveryError};
use async_trait::async_trait;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use greth::reth_primitives::B256;
use state::State;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;
use crate::metric::{RethCliMetric, METRICS};

pub struct RethCoordinator {
    reth_cli: RethCli,
    pending_buffer: Arc<Mutex<Vec<VerifiedTxnWithAccountSeqNum>>>,
    state: Arc<Mutex<State>>,
    // todo(gravity_byteyue): Use one elegant way
    block_number_to_txn_in_block: Arc<Mutex<HashMap<u64, u64>>>,
    execution_args_tx: Arc<Mutex<Option<oneshot::Sender<ExecutionArgs>>>>,
}

impl RethCoordinator {
    pub fn new(reth_cli: RethCli, execution_args_tx: oneshot::Sender<ExecutionArgs>) -> Self {
        let state = State::new();
        Self {
            reth_cli,
            pending_buffer: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(Mutex::new(state)),
            block_number_to_txn_in_block: Arc::new(Mutex::new(HashMap::new())),
            execution_args_tx: Arc::new(Mutex::new(Some(execution_args_tx))),
        }
    }

    pub async fn run(&self) {
        self.reth_cli.process_pending_transactions(self.pending_buffer.clone()).await.unwrap();
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
        debug!("check_block_txns with payload_attr: {:?}", payload_attr);
        for txn in txns {
            let mut state = self.state.lock().await;
            let txn = serde_json::from_slice(&txn.bytes).unwrap();
            if !state.check_new_txn(&payload_attr, txn) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        let now = std::time::Instant::now();
        let mut buffer = self.pending_buffer.lock().await;
        let res = std::mem::take(&mut *buffer);
        METRICS.get_or_init(|| RethCliMetric::default()).recv_txns_count.increment(res.len() as u64);
        debug!("recv_pending_txns with buffer size: {:?} in time {:?}", &res.len(), now.elapsed());
        Ok(res)
    }

    async fn send_ordered_block(
        &self,
        parent_id: BlockId,
        mut ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        debug!(
            "send_ordered_block with parent_id: {:?} and block num {:?} txn count {:?}",
            parent_id,
            ordered_block.block_meta.block_number,
            ordered_block.txns.len()
        );
        {
            let mut state = self.state.lock().await;
            ordered_block.txns = ordered_block
                .txns
                .into_iter()
                .filter(|txn| state.update_account_seq_num(txn))
                .collect();
        }
        {
            let mut map = self.block_number_to_txn_in_block.lock().await;
            map.insert(ordered_block.block_meta.block_number, ordered_block.txns.len() as u64);
        }
        self.reth_cli
            .push_ordered_block(ordered_block, B256::new(parent_id.bytes()))
            .await
            .unwrap();
        debug!("send_ordered_block done");
        Ok(())
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        debug!("recv_executed_block_hash with head: {:?}", head);
        let reth_block_id = B256::from_slice(&head.block_id.0);
        let block_hash = self.reth_cli.recv_compute_res(reth_block_id).await;
        {
            self.state.lock().await.insert_new_block(head.block_id, block_hash.unwrap().into());
        }
        debug!("recv_executed_block_hash done");
        let mut block_number = None;
        {
            let mut map = self.block_number_to_txn_in_block.lock().await;
            block_number = map.remove(&head.block_number);
        }
        Ok(ComputeRes::new(block_hash.unwrap().into(), block_number.unwrap()))
    }

    async fn send_committed_block_info(&self, block_id: BlockId) -> Result<(), ExecError> {
        debug!("commit_block with block_id: {:?}", block_id);
        let block_hash = { self.state.lock().await.get_block_hash(block_id).unwrap() };
        self.reth_cli.send_committed_block_info(block_id, block_hash.into()).await.unwrap();
        debug!("commit_block done");
        Ok(())
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
        let origin_block_hash = block.block_meta.block_hash;
        self.send_ordered_block(parent_id, block).await?;
        let reth_block_id = B256::from_slice(&block_id.0);
        let block_hash = self.reth_cli.recv_compute_res(reth_block_id).await.unwrap().into();
        if let Some(origin_block_hash) = origin_block_hash {
            let origin_block_hash = B256::new(origin_block_hash.data);
            assert_eq!(origin_block_hash, block_hash);
        }
        self.state.lock().await.insert_new_block(block_id, block_hash);
        Ok(())
    }
}
