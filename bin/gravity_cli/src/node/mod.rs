mod pprof;
mod start;
mod stop;

use clap::{Parser, Subcommand};

pub use pprof::PprofCommand;
pub use pprof::PprofSubCommands;

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
    /// Collect runtime profiles from a node that was started with --pprof_addr
    Pprof(PprofCommand),
}
