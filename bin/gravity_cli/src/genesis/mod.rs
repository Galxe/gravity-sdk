mod consensus_key;
mod account;

use clap::Subcommand;

use crate::genesis::consensus_key::GenerateConsensusKey;
use crate::genesis::account::GenerateAccount;


#[derive(Subcommand, Debug)]
pub enum GenesisCommand {
    GenerateConsensusKey(GenerateConsensusKey),
    GenerateAccount(GenerateAccount),
}
