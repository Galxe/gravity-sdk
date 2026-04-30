mod bootstrap;
pub mod config_storage;
pub mod consensus_api;
mod consensus_mempool_handler;
mod https;
mod logger;
mod network;

pub use bootstrap::check_bootstrap_config;
use clap::Parser;
pub use gaptos::aptos_config::config::NodeConfig;
use std::path::PathBuf;

/// Runs an Gravity validator or fullnode
#[derive(Clone, Debug, Parser)]
#[command(name = "Gravity Node", author, version)]
pub struct GravityNodeArgs {
    #[arg(long = "gravity_node_config", value_name = "CONFIG", global = true)]
    /// Path to node configuration file (or template for local test mode).
    pub node_config_path: Option<PathBuf>,

    #[arg(long = "relayer_config", value_name = "RELAYER_CONFIG", global = true)]
    /// Path to relayer configuration file (JSON format with URI to RPC URL mappings).
    pub relayer_config_path: Option<PathBuf>,

    #[arg(long = "pprof_addr", value_name = "ADDR", global = true)]
    /// Optional HTTP address (e.g. 127.0.0.1:6060) for an on-demand pprof
    /// server. When set, exposes `GET /debug/pprof/profile?seconds=N` which
    /// returns a protobuf-encoded CPU profile consumable by `go tool pprof`.
    /// Disables the periodic disk-dump mode activated by `ENABLE_PPROF=1`.
    pub pprof_addr: Option<String>,
}
