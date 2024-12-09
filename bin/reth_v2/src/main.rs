#![allow(missing_docs)]
#[cfg(feature = "grevm")]
use greth_node_builder as reth_node_builder;
#[cfg(feature = "grevm")]
use greth_node_ethereum as reth_node_ethereum;
#[cfg(feature = "grevm")]
use greth_node_core as reth_node_core;
#[cfg(feature = "grevm")]
use greth_provider as reth_provider;
#[cfg(feature = "grevm")]
use greth_cli_util as reth_cli_util;
#[cfg(feature = "grevm")]
use greth_ethereum_engine_primitives as reth_ethereum_engine_primitives;
#[cfg(feature = "grevm")]
use greth_rpc_api as reth_rpc_api;
mod cli;
mod reth_cli;
mod exec_layer;
mod reth_coordinator;

use crate::cli::Cli;
use alloy_eips::{BlockId, BlockNumberOrTag};
use api_types::BlockHashState;
use clap::Args;
use reth_provider::BlockReaderIdExt;
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
use crate::exec_layer::ExecLayer;
use crate::reth_cli::RethCli;

fn run_reth() {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = {
        let cli = Cli::<DefaultChainSpecParser, EngineArgs>::parse();
        cli.run(|builder, engine_args| async move {
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
            handle.node_exit_future.await
        })
    } {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn main() {
    thread::spawn(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let client = RethCli::new("/tmp/reth.ipc").await;
            info!("created reth_cli with ipc");
            let exec_layer = ExecLayer::new(client);
            exec_layer.run().await;
        });
    });
    run_reth();
}
