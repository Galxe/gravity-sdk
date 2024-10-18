#![allow(missing_docs)]

mod cli;
mod reth_client;


use clap::Args;
use reth_provider::BlockReaderIdExt;
use std::thread;
use alloy_eips::{BlockId, BlockNumberOrTag};
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
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_node_builder::engine_tree_config;
use reth_node_builder::EngineNodeLauncher;
use reth_node_core::args::utils::DefaultChainSpecParser;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_primitives::B256;
use reth_provider::providers::BlockchainProvider2;
use reth_rpc_api::EngineEthApiClient;
use api::{check_bootstrap_config, ExecutionApi};
use crate::reth_client::RethCli;

struct TestConsensusLayer<T> {
    safe_hash: [u8; 32],
    head_hash: [u8; 32],
    reth_cli: RethCli<T>
}

impl<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> TestConsensusLayer<T> {
    fn new(reth_cli: RethCli<T>, safe_hash: B256, head_hash: B256) -> Self {
        let mut safe_slice = [0u8; 32];
        safe_slice.copy_from_slice(safe_hash.as_slice());
        let mut head_slice = [0u8; 32];
        head_slice.copy_from_slice(head_hash.as_slice());
        Self {
            safe_hash: safe_slice,
            head_hash: head_slice,
            reth_cli
        }
    }

    async fn run(mut self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let txns = self.reth_cli.request_transactions(self.safe_hash, self.head_hash).await;
            self.reth_cli.send_ordered_block(txns).await;
            let hash = self.reth_cli.recv_executed_block_hash().await;
            self.reth_cli.commit_block_hash(vec![hash]).await;
            self.safe_hash = hash;
            self.head_hash = hash;
        }
    }
}

fn run_server() {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = {
        let cli = Cli::<DefaultChainSpecParser, EngineArgs>::parse();
        // let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
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
            let client = handle.node.engine_http_client();
            let genesis = handle.node.chain_spec().genesis();
            let head_hash = handle.node.provider.block_by_id(BlockId::Number(BlockNumberOrTag::Latest)).unwrap().unwrap().hash_slow();
            let safe_hash = handle.node.provider.block_by_id(BlockId::Number(BlockNumberOrTag::Finalized)).unwrap().unwrap().hash_slow();
            let id = handle.node.chain_spec().chain().id();
            let _ = thread::spawn(move || {
                let mut cl =
                    TestConsensusLayer::new(RethCli::new(client, id), safe_hash, head_hash);
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(cl.run());
            });
            handle.node_exit_future.await
        })
    } {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn main() {
    run_server();
}
