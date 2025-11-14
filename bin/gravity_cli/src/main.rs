pub mod command;
pub mod genesis;
pub mod validator;

use clap::Parser;
use command::{Command, Executable};

fn main() {
    let cmd = Command::parse();
    match cmd.command {
        command::SubCommands::Genesis(genesis_cmd) => match genesis_cmd.command {
            genesis::SubCommands::GenerateKey(gck) => {
                if let Err(e) = gck.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            genesis::SubCommands::GenerateWaypoint(gw) => {
                if let Err(e) = gw.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            genesis::SubCommands::GenerateAccount(generate_account) => {
                if let Err(e) = generate_account.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        command::SubCommands::Validator(validator_cmd) => match validator_cmd.command {
            validator::SubCommands::Join(join_cmd) => {
                if let Err(e) = join_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
    }
}
