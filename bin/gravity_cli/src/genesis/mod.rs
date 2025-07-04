mod key;

use clap::Subcommand;

use crate::genesis::key::GenerateKey;


#[derive(Subcommand, Debug)]
pub enum GenesisCommand {
    GenerateKey(GenerateKey),
}
