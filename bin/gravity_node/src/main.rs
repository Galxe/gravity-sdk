use greth::gravity_storage;
use greth::reth;
use greth::reth_cli_util;
use greth::reth_db;
use greth::reth_node_api;
use greth::reth_node_builder;
use greth::reth_node_core;
use greth::reth_node_ethereum;
use greth::reth_pipe_exec_layer_ext_v2;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use greth::reth_primitives;
use greth::reth_provider;
use greth::reth_transaction_pool;

use api::check_bootstrap_config;
use consensus::aptos::AptosConsensus;
use futures::StreamExt;
use gravity_storage::block_view_storage::BlockViewStorage;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_coordinator::RethCoordinator;
use reth_db::DatabaseEnv;
use reth_node_api::NodeTypesWithDBAdapter;
use reth_pipe_exec_layer_ext_v2::PipeExecLayerApi;
use reth_primitives::B256;
use reth_provider::BlockHashReader;
use reth_provider::BlockNumReader;
use reth_provider::BlockReader;
use reth_transaction_pool::TransactionPool;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::debug;
mod cli;
mod consensus;
mod exec_layer;
mod reth_cli;
mod reth_coordinator;

use crate::cli::Cli;
use clap::Args;
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;
use std::thread;
/// Parameters for configuring the engine
#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
#[command(next_help_heading = "Engine")]
pub struct EngineArgs {
    /// Enable the engine2 experimental features on reth binary
    #[arg(long = "engine.experimental", default_value = "false")]
    pub experimental: bool,
}

use crate::reth_cli::RethCli;
use clap::Parser;
use reth_node_builder::engine_tree_config;
use reth_node_builder::EngineNodeLauncher;
use reth_node_core::args::utils::DefaultChainSpecParser;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_provider::providers::BlockchainProvider2;

const consensus_gensis: [u8; 32] = [
    141, 91, 216, 66, 168, 139, 218, 32, 132, 186, 161, 251, 250, 51, 34, 197, 38, 71, 196, 135,
    49, 116, 247, 25, 67, 147, 163, 137, 28, 58, 62, 73,
];

struct ConsensusArgs {
    pub engine_api: AuthServerHandle,
    pub pipeline_api: PipeExecLayerApi,
    pub provider: BlockchainProvider2<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
    pub tx_listener: tokio::sync::mpsc::Receiver<reth::primitives::TxHash>,
    pub pool: reth_transaction_pool::Pool<
        reth_transaction_pool::TransactionValidationTaskExecutor<
            reth_transaction_pool::EthTransactionValidator<
                BlockchainProvider2<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
                reth_transaction_pool::EthPooledTransaction,
            >,
        >,
        reth_transaction_pool::CoinbaseTipOrdering<reth_transaction_pool::EthPooledTransaction>,
        reth_transaction_pool::blobstore::DiskFileBlobStore,
    >,
}

fn run_reth(
    tx: mpsc::Sender<ConsensusArgs>,
    cli: Cli<DefaultChainSpecParser, EngineArgs>,
    execution_args_rx: oneshot::Receiver<ExecutionArgs>,
) {
    reth_cli_util::sigsegv_handler::install();

    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = {
        cli.run(|builder, engine_args| {
            let tx = tx.clone();
            async move {
                let handle = builder
                    .with_types_and_provider::<EthereumNode, BlockchainProvider2<_>>()
                    .with_components(EthereumNode::components())
                    .with_add_ons::<EthereumAddOns>()
                    .launch_with_fn(|builder| {
                        let launcher = EngineNodeLauncher::new(
                            builder.task_executor().clone(),
                            builder.config().datadir(),
                            engine_tree_config::TreeConfig::default(),
                        );
                        builder.launch_with(launcher)
                    })
                    .await?;
                let chain_spec = handle.node.chain_spec();
                let pending_listener: tokio::sync::mpsc::Receiver<reth::primitives::TxHash> =
                    handle.node.pool.pending_transactions_listener();
                let engine_cli = handle.node.auth_server_handle().clone();
                let provider: BlockchainProvider2<
                    reth_node_api::NodeTypesWithDBAdapter<EthereumNode, Arc<reth_db::DatabaseEnv>>,
                > = handle.node.provider;
                let latest_block_number = provider.last_block_number();
                let latest_block_hash =
                    provider.block_hash(latest_block_number.clone().unwrap()).unwrap().unwrap();
                let latest_block = provider
                    .block(reth_primitives::BlockHashOrNumber::Number(latest_block_number.unwrap()))
                    .unwrap()
                    .unwrap();
                let pool: reth_transaction_pool::Pool<
                    reth_transaction_pool::TransactionValidationTaskExecutor<
                        reth_transaction_pool::EthTransactionValidator<
                            BlockchainProvider2<
                                NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>,
                            >,
                            reth_transaction_pool::EthPooledTransaction,
                        >,
                    >,
                    reth_transaction_pool::CoinbaseTipOrdering<
                        reth_transaction_pool::EthPooledTransaction,
                    >,
                    reth_transaction_pool::blobstore::DiskFileBlobStore,
                > = handle.node.pool;

                let storage = BlockViewStorage::new(
                    provider.clone(),
                    latest_block.number,
                    latest_block_hash,
                    BTreeMap::new(),
                );
                let pipeline_api_v2 = reth_pipe_exec_layer_ext_v2::new_pipe_exec_layer_api(
                    chain_spec,
                    storage,
                    latest_block.header,
                    latest_block_hash,
                    execution_args_rx,
                );
                let args = ConsensusArgs {
                    engine_api: engine_cli,
                    pipeline_api: pipeline_api_v2,
                    provider,
                    tx_listener: pending_listener,
                    pool,
                };
                tx.send(args).await.ok();
                handle.node_exit_future.await
            }
        })
    } {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let cli = Cli::<DefaultChainSpecParser, EngineArgs>::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let (execution_args_tx, execution_args_rx) = oneshot::channel();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Some(args) = rx.recv().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                let client = RethCli::new(args).await;
                let coordinator = Arc::new(RethCoordinator::new(client, execution_args_tx));
                let cloned = coordinator.clone();
                tokio::spawn(async move {
                    cloned.run().await;
                });
                // let c = MockConsensus::new(coordinator, genesis);
                // c.run().await;
                AptosConsensus::init(gcei_config, coordinator);
                tokio::signal::ctrl_c().await.unwrap();
            }
        });
    });
    run_reth(tx, cli, execution_args_rx);
}
