use api::GravityNodeArgs;
use build_info::{build_information, BUILD_PKG_VERSION};
use clap::{value_parser, Parser};
use greth::{
    reth::{chainspec::EthereumChainSpecParser, cli::Commands},
    reth_chainspec::ChainSpec,
    reth_cli::chainspec::ChainSpecParser,
    reth_cli_commands::{launcher::FnLauncher, node::NoArgs},
    reth_cli_runner::CliRunner,
    reth_db::DatabaseEnv,
    reth_node_builder::{NodeBuilder, WithLaunchContext},
    reth_node_core::args::LogArgs,
    reth_node_ethereum::{consensus::EthBeaconConsensus, EthEvmConfig, EthereumNode},
    reth_tracing::FileWorkerGuard,
};
use std::{
    collections::BTreeMap, ffi::OsString, fmt::{self}, sync::Arc
};
use tracing::debug;

static BUILD_INFO: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
static LONG_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn short_version() -> &'static str {
    BUILD_INFO.get_or_init(|| {
        let build_info = build_information!();
        build_info
    })
    .get(BUILD_PKG_VERSION).map(|s| s.as_str()).unwrap_or("unknown")
}

fn long_version() -> &'static str {
    LONG_VERSION.get_or_init(|| {
        let build_info = BUILD_INFO.get_or_init(|| {
            let build_info = build_information!();
            build_info
        });
        build_info
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<String>>()
            .join("\n")
    })
}

/// The main reth cli interface.
///
/// This is the entrypoint to the executable.
#[derive(Debug, Parser)]
#[command(author, about = "Gravity Node", long_about = None, version=short_version(), long_version=long_version())]
pub(crate) struct Cli<
    C: ChainSpecParser = EthereumChainSpecParser,
    Ext: clap::Args + fmt::Debug = NoArgs,
> {
    /// The command to run
    #[command(subcommand)]
    command: Commands<C, Ext>,

    /// The chain this node is running.
    ///
    /// Possible values are either a built-in chain or the path to a chain specification file.
    #[arg(
        long,
        value_name = "CHAIN_OR_PATH",
        long_help = C::help_message(),
        default_value = C::SUPPORTED_CHAINS[0],
        value_parser = C::parser(),
        global = true,
    )]
    chain: Arc<C::ChainSpec>,

    /// Add a new instance of a node.
    ///
    /// Configures the ports of the node to avoid conflicts with the defaults.
    /// This is useful for running multiple nodes on the same machine.
    ///
    /// Max number of instances is 200. It is chosen in a way so that it's not possible to have
    /// port numbers that conflict with each other.
    ///
    /// Changes to the following port numbers:
    /// - `DISCOVERY_PORT`: default + `instance` - 1
    /// - `AUTH_PORT`: default + `instance` * 100 - 100
    /// - `HTTP_RPC_PORT`: default - `instance` + 1
    /// - `WS_RPC_PORT`: default + `instance` * 2 - 2
    #[arg(long, value_name = "INSTANCE", global = true, default_value_t = 1, value_parser = value_parser!(u16).range(..=200))]
    instance: u16,

    #[command(flatten)]
    logs: LogArgs,

    #[command(flatten)]
    pub gravity_node_config: GravityNodeArgs,
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

impl<C: ChainSpecParser<ChainSpec = ChainSpec>, Ext: clap::Args + fmt::Debug> Cli<C, Ext> {
    /// Execute the configured cli command.
    ///
    /// This accepts a closure that is used to launch the node via the
    /// [`NodeCommand`](node::NodeCommand).
    ///
    ///
    /// # Example
    ///
    /// ```no_run
    /// use reth::cli::Cli;
    /// use reth_node_ethereum::EthereumNode;
    ///
    /// Cli::parse_args()
    ///     .run(|builder, _| async move {
    ///         let handle = builder.launch_node(EthereumNode::default()).await?;
    ///
    ///         handle.wait_for_node_exit().await
    ///     })
    ///     .unwrap();
    /// ```
    ///
    /// # Example
    ///
    /// Parse additional CLI arguments for the node command and use it to configure the node.
    ///
    /// ```no_run
    /// use clap::Parser;
    /// use reth::cli::Cli;
    /// use reth_ethereum_cli::chainspec::EthereumChainSpecParser;
    ///
    /// #[derive(Debug, Parser)]
    /// pub struct MyArgs {
    ///     pub enable: bool,
    /// }
    ///
    /// Cli::<EthereumChainSpecParser, MyArgs>::parse()
    ///     .run(|builder, my_args: MyArgs| async move {
    ///         // launch the node
    ///
    ///         Ok(())
    ///     })
    ///     .unwrap();
    /// ````
    pub(crate) fn run(
        mut self,
        launcher: impl AsyncFnOnce(
            WithLaunchContext<NodeBuilder<Arc<DatabaseEnv>, C::ChainSpec>>,
            Ext,
        ) -> eyre::Result<()>,
    ) -> eyre::Result<()> {
        // add network name to logs dir
        self.logs.log_file_directory =
            self.logs.log_file_directory.join(self.chain.chain.to_string());

        let _guard = self.init_tracing()?;
        debug!(target: "reth::cli", "Initialized tracing, log directory: {}, log level {:?}", self.logs.log_file_directory, self.logs.verbosity);

        let runner = CliRunner::try_default_runtime()?;
        let components = |spec: Arc<C::ChainSpec>| {
            (EthEvmConfig::ethereum(spec.clone()), EthBeaconConsensus::new(spec))
        };
        match self.command {
            Commands::Node(command) => {
                println!("Running node command, {:?}", command.dev);
                runner.run_command_until_exit(|ctx| {
                    command.execute(ctx, FnLauncher::new::<EthereumChainSpecParser, _>(launcher))
                })
            }
            Commands::Init(command) => {
                println!("Running init command");
                runner.run_blocking_until_ctrl_c(command.execute::<EthereumNode>())
            }
            Commands::InitState(command) => {
                runner.run_blocking_until_ctrl_c(command.execute::<EthereumNode>())
            }
            Commands::DumpGenesis(command) => runner.run_blocking_until_ctrl_c(command.execute()),
            Commands::Db(command) => {
                runner.run_blocking_until_ctrl_c(command.execute::<EthereumNode>())
            }
            Commands::P2P(command) => runner.run_until_ctrl_c(command.execute::<EthereumNode>()),
            #[cfg(feature = "dev")]
            Commands::TestVectors(command) => runner.run_until_ctrl_c(command.execute()),
            Commands::Config(command) => runner.run_until_ctrl_c(command.execute()),
            Commands::Prune(command) => runner.run_until_ctrl_c(command.execute::<EthereumNode>()),
            _ => todo!("not implemented"),
        }
    }

    /// Initializes tracing with the configured options.
    ///
    /// If file logging is enabled, this function returns a guard that must be kept alive to ensure
    /// that all logs are flushed to disk.
    pub fn init_tracing(&self) -> eyre::Result<Option<FileWorkerGuard>> {
        let guard = self.logs.init_tracing()?;
        Ok(guard)
    }
}
