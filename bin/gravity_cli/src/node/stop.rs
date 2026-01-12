use clap::Parser;
use std::{path::PathBuf, process::Command};

use crate::command::Executable;

#[derive(Debug, Parser)]
pub struct StopCommand {
    /// Deployment path containing script/stop.sh
    #[clap(long)]
    pub deploy_path: String,
}

impl Executable for StopCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let deploy_path = PathBuf::from(&self.deploy_path);
        let script_path = deploy_path.join("script").join("stop.sh");

        if !script_path.exists() {
            return Err(anyhow::anyhow!("Stop script not found: {}", script_path.display()));
        }

        println!("Stopping node from: {}", script_path.display());

        let output =
            Command::new("bash").arg(script_path.as_os_str()).current_dir(&deploy_path).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to stop node: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            print!("{stdout}");
        }

        println!("Node stopped successfully");
        Ok(())
    }
}
