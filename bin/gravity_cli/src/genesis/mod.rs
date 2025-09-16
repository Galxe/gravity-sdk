mod key;
mod waypoint;
mod account;

use clap::Subcommand;

use crate::genesis::key::GenerateKey;
use crate::genesis::waypoint::GenerateWaypoint;
use crate::genesis::account::GenerateAccount;

#[derive(Subcommand, Debug)]
pub enum GenesisCommand {
    GenerateKey(GenerateKey),
    GenerateWaypoint(GenerateWaypoint),
    GenerateAccount(GenerateAccount),
}