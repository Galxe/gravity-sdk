mod contract;
mod join;
mod leave;
mod util;

use clap::{Parser, Subcommand};

use crate::validator::join::JoinCommand;
use crate::validator::leave::LeaveCommand;

#[derive(Debug, Parser)]
pub struct ValidatorCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    Join(JoinCommand),
    Leave(LeaveCommand),
    // TODO: other commands
}
