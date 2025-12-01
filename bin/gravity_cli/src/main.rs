pub mod command;
pub mod dkg;
pub mod genesis;
pub mod node;
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
            validator::SubCommands::Leave(leave_cmd) => {
                if let Err(e) = leave_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        command::SubCommands::Node(node_cmd) => match node_cmd.command {
            node::SubCommands::Start(start_cmd) => {
                if let Err(e) = start_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            node::SubCommands::Stop(stop_cmd) => {
                if let Err(e) = stop_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        command::SubCommands::Dkg(dkg_cmd) => match dkg_cmd.command {
            // Example: gravity-cli dkg status --server_url="127.0.0.1:1998"
            dkg::SubCommands::Status(status_cmd) => {
                if let Err(e) = status_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            // Example: gravity-cli dkg randomness --server_url="127.0.0.1:1998" --block_number=100
            dkg::SubCommands::Randomness(randomness_cmd) => {
                if let Err(e) = randomness_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
    }
}
