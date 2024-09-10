use std::sync::Arc;

use reth_db::init_db;
use reth_db::mdbx::DatabaseArguments;
use reth_node_builder::{EngineNodeLauncher, NodeBuilder, NodeConfig};
use reth_node_ethereum::EthereumNode;
use reth_node_ethereum::node::EthereumAddOns;
use reth_provider::providers::BlockchainProvider2;
use reth_tasks::{TaskManager};

pub fn tokio_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()
}

#[tokio::main]
async fn main() {
    let node_config = NodeConfig::default();
    let data_dir = node_config.datadir();
    let db_path = data_dir.db();
    let tokio_runtime = tokio_runtime().unwrap();
    let task_manager = TaskManager::new(tokio_runtime.handle().clone());
    let task_executor = task_manager.executor();
    let db_args = DatabaseArguments::default();
    let database = Arc::new(init_db(db_path.clone(), db_args).unwrap().with_metrics());

    let builder = NodeBuilder::new(node_config)
        .with_database(database)
        .with_launch_context(task_executor);


    let handle = builder.launch_node(EthereumNode::default()).await.unwrap();
    handle.node_exit_future.await.unwrap();
    return;
}