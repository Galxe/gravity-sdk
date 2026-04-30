mod mempool;
mod metrics;
mod peers;
mod start;
mod stop;

use clap::{Parser, Subcommand};

pub use mempool::MempoolCommand;
pub use metrics::MetricsCommand;
pub use peers::PeersCommand;

use crate::node::{start::StartCommand, stop::StopCommand};

#[derive(Debug, Parser)]
pub struct NodeCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    Start(StartCommand),
    Stop(StopCommand),
    /// List peers connected to the execution layer via admin_peers
    Peers(PeersCommand),
    /// Show txpool status (and optionally content) via txpool_* RPC
    Mempool(MempoolCommand),
    /// Aggregate node health metrics (chain_id, block, peers, mempool, sync)
    Metrics(MetricsCommand),
}
