mod start;
mod stop;

use clap::{Parser, Subcommand};

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
}
