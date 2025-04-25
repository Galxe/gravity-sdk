use alloy_eips::BlockHashOrNumber;
use alloy_primitives::TxHash;
use block_buffer_manager::block_buffer_manager::BlockBufferManager;
use block_buffer_manager::get_block_buffer_manager;
use consensus::mock_consensus::mock::MockConsensus;
use greth::gravity_storage;
use greth::reth;
use greth::reth::chainspec::EthereumChainSpecParser;
use greth::reth_cli_util;
use greth::reth_db;
use greth::reth_node_api;
use greth::reth_node_builder;
use greth::reth_node_ethereum;
use greth::reth_pipe_exec_layer_ext_v2;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use greth::reth_provider;
use greth::reth_transaction_pool;

use api::check_bootstrap_config;
use consensus::aptos::AptosConsensus;
use gravity_storage::block_view_storage::BlockViewStorage;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_coordinator::RethCoordinator;
use reth_db::DatabaseEnv;
use reth_node_api::NodeTypesWithDBAdapter;
use reth_pipe_exec_layer_ext_v2::PipeExecLayerApi;
use reth_provider::BlockHashReader;
use reth_provider::BlockNumReader;
use reth_provider::BlockReader;
use reth_transaction_pool::TransactionPool;
use tikv_jemalloc_ctl::raw;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::info;
mod cli;
mod consensus;
mod reth_cli;
mod reth_coordinator;

use crate::cli::Cli;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread;

use crate::reth_cli::RethCli;
use clap::Parser;
use reth_node_builder::engine_tree_config;
use reth_node_builder::EngineNodeLauncher;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_provider::providers::BlockchainProvider;

struct ConsensusArgs {
    pub engine_api: AuthServerHandle,
    pub pipeline_api: PipeExecLayerApi<BlockViewStorage<BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>>>,
    pub provider: BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
    pub tx_listener: tokio::sync::mpsc::Receiver<TxHash>,
    pub pool: reth_transaction_pool::Pool<
        reth_transaction_pool::TransactionValidationTaskExecutor<
            reth_transaction_pool::EthTransactionValidator<
                BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
                reth_transaction_pool::EthPooledTransaction,
            >,
        >,
        reth_transaction_pool::CoinbaseTipOrdering<reth_transaction_pool::EthPooledTransaction>,
        reth_transaction_pool::blobstore::DiskFileBlobStore,
    >,
}

fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let (execution_args_tx, execution_args_rx) = oneshot::channel();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Some((args, latest_block_number)) = rx.recv().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                let client = RethCli::new(args).await;
                let chain_id = client.chain_id();
                let coordinator =
                    Arc::new(RethCoordinator::new(client, latest_block_number, execution_args_tx));
                if std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap() {
                    let mock = MockConsensus::new().await;
                    tokio::spawn(async move {
                        mock.run().await;
                    });
                } else {
                    AptosConsensus::init(gcei_config, coordinator.clone(), chain_id, latest_block_number).await;
                }
                coordinator.send_execution_args().await;
                coordinator.run().await;
                tokio::signal::ctrl_c().await.unwrap();
            }
        });
    });
    run_reth(tx, cli, execution_args_rx);
}
