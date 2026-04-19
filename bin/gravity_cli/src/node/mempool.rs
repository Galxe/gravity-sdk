use alloy_provider::{Provider, ProviderBuilder};
use clap::Parser;
use serde_json::Value;
use std::borrow::Cow;

use crate::{command::Executable, output::OutputFormat};

#[derive(Debug, Parser)]
pub struct MempoolCommand {
    /// RPC URL
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Also fetch and display `txpool_content` (heavier — emits every tx).
    #[clap(long)]
    pub content: bool,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

impl Executable for MempoolCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl MempoolCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self
            .rpc_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--rpc-url is required"))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        let status: Value = provider
            .client()
            .request(Cow::Borrowed("txpool_status"), ())
            .await?;
        let content: Option<Value> = if self.content {
            Some(
                provider
                    .client()
                    .request(Cow::Borrowed("txpool_content"), ())
                    .await?,
            )
        } else {
            None
        };

        match self.output_format {
            OutputFormat::Json => {
                let combined = serde_json::json!({ "status": status, "content": content });
                println!("{}", serde_json::to_string_pretty(&combined)?);
            }
            OutputFormat::Plain => {
                let pending = hex_u64(status.get("pending")).unwrap_or(0);
                let queued = hex_u64(status.get("queued")).unwrap_or(0);
                println!("pending: {pending}");
                println!("queued:  {queued}");
                if let Some(c) = content.as_ref() {
                    if let Some(p) = c.get("pending").and_then(Value::as_object) {
                        println!("\npending by sender:");
                        for (sender, nonces) in p {
                            let n_txns =
                                nonces.as_object().map(|o| o.len()).unwrap_or(0);
                            println!("  {sender}  {n_txns} tx(s)");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn hex_u64(v: Option<&Value>) -> Option<u64> {
    v.and_then(Value::as_str).and_then(|s| {
        u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
    })
}
