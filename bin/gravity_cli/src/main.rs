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
            // Example: gravity-cli genesis generate-key --output-file="./identity.yaml"
            genesis::SubCommands::GenerateKey(gck) => {
                if let Err(e) = gck.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            // Example: gravity-cli genesis generate-waypoint --input-file="./validator_genesis.json" --output-file="./waypoint.txt"
            genesis::SubCommands::GenerateWaypoint(gw) => {
                if let Err(e) = gw.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            // Example: gravity-cli genesis generate-account --output-file="./account.yaml"
            genesis::SubCommands::GenerateAccount(generate_account) => {
                if let Err(e) = generate_account.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        command::SubCommands::Validator(validator_cmd) => match validator_cmd.command {
            // Example: gravity-cli validator join --rpc-url="http://127.0.0.1:8545" --contract-address="0x..." --private-key="0x..." --stake-amount="1000" --validator-address="0x..." --consensus-public-key="..." --validator-network-address="/ip4/127.0.0.1/tcp/6180/..." --fullnode-network-address="/ip4/127.0.0.1/tcp/6181/..." --aptos-address="..."
            validator::SubCommands::Join(join_cmd) => {
                if let Err(e) = join_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            // Example: gravity-cli validator leave --rpc-url="http://127.0.0.1:8545" --contract-address="0x..." --private-key="0x..." --validator-address="0x..."
            validator::SubCommands::Leave(leave_cmd) => {
                if let Err(e) = leave_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        command::SubCommands::Node(node_cmd) => match node_cmd.command {
            // Example: gravity-cli node start --deploy-path="./deploy_utils/node1"
            node::SubCommands::Start(start_cmd) => {
                if let Err(e) = start_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            // Example: gravity-cli node stop --deploy-path="./deploy_utils/node1"
            node::SubCommands::Stop(stop_cmd) => {
                if let Err(e) = stop_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        command::SubCommands::Dkg(dkg_cmd) => match dkg_cmd.command {
            // Example: gravity-cli dkg status --server-url="127.0.0.1:1998"
            dkg::SubCommands::Status(status_cmd) => {
                if let Err(e) = status_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
            // Example: gravity-cli dkg randomness --server-url="127.0.0.1:1998" --block-number=100
            dkg::SubCommands::Randomness(randomness_cmd) => {
                if let Err(e) = randomness_cmd.execute() {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
    }
}
