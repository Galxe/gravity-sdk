pub mod command;
pub mod genesis;

use clap::Parser;
use command::{Command, Executable};

fn main() {
    let cmd = Command::parse();
    match cmd.genesis {
 genesis::GenesisCommand::GenerateKey(gck) => {
            if let Err(e) = gck.execute() {
                eprintln!("Error: {:?}", e);
            }
        }
        genesis::GenesisCommand::GenerateWaypoint(gw) => {
            if let Err(e) = gw.execute() {
                eprintln!("Error: {:?}", e);
            }
        }
        genesis::GenesisCommand::GenerateAccount(generate_account) => {
            if let Err(e) = generate_account.execute() {
                eprintln!("Error: {:?}", e);
            }
        },
    }
}
