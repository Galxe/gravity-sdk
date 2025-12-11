use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::command::Executable;

#[derive(Debug, Parser)]
pub struct StartCommand {
    /// Deployment path containing script/start.sh
    #[clap(long)]
    pub deploy_path: String,
}

impl StartCommand {
    /// Check if PID file exists and if the process is still running
    fn check_pid_file(pid_path: &PathBuf) -> Result<(), anyhow::Error> {
        if !pid_path.exists() {
            return Ok(());
        }

        // Read PID from file
        let pid_str = fs::read_to_string(pid_path)?;
        let pid_str = pid_str.trim();
        let pid: i32 = pid_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid PID in file: {}", e))?;

        // Check if process is still running using ps command
        // This works on Unix-like systems (Linux, macOS, etc.)
        let output = Command::new("ps")
            .arg("-p")
            .arg(pid_str)
            .output()?;

        if output.status.success() {
            // Process is still running
            return Err(anyhow::anyhow!(
                "Node is already running with PID {} (PID file: {})",
                pid,
                pid_path.display()
            ));
        } else {
            // Process doesn't exist, but PID file exists (zombie PID file)
            return Err(anyhow::anyhow!(
                "PID file exists but process {} is not running (zombie PID file: {})",
                pid,
                pid_path.display()
            ));
        }
    }
}

impl Executable for StartCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let deploy_path = PathBuf::from(&self.deploy_path);
        let script_path = deploy_path.join("script").join("start.sh");
        let pid_path = deploy_path.join("script").join("node.pid");

        if !script_path.exists() {
            return Err(anyhow::anyhow!(
                "Start script not found: {}",
                script_path.display()
            ));
        }

        // Check PID file before starting
        Self::check_pid_file(&pid_path)?;

        println!("Starting node from: {}", script_path.display());

        // Use status() instead of output() to avoid waiting for output
        // The start.sh script starts the node in background and returns immediately
        let status = Command::new("bash")
            .arg(script_path.as_os_str())
            .current_dir(&deploy_path)
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Failed to start node: script exited with code {}",
                status.code().unwrap_or(-1)
            ));
        }

        // Wait a moment for PID file to be created
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Verify that PID file was created and process is running
        if pid_path.exists() {
            let pid_str = fs::read_to_string(&pid_path)?;
            let pid_str = pid_str.trim();
            if !pid_str.is_empty() {
                println!("Node started successfully (PID: {})", pid_str);
            } else {
                println!("Node started successfully");
            }
        } else {
            println!("Node started successfully (PID file will be created by the script)");
        }

        Ok(())
    }
}

