use alloy_primitives::{Bytes, TxKind};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{TransactionInput, TransactionRequest};
use alloy_sol_types::{SolCall, SolType};
use clap::Parser;
use serde::Serialize;

use crate::{
    command::Executable,
    validator::{
        contract::{ValidatorManager, ValidatorSet, VALIDATOR_MANAGER_ADDRESS},
        util::format_ether,
    },
};

#[derive(Debug, Parser)]
pub struct ListCommand {
    /// RPC URL for gravity node
    #[clap(long)]
    pub rpc_url: String,
}

// Serializable versions of the contract types
#[derive(Debug, Serialize)]
struct SerializableValidatorSet {
    active_validators: Vec<SerializableValidatorInfo>,
    pending_inactive: Vec<SerializableValidatorInfo>,
    pending_active: Vec<SerializableValidatorInfo>,
    total_voting_power: String,
    total_joining_power: String,
}

#[derive(Debug, Serialize)]
struct SerializableValidatorInfo {
    consensus_public_key: String,
    commission: SerializableCommission,
    moniker: String,
    registered: bool,
    stake_credit_address: String,
    status: String,
    voting_power: String,
    validator_index: String,
    update_time: String,
    operator: String,
    validator_network_addresses: String,
    fullnode_network_addresses: String,
    aptos_address: String,
}

#[derive(Debug, Serialize)]
struct SerializableCommission {
    rate: u64,
    max_rate: u64,
    max_change_rate: u64,
}

impl Executable for ListCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        // Use tokio runtime to run async code
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl ListCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        // Initialize Provider
        let provider = ProviderBuilder::new().connect_http(self.rpc_url.parse()?);

        // Call getValidatorSet
        let call = ValidatorManager::getValidatorSetCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;

        // Decode ValidatorSet
        let validator_set = <ValidatorSet as SolType>::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode validator set: {}", e))?;

        // Convert to serializable format
        let serializable_set = SerializableValidatorSet {
            active_validators: validator_set
                .activeValidators
                .iter()
                .map(|v| convert_validator_info(v))
                .collect(),
            pending_inactive: validator_set
                .pendingInactive
                .iter()
                .map(|v| convert_validator_info(v))
                .collect(),
            pending_active: validator_set
                .pendingActive
                .iter()
                .map(|v| convert_validator_info(v))
                .collect(),
            total_voting_power: format_ether(validator_set.totalVotingPower),
            total_joining_power: format_ether(validator_set.totalJoiningPower),
        };

        // Output as JSON
        let json = serde_json::to_string_pretty(&serializable_set)?;
        println!("{}", json);

        Ok(())
    }
}

fn convert_validator_info(
    info: &crate::validator::contract::ValidatorInfo,
) -> SerializableValidatorInfo {
    use crate::validator::contract::ValidatorStatus;

    SerializableValidatorInfo {
        consensus_public_key: hex::encode(&info.consensusPublicKey),
        commission: SerializableCommission {
            rate: info.commission.rate,
            max_rate: info.commission.maxRate,
            max_change_rate: info.commission.maxChangeRate,
        },
        moniker: info.moniker.clone(),
        registered: info.registered,
        stake_credit_address: format!("{:?}", info.stakeCreditAddress),
        status: match info.status {
            ValidatorStatus::PENDING_ACTIVE => "PENDING_ACTIVE".to_string(),
            ValidatorStatus::ACTIVE => "ACTIVE".to_string(),
            ValidatorStatus::PENDING_INACTIVE => "PENDING_INACTIVE".to_string(),
            ValidatorStatus::INACTIVE => "INACTIVE".to_string(),
            _ => "UNKNOWN".to_string(),
        },
        voting_power: format_ether(info.votingPower),
        validator_index: info.validatorIndex.to_string(),
        update_time: info.updateTime.to_string(),
        operator: format!("{:?}", info.operator),
        validator_network_addresses: match bcs::from_bytes::<String>(
            &info.validatorNetworkAddresses,
        ) {
            Ok(addr) => addr,
            Err(_) => hex::encode(&info.validatorNetworkAddresses),
        },
        fullnode_network_addresses: match bcs::from_bytes::<String>(&info.fullnodeNetworkAddresses)
        {
            Ok(addr) => addr,
            Err(_) => hex::encode(&info.fullnodeNetworkAddresses),
        },
        aptos_address: hex::encode(&info.aptosAddress),
    }
}
