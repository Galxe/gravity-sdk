use crate::{genesis::GenesisCommand, validator::ValidatorCommand};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "gravity-cli")]
pub struct Command {
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Subcommand, Debug)]
pub enum SubCommands {
    Genesis(GenesisCommand),
    Validator(ValidatorCommand),
}

pub trait Executable {
    fn execute(self) -> Result<(), anyhow::Error>;
}
