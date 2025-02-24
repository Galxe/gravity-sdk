use std::collections::HashMap;

use api_types::{
    compute_res::ComputeRes,
    u256_define::{BlockId, TxnHash},
    ExecError, ExecTxn, ExecutionChannel, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr,
    VerifiedTxn, VerifiedTxnWithAccountSeqNum,
};
use async_trait::async_trait;
use futures::{
    channel::{
        mpsc,
        oneshot::{channel, Receiver, Sender},
    },
    SinkExt,
};
use log::info;
use tokio::sync::Mutex;

use crate::{mempool::Mempool, ExecutableBlock, RawBlock, TransactionWithAccount};

pub struct ExecutionChannelImpl {
    ordered_block_sender: mpsc::Sender<ExecutableBlock>,
    // to receive the state root
    block_hash_receivers: Mutex<HashMap<BlockId, Receiver<(ComputeRes, Sender<u64>)>>>,
    block_commit_sender: Mutex<HashMap<BlockId, (u64, Sender<u64>)>>,
    mempool: Mempool,
}

impl ExecutionChannelImpl {
    pub fn new(ordered_block_sender: mpsc::Sender<ExecutableBlock>) -> Self {
        Self {
            ordered_block_sender,
            block_hash_receivers: Mutex::new(HashMap::new()),
            block_commit_sender: Mutex::new(HashMap::new()),
            mempool: Mempool::new(),
        }
    }
}

#[async_trait]
impl ExecutionChannel for ExecutionChannelImpl {
    async fn send_user_txn(&self, bytes: ExecTxn) -> Result<TxnHash, ExecError> {
        let hash = match bytes {
            ExecTxn::RawTxn(txn) => self.mempool.add_raw_txn(txn).await,
            ExecTxn::VerifiedTxn(txn) => self.mempool.add_verified_txn(txn).await,
        };
        Ok(hash)
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        Ok(self.mempool.recv_unbroadcasted_txn().await)
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
        unimplemented!("Currently not implemented");
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        Ok(self.mempool.pending_txns().await)
    }

    async fn send_ordered_block(
        &self,
        _parent_id: BlockId,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        let transactions = ordered_block
            .txns
            .into_iter()
            .map(|verified_txn| TransactionWithAccount::from_bytes(verified_txn.bytes).txn)
            .collect();

        // TODO(): maybe we should send parent block id to get parent block number to read the state root
        let block = RawBlock { block_number: ordered_block.block_meta.block_number, transactions };
        info!("send ordered block {:?}, empty block {}", ordered_block.block_meta.block_number, block.transactions.is_empty());

        let (sender, receiver) = channel();
        let executable_block = ExecutableBlock { block, callbacks: sender };

        self.ordered_block_sender
            .clone()
            .send(executable_block)
            .await
            .map_err(|_| ExecError::InternalError)?;
        self.block_hash_receivers
            .lock()
            .await
            .insert(ordered_block.block_meta.block_id.clone(), receiver);
        Ok(())
    }

    // the block hash is the hash of the block that has been executed, which is passed by the send_ordered_block
    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        info!("recv executed block hash {:?}", head);
        let (hash, sender) = self
            .block_hash_receivers
            .lock()
            .await
            .get_mut(&head.block_id)
            .expect("Failed to get receiver")
            .await
            .map_err(|_| ExecError::InternalError)?;
        info!("recv executed block hash get {:?}, hash {:?}", head, hash);
        self.block_commit_sender
            .lock()
            .await
            .insert(head.block_id.clone(), (head.block_number, sender));
        info!("recv executed block hash {:?}, hash {:?}", head, hash);
        Ok(hash)
    }

    // this function is called by the execution layer commit the block hash
    async fn send_committed_block_info(&self, block_id: BlockId) -> Result<(), ExecError> {
        info!("send committed block info {:?}", block_id);
        let (block_number, sender) =
            self.block_commit_sender.lock().await.remove(&block_id).expect("Failed to get sender");
        sender.send(block_number).unwrap();
        Ok(())
    }
}
