use api::GravityNodeArgs;
use clap::Parser;
use std::ffi::OsString;

/// This is the entrypoint to the executable.
#[derive(Debug, Parser)]
#[command(name = "KVStore", version, about = "For bench")]
pub(crate) struct Cli {
    #[command(flatten)]
    pub gravity_node_config: GravityNodeArgs,

    #[arg(long)]
    pub log_dir: String,

    #[arg(long)]
    pub leader: bool,
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
