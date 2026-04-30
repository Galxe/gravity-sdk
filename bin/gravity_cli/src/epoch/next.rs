use alloy_primitives::{Bytes, TxKind};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_sol_types::SolCall;
use clap::Parser;
use serde::Serialize;

use crate::{
    command::Executable,
    contract::{EpochConfig, Reconfiguration, EPOCH_CONFIG_ADDRESS, RECONFIGURATION_ADDRESS},
    output::OutputFormat,
};

#[derive(Debug, Parser)]
pub struct NextCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Serialize)]
struct NextInfo {
    current_epoch: u64,
    predicted_transition_unix_secs: u64,
    seconds_until_transition: i64,
}

impl Executable for NextCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl NextCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self.rpc_url.ok_or_else(|| anyhow::anyhow!("--rpc-url is required"))?;
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        let (current_epoch, last_time_micros, interval_micros, block_ts) =
            fetch_epoch_timing(&provider).await?;

        // Predicted transition (seconds since unix epoch)
        let predicted = (last_time_micros + interval_micros) / 1_000_000;
        let delta: i64 = predicted as i64 - block_ts as i64;

        match self.output_format {
            OutputFormat::Json => {
                let info = NextInfo {
                    current_epoch,
                    predicted_transition_unix_secs: predicted,
                    seconds_until_transition: delta,
                };
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
            OutputFormat::Plain => {
                if delta >= 0 {
                    println!(
                        "epoch {current_epoch}: next transition in {} (≈ unix {predicted})",
                        format_hms(delta as u64)
                    );
                } else {
                    println!(
                        "epoch {current_epoch}: transition overdue by {} (expected at unix {predicted})",
                        format_hms((-delta) as u64)
                    );
                }
            }
        }
        Ok(())
    }
}

pub(crate) async fn fetch_epoch_timing(
    provider: &impl Provider,
) -> Result<(u64, u64, u64, u64), anyhow::Error> {
    // current epoch
    let call = Reconfiguration::currentEpochCall {};
    let result = provider
        .call(TransactionRequest {
            to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
            input: TransactionInput::new(Bytes::from(call.abi_encode())),
            ..Default::default()
        })
        .await?;
    let current_epoch = Reconfiguration::currentEpochCall::abi_decode_returns(&result)?;

    // last reconfig time (micros)
    let call = Reconfiguration::lastReconfigurationTimeCall {};
    let result = provider
        .call(TransactionRequest {
            to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
            input: TransactionInput::new(Bytes::from(call.abi_encode())),
            ..Default::default()
        })
        .await?;
    let last_time = Reconfiguration::lastReconfigurationTimeCall::abi_decode_returns(&result)?;

    // interval (micros)
    let call = EpochConfig::epochIntervalMicrosCall {};
    let result = provider
        .call(TransactionRequest {
            to: Some(TxKind::Call(EPOCH_CONFIG_ADDRESS)),
            input: TransactionInput::new(Bytes::from(call.abi_encode())),
            ..Default::default()
        })
        .await?;
    let interval = EpochConfig::epochIntervalMicrosCall::abi_decode_returns(&result)?;

    // latest block timestamp (seconds)
    let latest_block = provider
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| anyhow::anyhow!("failed to fetch latest block"))?;
    let block_ts = latest_block.header.timestamp;

    Ok((current_epoch, last_time, interval, block_ts))
}

pub(crate) fn format_hms(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}
