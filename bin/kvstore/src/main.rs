mod cli;
mod kv;
mod server;
mod stateful_mempool;
mod txn;

use std::{sync::Arc, thread};

use api::{check_bootstrap_config, consensus_api::{ConsensusEngine, ConsensusEngineArgs}, NodeConfig};
use clap::Parser;
use cli::Cli;
use flexi_logger::{FileSpec, Logger, WriteMode};
use server::Server;
use block_buffer_manager::block_buffer_manager::EmptyTxPool;

struct TestConsensusLayer {
    node_config: NodeConfig,
}

impl TestConsensusLayer {
    fn new(node_config: NodeConfig) -> Self {
        Self { node_config }
    }

    async fn run(self) {
        let _consensus_engine = ConsensusEngine::init(ConsensusEngineArgs {
            node_config: self.node_config,
            chain_id: 1337,
            latest_block_number: 0,
            config_storage: None,
        }, EmptyTxPool::new()).await;
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
            let _ = thread::spawn(move || {
                let cl = TestConsensusLayer::new(gcei_config);
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });

            server.start(&listen_url).await.unwrap();
        })
    })
    .await;
}
