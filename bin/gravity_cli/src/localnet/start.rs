use alloy_provider::{Provider, ProviderBuilder};
use clap::Parser;
use colored::Colorize;
use std::{
    process::Command,
    time::{Duration, Instant},
};

use crate::{
    command::Executable,
    localnet::common::{derive_rpc_url, load_cluster_toml, resolve_cluster_dir, resolve_config},
};

#[derive(Debug, Parser)]
pub struct StartCommand {
    /// Directory containing the cluster scripts. Defaults to walking up from
    /// CWD looking for `cluster/start.sh`.
    #[clap(long, env = "GRAVITY_CLUSTER_DIR")]
    pub cluster_dir: Option<String>,

    /// Path to cluster.toml (defaults to <cluster_dir>/cluster.toml)
    #[clap(long)]
    pub config: Option<String>,

    /// Comma-separated list of node IDs to start (defaults to all)
    #[clap(long)]
    pub nodes: Option<String>,

    /// Poll eth_blockNumber until it advances past 0 (or timeout)
    #[clap(long)]
    pub wait_ready: bool,

    /// Timeout for --wait-ready in seconds
    #[clap(long, default_value = "60")]
    pub wait_timeout_secs: u64,
}

impl Executable for StartCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let cluster_dir = resolve_cluster_dir(self.cluster_dir.as_deref())?;
        let config = resolve_config(&cluster_dir, self.config.as_deref());
        let script = cluster_dir.join("start.sh");

        println!(
            "{} {}",
            "[localnet start]".cyan(),
            format!("running {} with config {}", script.display(), config.display())
        );

        let mut cmd = Command::new("bash");
        cmd.arg(&script)
            .arg("--config")
            .arg(&config)
            .current_dir(&cluster_dir);
        if let Some(ref nodes) = self.nodes {
            cmd.arg("--nodes").arg(nodes);
        }

        let status = cmd.status().map_err(|e| {
            anyhow::anyhow!("failed to execute {}: {e}", script.display())
        })?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "start.sh exited with code {}",
                status.code().unwrap_or(-1)
            ));
        }

        if self.wait_ready {
            let cfg = load_cluster_toml(&config)?;
            let rpc_url = derive_rpc_url(&cfg)?;
            wait_for_rpc_ready(&rpc_url, Duration::from_secs(self.wait_timeout_secs))?;
        }
        Ok(())
    }
}

fn wait_for_rpc_ready(rpc_url: &str, timeout: Duration) -> Result<(), anyhow::Error> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let url = rpc_url.parse().map_err(|e| anyhow::anyhow!("invalid rpc url: {e}"))?;
        let provider = ProviderBuilder::new().connect_http(url);
        let deadline = Instant::now() + timeout;
        println!(
            "{} {}",
            "[localnet start]".cyan(),
            format!("waiting for {rpc_url} (timeout {}s)…", timeout.as_secs())
        );
        loop {
            let err: String = match provider.get_block_number().await {
                Ok(n) if n > 0 => {
                    println!(
                        "{} {}",
                        "[localnet start]".cyan(),
                        format!("RPC ready — block={n}").green()
                    );
                    return Ok(());
                }
                Ok(_) => "block number still 0".into(),
                Err(e) => e.to_string(),
            };
            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "RPC not ready after {}s: {err}",
                    timeout.as_secs()
                ));
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}
