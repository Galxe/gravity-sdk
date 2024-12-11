mod cli;
mod kv;
mod stateful_mempool;
mod txn;

use std::{sync::Arc, thread};

use api::{check_bootstrap_config, consensus_api::ConsensusEngine, NodeConfig};
use api_types::{
    account::ExternalAccountAddress, BlockHashState, ConsensusApi, ExecTxn, ExecutionApiV2,
};
use clap::Parser;
use cli::Cli;
use flexi_logger::{FileSpec, Logger, WriteMode};
use kv::KvStore;
use log::info;
use rand::Rng;
use txn::RawTxn;

struct TestConsensusLayer {
    consensus_engine: Arc<dyn ConsensusApi>,
    execution_api: Arc<dyn ExecutionApiV2>,
}

impl TestConsensusLayer {
    fn new(node_config: NodeConfig, execution_client: Arc<dyn ExecutionApiV2>) -> Self {
        let safe_hash = [0u8; 32];
        let head_hash = [0u8; 32];
        let finalized_hash = [0u8; 32];
        let block_hash_state = BlockHashState { safe_hash, head_hash, finalized_hash };
        Self {
            consensus_engine: ConsensusEngine::init(
                node_config,
                execution_client.clone(),
                block_hash_state.clone(),
                1337,
            ),
            execution_api: execution_client,
        }
    }

    async fn random_txns(num: u64) -> Vec<ExecTxn> {
        let mut txns = Vec::with_capacity(num as usize);
        for i in 0..num {
            let key = format!("random_key_{}", i);
            let val = format!("random_value_{}", i);
            let raw_txn =
                RawTxn { account: generate_random_address(), sequence_number: 1, key, val };
            let exec_txn = ExecTxn::RawTxn(raw_txn.to_bytes());
            txns.push(exec_txn);
        }
        txns
    }

    async fn run(mut self) {
        loop {
            info!("start produce new txn");
            let txn_num_in_block =
                std::env::var("BLOCK_TXN_NUMS").map(|s| s.parse().unwrap()).unwrap_or(1000);
            TestConsensusLayer::random_txns(txn_num_in_block).await.into_iter().for_each(|txn| {
                let execution = self.execution_api.clone();
                tokio::spawn(async move {
                    let _ = execution.add_txn(txn).await;
                });
            });
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await
        }
    }
}

fn generate_random_address() -> ExternalAccountAddress {
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    ExternalAccountAddress::new(random_bytes)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(cli.log_dir.clone()))
        .write_mode(WriteMode::BufferAndFlush)
        .start()
        .unwrap();

    cli.run(move || {
        tokio::spawn(async move {
            let execution_api = Arc::new(KvStore::new());
            let execution = execution_api.clone();
            let _ = thread::spawn(move || {
                let cl = TestConsensusLayer::new(gcei_config, execution.clone());
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });

            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await
            }
        })
    })
    .await;
}
