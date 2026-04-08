use alloy_primitives::{Bytes, TxKind};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_sol_types::SolCall;
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::{
    command::Executable,
    contract::{
        EpochConfig, Reconfiguration, ValidatorManagement, EPOCH_CONFIG_ADDRESS,
        RECONFIGURATION_ADDRESS, VALIDATOR_MANAGER_ADDRESS,
    },
    output::OutputFormat,
    util::format_ether,
};

#[derive(Debug, Parser)]
pub struct StatusCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Server address for DKG queries (e.g., 127.0.0.1:1024)
    #[clap(long, env = "GRAVITY_SERVER_URL")]
    pub server_url: Option<String>,

    /// Output format
    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Serialize)]
struct CombinedStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch: Option<EpochInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validators: Option<ValidatorSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dkg: Option<DkgInfo>,
}

#[derive(Serialize)]
struct EpochInfo {
    current_epoch: u64,
    running_time_secs: u64,
    interval_secs: u64,
}

#[derive(Serialize)]
struct ValidatorSummary {
    active_count: u64,
    pending_active_count: u64,
    pending_inactive_count: u64,
    total_voting_power: String,
}

#[derive(Serialize, Deserialize)]
struct DkgInfo {
    epoch: u64,
    round: u64,
    block_number: u64,
    participating_nodes: usize,
}

impl Executable for StatusCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl StatusCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        if self.rpc_url.is_none() && self.server_url.is_none() {
            return Err(anyhow::anyhow!(
                "At least one of --rpc-url or --server-url is required for status display"
            ));
        }

        let mut combined = CombinedStatus { epoch: None, validators: None, dkg: None };

        // Fetch RPC-based data if rpc_url is available
        if let Some(ref rpc_url) = self.rpc_url {
            let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

            // Epoch info
            let epoch_info = self.fetch_epoch_info(&provider).await;
            if let Ok(info) = epoch_info {
                combined.epoch = Some(info);
            }

            // Validator summary
            let validator_info = self.fetch_validator_summary(&provider).await;
            if let Ok(info) = validator_info {
                combined.validators = Some(info);
            }
        }

        // Fetch DKG status if server_url is available
        if let Some(ref server_url) = self.server_url {
            let dkg_info = self.fetch_dkg_info(server_url).await;
            if let Ok(info) = dkg_info {
                combined.dkg = Some(info);
            }
        }

        match self.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&combined)?);
            }
            OutputFormat::Plain => {
                println!("=== Gravity Node Status ===\n");
                if let Some(ref epoch) = combined.epoch {
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
                    let remaining = if epoch.running_time_secs < epoch.interval_secs {
                        format!(
                            "Remaining: {}",
                            format_hms(epoch.interval_secs - epoch.running_time_secs)
                        )
                    } else {
                        format!(
                            "Overdue: {}",
                            format_hms(epoch.running_time_secs - epoch.interval_secs)
                        )
                    };
                    println!(
                        "Epoch:      {}  |  Running: {}  |  {}",
                        epoch.current_epoch,
                        format_hms(epoch.running_time_secs),
                        remaining
                    );
                }
                if let Some(ref v) = combined.validators {
                    print!("Validators: {} active", v.active_count);
                    if v.pending_active_count > 0 {
                        print!(", {} pending active", v.pending_active_count);
                    }
                    if v.pending_inactive_count > 0 {
                        print!(", {} pending inactive", v.pending_inactive_count);
                    }
                    println!("  |  Voting Power: {} ETH", v.total_voting_power);
                }
                if let Some(ref dkg) = combined.dkg {
                    println!(
                        "DKG:        Round {}  |  Block: {}  |  Nodes: {}",
                        dkg.round, dkg.block_number, dkg.participating_nodes
                    );
                }
                println!();
            }
        }

        Ok(())
    }

    async fn fetch_epoch_info(&self, provider: &impl Provider) -> Result<EpochInfo, anyhow::Error> {
        // Get current epoch
        let call = Reconfiguration::currentEpochCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
                input: TransactionInput::new(call.abi_encode().into()),
                ..Default::default()
            })
            .await?;
        let current_epoch = Reconfiguration::currentEpochCall::abi_decode_returns(&result)?;

        // Get last reconfiguration time
        let call = Reconfiguration::lastReconfigurationTimeCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
                input: TransactionInput::new(call.abi_encode().into()),
                ..Default::default()
            })
            .await?;
        let last_time = Reconfiguration::lastReconfigurationTimeCall::abi_decode_returns(&result)?;

        // Get epoch interval
        let call = EpochConfig::epochIntervalMicrosCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(EPOCH_CONFIG_ADDRESS)),
                input: TransactionInput::new(call.abi_encode().into()),
                ..Default::default()
            })
            .await?;
        let interval = EpochConfig::epochIntervalMicrosCall::abi_decode_returns(&result)?;

        // Get latest block timestamp
        let latest_block = provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch latest block"))?;

        let block_micros = latest_block.header.timestamp * 1_000_000;
        let running_micros = block_micros.saturating_sub(last_time);

        Ok(EpochInfo {
            current_epoch,
            running_time_secs: running_micros / 1_000_000,
            interval_secs: interval / 1_000_000,
        })
    }

    async fn fetch_validator_summary(
        &self,
        provider: &impl Provider,
    ) -> Result<ValidatorSummary, anyhow::Error> {
        // Total voting power
        let call = ValidatorManagement::getTotalVotingPowerCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(call.abi_encode().into()),
                ..Default::default()
            })
            .await?;
        let total_voting_power =
            ValidatorManagement::getTotalVotingPowerCall::abi_decode_returns(&result)?;

        // Active count
        let call = ValidatorManagement::getActiveValidatorCountCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(call.abi_encode().into()),
                ..Default::default()
            })
            .await?;
        let active_count: u64 =
            ValidatorManagement::getActiveValidatorCountCall::abi_decode_returns(&result)?
                .try_into()
                .unwrap_or(0);

        // Pending active
        let call = ValidatorManagement::getPendingActiveValidatorsCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(call.abi_encode().into()),
                ..Default::default()
            })
            .await?;
        let pending_active =
            ValidatorManagement::getPendingActiveValidatorsCall::abi_decode_returns(&result)?;

        // Pending inactive
        let call = ValidatorManagement::getPendingInactiveValidatorsCall {};
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(Bytes::from(call.abi_encode())),
                ..Default::default()
            })
            .await?;
        let pending_inactive =
            ValidatorManagement::getPendingInactiveValidatorsCall::abi_decode_returns(&result)?;

        Ok(ValidatorSummary {
            active_count,
            pending_active_count: pending_active.len() as u64,
            pending_inactive_count: pending_inactive.len() as u64,
            total_voting_power: format_ether(total_voting_power),
        })
    }

    async fn fetch_dkg_info(&self, server_url: &str) -> Result<DkgInfo, anyhow::Error> {
        let url = server_url.trim_end_matches('/');
        let base_url = if url.starts_with("https://") || url.starts_with("http://") {
            url.to_string()
        } else {
            format!("http://{url}")
        };

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?;

        let response = client.get(format!("{base_url}/dkg/status")).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("DKG status request failed: HTTP {}", response.status()));
        }

        let status: DkgInfo = response.json().await?;
        Ok(status)
    }
}
