use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{TransactionInput, TransactionRequest};
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolEvent, SolType};
use clap::Parser;
use std::{
    fmt::{Debug, Formatter},
    str::FromStr,
};

use crate::command::Executable;

// Define contract interface using alloy_sol_macro
alloy_sol_macro::sol! {
    enum ValidatorStatus {
        PENDING_ACTIVE, // 0
        ACTIVE, // 1
        PENDING_INACTIVE, // 2
        INACTIVE // 3
    }

    struct ValidatorInfo {
        // Basic information (from ValidatorManager)
        bytes consensusPublicKey;
        Commission commission;
        string moniker;
        bool registered;
        address stakeCreditAddress;
        ValidatorStatus status;
        uint256 votingPower; // Changed from uint64 to uint256 to prevent overflow
        uint256 validatorIndex;
        uint256 updateTime;
        address operator;
        bytes validatorNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes fullnodeNetworkAddresses; // BCS serialized Vec<NetworkAddress>
        bytes aptosAddress; // Aptos validator address
    }

    struct Commission {
        uint64 rate; // the commission rate charged to delegators(10000 is 100%)
        uint64 maxRate; // maximum commission rate which validator can ever charge
        uint64 maxChangeRate; // maximum daily increase of the validator commission
    }

    struct ValidatorSetData {
        uint256 totalVotingPower; // Total voting power - Changed from uint128 to uint256
        uint256 totalJoiningPower; // Total pending voting power - Changed from uint128 to uint256
    }

    struct ValidatorSet {
        ValidatorInfo[] activeValidators; // Active validators for the current epoch
        ValidatorInfo[] pendingInactive; // Pending validators to leave in next epoch (still active)
        ValidatorInfo[] pendingActive; // Pending validators to join in next epoch
        uint256 totalVotingPower; // Current total voting power
        uint256 totalJoiningPower; // Total voting power waiting to join in the next epoch
    }

    contract ValidatorManager {
        function leaveValidatorSet(address validator) external;

        function getValidatorInfo(
            address validator
        ) external view returns (ValidatorInfo memory);

        function getValidatorStatus(address validator) external view returns (uint8);

        event ValidatorLeaveRequested(
            address indexed validator,
            uint64 epoch
        );

        event ValidatorStatusChanged(
            address indexed validator,
            uint8 oldStatus,
            uint8 newStatus,
            uint64 epoch
        );
    }
}

impl Debug for ValidatorStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidatorStatus::PENDING_ACTIVE => write!(f, "PENDING_ACTIVE"),
            ValidatorStatus::ACTIVE => write!(f, "ACTIVE"),
            ValidatorStatus::PENDING_INACTIVE => write!(f, "PENDING_INACTIVE"),
            ValidatorStatus::INACTIVE => write!(f, "INACTIVE"),
            _ => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Parser)]
pub struct LeaveCommand {
    /// RPC URL for gravity node
    #[clap(long)]
    pub rpc_url: String,

    /// ValidatorManager contract address (40 bytes hex string with 0x prefix)
    #[clap(long)]
    pub contract_address: String,

    /// Private key for signing transactions (hex string with or without 0x prefix)
    #[clap(long)]
    pub private_key: String,

    /// Gas limit for the transaction
    #[clap(long, default_value = "2000000")]
    pub gas_limit: u64,

    /// Gas price in wei
    #[clap(long, default_value = "20")]
    pub gas_price: u128,

    /// EVM compatible validator address (40 bytes hex string with 0x prefix)
    #[clap(long)]
    pub validator_address: String,
}

