mod kv;
mod stateful_mempool;
mod txn;
mod cli;

use std::{sync::Arc, thread};

use api::{check_bootstrap_config, consensus_api::ConsensusEngine, NodeConfig};
use api_types::{BlockHashState, ConsensusApi, ExecutionApiV2};
use clap::Parser;
use cli::Cli;
use kv::KvStore;

struct TestConsensusLayer {
    consensus_engine: Arc<dyn ConsensusApi>,
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
                execution_client,
                block_hash_state.clone(),
                1337,
            ),
        }
    }

    async fn run(mut self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());

    // Run the server logic
    cli.run(|| {
        tokio::spawn(async move {
            let execution_client = Arc::new(KvStore::new());

            let _ = thread::spawn(move || {
                let mut cl = TestConsensusLayer::new(gcei_config, execution_client);
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            }
        })
    })
    .await;
}
