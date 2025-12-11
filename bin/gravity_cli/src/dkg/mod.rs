mod randomness;
mod status;

use clap::{Parser, Subcommand};

use crate::dkg::randomness::RandomnessCommand;
use crate::dkg::status::StatusCommand;

#[derive(Debug, Parser)]
pub struct DKGCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Debug, Subcommand)]
pub enum SubCommands {
    Status(StatusCommand),
    Randomness(RandomnessCommand),
}

