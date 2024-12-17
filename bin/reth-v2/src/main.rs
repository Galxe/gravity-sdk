use mock_consensus::MockConsensus;
use reth::{consensus, rpc::builder::auth::AuthServerHandle};
use reth_cli_util as reth_cli_util;
use reth_coordinator::RethCoordinator;
use reth_ethereum_engine_primitives as reth_ethereum_engine_primitives;
use reth_node_builder as reth_node_builder;
use reth_node_core as reth_node_core;
use reth_node_ethereum as reth_node_ethereum;
use reth_provider as reth_provider;
use reth_rpc_api as reth_rpc_api;
use tokio::sync::mpsc;
mod cli;
mod exec_layer;
mod reth_cli;
mod reth_coordinator;
mod mock_consensus;

use crate::cli::Cli;
use alloy_eips::{BlockId, BlockNumberOrTag};
use api_types::BlockHashState;
use clap::Args;
use jsonrpsee::{
    core::RegisterMethodError,
    http_client::{transport::HttpBackend, HeaderMap},
    server::{AlreadyStoppedError, RpcModule},
    Methods,
};
use reth_provider::BlockReaderIdExt;
use reth_rpc_layer::{
    AuthClientLayer, AuthClientService, AuthLayer, JwtAuthValidator,
    JwtSecret,
};
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
use api::consensus_api::ConsensusEngine;
use api::{check_bootstrap_config, NodeConfig};
use api_types::ConsensusApi;
use clap::Parser;
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_node_builder::engine_tree_config;
use reth_node_builder::EngineNodeLauncher;
use reth_node_core::args::utils::DefaultChainSpecParser;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_provider::providers::BlockchainProvider2;
use reth_rpc_api::EngineEthApiClient;

fn run_reth(tx: mpsc::Sender<AuthServerHandle>)
{
    reth_cli_util::sigsegv_handler::install();

    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = {
        let cli = Cli::<DefaultChainSpecParser, EngineArgs>::parse();

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
                let engine_cli = handle.node.auth_server_handle().clone();
                tx.send(engine_cli).await.ok();
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

    // 启动consensus线程
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            // 等待engine_cli可用
            if let Some(engine_cli) = rx.recv().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let client = RethCli::new("/tmp/reth.ipc", engine_cli).await;
                info!("created reth_cli with ipc");
                let gensis = client.get_latest_block_hash().await.unwrap();
                let coordinator = Arc::new(RethCoordinator::new(client, gensis));
                let consensus = MockConsensus::new(coordinator.clone(), gensis);
                tokio::spawn(async move {
                    coordinator.run().await;
                });
                consensus.run().await;
            }
        });
    });

    run_reth(tx);
}
