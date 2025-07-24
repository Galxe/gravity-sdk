use clap::Parser;
use crate::genesis::GenesisCommand;

#[derive(Parser, Debug)]
#[command(name = "gravity-cli")]
pub struct Command {
    #[command(subcommand)]
    pub genesis: GenesisCommand,
}

pub trait Executable {
    fn execute(self) -> Result<(), anyhow::Error>;
}
