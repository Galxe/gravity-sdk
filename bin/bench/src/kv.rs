use crate::should_produce_txn;
use crate::stateful_mempool::Mempool;
use crate::txn::RawTxn;
use api_types::compute_res::ComputeRes;
use api_types::u256_define::TxnHash;
use api_types::{
    u256_define::BlockId, ExecError, ExecTxn, ExecutionChannel, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn, VerifiedTxnWithAccountSeqNum
};
use async_trait::async_trait;
use log::info;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Default)]
struct BlockStatus {
    txn_number: u64,
}

struct CounterTimer {
    call_count: u32,
    last_time: Instant,
    txn_num_in_block: u32,
    count_round: u32,
}

impl CounterTimer {
    pub fn new() -> Self {
        Self {
            call_count: 0,
            last_time: Instant::now(),
            txn_num_in_block: std::env::var("BLOCK_TXN_NUMS")
                .map(|s| s.parse().unwrap())
                .unwrap_or(1000),
            count_round: std::env::var("COUNT_ROUND").map(|s| s.parse().unwrap()).unwrap_or(10),
        }
    }
    pub fn count(&mut self) {
        self.call_count += 1;
        if self.call_count == self.count_round {
            let now = Instant::now();
            let duration = now.duration_since(self.last_time);
            info!(
                "Time taken for the last {:?} blocks to be produced: {:?}",
                self.count_round, duration,
            );
            self.call_count = 0;
            self.last_time = now;
        }
    }
}

pub struct KvStore {
    store: Mutex<HashMap<String, String>>,
    mempool: Mempool,
    block_status: Mutex<HashMap<ExternalPayloadAttr, BlockStatus>>,
    compute_res_recv: Mutex<HashMap<ExternalBlockMeta, Receiver<ComputeRes>>>,
    counter: Mutex<CounterTimer>,
    not_empty_sets: Mutex<HashSet<BlockId>>,
}

impl KvStore {
    pub fn new() -> Self {
        KvStore {
            store: Mutex::new(HashMap::new()),
            mempool: Mempool::new(),
            block_status: Mutex::new(HashMap::new()),
            compute_res_recv: Mutex::new(HashMap::new()),
            counter: Mutex::new(CounterTimer::new()),
            not_empty_sets: Mutex::new(HashSet::new()),
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
}

static RECV_TXN_SUM: AtomicU64 = AtomicU64::new(0);
static SEND_TXN_SUM: AtomicU64 = AtomicU64::new(0);
static COMPUTE_RES_SUM: AtomicU64 = AtomicU64::new(0);
#[async_trait]
impl ExecutionChannel for KvStore {
    async fn send_user_txn(&self, txn: ExecTxn) -> Result<TxnHash, ExecError> {
        match txn {
            ExecTxn::RawTxn(bytes) => self.mempool.add_raw_txn(bytes).await,
            ExecTxn::VerifiedTxn(verified_txn) => self.mempool.add_verified_txn(verified_txn).await,
        }
        Ok(TxnHash::random())
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        Ok(self.mempool.recv_unbroadcasted_txn().await)
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
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

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        let txns = match should_produce_txn().await {
            true => Ok(self.mempool.pending_txns().await),
            false => Ok(vec![]),
        }?;
        RECV_TXN_SUM.fetch_add(txns.len() as u64, Ordering::Relaxed);
        info!("RECV_TXN_SUM: {}", RECV_TXN_SUM.load(Ordering::Relaxed));
        Ok(txns)
    }

    async fn send_ordered_block(
        &self,
        parent_id: BlockId,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        let mut res = vec![];

        if !ordered_block.txns.is_empty() {
            self.not_empty_sets.lock().await.insert(ordered_block.block_meta.block_id);
        }
        SEND_TXN_SUM.fetch_add(ordered_block.txns.len() as u64, Ordering::Relaxed);
        info!("SEND_TXN_SUM: {}", SEND_TXN_SUM.load(Ordering::Relaxed));
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
        send.send(ComputeRes::new(v, ordered_block.txns.len() as u64)).await.unwrap();
        let mut r = self.compute_res_recv.lock().await;
        r.insert(ordered_block.block_meta.clone(), recv);
        Ok(())
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        let mut receiver = {
                        let mut r = self.compute_res_recv.lock().await;
                        r.remove(&head).expect("Failed to get receiver")
                    
                    };
        let res = receiver.recv().await;
        match res {
            Some(r) => {
                COMPUTE_RES_SUM.fetch_add(r.txn_num, Ordering::Relaxed);
                info!("COMPUTE_RES_SUM: {}", COMPUTE_RES_SUM.load(Ordering::Relaxed));
                Ok(r)
            },
            None => Err(ExecError::InternalError),
        }
    }

    async fn send_committed_block_info(&self, head: BlockId) -> Result<(), ExecError> {
        let mut guard = self.not_empty_sets.lock().await;
        info!("enter send_committed_block_info");
        if guard.contains(&head) {
            info!("enter one execute");
            self.counter.lock().await.count();
            guard.remove(&head);
        }
        Ok(())
    }
}
