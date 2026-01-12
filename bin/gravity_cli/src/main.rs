pub mod command;
pub mod dkg;
pub mod genesis;
pub mod node;
pub mod validator;

use clap::Parser;
use command::{Command, Executable};

fn main() {
    let cmd = Command::parse();
    let result = match cmd.command {
        command::SubCommands::Genesis(genesis_cmd) => match genesis_cmd.command {
            // Example: gravity-cli genesis generate-key --output-file="./identity.yaml"
            genesis::SubCommands::GenerateKey(gck) => gck.execute(),
            // Example: gravity-cli genesis generate-waypoint
            // --input-file="./validator_genesis.json" --output-file="./waypoint.txt"
            genesis::SubCommands::GenerateWaypoint(gw) => gw.execute(),
            // Example: gravity-cli genesis generate-account --output-file="./account.yaml"
            genesis::SubCommands::GenerateAccount(generate_account) => generate_account.execute(),
        },
        command::SubCommands::Validator(validator_cmd) => match validator_cmd.command {
            // Example: gravity-cli validator join --rpc-url="http://127.0.0.1:8545" --contract-address="0x..." --private-key="0x..." --stake-amount="1000" --validator-address="0x..." --consensus-public-key="..." --validator-network-address="/ip4/127.0.0.1/tcp/6180/..." --fullnode-network-address="/ip4/127.0.0.1/tcp/6181/..." --aptos-address="..."
            validator::SubCommands::Join(join_cmd) => join_cmd.execute(),
            // Example: gravity-cli validator leave --rpc-url="http://127.0.0.1:8545" --contract-address="0x..." --private-key="0x..." --validator-address="0x..."
            validator::SubCommands::Leave(leave_cmd) => leave_cmd.execute(),
            // Example: gravity-cli validator list --rpc-url="http://127.0.0.1:8545"
            validator::SubCommands::List(list_cmd) => list_cmd.execute(),
        },
        command::SubCommands::Node(node_cmd) => match node_cmd.command {
            // Example: gravity-cli node start --deploy-path="./deploy_utils/node1"
            node::SubCommands::Start(start_cmd) => start_cmd.execute(),
            // Example: gravity-cli node stop --deploy-path="./deploy_utils/node1"
            node::SubCommands::Stop(stop_cmd) => stop_cmd.execute(),
        },
        command::SubCommands::Dkg(dkg_cmd) => match dkg_cmd.command {
            // Example: gravity-cli dkg status --server-url="127.0.0.1:1024"
            dkg::SubCommands::Status(status_cmd) => status_cmd.execute(),
            // Example: gravity-cli dkg randomness --server-url="127.0.0.1:1024" --block-number=100
            dkg::SubCommands::Randomness(randomness_cmd) => randomness_cmd.execute(),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}
