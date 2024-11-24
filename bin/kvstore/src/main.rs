use std::sync::Arc;

use api::{check_bootstrap_config, consensus_api::ConsensusEngine, ExecutionApi, NodeConfig};
use api_types::{BlockHashState, ConsensusApi};
use clap::Parser;
use cli::Cli;
use kvstore::KvStore;
use mock_client::MockClient;
use server::Server;
use tokio::sync::Mutex;

mod batch_manager;
mod cli;
mod kvstore;
mod mock_client;
mod request;
pub mod server;

struct TestConsensusLayer {
    consensus_engine: Arc<dyn ConsensusApi>,
}

impl TestConsensusLayer {
    fn new(node_config: NodeConfig, execution_client: Arc<dyn ExecutionApi>) -> Self {
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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let db_path = cli.db_path.clone();
    let path = std::path::Path::new(&db_path);
    let mut options = leveldb::options::Options::new();
    options.create_if_missing = true;

    let db = Arc::new(Mutex::new(leveldb::database::Database::open(path, options).unwrap()));
    let listen_url = cli.listen_url.clone();
    // Run the server logic
    cli.run(|| {
        tokio::spawn(async move {
            // Create the KvStore which includes a BatchManager
            let kv_store = Arc::new(KvStore::new(db, 1337));
            let execution_client = Arc::new(MockClient::new(kv_store.clone()));
            let consensus = TestConsensusLayer::new(gcei_config, execution_client);
            let server = Server::new(kv_store);
            // Start the server
            server.start(&listen_url).await.unwrap();
        })
    })
    .await;
}
