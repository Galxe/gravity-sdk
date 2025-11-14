mod join;

use clap::{Parser, Subcommand};

use crate::validator::join::JoinCommand;

#[derive(Debug, Parser)]
pub struct ValidatorCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    Join(JoinCommand),
    // TODO: other commands
}
