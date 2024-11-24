use api::GravityNodeArgs;
use clap::Parser;
use std::{
    ffi::OsString,
    future::Future,
};

use crate::server::Server;


/// This is the entrypoint to the executable.
#[derive(Debug, Parser)]
#[command(name = "KVStore", version, about = "An example of running gravity-sdk")]
pub(crate) struct Cli {
    // /// The command to run
    // #[command(subcommand)]
    // command: Commands<C, Ext>,

    #[command(flatten)]
    pub gravity_node_config: GravityNodeArgs,

    /// Path to the configuration file
    #[arg(long)]
    pub listen_url: String,

    /// Path to the configuration file
    #[arg(long)]
    pub db_path: String,
}

impl Cli {
    /// Parsers only the default CLI arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Parsers only the default CLI arguments from the given iterator
    pub fn try_parse_args_from<I, T>(itr: I) -> Result<Self, clap::error::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        Self::try_parse_from(itr)
    }
}

impl Cli {
    pub async fn run<F>(self, server_logic: F)
    where
        F: FnOnce() -> tokio::task::JoinHandle<()> + Send + 'static,
    {
        server_logic().await.unwrap();
    }
}
