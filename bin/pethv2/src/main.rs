use alloy_eips::BlockHashOrNumber;
use alloy_trie::HashMap;
use consensus::mock::MockConsensus;
use gravity_storage::block_view_storage::BlockViewStorage;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_cli_util;
use reth_coordinator::RethCoordinator;
use reth_node_builder;
use reth_node_core;
use reth_node_ethereum;
use reth_primitives::B256;
use reth_provider;
use reth_provider::BlockHashReader;
use reth_provider::BlockNumReader;
use reth_provider::BlockReader;
use tokio::sync::mpsc;
mod cli;
mod consensus;
mod exec_layer;
mod reth_cli;
mod reth_coordinator;

use crate::cli::Cli;
use clap::Args;
use reth_pipe_exec_layer_ext_v2;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use tracing::info;
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

const consensus_gensis: [u8; 32] = [47, 152, 5, 118, 113, 30, 54, 23, 165, 228, 216, 61, 213, 57, 84, 142, 192, 247, 121, 32, 7, 213, 5, 163, 210, 233, 103, 72, 51, 175, 45, 124];

fn run_reth(
    tx: mpsc::Sender<(AuthServerHandle, reth_pipe_exec_layer_ext_v2::PipeExecLayerApi)>,
    cli: Cli<DefaultChainSpecParser, EngineArgs>,
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
                let engine_cli = handle.node.auth_server_handle().clone();
                let provider = handle.node.provider;
                let latest_block_number = provider.last_block_number();
                let latest_block_hash =
                    provider.block_hash(latest_block_number.clone().unwrap()).unwrap().unwrap();
                let latest_block = provider
                    .block(BlockHashOrNumber::Number(latest_block_number.unwrap()))
                    .unwrap()
                    .unwrap();
                let mut block_number_to_id = BTreeMap::new();
                let genesis_id = B256::new(consensus_gensis);
                info!("genesis_id: {:?}", genesis_id);
                block_number_to_id.insert(0u64, genesis_id);
                let storage = BlockViewStorage::new(
                    provider,
                    latest_block.number,
                    latest_block_hash,
                    block_number_to_id,
                );
                let pipeline_api_v2 = reth_pipe_exec_layer_ext_v2::new_pipe_exec_layer_api(
                    chain_spec,
                    storage,
                    latest_block.header,
                    latest_block_hash,
                );
                tx.send((engine_cli, pipeline_api_v2)).await.ok();
                handle.node_exit_future.await
            }
        })
    } {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn main() {
    // 创建tokio通道
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let cli = Cli::<DefaultChainSpecParser, EngineArgs>::parse();
    // let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());

    // 启动consensus线程
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            // 等待engine_cli可用
            if let Some((engine_cli, pipe_api)) = rx.recv().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let client = RethCli::new("/tmp/reth.ipc", engine_cli, pipe_api).await;
                let genesis = client.get_latest_block_hash().await.unwrap();
                let coordinator = Arc::new(RethCoordinator::new(client));
                let cloned = coordinator.clone();
                info!("created reth_cli with ipc");
                tokio::spawn(async move {
                    cloned.run().await;
                });
                let c = MockConsensus::new(coordinator, genesis);
                c.run().await;
                // AptosConsensus::init(gcei_config, coordinator);
                tokio::signal::ctrl_c().await.unwrap();
            }
        });
    });

    run_reth(tx, cli);
}
