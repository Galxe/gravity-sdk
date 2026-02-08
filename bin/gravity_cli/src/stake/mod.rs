mod create;
mod get;

use clap::{Parser, Subcommand};

use crate::stake::{create::CreateCommand, get::GetCommand};

#[derive(Debug, Parser)]
pub struct StakeCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    /// Create a new StakePool
    Create(CreateCommand),
    /// Query StakePools by owner address
    Get(GetCommand),
}
