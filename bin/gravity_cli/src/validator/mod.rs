mod contract;
mod join;
mod leave;
mod list;
mod util;

use clap::{Parser, Subcommand};

use crate::validator::{join::JoinCommand, leave::LeaveCommand, list::ListCommand};

#[derive(Debug, Parser)]
pub struct ValidatorCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    Join(JoinCommand),
    Leave(LeaveCommand),
    List(ListCommand),
    // TODO: other commands
}
