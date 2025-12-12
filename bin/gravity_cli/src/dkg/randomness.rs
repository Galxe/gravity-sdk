use clap::Parser;

use crate::command::Executable;
use serde::Deserialize;

#[derive(Debug, Parser)]
pub struct RandomnessCommand {
    /// Server address and port (e.g., 127.0.0.1:1024)
    #[clap(long)]
    pub server_url: String,

    /// Block number to query randomness for
    #[clap(long)]
    pub block_number: u64,
}

#[derive(Deserialize, Debug)]
struct RandomnessResponse {
    block_number: u64,
    randomness: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ErrorResponse {
    error: String,
}

impl Executable for RandomnessCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        // Use tokio runtime to run async code
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl RandomnessCommand {
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
        let url = format!("{}/dkg/randomness/{}", base_url, self.block_number);

        println!("Querying Randomness for block {} from: {}", self.block_number, url);

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?;

        let response = client
            .get(&url)
            .send()
            .await?;

        let status_code = response.status();
        if !status_code.is_success() {
            // Try to parse error message from response
            let error_msg = match response.json::<ErrorResponse>().await {
                Ok(error_response) => format!("HTTP {}: {}", status_code, error_response.error),
                Err(_) => format!("HTTP {}", status_code),
            };
            return Err(anyhow::anyhow!("Failed to get randomness: {}", error_msg));
        }

        let result: RandomnessResponse = response.json().await?;

        match &result.randomness {
            Some(hex) => {
                println!("Block {}: {}", result.block_number, hex);
            }
            None => {
                return Err(anyhow::anyhow!(
                    "No randomness found for block {}",
                    result.block_number
                ));
            }
        }

        Ok(())
    }
}
