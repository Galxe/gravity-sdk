use crate::{
    dkg::DKGCommand, genesis::GenesisCommand, node::NodeCommand, validator::ValidatorCommand,
};
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
    Node(NodeCommand),
    Dkg(DKGCommand),
}

pub trait Executable {
    fn execute(self) -> Result<(), anyhow::Error>;
}
