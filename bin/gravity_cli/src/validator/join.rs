use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{TransactionInput, TransactionRequest};
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolEvent, SolType, SolValue};
use clap::Parser;
use std::str::FromStr;

use crate::command::Executable;
use crate::validator::contract::{
    Commission, ValidatorInfo, ValidatorManager, ValidatorRegistrationParams, ValidatorSet,
    ValidatorSetData, ValidatorStatus,
};
use crate::validator::util::{format_ether, parse_ether};

#[derive(Debug, Parser)]
pub struct JoinCommand {
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

    /// Stake amount in ETH
    #[clap(long)]
    pub stake_amount: String,

    /// Moniker
    #[clap(long, default_value = "Gravity1")]
    pub moniker: String,

    /// EVM compatible validator address (40 bytes hex string with 0x prefix)
    #[clap(long)]
    pub validator_address: String,

    /// Consensus public key
    #[clap(long)]
    pub consensus_public_key: String,

    /// Validator network address (/ip4/{host}/tcp/{port}/noise-ik/{public-key}/handshake/0)
    #[clap(long)]
    pub validator_network_address: String,

    /// Fullnode network address (/ip4/{host}/tcp/{port}/noise-ik/{public-key}/handshake/0)
    #[clap(long)]
    pub fullnode_network_address: String,

    /// Aptos validator identity address (64 characters hex string)
    #[clap(long)]
    pub aptos_address: String,
}

