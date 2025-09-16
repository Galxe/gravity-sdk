use api::{check_bootstrap_config, consensus_api::{ConsensusEngine, ConsensusEngineArgs}, NodeConfig};
use clap::Parser;
use cli::Cli;
use flexi_logger::{detailed_format, FileSpec, Logger, WriteMode};
use gravity_sdk_kvstore::*;
use secp256k1::SecretKey;
use server::ServerApp;
use std::{error::Error, sync::Arc, thread};
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

/// **Note:** This code serves as a minimum viable implementation for demonstrating how to build a DApp using `gravity-sdk`.
/// It does not include account balance validation, comprehensive error handling, or robust runtime fault tolerance.
/// Current limitations and future tasks include:
/// 1. Block Synchronization: Block synchronization is not yet implemented.
/// A basic Recover API implementation is required for block synchronization functionality.
///
/// 2. State Persistence: The server does not load persisted state data on restart,
/// leading to state resets after each restart.
///
/// 3. Execution Pipeline: Although the execution layer pipeline is designed with
/// five stages, it currently executes blocks serially instead of in a pipelined manner.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let log_dir = cli.log_dir.clone();
    let _handle = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(log_dir))
        .write_mode(WriteMode::BufferAndFlush)
        .format(detailed_format)
        .start()
        .unwrap();
    let storage = Arc::new(SledStorage::new("blockchain_db")?);
    let genesis_path = cli.genesis_path.clone();
    let mut blockchain = Blockchain::new(storage.clone(), genesis_path);

    blockchain.run().await;
    let listen_url = cli.listen_url.clone();

    cli.run(move || {
        tokio::spawn(async move {
            let server = ServerApp::new(blockchain.state(), storage);
            let _ = thread::spawn(move || {
                let cl = TestConsensusLayer::new(gcei_config);
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });
            server.start(listen_url.as_str()).await.unwrap();
        })
    })
    .await;

    Ok(())
}
