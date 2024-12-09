use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use api_types::{BlockBatch, ComputeRes, ExecError, ExecTxn, ExecutionApiV2, ExecutionBlocks, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, GTxn, VerifiedTxn};
use crate::stateful_mempool::Mempool;
use crate::txn::RawTxn;
use async_trait::async_trait;

#[derive(Default)]
struct BlockStatus {
    txn_number: u64,
}

pub struct KvStore {
    store: Mutex<HashMap<String, String>>,
    mempool: Mempool,
    block_status: Mutex<HashMap<ExternalPayloadAttr, BlockStatus>>,
    compute_res_recv: Mutex<HashMap<ExternalBlockMeta, Receiver<ComputeRes>>>,
    ordered_block: Mutex<HashMap<ExternalBlockMeta, ExternalBlock>>,
}


impl KvStore {
    pub fn new() -> Self {
        KvStore {
            store: Mutex::new(HashMap::new()),
            mempool: Mempool::new(),
            block_status: Mutex::new(HashMap::new()),
            compute_res_recv: Mutex::new(HashMap::new()),
            ordered_block: Mutex::new(HashMap::new())
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let data = self.store.lock().await;
        data.get(key).cloned()
    }

    pub async fn set(&self, key: String, val: String) {
        let mut store = self.store.lock().await;
        store.insert(key, val);
    }

    pub async fn remove(&mut self, key: String) {
        let mut store = self.store.lock().await;
        store.remove(&key);
    }
}

#[async_trait]
impl ExecutionApiV2 for KvStore {
    async fn add_txn(&self, txn: ExecTxn) -> Result<(), ExecError> {
        match txn {
            ExecTxn::RawTxn(bytes) => self.mempool.add_raw_txn(bytes).await,
            ExecTxn::VerifiedTxn(verified_txn) => self.mempool.add_verified_txn(verified_txn).await
        }
        Ok(())
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        Ok(self.mempool.recv_unbroadcasted_txn().await)
    }

    async fn check_block_txns(&self, payload_attr: ExternalPayloadAttr, txns: Vec<VerifiedTxn>) -> Result<bool, ExecError> {
        let mut block = self.block_status.lock().await;
        let status = block.entry(payload_attr).or_insert(BlockStatus::default());
        let new_txn_number = status.txn_number + txns.len() as u64;
        if new_txn_number > 100 {
            Ok(false)
        } else {
            status.txn_number = new_txn_number;
            Ok(true)
        }
    }

    async fn recv_pending_txns(&self) -> Result<Vec<(VerifiedTxn, u64)>, ExecError> {
        Ok(self.mempool.pending_txns().await)
    }

    async fn send_ordered_block(&self, ordered_block: ExternalBlock) -> Result<(), ExecError> {
        let mut res = vec![];

        for txn in &ordered_block.txns {
            let raw_txn = RawTxn::from_bytes(txn.bytes().to_vec());
            self.set(raw_txn.key().clone(), raw_txn.val().clone()).await;
            let val = self.get(raw_txn.key()).await;
            res.push(val);
        }
        let mut hasher = DefaultHasher::new();
        res.hash(&mut hasher);
        let hash_value = hasher.finish();
        let mut v = [0; 32];
        let bytes = hash_value.to_le_bytes();
        v[0..8].copy_from_slice(&bytes);

        let (send, recv) = tokio::sync::mpsc::channel::<ComputeRes>(1);
        send.send(ComputeRes::new(v)).await.unwrap();
        let mut r = self.compute_res_recv.lock().await;
        r.insert(ordered_block.block_meta.clone(), recv);


        let mut block = self.ordered_block.lock().await;
        block.insert(ordered_block.block_meta.clone(), ordered_block);
        Ok(())
    }

    async fn recv_executed_block_hash(&self, head: ExternalBlockMeta) -> Result<ComputeRes, ExecError> {
        let mut r = self.compute_res_recv.lock().await;
        let receiver = r.get_mut(&head).expect("Failed to get receiver");
        let res = receiver.recv().await;
        match res {
            Some(r) => Ok(r),
            None => Err(ExecError::InternalError),
        }
    }

    async fn commit_block(&self, head: ExternalBlockMeta) -> Result<(), ExecError> {
        let block = self.ordered_block.lock().await;
        for txn in &block.get(&head).unwrap().txns {
            self.mempool.remove_txn(txn).await
        }
        Ok(())
    }
}