impl Executable for JoinCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        // Use tokio runtime to run async code
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl JoinCommand {
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
        let balance = provider.get_balance(wallet_address).await?;
        println!("   Wallet balance: {} ETH\n", format_ether(balance));

        println!("2. Preparing registration parameters...");
        let validator_address = Address::from_str(&self.validator_address)?;
        let validator_params = ValidatorRegistrationParams {
            consensusPublicKey: self.consensus_public_key.clone().into_bytes().into(),
            blsProof: Bytes::new(),
            commission: Commission { rate: 0, maxRate: 5000, maxChangeRate: 500 },
            moniker: self.moniker,
            initialOperator: wallet_address,
            initialBeneficiary: wallet_address,
            validatorNetworkAddresses: bcs::to_bytes(&self.validator_network_address)?.into(),
            fullnodeNetworkAddresses: bcs::to_bytes(&self.fullnode_network_address)?.into(),
            aptosAddress: hex::decode(&self.aptos_address)?.into(),
        };
        println!("   Registration parameters:");
        println!(
            "   - Consensus public key: {} (length: {} bytes)",
            self.consensus_public_key,
            validator_params.consensusPublicKey.len()
        );
        println!(
            "   - Validator moniker: \"{}\" (length: {})",
            validator_params.moniker,
            validator_params.moniker.len()
        );
        println!("   - Operator address: {}", validator_params.initialOperator);
        println!("   - Beneficiary address: {}", validator_params.initialBeneficiary);
        println!(
            "   - Commission settings: {}% current ({}% max, {}% max daily change)",
            validator_params.commission.rate / 100,
            validator_params.commission.maxRate / 100,
            validator_params.commission.maxChangeRate / 100
        );
        let stake_wei = parse_ether(&self.stake_amount)?;
        println!("   - Stake amount: {} ETH", self.stake_amount);
        println!(
            "   - Aptos address: {} (length: {} bytes)",
            self.aptos_address,
            validator_params.aptosAddress.len()
        );

        // 3. Check system status
        println!("3. Checking system status...");
        let call = ValidatorManager::getValidatorSetDataCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let validator_set_data = <ValidatorSetData as SolType>::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode validator set data: {}", e))?;
        println!(
            "   Current total voting power: {} ETH",
            format_ether(validator_set_data.totalVotingPower)
        );
        println!(
            "   Current total joining voting power: {} ETH",
            format_ether(validator_set_data.totalJoiningPower)
        );
        let call = ValidatorManager::getValidatorSetCall {};
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let validator_set = <ValidatorSet as SolType>::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode validator set: {}", e))?;
        println!("   Active validators count: {}", validator_set.activeValidators.len());
        println!("   Pending active validators count: {}", validator_set.pendingActive.len());

        // 4. Check if already registered
        println!("4. Checking validator status...");
        let call = ValidatorManager::isValidatorRegisteredCall { validator: validator_address };
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let is_registered = bool::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode is registered: {}", e))?;
        println!("   Is registered: {}", is_registered);
        if is_registered {
            println!("   Validator is already registered, skipping registration step\n");
        } else {
            println!("5. Registering validator...");
            let call = ValidatorManager::registerValidatorCall { params: validator_params };
            let input: Bytes = call.abi_encode().into();
            let tx_hash = provider
                .send_transaction(TransactionRequest {
                    from: Some(wallet_address),
                    to: Some(TxKind::Call(contract_address)),
                    input: TransactionInput::new(input),
                    value: Some(stake_wei),
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
                format_ether(
                    U256::from(receipt.effective_gas_price) * U256::from(receipt.gas_used)
                )
            );
            // Check registration event
            let mut found = false;
            for log in receipt.logs() {
                match ValidatorManager::ValidatorRegistered::decode_log(&log.inner) {
                    Ok(event) => {
                        println!("   Registration successful! Event details:");
                        println!("   - Validator address: {}", event.validator);
                        println!("   - Operator address: {}", event.operator);
                        println!("   - Validator moniker: {}", event.moniker);
                        found = true;
                        break;
                    }
                    Err(_) => {}
                }
            }
            if !found {
                println!("   Registration event not found\n");
                return Err(anyhow::anyhow!("Failed to find register event"));
            }
        }

        println!("6. Checking validator information...");
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
        println!("   - Voting power: {}", format_ether(validator_info.votingPower));
        println!("   - StakeCredit address: {}", validator_info.stakeCreditAddress);
        println!("   - Operator: {}", validator_info.operator);
        println!("   - Validator moniker: {}", validator_info.moniker);
        println!("   - Consensus public key: {}", validator_info.consensusPublicKey);
        println!(
            "   - Validator network addresses: {}",
            bcs::from_bytes::<String>(&validator_info.validatorNetworkAddresses).map_err(
                |e| anyhow::anyhow!("Failed to decode validator network addresses: {}", e)
            )?
        );
        println!(
            "   - Fullnode network addresses: {}",
            bcs::from_bytes::<String>(&validator_info.fullnodeNetworkAddresses).map_err(
                |e| anyhow::anyhow!("Failed to decode fullnode network addresses: {}", e)
            )?
        );
        println!("   - Aptos address: {}", validator_info.aptosAddress);
        if !matches!(validator_info.status, ValidatorStatus::INACTIVE) {
            println!("   Validator status is not INACTIVE, skipping join step\n");
            return Ok(());
        }

        println!("7. Joining validator set...");
        let call = ValidatorManager::joinValidatorSetCall { validator: validator_address };
        let input: Bytes = call.abi_encode().into();
        let tx_hash = provider
            .send_transaction(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(contract_address)),
                input: TransactionInput::new(input),
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
        // Check join event
        let mut found = false;
        for log in receipt.logs() {
            match ValidatorManager::ValidatorJoinRequested::decode_log(&log.inner) {
                Ok(event) => {
                    println!("   Join successful! Event details:");
                    println!("   - Validator address: {}", event.validator);
                    println!("   - Voting power: {}", format_ether(event.votingPower));
                    println!("   - Epoch: {}", event.epoch);
                    found = true;
                    break;
                }
                Err(_) => {}
            }
        }
        if !found {
            println!("   Join event not found\n");
            return Err(anyhow::anyhow!("Failed to find join event"));
        }

        println!("8. Final status check...");
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
            ValidatorStatus::PENDING_ACTIVE => {
                println!("   Validator status is PENDING_ACTIVE, please wait for the next epoch to automatically become ACTIVE\n");
            }
            ValidatorStatus::ACTIVE => {
                println!("   Validator status is ACTIVE, successfully joined the validator set\n");
            }
            _ => {
                println!("   Validator status is {:?}, unexpected status\n", validator_status);
                return Err(anyhow::anyhow!("Unexpected validator status: {:?}", validator_status));
            }
        }
        Ok(())
    }
}