impl Executable for LeaveCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        // Use tokio runtime to run async code
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl LeaveCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        // 1. Initialize Provider and Wallet
        println!("1. Initializing connection...");

        println!("   RPC URL: {}", self.rpc_url);
        let private_key_str = self.private_key.strip_prefix("0x").unwrap_or(&self.private_key);
        let private_key_bytes = hex::decode(private_key_str)?;
        let private_key = SigningKey::from_slice(private_key_bytes.as_slice())
            .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
        let signer = PrivateKeySigner::from(private_key);
        let wallet_address = signer.address();
        println!("   Wallet address: {:?}", wallet_address);

        let contract_address = Address::from_str(&self.contract_address)?;
        println!("   Contract address: {:?}", contract_address);

        // Create provider
        let provider = ProviderBuilder::new().wallet(signer).connect_http(self.rpc_url.parse()?);

        let chain_id = provider.get_chain_id().await?;
        println!("   Chain ID: {}", chain_id);

        // 2. Check validator information
        println!("2. Checking validator information...");
        let validator_address = Address::from_str(&self.validator_address)?;
        let call = ValidatorManager::getValidatorInfoCall { validator: validator_address };
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let validator_info = <ValidatorInfo as SolType>::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode validator info: {}", e))?;
        println!("   Validator information:");
        println!("   - Registered: {}", validator_info.registered);
        println!("   - Status: {:?}", validator_info.status);
        println!("   - Voting power: {} ETH", format_ether(validator_info.votingPower));
        println!("   - StakeCredit address: {}", validator_info.stakeCreditAddress);
        println!("   - Operator: {}", validator_info.operator);
        println!("   - Validator moniker: {}", validator_info.moniker);

        // Check if validator is registered
        if !validator_info.registered {
            return Err(anyhow::anyhow!("Validator is not registered"));
        }

        // Check if validator status allows leaving
        match validator_info.status {
            ValidatorStatus::PENDING_ACTIVE | ValidatorStatus::ACTIVE => {
                println!("   Validator status allows leaving\n");
            }
            ValidatorStatus::PENDING_INACTIVE => {
                println!("   Validator is already PENDING_INACTIVE, no need to leave again\n");
                return Ok(());
            }
            ValidatorStatus::INACTIVE => {
                println!("   Validator is already INACTIVE, no need to leave\n");
                return Ok(());
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Validator status {:?} does not allow leaving",
                    validator_info.status
                ));
            }
        }

        // 3. Leave validator set
        println!("3. Leaving validator set...");
        let call = ValidatorManager::leaveValidatorSetCall { validator: validator_address };
        let input: Bytes = call.abi_encode().into();
        let tx_hash = provider
            .send_transaction(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
                gas: Some(self.gas_limit),
                gas_price: Some(self.gas_price),
                ..Default::default()
            })
            .await?
            .with_required_confirmations(2)
            .with_timeout(Some(std::time::Duration::from_secs(60)))
            .watch()
            .await?;
        println!("   Transaction hash: {}", tx_hash);
        let receipt = provider
            .get_transaction_receipt(tx_hash)
            .await?
            .ok_or(anyhow::anyhow!("Failed to get transaction receipt"))?;
        println!(
            "   Transaction confirmed, block number: {}",
            receipt.block_number.ok_or(anyhow::anyhow!("Failed to get block number"))?
        );
        println!("   Gas used: {}", receipt.gas_used);
        println!(
            "   Transaction cost: {} ETH",
            format_ether(U256::from(receipt.effective_gas_price) * U256::from(receipt.gas_used))
        );

        // Check leave events
        let mut found_leave_event = false;
        let mut found_status_change_event = false;
        for log in receipt.logs() {
            // Check for ValidatorLeaveRequested event
            match ValidatorManager::ValidatorLeaveRequested::decode_log(&log.inner) {
                Ok(event) => {
                    println!("   Leave request successful! Event details:");
                    println!("   - Validator address: {}", event.validator);
                    println!("   - Epoch: {}", event.epoch);
                    found_leave_event = true;
                    continue;
                }
                Err(_) => {}
            }

            // Check for ValidatorStatusChanged event
            match ValidatorManager::ValidatorStatusChanged::decode_log(&log.inner) {
                Ok(event) => {
                    if event.validator == validator_address {
                        println!("   Status changed! Event details:");
                        println!("   - Validator address: {}", event.validator);
                        println!("   - Old status: {}", event.oldStatus);
                        println!("   - New status: {}", event.newStatus);
                        println!("   - Epoch: {}", event.epoch);
                        found_status_change_event = true;
                        continue;
                    }
                }
                Err(_) => {}
            }
        }

        if !found_leave_event {
            println!("   Leave event not found\n");
            return Err(anyhow::anyhow!("Failed to find leave event"));
        }

        if !found_status_change_event {
            println!("   Status change event not found\n");
            return Err(anyhow::anyhow!("Failed to find status change event"));
        }

        // 4. Final status check
        println!("4. Final status check...");
        let call = ValidatorManager::getValidatorStatusCall { validator: validator_address };
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let validator_status = <ValidatorStatus as SolType>::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode validator status: {}", e))?;
        match validator_status {
            ValidatorStatus::PENDING_INACTIVE => {
                println!(
                    "   Validator status is PENDING_INACTIVE, will become INACTIVE in the next epoch\n"
                );
            }
            ValidatorStatus::INACTIVE => {
                println!("   Validator status is INACTIVE, successfully left the validator set\n");
            }
            _ => {
                println!("   Validator status is {:?}, unexpected status\n", validator_status);
                return Err(anyhow::anyhow!("Unexpected validator status: {:?}", validator_status));
            }
        }
        Ok(())
    }
}

// Helper function: format ether amount
fn format_ether(wei: U256) -> String {
    let wei_str = wei.to_string();
    let len = wei_str.len();
    if len <= 18 {
        format!("0.{}", "0".repeat(18 - len) + &wei_str)
    } else {
        let (integer, decimal) = wei_str.split_at(len - 18);
        format!("{}.{}", integer, decimal.trim_end_matches('0').trim_end_matches('.'))
    }
}
