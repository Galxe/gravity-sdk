#[cfg(feature = "grevm")]
use greth_primitives as reth_primitives;

#[cfg(feature = "preth")]
use reth_primitives as reth_primitives;
use alloy_primitives::Address;
use api_types::{BlockBatch, BlockHashState, ExecutionApi, ExecutionBlocks, GTxn};
use jsonrpsee::core::async_trait;
use rand::Rng;
use reth_primitives::{Block, ChainId};
use revm::primitives::{alloy_primitives::U160, TransactTo, TxEnv, U256};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::info;

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

pub(crate) struct MockCli {
    chain_id: ChainId,
    count_timer: Mutex<CounterTimer>,
}

const TRANSFER_GAS_LIMIT: u64 = 21_000;
// skip precompile address
const START_ADDRESS: usize = 1000;

fn get_account_idx(num_eoa: usize, hot_start_idx: usize, hot_ratio: f64) -> usize {
    if hot_ratio <= 0.0 {
        // Uniform workload
        rand::random::<usize>() % num_eoa
    } else if rand::thread_rng().gen_range(0.0..1.0) < hot_ratio {
        // Access hot
        hot_start_idx + rand::random::<usize>() % (num_eoa - hot_start_idx)
    } else {
        rand::random::<usize>() % (num_eoa - hot_start_idx)
    }
}

impl MockCli {
    pub(crate) fn new(chain_id: u64) -> Self {
        Self { chain_id, count_timer: Mutex::new(CounterTimer::new()) }
    }

    fn construct_reth_txn(&self) -> Vec<TxEnv> {
        let num_eoa = std::env::var("NUM_EOA").map(|s| s.parse().unwrap()).unwrap_or(1500);
        let hot_ratio = std::env::var("HOT_RATIO").map(|s| s.parse().unwrap()).unwrap_or(0.0);
        let hot_start_idx = START_ADDRESS + (num_eoa as f64 * 0.9) as usize;
        let txn_nums = std::env::var("BLOCK_TXN_NUMS").map(|s| s.parse().unwrap()).unwrap_or(1000);
        (0..txn_nums)
            .map(|_| {
                let from = Address::from(U160::from(
                    START_ADDRESS + get_account_idx(num_eoa, hot_start_idx, hot_ratio),
                ));
                let to = Address::from(U160::from(
                    START_ADDRESS + get_account_idx(num_eoa, hot_start_idx, hot_ratio),
                ));
                TxEnv {
                    caller: from,
                    transact_to: TransactTo::Call(to),
                    value: U256::from(1),
                    gas_limit: TRANSFER_GAS_LIMIT,
                    gas_price: U256::from(1),
                    chain_id: Some(114),
                    ..TxEnv::default()
                }
            })
            .collect::<Vec<_>>()
    }

    fn produce_gtxns(&self) -> Vec<GTxn> {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24;

        self.construct_reth_txn()
            .iter_mut()
            .map(|txn| GTxn {
                sequence_number: txn.nonce.map(|x| x as u64).unwrap_or(0),
                max_gas_amount: txn.gas_limit,
                gas_unit_price: txn.gas_price,
                expiration_timestamp_secs: secs,
                chain_id: txn.chain_id.unwrap(),
                txn_bytes: bincode::serialize(&txn.clone()).expect("failed to serialize"),
            })
            .collect::<Vec<_>>()
    }

    fn transform_to_reth_txn(gtxn: GTxn) -> TxEnv {
        bincode::deserialize::<TxEnv>(&gtxn.txn_bytes).expect("failed to deserialize")
    }
}

#[async_trait]
impl ExecutionApi for MockCli {
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch {
        BlockBatch { txns: self.produce_gtxns(), block_hash: [0; 32] }
    }

    async fn send_ordered_block(&self, txns: Vec<GTxn>) {
        let _reth_txns = txns
            .iter()
            .map(|gtxn| MockCli::transform_to_reth_txn(gtxn.clone()))
            .collect::<Vec<_>>();
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        [0; 32]
    }

    async fn commit_block_hash(&self, _block_ids: Vec<[u8; 32]>) {
        self.count_timer.lock().await.count();
    }

    fn latest_block_number(&self) -> u64 {
        0
    }
    
    fn finalized_block_number(&self) -> u64 {
        0
    }

    async fn recover_ordered_block(
        &self,
        block_batch: BlockBatch,
    ) {
        unimplemented!("No need for bench mode")
    }

    fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> ExecutionBlocks {
        unimplemented!("No need for bench mode")
    }

    async fn recover_execution_blocks(&self, block: ExecutionBlocks) {
        unimplemented!("No need for bench mode")
    }

}
