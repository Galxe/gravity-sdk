mod cli;
mod kv;
mod stateful_mempool;
mod txn;

use std::{sync::Arc, thread};

use api::{check_bootstrap_config, consensus_api::{ConsensusEngine, ConsensusEngineArgs}, NodeConfig};
use gaptos::api_types::{
    account::ExternalAccountAddress, ExecTxn
};
use clap::Parser;
use cli::Cli;
use flexi_logger::{FileSpec, Logger, WriteMode};
use kv::KvStore;
use log::info;
use once_cell::sync::OnceCell;
use rand::Rng;
use tokio::sync::RwLock;
use txn::RawTxn;
use warp::Filter;

struct TestConsensusLayer {
    consensus_engine: Arc<ConsensusEngine>,
}

impl TestConsensusLayer {
    async fn new(node_config: NodeConfig) -> Self {
        Self {
            consensus_engine: ConsensusEngine::init(ConsensusEngineArgs {
                node_config,
                chain_id: 1337,
                latest_block_number: 0,
            }).await,
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

    async fn run(self) {
        loop {
            if should_produce_txn().await {
                info!("start produce new txn");
                let txn_num_in_block =
                    std::env::var("BLOCK_TXN_NUMS").map(|s| s.parse().unwrap()).unwrap_or(1000);
                TestConsensusLayer::random_txns(txn_num_in_block).await.into_iter().for_each(
                    |txn| {
                        tokio::spawn(async move {
                            todo!()
                        });
                    },
                );
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await
        }
    }
}

static IS_LEADER: OnceCell<bool> = OnceCell::new();
static PRODUCE_TXN: OnceCell<RwLock<bool>> = OnceCell::new();

#[derive(Debug, serde::Deserialize)]
struct ProduceTxnQuery {
    value: bool,
}

async fn handle_produce_txn(query: ProduceTxnQuery) -> Result<impl warp::Reply, warp::Rejection> {
    let global_flag = PRODUCE_TXN.get().expect("PRODUCE_TXN is not initialized");
    {
        let mut flag = global_flag.write().await;
        *flag = query.value;
    }
    Ok(format!("ProduceTxn updated to: {}", query.value))
}

async fn get_produce_txn() -> bool {
    let global_flag = PRODUCE_TXN.get().expect("PRODUCE_TXN is not initialized");
    let flag = global_flag.read().await;
    *flag
}

pub async fn should_produce_txn() -> bool {
    *IS_LEADER.get().expect("No is leader set") && get_produce_txn().await
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
    let is_leader = cli.leader;
    if is_leader && cli.port.is_none() {
        panic!("Please also set port when enable leader");
    }
    IS_LEADER.set(cli.leader).expect("Failed to set is leader");
    PRODUCE_TXN.set(RwLock::new(true)).expect("Failed to initialize PRODUCE_TXN");
    let port = cli.port.clone();

    cli.run(move || {
        tokio::spawn(async move {
            let execution_api = Arc::new(KvStore::new());
            let execution = execution_api.clone();
            let _ = thread::spawn(move || {
                tokio::runtime::Runtime::new().unwrap().block_on(async move {
                    let cl = TestConsensusLayer::new(gcei_config).await;
                    cl.run()
                });
            });

            if *IS_LEADER.get().expect("No is leader") {
                let route = warp::path!("ProduceTxn")
                    .and(warp::query::<ProduceTxnQuery>())
                    .and_then(handle_produce_txn);

                warp::serve(route).run(([0, 0, 0, 0], port.expect("No port"))).await;
            } else {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                }
            }
        })
    })
    .await;
}
