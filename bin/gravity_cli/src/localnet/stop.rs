use clap::Parser;
use colored::Colorize;
use std::process::Command;

use crate::{
    command::Executable,
    localnet::common::{resolve_cluster_dir, resolve_config},
};

#[derive(Debug, Parser)]
pub struct StopCommand {
    /// Directory containing the cluster scripts
    #[clap(long, env = "GRAVITY_CLUSTER_DIR")]
    pub cluster_dir: Option<String>,

    /// Path to cluster.toml (defaults to <cluster_dir>/cluster.toml)
    #[clap(long)]
    pub config: Option<String>,

    /// Comma-separated list of node IDs to stop (defaults to all)
    #[clap(long)]
    pub nodes: Option<String>,
}

impl Executable for StopCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let cluster_dir = resolve_cluster_dir(self.cluster_dir.as_deref())?;
        let config = resolve_config(&cluster_dir, self.config.as_deref());
        let script = cluster_dir.join("stop.sh");

        println!(
            "{} {}",
            "[localnet stop]".cyan(),
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
                "stop.sh exited with code {}",
                status.code().unwrap_or(-1)
            ));
        }
        Ok(())
    }
}
