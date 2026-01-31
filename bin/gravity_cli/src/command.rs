use crate::{
    dkg::DKGCommand, genesis::GenesisCommand, node::NodeCommand, validator::ValidatorCommand,
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
    #[command(subcommand)]
    pub command: SubCommands,
}

#[derive(Subcommand, Debug)]
pub enum SubCommands {
    Genesis(GenesisCommand),
    Validator(ValidatorCommand),
    Node(NodeCommand),
    Dkg(DKGCommand),
}

pub trait Executable {
    fn execute(self) -> Result<(), anyhow::Error>;
}
