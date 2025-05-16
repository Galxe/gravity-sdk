use crate::should_produce_txn;
use crate::stateful_mempool::Mempool;
use crate::txn::RawTxn;
use gaptos::api_types::compute_res::ComputeRes;
use gaptos::api_types::u256_define::TxnHash;
use gaptos::api_types::{
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
    _store: Mutex<HashMap<String, String>>,
    mempool: Mempool,
    block_status: Mutex<HashMap<ExternalPayloadAttr, BlockStatus>>,
    block_len: Mutex<HashMap<BlockId, usize>>,
}

impl KvStore {
    pub fn new() -> Self {
        KvStore {
            _store: Mutex::new(HashMap::new()),
            mempool: Mempool::new(),
            block_status: Mutex::new(HashMap::new()),
            block_len: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let data = self._store.lock().await;
        data.get(key).cloned()
    }

    pub async fn set(&self, key: String, val: String) {
        let mut store = self._store.lock().await;
        store.insert(key, val);
    }
}

static RECV_TXN_SUM: AtomicU64 = AtomicU64::new(0);
static SEND_TXN_SUM: AtomicU64 = AtomicU64::new(0);
static COMPUTE_RES_SUM: AtomicU64 = AtomicU64::new(0);

// #[async_trait]
// impl ExecutionChannel for KvStore {
//     async fn send_user_txn(&self, txn: ExecTxn) -> Result<TxnHash, ExecError> {
//         match txn {
//             ExecTxn::RawTxn(bytes) => self.mempool.add_raw_txn(bytes).await,
//             ExecTxn::VerifiedTxn(verified_txn) => self.mempool.add_verified_txn(verified_txn).await,
//         }
//         Ok(TxnHash::random())
//     }

//     async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
//         Ok(self.mempool.recv_unbroadcasted_txn().await)
//     }

//     async fn check_block_txns(
//         &self,
//         payload_attr: ExternalPayloadAttr,
//         txns: Vec<VerifiedTxn>,
//     ) -> Result<bool, ExecError> {
//         let mut block = self.block_status.lock().await;
//         let status = block.entry(payload_attr).or_insert(BlockStatus::default());
//         let new_txn_number = status.txn_number + txns.len() as u64;
//         if new_txn_number > 100 {
//             Ok(false)
//         } else {
//             status.txn_number = new_txn_number;
//             Ok(true)
//         }
//     }

//     async fn send_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
//         let txns = match should_produce_txn().await {
//             true => Ok(self.mempool.pending_txns().await),
//             false => Ok(vec![]),
//         }?;
//         RECV_TXN_SUM.fetch_add(txns.len() as u64, Ordering::Relaxed);
//         info!("RECV_TXN_SUM: {}", RECV_TXN_SUM.load(Ordering::Relaxed));
//         Ok(txns)
//     }

//     async fn recv_ordered_block(
//         &self,
//         parent_id: BlockId,
//         ordered_block: ExternalBlock,
//     ) -> Result<(), ExecError> {
//         {
//             let mut block_len = self.block_len.lock().await;
//             block_len.insert(ordered_block.block_meta.block_id, ordered_block.txns.len());
//         }
//         SEND_TXN_SUM.fetch_add(ordered_block.txns.len() as u64, Ordering::Relaxed);
//         info!("SEND_TXN_SUM: {}", SEND_TXN_SUM.load(Ordering::Relaxed));
//         Ok(())
//     }

//     async fn send_executed_block_hash(
//         &self,
//         head: ExternalBlockMeta,
//     ) -> Result<ComputeRes, ExecError> {
//         let mut r = ComputeRes::random();
//         r.txn_num = {
//             let mut block_len = self.block_len.lock().await;
//             block_len.remove(&head.block_id).unwrap().clone()
//         } as u64;
//         COMPUTE_RES_SUM.fetch_add(r.txn_num, Ordering::Relaxed);
//         info!("COMPUTE_RES_SUM: {}", COMPUTE_RES_SUM.load(Ordering::Relaxed));
//         Ok(r)
//     }

//     async fn recv_committed_block_info(&self, head: BlockId) -> Result<(), ExecError> {
//         info!("enter send_committed_block_info");
//         Ok(())
//     }
// }
