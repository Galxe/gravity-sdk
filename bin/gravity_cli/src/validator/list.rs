use alloy_primitives::{Bytes, TxKind};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{TransactionInput, TransactionRequest};
use alloy_sol_types::SolCall;
use clap::Parser;
use serde::Serialize;

use crate::{
    command::Executable,
    contract::{ValidatorManagement, ValidatorStatus, VALIDATOR_MANAGER_ADDRESS},
    output::OutputFormat,
    util::format_ether,
};

#[derive(Debug, Parser)]
pub struct ListCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Output format
    #[clap(skip)]
    pub output_format: OutputFormat,
}

// Serializable versions of the contract types
#[derive(Debug, Serialize)]
struct SerializableValidatorSet {
    active_validators: Vec<SerializableValidatorInfo>,
    pending_inactive: Vec<SerializableValidatorInfo>,
    pending_active: Vec<SerializableValidatorInfo>,
    total_voting_power: String,
    active_count: u64,
    current_epoch: u64,
}

#[derive(Debug, Serialize)]
struct SerializableValidatorInfo {
    validator: String,
    consensus_pubkey: String,
    voting_power: String,
    validator_index: u64,
    network_addresses: String,
    fullnode_addresses: String,
    status: String,
}

impl Executable for ListCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl ListCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self.rpc_url.ok_or_else(|| {
            anyhow::anyhow!(
                "--rpc-url is required. Set via CLI flag, GRAVITY_RPC_URL env var, or ~/.gravity/config.toml"
            )
        })?;

        // Initialize Provider
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);

        // Get current epoch
        let call = ValidatorManagement::getCurrentEpochCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let decoded = ValidatorManagement::getCurrentEpochCall::abi_decode_returns(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode current epoch: {e}"))?;
        let current_epoch = decoded;

        // Get total voting power
        let call = ValidatorManagement::getTotalVotingPowerCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let total_voting_power =
            ValidatorManagement::getTotalVotingPowerCall::abi_decode_returns(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode total voting power: {e}"))?;

        // Get active validator count
        let call = ValidatorManagement::getActiveValidatorCountCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let active_count =
            ValidatorManagement::getActiveValidatorCountCall::abi_decode_returns(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode active count: {e}"))?;

        // Get active validators
        let call = ValidatorManagement::getActiveValidatorsCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let active_validators =
            ValidatorManagement::getActiveValidatorsCall::abi_decode_returns(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode active validators: {e}"))?;

        // Get pending active validators
        let call = ValidatorManagement::getPendingActiveValidatorsCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let pending_active =
            ValidatorManagement::getPendingActiveValidatorsCall::abi_decode_returns(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode pending active validators: {e}"))?;

        // Get pending inactive validators
        let call = ValidatorManagement::getPendingInactiveValidatorsCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let pending_inactive =
            ValidatorManagement::getPendingInactiveValidatorsCall::abi_decode_returns(&result)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to decode pending inactive validators: {e}")
                })?;

        // Convert to serializable format
        let serializable_set = SerializableValidatorSet {
            active_validators: active_validators
                .iter()
                .map(|v| convert_validator_info(v, ValidatorStatus::ACTIVE))
                .collect(),
            pending_inactive: pending_inactive
                .iter()
                .map(|v| convert_validator_info(v, ValidatorStatus::PENDING_INACTIVE))
                .collect(),
            pending_active: pending_active
                .iter()
                .map(|v| convert_validator_info(v, ValidatorStatus::PENDING_ACTIVE))
                .collect(),
            total_voting_power: format_ether(total_voting_power),
            active_count: active_count.try_into().unwrap_or(0),
            current_epoch,
        };

        // Output based on format
        match self.output_format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&serializable_set)?;
                println!("{json}");
            }
            _ => {
                println!(
                    "Epoch: {}  |  Active: {}  |  Total Voting Power: {} ETH",
                    serializable_set.current_epoch,
                    serializable_set.active_count,
                    serializable_set.total_voting_power,
                );
                println!();
                if !serializable_set.active_validators.is_empty() {
                    println!("Active Validators:");
                    println!(
                        "{:<6} {:<44} {:<16} Moniker/Network",
                        "#", "Validator", "Voting Power"
                    );
                    println!("{}", "-".repeat(90));
                    for v in &serializable_set.active_validators {
                        println!(
                            "{:<6} {:<44} {:<16} {}",
                            v.validator_index, v.validator, v.voting_power, v.network_addresses
                        );
                    }
                    println!();
                }
                if !serializable_set.pending_active.is_empty() {
                    println!("Pending Active:");
                    for v in &serializable_set.pending_active {
                        println!("  {} (voting power: {})", v.validator, v.voting_power);
                    }
                    println!();
                }
                if !serializable_set.pending_inactive.is_empty() {
                    println!("Pending Inactive:");
                    for v in &serializable_set.pending_inactive {
                        println!("  {} (voting power: {})", v.validator, v.voting_power);
                    }
                    println!();
                }
            }
        }

        Ok(())
    }
}

fn convert_validator_info(
    info: &crate::contract::ValidatorConsensusInfo,
    status: ValidatorStatus,
) -> SerializableValidatorInfo {
    SerializableValidatorInfo {
        validator: format!("{:?}", info.validator),
        consensus_pubkey: hex::encode(&info.consensusPubkey),
        voting_power: format_ether(info.votingPower),
        validator_index: info.validatorIndex,
        network_addresses: bcs::from_bytes::<String>(&info.networkAddresses)
            .unwrap_or_else(|_| hex::encode(&info.networkAddresses)),
        fullnode_addresses: bcs::from_bytes::<String>(&info.fullnodeAddresses)
            .unwrap_or_else(|_| hex::encode(&info.fullnodeAddresses)),
        status: format!("{status:?}"),
    }
}
