pub mod state;
pub mod queue;
use std::sync::Arc;

use crate::reth_cli::RethCli;
use api_types::u256_define::TxnHash;
use api_types::{
    u256_define::BlockId, u256_define::ComputeRes, ExecError, ExecTxn, ExecutionApiV2, ExternalBlock, ExternalBlockMeta,
    ExternalPayloadAttr, VerifiedTxn, VerifiedTxnWithAccountSeqNum,
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
    pub fn new(reth_cli: RethCli, gensis_id: [u8; 32], gensis_hash: B256) -> Self {
        let mut state = State::new();
        state.insert_new_block(BlockId(gensis_id), gensis_hash);
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
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        info!("send_ordered_block with parent_id: {:?}", parent_id);
        self.queue.send_exec(ordered_block.block_meta.block_id).await;
        let state = self.state.lock().await;
        let parent_hash = state
        .get_block_hash(parent_id.clone());
        if parent_hash.is_none() {
            panic!("parent block not found {}", parent_id);
        }
        let parent_hash = parent_hash.unwrap();
        let payload_id = self.reth_cli.push_ordered_block(ordered_block, parent_hash).await;
        self.pending_payload_id.send(payload_id.clone().unwrap()).await;
        self.commit_buffer.send((payload_id.unwrap(), parent_hash)).await;
        Ok(())
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        info!("recv_executed_block_hash with head: {:?}", head);
        self.queue.recv_exec().await;
        self.queue.send_commit(head.block_id.clone()).await;
        let block_id = &head.block_id;
        let reth_block_id = B256::from_slice(&block_id.0);
        let payload_id = self.pending_payload_id.recv().await;
        let block_hash = self.reth_cli.process_payload_id(reth_block_id, payload_id).await;
        self.state.lock().await.insert_new_block(head.block_id, block_hash.unwrap().into());
        Ok(ComputeRes::new(block_hash.unwrap().into()))
    }

    async fn commit_block(&self, block_id: BlockId) -> Result<(), ExecError> {
        info!("commit_block with block_id: {:?}", block_id);
        self.queue.recv_commit().await;
        let (payload_id, parent_hash) = self.commit_buffer.recv().await;
        let block_hash = self.state.lock().await.get_block_hash(block_id).unwrap();
        self.reth_cli.commit_block(parent_hash, payload_id, block_hash.into()).await.unwrap();
        Ok(())
    }
}
