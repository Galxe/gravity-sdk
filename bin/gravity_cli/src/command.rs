use crate::{
    completions::CompletionsCommand, dkg::DKGCommand, epoch::EpochCommand, genesis::GenesisCommand,
    init::InitCommand, node::NodeCommand, output::OutputFormat, stake::StakeCommand,
    status::StatusCommand, tx::TxCommand, unwind::UnwindCommand, validator::ValidatorCommand,
};
use build_info::{build_information, BUILD_PKG_VERSION};
use clap::{Parser, Subcommand};
use std::collections::BTreeMap;

static BUILD_INFO: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
static LONG_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn short_version() -> &'static str {
    BUILD_INFO
        .get_or_init(|| {
            let build_info = build_information!();
            build_info
        })
        .get(BUILD_PKG_VERSION)
        .map(|s| s.as_str())
        .unwrap_or("unknown")
}

fn long_version() -> &'static str {
    LONG_VERSION.get_or_init(|| {
        let build_info = BUILD_INFO.get_or_init(|| {
            let build_info = build_information!();
            build_info
        });
        build_info.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<String>>().join("\n")
    })
}

#[derive(Parser, Debug)]
#[command(name = "gravity-cli", version = short_version(), long_version = long_version())]
pub struct Command {
    /// Config profile to use (overrides active_profile in config.toml)
    #[clap(long, global = true, env = "GRAVITY_PROFILE")]
    pub profile: Option<String>,

    /// Output format for query commands
    #[clap(long, global = true, value_enum, default_value = "plain", env = "GRAVITY_OUTPUT")]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Subcommand, Debug)]
pub enum SubCommands {
    /// Genesis setup and key management
    Genesis(GenesisCommand),
    /// Validator lifecycle management
    Validator(ValidatorCommand),
    /// Stake pool operations
    Stake(StakeCommand),
    /// Node lifecycle management
    Node(NodeCommand),
    /// Distributed key generation queries
    Dkg(DKGCommand),
    /// Unwind consensus state to a specific block number
    Unwind(UnwindCommand),
    /// Epoch management
    Epoch(EpochCommand),
    /// Show combined node status (epoch, validators, DKG)
    Status(StatusCommand),
    /// Generate shell completions
    Completions(CompletionsCommand),
    /// Initialize configuration interactively
    Init(InitCommand),
    /// Transaction simulation and tracing helpers
    Tx(TxCommand),
}

pub trait Executable {
    fn execute(self) -> Result<(), anyhow::Error>;
}
