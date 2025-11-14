mod account;
mod key;
mod waypoint;

use clap::{Parser, Subcommand};

use crate::genesis::{account::GenerateAccount, key::GenerateKey, waypoint::GenerateWaypoint};

#[derive(Debug, Parser)]
pub struct GenesisCommand {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Subcommand, Debug)]
pub enum SubCommands {
    GenerateKey(GenerateKey),
    GenerateWaypoint(GenerateWaypoint),
    GenerateAccount(GenerateAccount),
}
