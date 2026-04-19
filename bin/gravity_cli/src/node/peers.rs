use alloy_provider::{Provider, ProviderBuilder};
use clap::Parser;
use serde_json::Value;
use std::borrow::Cow;

use crate::{command::Executable, output::OutputFormat};

#[derive(Debug, Parser)]
pub struct PeersCommand {
    /// RPC URL
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Emit full JSON for every peer instead of a summary line.
    #[clap(long)]
    pub detail: bool,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

impl Executable for PeersCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl PeersCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self
            .rpc_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--rpc-url is required"))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        let peers: Value = provider
            .client()
            .request(Cow::Borrowed("admin_peers"), ())
            .await?;

        match self.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&peers)?);
            }
            OutputFormat::Plain => render_plain(&peers, self.detail),
        }
        Ok(())
    }
}

fn render_plain(peers: &Value, detail: bool) {
    let Some(arr) = peers.as_array() else {
        println!("{peers}");
        return;
    };
    if arr.is_empty() {
        println!("(no peers)");
        return;
    }
    println!("{} peer(s) connected:", arr.len());
    for peer in arr {
        if detail {
            println!("{}", serde_json::to_string_pretty(peer).unwrap_or_default());
            continue;
        }
        // Summary: id, direction, enode snippet, protocol hints
        let id = peer.get("id").and_then(Value::as_str).unwrap_or("?");
        let enode = peer.get("enode").and_then(Value::as_str).unwrap_or("?");
        let network = peer
            .get("network")
            .map(|n| {
                let inbound = n.get("inbound").and_then(Value::as_bool).unwrap_or(false);
                if inbound {
                    "inbound"
                } else {
                    "outbound"
                }
            })
            .unwrap_or("?");
        let name = peer.get("name").and_then(Value::as_str).unwrap_or("?");
        // Shorten id for the table
        let short_id = if id.len() > 16 { &id[..16] } else { id };
        let short_enode = if enode.len() > 60 { &enode[..60] } else { enode };
        println!("  {short_id}…  {network:8}  {name}  {short_enode}…");
    }
}
