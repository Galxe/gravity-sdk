use clap::Parser;
use colored::Colorize;
use std::{
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use crate::{
    command::Executable,
    localnet::common::{load_cluster_toml, resolve_cluster_dir, resolve_config},
};

#[derive(Debug, Parser)]
pub struct ResetCommand {
    #[clap(long, env = "GRAVITY_CLUSTER_DIR")]
    pub cluster_dir: Option<String>,

    #[clap(long)]
    pub config: Option<String>,

    /// Skip interactive confirmation when base_dir is not under /tmp
    #[clap(long)]
    pub force: bool,

    /// Skip the implicit stop.sh call (use when cluster is already stopped)
    #[clap(long)]
    pub skip_stop: bool,
}

impl Executable for ResetCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let cluster_dir = resolve_cluster_dir(self.cluster_dir.as_deref())?;
        let config = resolve_config(&cluster_dir, self.config.as_deref());
        let cfg = load_cluster_toml(&config)?;
        let base_dir = PathBuf::from(&cfg.cluster.base_dir);

        if !base_dir.starts_with("/tmp") && !self.force {
            print!(
                "{} base_dir {} is outside /tmp — delete anyway? [y/N] ",
                "[localnet reset]".yellow(),
                base_dir.display()
            );
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !matches!(input.trim(), "y" | "Y") {
                println!("{} aborted", "[localnet reset]".cyan());
                return Ok(());
            }
        }

        if !self.skip_stop {
            let stop_script = cluster_dir.join("stop.sh");
            if stop_script.exists() {
                println!("{} stopping cluster first…", "[localnet reset]".cyan());
                let _ = Command::new("bash")
                    .arg(&stop_script)
                    .arg("--config")
                    .arg(&config)
                    .current_dir(&cluster_dir)
                    .status();
            }
        }

        if base_dir.exists() {
            println!("{} removing {}", "[localnet reset]".cyan(), base_dir.display());
            std::fs::remove_dir_all(&base_dir)
                .map_err(|e| anyhow::anyhow!("failed to remove {}: {e}", base_dir.display()))?;
            println!("{} {}", "[localnet reset]".cyan(), "done".green());
        } else {
            println!(
                "{} {} does not exist — nothing to remove",
                "[localnet reset]".cyan(),
                base_dir.display()
            );
        }
        Ok(())
    }
}
