mod key;
mod waypoint;

use clap::Subcommand;

use crate::genesis::key::GenerateKey;
use crate::genesis::waypoint::GenerateWaypoint;

#[derive(Subcommand, Debug)]
pub enum GenesisCommand {
    GenerateKey(GenerateKey),
    GenerateWaypoint(GenerateWaypoint),
}
