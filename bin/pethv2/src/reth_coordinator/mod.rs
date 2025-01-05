pub mod queue;
pub mod state;
use std::sync::Arc;

use crate::reth_cli::RethCli;
use api_types::u256_define::TxnHash;
use api_types::{
    u256_define::BlockId, u256_define::ComputeRes, ExecError, ExecTxn, ExecutionApiV2,
    ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn,
    VerifiedTxnWithAccountSeqNum,
};
use async_trait::async_trait;
use reth_payload_builder::PayloadId;
use reth_primitives::B256;
use state::State;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::info;

pub struct Buffer<T> {
    sender: mpsc::Sender<T>,
    receiver: Arc<Mutex<mpsc::Receiver<T>>>,
}

impl<T> Buffer<T> {
    pub fn new(size: usize) -> Buffer<T> {
        let (sender, receiver) = mpsc::channel(size);
        Buffer { sender, receiver: Arc::new(Mutex::new(receiver)) }
    }

    pub async fn send(&self, item: T) {
        self.sender.clone().send(item).await.unwrap();
    }

    pub async fn recv(&self) -> T {
        let mut recv = self.receiver.lock().await;
        recv.recv().await.unwrap()
    }
}

pub struct RethCoordinator {
    reth_cli: RethCli,
    pending_buffer: Arc<Mutex<Vec<VerifiedTxnWithAccountSeqNum>>>,
    state: Arc<Mutex<State>>,
    queue: queue::Queue,
    pending_payload_id: Buffer<PayloadId>,
    // parent block id and payload id
    commit_buffer: Buffer<(PayloadId, B256)>,
}

impl RethCoordinator {
    pub fn new(reth_cli: RethCli) -> Self {
        let state = State::new();
        Self {
            queue: queue::Queue::new(1),
            reth_cli,
            pending_buffer: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(Mutex::new(state)),
            pending_payload_id: Buffer::new(1),
            commit_buffer: Buffer::new(1),
        }
    }

    pub async fn run(&self) {
        self.reth_cli.process_pending_transactions(self.pending_buffer.clone()).await.unwrap();
    }
}

#[async_trait]
impl ExecutionApiV2 for RethCoordinator {
    async fn add_txn(&self, _bytes: ExecTxn) -> Result<TxnHash, ExecError> {
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
        info!("check_block_txns with payload_attr: {:?}", payload_attr);
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
        let mut buffer = self.pending_buffer.lock().await;
        info!("recv_pending_txns with buffer size: {:?}", buffer.len());
        let res = buffer.drain(..).collect();
        Ok(res)
    }

    async fn send_ordered_block(
        &self,
        parent_id: BlockId,
        mut ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        info!(
            "send_ordered_block with parent_id: {:?} and block num {:?}",
            parent_id, ordered_block.block_meta.block_number
        );
        println!("parent id is {:?}", parent_id.bytes());
        self.queue.send_exec(ordered_block.block_meta.block_id).await;
        let mut state = self.state.lock().await;
        ordered_block.txns = ordered_block
            .txns
            .into_iter()
            .filter(|txn| state.update_account_seq_num(txn))
            .collect();
        self.reth_cli
            .push_ordered_block(ordered_block, B256::new(parent_id.bytes()))
            .await
            .unwrap();
        Ok(())
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        info!("recv_executed_block_hash with head: {:?}", head);
        let reth_block_id = B256::from_slice(&head.block_id.0);
        let block_hash = self.reth_cli.recv_compute_res(reth_block_id).await;
        self.state.lock().await.insert_new_block(head.block_id, block_hash.unwrap().into());
        Ok(ComputeRes::new(block_hash.unwrap().into()))
    }

    async fn commit_block(&self, block_id: BlockId) -> Result<(), ExecError> {
        info!("commit_block with block_id: {:?}", block_id);
        let block_hash = self.state.lock().await.get_block_hash(block_id).unwrap();
        self.reth_cli.commit_block(block_id, block_hash.into()).await.unwrap();
        self.queue.recv_commit().await;
        Ok(())
    }
}
