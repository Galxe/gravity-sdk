use alloy_provider::{Provider, ProviderBuilder};
use clap::Parser;
use serde::Serialize;
use serde_json::Value;
use std::{borrow::Cow, time::Duration};

use crate::{command::Executable, output::OutputFormat};

#[derive(Debug, Parser)]
pub struct MetricsCommand {
    /// RPC URL
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Serialize)]
struct NodeMetrics {
    chain_id: Option<u64>,
    block_number: Option<u64>,
    peer_count: Option<u64>,
    txpool_pending: Option<u64>,
    txpool_queued: Option<u64>,
    syncing: Option<Value>,
}

impl Executable for MetricsCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl MetricsCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self
            .rpc_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--rpc-url is required"))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        // Fire all queries in parallel — each is independent. Any single
        // failure becomes a None field rather than killing the whole command,
        // so `metrics` works even on nodes with partial RPC namespaces.
        let peer_count_fut = provider
            .client()
            .request::<_, String>(Cow::Borrowed("net_peerCount"), ());
        let txpool_fut = provider
            .client()
            .request::<_, Value>(Cow::Borrowed("txpool_status"), ());
        let syncing_fut = provider
            .client()
            .request::<_, Value>(Cow::Borrowed("eth_syncing"), ());

        let (chain_id, block_number, peer_count_hex, txpool_status, syncing) = tokio::join!(
            with_timeout(provider.get_chain_id()),
            with_timeout(provider.get_block_number()),
            with_timeout(peer_count_fut),
            with_timeout(txpool_fut),
            with_timeout(syncing_fut),
        );

        let metrics = NodeMetrics {
            chain_id: chain_id.ok(),
            block_number: block_number.ok(),
            peer_count: peer_count_hex.ok().and_then(parse_hex_u64),
            txpool_pending: txpool_status
                .as_ref()
                .ok()
                .and_then(|v| v.get("pending"))
                .and_then(hex_u64_from_val),
            txpool_queued: txpool_status
                .as_ref()
                .ok()
                .and_then(|v| v.get("queued"))
                .and_then(hex_u64_from_val),
            syncing: syncing.ok(),
        };

        match self.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&metrics)?);
            }
            OutputFormat::Plain => {
                let show = |label: &str, value: String| println!("{label:18} {value}");
                show(
                    "chain_id",
                    metrics.chain_id.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                );
                show(
                    "block_number",
                    metrics.block_number.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                );
                show(
                    "peer_count",
                    metrics.peer_count.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                );
                show(
                    "txpool_pending",
                    metrics
                        .txpool_pending
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".into()),
                );
                show(
                    "txpool_queued",
                    metrics
                        .txpool_queued
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".into()),
                );
                let sync = match &metrics.syncing {
                    Some(Value::Bool(false)) => "no".to_string(),
                    Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "?".into()),
                    None => "?".into(),
                };
                show("syncing", sync);
            }
        }
        Ok(())
    }
}

async fn with_timeout<T, E, F>(fut: F) -> Result<T, anyhow::Error>
where
    F: std::future::IntoFuture<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(Duration::from_secs(3), fut.into_future())
        .await
        .map_err(|_| anyhow::anyhow!("rpc timeout"))?
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn parse_hex_u64(s: String) -> Option<u64> {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

fn hex_u64_from_val(v: &Value) -> Option<u64> {
    v.as_str().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
}
