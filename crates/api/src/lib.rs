mod bootstrap;
mod consensus_mempool_handler;
mod logger;
mod mock_db;
mod network;
mod https;
mod utils;
pub mod consensus_api;
pub mod config_storage;

pub use gaptos::aptos_config::config::NodeConfig;
pub use bootstrap::check_bootstrap_config;
use clap::Parser;
use std::path::PathBuf;

/// Runs an Gravity validator or fullnode
#[derive(Clone, Debug, Parser)]
#[command(name = "Gravity Node", author, version)]
pub struct GravityNodeArgs {
    #[arg(long = "gravity_node_config", value_name = "CONFIG", global = true)]
    /// Path to node configuration file (or template for local test mode).
    pub node_config_path: Option<PathBuf>,
}
