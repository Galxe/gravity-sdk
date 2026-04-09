use clap::Parser;

use crate::{command::Executable, output::OutputFormat};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
pub struct StatusCommand {
    /// Server address and port (e.g., 127.0.0.1:1024)
    #[clap(long, env = "GRAVITY_SERVER_URL")]
    pub server_url: Option<String>,

    /// Output format
    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DKGStatusResponse {
    epoch: u64,
    round: u64,
    block_number: u64,
    participating_nodes: usize,
}

#[derive(Deserialize, Debug)]
struct ErrorResponse {
    error: String,
}

impl Executable for StatusCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        // Use tokio runtime to run async code
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl StatusCommand {
    fn normalize_url(url: &str) -> String {
        let url = url.trim_end_matches('/');
        if url.starts_with("https://") || url.starts_with("http://") {
            url.to_string()
        } else {
            format!("http://{url}")
        }
    }

    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let server_url = self.server_url.ok_or_else(|| {
            anyhow::anyhow!(
                "--server-url is required. Set via CLI flag, GRAVITY_SERVER_URL env var, or ~/.gravity/config.toml"
            )
        })?;

        let base_url = Self::normalize_url(&server_url);
        let url = format!("{base_url}/dkg/status");

        println!("Fetching DKG status from: {url}");

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?;

        let response = client.get(&url).send().await?;

        let status_code = response.status();
        if !status_code.is_success() {
            // Try to parse error message from response
            let error_msg = match response.json::<ErrorResponse>().await {
                Ok(error_response) => format!("HTTP {}: {}", status_code, error_response.error),
                Err(_) => format!("HTTP {status_code}"),
            };
            return Err(anyhow::anyhow!("Failed to get DKG status: {error_msg}"));
        }

        let status: DKGStatusResponse = response.json().await?;

        // Display status
        match self.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
            _ => {
                println!("DKG Status:");
                println!("  Current Epoch: {}", status.epoch);
                println!("  Current Round: {}", status.round);
                println!("  Current Block Number: {}", status.block_number);
                println!("  Participating Nodes: {}", status.participating_nodes);
            }
        }

        Ok(())
    }
}
