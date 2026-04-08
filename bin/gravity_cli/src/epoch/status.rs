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

#[derive(Serialize)]
struct EpochStatusInfo {
    current_epoch: u64,
    running_time_secs: u64,
    interval_secs: u64,
    remaining_secs: Option<u64>,
    overdue_secs: Option<u64>,
}

#[derive(Debug, Parser)]
pub struct StatusCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Output format
    #[clap(skip)]
    pub output_format: OutputFormat,
}

impl Executable for StatusCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl StatusCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self.rpc_url.ok_or_else(|| {
            anyhow::anyhow!(
                "--rpc-url is required. Set via CLI flag, GRAVITY_RPC_URL env var, or ~/.gravity/config.toml"
            )
        })?;

        // Initialize Provider
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        // Get current epoch
        let call = Reconfiguration::currentEpochCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let current_epoch = Reconfiguration::currentEpochCall::abi_decode_returns(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode current epoch: {e}"))?;

        // Get last reconfiguration time
        let call = Reconfiguration::lastReconfigurationTimeCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let last_time =
            Reconfiguration::lastReconfigurationTimeCall::abi_decode_returns(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode lastReconfigurationTime: {e}"))?;

        // Get epoch interval
        let call = EpochConfig::epochIntervalMicrosCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(EPOCH_CONFIG_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let interval = EpochConfig::epochIntervalMicrosCall::abi_decode_returns(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode epoch interval: {e}"))?;

        // Get latest block timestamp
        let latest_block = provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch latest block"))?;

        let block_timestamp = latest_block.header.timestamp;
        let block_micros = block_timestamp * 1_000_000;
        let running_micros = block_micros.saturating_sub(last_time);

        let running_secs = running_micros / 1_000_000;
        let expected_duration_secs = interval / 1_000_000;

        // Format times
        let format_hms = |secs: u64| {
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
        };

        let (remaining_secs, overdue_secs) = if running_secs < expected_duration_secs {
            (Some(expected_duration_secs - running_secs), None)
        } else {
            (None, Some(running_secs - expected_duration_secs))
        };

        match self.output_format {
            OutputFormat::Json => {
                let info = EpochStatusInfo {
                    current_epoch,
                    running_time_secs: running_secs,
                    interval_secs: expected_duration_secs,
                    remaining_secs,
                    overdue_secs,
                };
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
            _ => {
                println!("Epoch Status:");
                println!("  Current Epoch: {current_epoch}");
                println!(
                    "  Running Time:  {} (out of {} configured interval)",
                    format_hms(running_secs),
                    format_hms(expected_duration_secs)
                );
                if let Some(remaining) = remaining_secs {
                    println!("  Remaining:     {}", format_hms(remaining));
                }
                if let Some(overdue) = overdue_secs {
                    println!(
                        "  Overdue:       {} (epoch transition should occur soon)",
                        format_hms(overdue)
                    );
                }
            }
        }

        Ok(())
    }
}
