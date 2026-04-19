mod common;
mod faucet;
mod reset;
mod start;
mod stop;

use clap::{Parser, Subcommand};

pub use faucet::FaucetCommand;
pub use reset::ResetCommand;
pub use start::StartCommand;
pub use stop::StopCommand;

#[derive(Debug, Parser)]
pub struct LocalnetCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    /// Start the local cluster (invokes cluster/start.sh)
    Start(StartCommand),
    /// Stop the local cluster (invokes cluster/stop.sh)
    Stop(StopCommand),
    /// Send a single transfer from the faucet key to an address
    Faucet(FaucetCommand),
    /// Stop the cluster and remove its base_dir (destructive)
    Reset(ResetCommand),
}
