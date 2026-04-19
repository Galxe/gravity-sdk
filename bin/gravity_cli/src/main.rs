pub mod command;
pub mod completions;
pub mod config;
pub mod contract;
pub mod dkg;
pub mod epoch;
pub mod errors;
pub mod genesis;
pub mod init;
pub mod localnet;
pub mod node;
pub mod output;
pub mod stake;
pub mod status;
pub mod unwind;
pub mod util;
pub mod validator;

use clap::Parser;
use colored::Colorize;
use command::{Command, Executable};
use config::GravityConfig;

fn main() {
    let mut cmd = Command::parse();

    // Load config and resolve profile
    let config = match GravityConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} Failed to load config: {e}", "warning:".yellow().bold());
            None
        }
    };
    let profile = config.as_ref().and_then(|c| c.active_profile(cmd.profile.as_deref()).cloned());
    let output_format = cmd.output;

    // Inject config defaults into subcommands
    apply_config_defaults(&mut cmd, &profile);

    let result = match cmd.command {
        command::SubCommands::Genesis(genesis_cmd) => match genesis_cmd.command {
            genesis::SubCommands::GenerateKey(gck) => gck.execute(),
            genesis::SubCommands::GenerateWaypoint(gw) => gw.execute(),
            genesis::SubCommands::GenerateAccount(generate_account) => generate_account.execute(),
        },
        command::SubCommands::Validator(validator_cmd) => match validator_cmd.command {
            validator::SubCommands::Join(join_cmd) => join_cmd.execute(),
            validator::SubCommands::Leave(leave_cmd) => leave_cmd.execute(),
            validator::SubCommands::List(mut list_cmd) => {
                list_cmd.output_format = output_format;
                list_cmd.execute()
            }
        },
        command::SubCommands::Stake(stake_cmd) => match stake_cmd.command {
            stake::SubCommands::Create(mut create_cmd) => {
                create_cmd.output_format = output_format;
                create_cmd.execute()
            }
            stake::SubCommands::Get(mut get_cmd) => {
                get_cmd.output_format = output_format;
                get_cmd.execute()
            }
        },
        command::SubCommands::Node(node_cmd) => match node_cmd.command {
            node::SubCommands::Start(start_cmd) => start_cmd.execute(),
            node::SubCommands::Stop(stop_cmd) => stop_cmd.execute(),
        },
        command::SubCommands::Dkg(dkg_cmd) => match dkg_cmd.command {
            dkg::SubCommands::Status(mut status_cmd) => {
                status_cmd.output_format = output_format;
                status_cmd.execute()
            }
            dkg::SubCommands::Randomness(randomness_cmd) => randomness_cmd.execute(),
        },
        command::SubCommands::Unwind(unwind_cmd) => unwind_cmd.execute(),
        command::SubCommands::Epoch(epoch_cmd) => match epoch_cmd.command {
            epoch::SubCommands::Status(mut status_cmd) => {
                status_cmd.output_format = output_format;
                status_cmd.execute()
            }
        },
        command::SubCommands::Status(mut status_cmd) => {
            status_cmd.output_format = output_format;
            status_cmd.execute()
        }
        command::SubCommands::Completions(completions_cmd) => completions_cmd.execute(),
        command::SubCommands::Init(init_cmd) => init_cmd.execute(),
        command::SubCommands::Localnet(ln_cmd) => match ln_cmd.command {
            localnet::SubCommands::Start(c) => c.execute(),
            localnet::SubCommands::Stop(c) => c.execute(),
            localnet::SubCommands::Faucet(mut c) => {
                c.output_format = output_format;
                c.execute()
            }
            localnet::SubCommands::Reset(c) => c.execute(),
        },
    };

    if let Err(e) = result {
        eprintln!("{} {e}", "error:".red().bold());
        for cause in e.chain().skip(1) {
            eprintln!("  {} {cause}", "caused by:".yellow());
        }
        if let Some(hint) = errors::suggest_fix(&e) {
            eprintln!("\n{} {hint}", "hint:".cyan().bold());
        }
        std::process::exit(1);
    }
}

/// Apply config profile defaults to command fields that are still None after CLI/env parsing.
fn apply_config_defaults(cmd: &mut Command, profile: &Option<config::ProfileConfig>) {
    let Some(profile) = profile else { return };

    match &mut cmd.command {
        command::SubCommands::Validator(ref mut v) => match &mut v.command {
            validator::SubCommands::Join(ref mut c) => {
                if c.rpc_url.is_none() {
                    c.rpc_url.clone_from(&profile.rpc_url);
                }
                if c.gas_limit.is_none() {
                    c.gas_limit = profile.gas_limit;
                }
                if c.gas_price.is_none() {
                    c.gas_price = profile.gas_price;
                }
            }
            validator::SubCommands::Leave(ref mut c) => {
                if c.rpc_url.is_none() {
                    c.rpc_url.clone_from(&profile.rpc_url);
                }
                if c.gas_limit.is_none() {
                    c.gas_limit = profile.gas_limit;
                }
                if c.gas_price.is_none() {
                    c.gas_price = profile.gas_price;
                }
            }
            validator::SubCommands::List(ref mut c) => {
                if c.rpc_url.is_none() {
                    c.rpc_url.clone_from(&profile.rpc_url);
                }
            }
        },
        command::SubCommands::Stake(ref mut s) => match &mut s.command {
            stake::SubCommands::Create(ref mut c) => {
                if c.rpc_url.is_none() {
                    c.rpc_url.clone_from(&profile.rpc_url);
                }
                if c.gas_limit.is_none() {
                    c.gas_limit = profile.gas_limit;
                }
                if c.gas_price.is_none() {
                    c.gas_price = profile.gas_price;
                }
            }
            stake::SubCommands::Get(ref mut c) => {
                if c.rpc_url.is_none() {
                    c.rpc_url.clone_from(&profile.rpc_url);
                }
            }
        },
        command::SubCommands::Node(ref mut n) => match &mut n.command {
            node::SubCommands::Start(ref mut c) => {
                if c.deploy_path.is_none() {
                    c.deploy_path.clone_from(&profile.deploy_path);
                }
            }
            node::SubCommands::Stop(ref mut c) => {
                if c.deploy_path.is_none() {
                    c.deploy_path.clone_from(&profile.deploy_path);
                }
            }
        },
        command::SubCommands::Dkg(ref mut d) => match &mut d.command {
            dkg::SubCommands::Status(ref mut c) => {
                if c.server_url.is_none() {
                    c.server_url.clone_from(&profile.server_url);
                }
            }
            dkg::SubCommands::Randomness(ref mut c) => {
                if c.server_url.is_none() {
                    c.server_url.clone_from(&profile.server_url);
                }
            }
        },
        command::SubCommands::Epoch(ref mut ep) => match &mut ep.command {
            epoch::SubCommands::Status(ref mut c) => {
                if c.rpc_url.is_none() {
                    c.rpc_url.clone_from(&profile.rpc_url);
                }
            }
        },
        command::SubCommands::Status(ref mut c) => {
            if c.rpc_url.is_none() {
                c.rpc_url.clone_from(&profile.rpc_url);
            }
            if c.server_url.is_none() {
                c.server_url.clone_from(&profile.server_url);
            }
        }
        // Genesis, Unwind, Completions, Init don't use profile config
        _ => {}
    }
}
