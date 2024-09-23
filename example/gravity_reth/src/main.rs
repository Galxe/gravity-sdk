#![allow(missing_docs)]
// mod engine_api_adaptor;
mod cli;
mod mock_eth_consensus_layer;
mod gcei_sender;
use std::thread;
/// clap [Args] for Engine related arguments.
use clap::Args;

use crate::cli::Cli;
/// Parameters for configuring the engine
#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
#[command(next_help_heading = "Engine")]
pub struct EngineArgs {
    /// Enable the engine2 experimental features on reth binary
    #[arg(long = "engine.experimental", default_value = "false")]
    pub experimental: bool,
}

use clap::Parser;
use reth_node_builder::engine_tree_config;
use reth_node_core::args::utils::DefaultChainSpecParser;
use reth_node_builder::EngineNodeLauncher;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_provider::providers::BlockchainProvider2;

fn run_server() {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) =
        Cli::<DefaultChainSpecParser, EngineArgs>::parse().run(|builder, engine_args| async move {
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
            let client = handle.node.engine_http_client();
            let genesis_hash = handle.node.chain_spec().genesis_hash();
            let _ = thread::spawn(
                move || {
                    let mock_eth_consensus_layer = mock_eth_consensus_layer::MockEthConsensusLayer::new(client);

                    tokio::runtime::Runtime::new().unwrap().block_on(mock_eth_consensus_layer.start_round(genesis_hash)).expect("Failed to run round");
                }
            );
            handle.node_exit_future.await
        })
    {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn main() {
    run_server();
}