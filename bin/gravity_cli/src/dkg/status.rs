use clap::Parser;

use crate::command::Executable;
use serde::Deserialize;

#[derive(Debug, Parser)]
pub struct StatusCommand {
    /// Server address and port (e.g., 127.0.0.1:1024)
    #[clap(long)]
    pub server_url: String,
}

#[derive(Deserialize, Debug)]
struct DKGStatusResponse {
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
            format!("http://{}", url)
        }
    }

    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let base_url = Self::normalize_url(&self.server_url);
        let url = format!("{}/dkg/status", base_url);

        println!("Fetching DKG status from: {}", url);

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
                Err(_) => format!("HTTP {}", status_code),
            };
            return Err(anyhow::anyhow!("Failed to get DKG status: {}", error_msg));
        }

        let status: DKGStatusResponse = response.json().await?;

        // Display status
        println!("DKG Status:");
        println!("  Current Epoch: {}", status.epoch);
        println!("  Current Round: {}", status.round);
        println!("  Current Block Number: {}", status.block_number);
        println!("  Participating Nodes: {}", status.participating_nodes);

        Ok(())
    }
}
