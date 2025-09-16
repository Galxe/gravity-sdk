pub mod command;
pub mod genesis;

use clap::Parser;
use command::{Command, Executable};

fn main() {
    let cmd = Command::parse();
    match cmd.genesis {
        genesis::GenesisCommand::GenerateKey(gk) => {
            if let Err(e) = gk.execute() {
                eprintln!("Error: {:?}", e);
            }
        }
        genesis::GenesisCommand::GenerateWaypoint(gw) => {
            if let Err(e) = gw.execute() {
                eprintln!("Error: {:?}", e);
            }
        }
    }
}
