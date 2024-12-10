use std::sync::Arc;

use api_types::{
    account::ExternalAccountAddress, BlockBatch, ComputeRes, ExecError, ExecTxn, ExecutionApiV2,
    ExecutionBlocks, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn,
};
use async_trait::async_trait;
use log::info;
use rand::Rng;
use tokio::{sync::Mutex, time::Instant};

use crate::{kv::KvStore, server_trait::IServer, txn::RawTxn};

fn generate_random_address() -> ExternalAccountAddress {
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    ExternalAccountAddress::new(random_bytes)
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
                "Time taken for the last {:?} blocks to be produced: {:?}, txn num in block {:?}",
                self.count_round, duration, self.txn_num_in_block
            );

            self.call_count = 0;
            self.last_time = now;
        }
    }
}

struct InnerExecution {
    inner: Arc<dyn ExecutionApiV2>,
    counter: Mutex<CounterTimer>,
}

impl InnerExecution {
    fn new(inner: Arc<dyn ExecutionApiV2>) -> Self {
        Self { inner, counter: Mutex::new(CounterTimer::new()) }
    }
}

#[async_trait]
impl ExecutionApiV2 for InnerExecution {
    async fn add_txn(&self, bytes: ExecTxn) -> Result<(), ExecError> {
        info!("called into execution api v2 add_txn");
        self.inner.add_txn(bytes).await
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        self.inner.recv_unbroadcasted_txn().await
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
        self.inner.check_block_txns(payload_attr, txns).await
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        println!("into inner recv_pending_txns");
        info!("into inner recv_pending_txns");
        self.inner.recv_pending_txns().await
    }

    async fn send_ordered_block(&self, ordered_block: ExternalBlock) -> Result<(), ExecError> {
        info!("enter inner send_ordered_block");
        let should_count = !ordered_block.txns.is_empty();
        let r = self.inner.send_ordered_block(ordered_block).await;
        if should_count {
            self.counter.lock().await.count();
        }
        r
    }

    // the block hash is the hash of the block that has been executed, which is passed by the send_ordered_block
    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        info!("enter inner recv_executed_block_hash");
        self.inner.recv_executed_block_hash(head).await
    }

    // this function is called by the execution layer commit the block hash
    async fn commit_block(&self, head: ExternalBlockMeta) -> Result<(), ExecError> {
        info!("enter inner commit_block");
        self.inner.commit_block(head).await
    }

    fn latest_block_number(&self) -> u64 {
        self.inner.latest_block_number()
    }

    fn finalized_block_number(&self) -> u64 {
        self.inner.finalized_block_number()
    }

    async fn recover_ordered_block(&self, block_batch: BlockBatch) {
        self.inner.recover_ordered_block(block_batch).await
    }

    async fn recover_execution_blocks(&self, blocks: ExecutionBlocks) {
        self.inner.recover_execution_blocks(blocks).await
    }

    fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> ExecutionBlocks {
        self.inner.get_blocks_by_range(start_block_number, end_block_number)
    }
}

pub struct BenchServer {
    kv_store: Arc<Mutex<Arc<InnerExecution>>>,
}

#[async_trait]
impl IServer for BenchServer {
    /// Starts the TCP server
    async fn start(&self, addr: &str) -> tokio::io::Result<()> {
        loop {
            let txn_num_in_block =
                std::env::var("BLOCK_TXN_NUMS").map(|s| s.parse().unwrap()).unwrap_or(1000);
            self.random_txns(txn_num_in_block).await.into_iter().for_each(|txn| {
                let store = self.kv_store.clone();
                info!("start new add txn");
                tokio::spawn(async move {
                    let _ = store.lock().await.add_txn(txn).await;
                });
            });
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await
        }
    }

    async fn execution_client(&self) -> Arc<dyn ExecutionApiV2> {
        self.kv_store.lock().await.clone()
    }
}

impl BenchServer {
    pub fn new() -> Self {
        Self {
            kv_store: Arc::new(Mutex::new(Arc::new(InnerExecution::new(Arc::new(KvStore::new()))))),
        }
    }

    async fn random_txns(&self, num: u64) -> Vec<ExecTxn> {
        let mut txns = Vec::with_capacity(num as usize);

        for i in 0..num {
            let key = format!("random_key_{}", i);
            let val = format!("random_value_{}", i);
            let raw_txn =
                RawTxn { account: generate_random_address(), sequence_number: i + 1, key, val };
            let exec_txn = ExecTxn::RawTxn(raw_txn.to_bytes());
            txns.push(exec_txn);
        }
        txns
    }
}
