mod cli;
mod kv;
mod server;
mod stateful_mempool;
mod txn;

use std::{sync::Arc, thread};

use api::{check_bootstrap_config, consensus_api::ConsensusEngine, NodeConfig};
use api_types::{BlockHashState, ConsensusApi, DefaultRecovery, ExecutionApiV2, ExecutionLayer};
use clap::Parser;
use cli::Cli;
use server::Server;
use flexi_logger::{FileSpec, Logger, WriteMode};

struct TestConsensusLayer {
    consensus_engine: Arc<dyn ConsensusApi>,
}

impl TestConsensusLayer {
    fn new(node_config: NodeConfig, execution_client: Arc<dyn ExecutionApiV2>) -> Self {
        let safe_hash = [0u8; 32];
        let head_hash = [0u8; 32];
        let finalized_hash = [0u8; 32];
        let block_hash_state = BlockHashState { safe_hash, head_hash, finalized_hash };
        let execution_layer = ExecutionLayer {
            execution_api: execution_client,
            recovery_api: Arc::new(DefaultRecovery{}),
        };
        Self {
            consensus_engine: ConsensusEngine::init(
                node_config,
                execution_layer,
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
    let listen_url = cli.listen_url.clone();
    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(cli.log_dir.clone()))
        .write_mode(WriteMode::BufferAndFlush)
        .start()
        .unwrap();

    cli.run(move || {
        tokio::spawn(async move {
            let server = Arc::new(Server::new());
            let execution_api = server.execution_client().await;
            let _ = thread::spawn(move || {
                let cl = TestConsensusLayer::new(gcei_config, execution_api);
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });

            server.start(&listen_url).await.unwrap();
        })
    })
    .await;
}
