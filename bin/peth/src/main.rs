use consensus::aptos::AptosConsensus;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_cli_util;
use reth_coordinator::RethCoordinator;
use reth_node_builder;
use reth_node_core;
use reth_node_ethereum;
use reth_provider;
use tokio::sync::mpsc;
mod cli;
mod consensus;
mod exec_layer;
mod reth_cli;
mod reth_coordinator;

use crate::cli::Cli;
use clap::Args;
use reth_pipe_exec_layer_ext;
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
use api::check_bootstrap_config;
use clap::Parser;
use reth_node_builder::engine_tree_config;
use reth_node_builder::EngineNodeLauncher;
use reth_node_core::args::utils::DefaultChainSpecParser;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_provider::providers::BlockchainProvider2;

fn run_reth(tx: mpsc::Sender<AuthServerHandle>, cli: Cli<DefaultChainSpecParser, EngineArgs>) {
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
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let pipeline_cli = reth_pipe_exec_layer_ext::new_pipe_exec_layer_api();
    let cli = Cli::<DefaultChainSpecParser, EngineArgs>::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let consensus_gensis: [u8; 32] = [
        0x43, 0xbf, 0x83, 0x6b, 0x97, 0x02, 0x74, 0x90, 0x9c, 0xe1, 0x89, 0xef, 0xf8, 0xf4, 0x2e,
        0xea, 0x6e, 0x53, 0x06, 0x04, 0xeb, 0x3a, 0x76, 0xae, 0xbd, 0x9a, 0x6c, 0xd6, 0x45, 0xa6,
        0xe7, 0x7e,
    ];

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Some(engine_cli) = rx.recv().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let client = RethCli::new("/tmp/reth.ipc", engine_cli, pipeline_cli).await;
                info!("created reth_cli with ipc");
                let gensis = client.get_latest_block_hash().await.unwrap();
                let coordinator = Arc::new(RethCoordinator::new(client, consensus_gensis, gensis));
                let cloned = coordinator.clone();
                tokio::spawn(async move {
                    cloned.run().await;
                });
                AptosConsensus::init(gcei_config, coordinator);

                // block until the runtime is shutdown
                tokio::signal::ctrl_c().await.unwrap();
            }
        });
    });

    run_reth(tx, cli);
}
