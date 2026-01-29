use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{TransactionInput, TransactionRequest};
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolEvent, SolType, SolValue};
use clap::Parser;
use std::str::FromStr;

use crate::{
    command::Executable,
    validator::{
        contract::{
            status_from_u8, Staking, ValidatorManagement, ValidatorRecord, ValidatorStatus,
            STAKING_ADDRESS, VALIDATOR_MANAGER_ADDRESS,
        },
        util::{format_ether, parse_ether},
    },
};

#[derive(Debug, Parser)]
pub struct JoinCommand {
    /// RPC URL for gravity node
    #[clap(long)]
    pub rpc_url: String,

    /// Private key for signing transactions (hex string with or without 0x prefix)
    #[clap(long)]
    pub private_key: String,

    /// Gas limit for the transaction
    #[clap(long, default_value = "2000000")]
    pub gas_limit: u64,

    /// Gas price in wei
    #[clap(long, default_value = "20")]
    pub gas_price: u128,

    /// Stake amount in ETH (for creating new StakePool)
    #[clap(long)]
    pub stake_amount: String,

    /// Moniker (display name, max 31 bytes)
    #[clap(long, default_value = "Gravity1")]
    pub moniker: String,

    /// Existing StakePool address to use (if not provided, creates a new one)
    #[clap(long)]
    pub stake_pool: Option<String>,

    /// Consensus public key (BLS key)
    #[clap(long)]
    pub consensus_public_key: String,

    /// Proof of possession for the BLS key
    #[clap(long, default_value = "")]
    pub consensus_pop: String,

    /// Validator network address (/ip4/{host}/tcp/{port}/noise-ik/{public-key}/handshake/0)
    #[clap(long)]
    pub validator_network_address: String,

    /// Fullnode network address (/ip4/{host}/tcp/{port}/noise-ik/{public-key}/handshake/0)
    #[clap(long)]
    pub fullnode_network_address: String,

    /// Lockup duration in seconds (default 30 days, used when creating new StakePool)
    #[clap(long, default_value = "2592000")]
    pub lockup_duration: u64,
}

impl Executable for JoinCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
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
            .map_err(|e| anyhow::anyhow!("Invalid private key: {e}"))?;
        let signer = PrivateKeySigner::from(private_key);
        let wallet_address = signer.address();
        println!("   Wallet address: {wallet_address:?}");

        println!("   ValidatorManagement: {VALIDATOR_MANAGER_ADDRESS:?}");
        println!("   Staking: {STAKING_ADDRESS:?}");

        // Create provider
        let provider = ProviderBuilder::new().wallet(signer).connect_http(self.rpc_url.parse()?);

        let chain_id = provider.get_chain_id().await?;
        println!("   Chain ID: {chain_id}");
        let balance = provider.get_balance(wallet_address).await?;
        println!("   Wallet balance: {} ETH\n", format_ether(balance));

        // 2. Determine StakePool address (use existing or create new)
        let stake_pool: Address;

        if let Some(pool_str) = &self.stake_pool {
            // Use existing StakePool
            stake_pool = Address::from_str(pool_str)?;
            println!("2. Using existing StakePool: {stake_pool:?}");

            // Verify it's a valid pool
            let call = Staking::isPoolCall { pool: stake_pool };
            let input: Bytes = call.abi_encode().into();
            let result = provider
                .call(TransactionRequest {
                    from: Some(wallet_address),
                    to: Some(TxKind::Call(STAKING_ADDRESS)),
                    input: TransactionInput::new(input),
                    ..Default::default()
                })
                .await?;
            let is_pool = bool::abi_decode(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode isPool result: {e}"))?;
            if !is_pool {
                return Err(anyhow::anyhow!("Address is not a valid StakePool"));
            }

            // Check voting power
            let call = Staking::getPoolVotingPowerNowCall { pool: stake_pool };
            let input: Bytes = call.abi_encode().into();
            let result = provider
                .call(TransactionRequest {
                    from: Some(wallet_address),
                    to: Some(TxKind::Call(STAKING_ADDRESS)),
                    input: TransactionInput::new(input),
                    ..Default::default()
                })
                .await?;
            let voting_power = U256::abi_decode(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode voting power: {e}"))?;
            println!("   Current voting power: {} ETH\n", format_ether(voting_power));
        } else {
            // Create new StakePool
            println!("2. Creating new StakePool...");
            let stake_wei = parse_ether(&self.stake_amount)?;
            println!("   Stake amount: {} ETH", self.stake_amount);

            // Calculate lockup timestamp (current time + lockup duration in microseconds)
            let current_block = provider.get_block_number().await?;
            let block = provider.get_block_by_number(current_block.into()).await?;
            let current_timestamp =
                block.ok_or(anyhow::anyhow!("Failed to get current block"))?.header.timestamp;
            println!("   Current timestamp: {current_timestamp}");
            println!("   Lockup duration: {}", self.lockup_duration);
            // Convert to microseconds and add lockup duration
            let locked_until = (current_timestamp + self.lockup_duration) * 1_000_000;

            let call = Staking::createPoolCall {
                owner: wallet_address,
                staker: wallet_address,
                operator: wallet_address,
                voter: wallet_address,
                lockedUntil: locked_until,
            };
            let input: Bytes = call.abi_encode().into();
            let tx_hash = provider
                .send_transaction(TransactionRequest {
                    from: Some(wallet_address),
                    to: Some(TxKind::Call(STAKING_ADDRESS)),
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
            println!("   Transaction hash: {tx_hash}");

            let receipt = provider
                .get_transaction_receipt(tx_hash)
                .await?
                .ok_or(anyhow::anyhow!("Failed to get transaction receipt"))?;
            println!(
                "   Transaction confirmed, block number: {}",
                receipt.block_number.ok_or(anyhow::anyhow!("Failed to get block number"))?
            );
            println!("   Gas used: {}", receipt.gas_used);

            // Parse PoolCreated event to get the new pool address
            let mut found_pool = None;
            for log in receipt.logs() {
                if let Ok(event) = Staking::PoolCreated::decode_log(&log.inner) {
                    println!("   StakePool created successfully!");
                    println!("   - Pool address: {}", event.pool);
                    println!("   - Owner: {}", event.owner);
                    println!("   - Pool index: {}", event.poolIndex);
                    found_pool = Some(event.pool);
                    break;
                }
            }
            stake_pool = found_pool.ok_or(anyhow::anyhow!("Failed to find PoolCreated event"))?;
            println!();
        }

        // 3. Check if already registered as validator
        println!("3. Checking if already registered as validator...");
        let call = ValidatorManagement::isValidatorCall { stakePool: stake_pool };
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let is_validator = bool::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode isValidator result: {e}"))?;
        println!("   Is registered: {is_validator}");

        if is_validator {
            println!("   Validator is already registered, skipping registration step\n");
        } else {
            // 4. Register validator
            println!("4. Registering validator...");
            println!("   Moniker: \"{}\"", self.moniker);
            println!("   Consensus public key length: {} bytes", self.consensus_public_key.len());

            let call = ValidatorManagement::registerValidatorCall {
                stakePool: stake_pool,
                moniker: self.moniker.clone(),
                consensusPubkey: self.consensus_public_key.clone().into_bytes().into(),
                consensusPop: if self.consensus_pop.is_empty() {
                    Bytes::new()
                } else {
                    hex::decode(&self.consensus_pop)?.into()
                },
                networkAddresses: bcs::to_bytes(&self.validator_network_address)?.into(),
                fullnodeAddresses: bcs::to_bytes(&self.fullnode_network_address)?.into(),
            };
            let input: Bytes = call.abi_encode().into();
            let tx_hash = provider
                .send_transaction(TransactionRequest {
                    from: Some(wallet_address),
                    to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
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
            println!("   Transaction hash: {tx_hash}");

            let receipt = provider
                .get_transaction_receipt(tx_hash)
                .await?
                .ok_or(anyhow::anyhow!("Failed to get transaction receipt"))?;
            println!(
                "   Transaction confirmed, block number: {}",
                receipt.block_number.ok_or(anyhow::anyhow!("Failed to get block number"))?
            );
            println!("   Gas used: {}", receipt.gas_used);

            // Check registration event
            let mut found = false;
            for log in receipt.logs() {
                if let Ok(event) = ValidatorManagement::ValidatorRegistered::decode_log(&log.inner)
                {
                    println!("   Registration successful!");
                    println!("   - StakePool: {}", event.stakePool);
                    println!("   - Moniker: {}", event.moniker);
                    found = true;
                    break;
                }
            }
            if !found {
                println!("   Registration event not found\n");
                return Err(anyhow::anyhow!("Failed to find ValidatorRegistered event"));
            }
            println!();
        }

        // 5. Check validator information
        println!("5. Checking validator information...");
        let call = ValidatorManagement::getValidatorCall { stakePool: stake_pool };
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let validator_record = <ValidatorRecord as SolType>::abi_decode(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode validator record: {e}"))?;
        let status = status_from_u8(validator_record.status);
        println!("   Validator information:");
        println!("   - Validator: {}", validator_record.validator);
        println!("   - Moniker: {}", validator_record.moniker);
        println!("   - Status: {status:?}");
        println!("   - Bond: {} ETH", format_ether(validator_record.bond));
        println!("   - Fee recipient: {}", validator_record.feeRecipient);
        println!("   - StakePool: {}", validator_record.stakingPool);
        println!(
            "   - Network addresses: {}",
            bcs::from_bytes::<String>(&validator_record.networkAddresses)
                .unwrap_or_else(|_| hex::encode(&validator_record.networkAddresses))
        );
        println!(
            "   - Fullnode addresses: {}",
            bcs::from_bytes::<String>(&validator_record.fullnodeAddresses)
                .unwrap_or_else(|_| hex::encode(&validator_record.fullnodeAddresses))
        );

        if !matches!(status, ValidatorStatus::INACTIVE) {
            println!("   Validator status is not INACTIVE, skipping join step\n");
            return Ok(());
        }
        println!();

        // 6. Join validator set
        println!("6. Joining validator set...");
        let call = ValidatorManagement::joinValidatorSetCall { stakePool: stake_pool };
        let input: Bytes = call.abi_encode().into();
        let tx_hash = provider
            .send_transaction(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
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
        println!("   Transaction hash: {tx_hash}");

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
            if let Ok(event) = ValidatorManagement::ValidatorJoinRequested::decode_log(&log.inner) {
                println!("   Join request successful!");
                println!("   - StakePool: {}", event.stakePool);
                found = true;
                break;
            }
        }
        if !found {
            println!("   Join event not found\n");
            return Err(anyhow::anyhow!("Failed to find ValidatorJoinRequested event"));
        }
        println!();

        // 7. Final status check
        println!("7. Final status check...");
        let call = ValidatorManagement::getValidatorStatusCall { stakePool: stake_pool };
        let input: Bytes = call.abi_encode().into();
        let result = provider
            .call(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let status_u8 = result.last().copied().unwrap_or(0);
        let validator_status = status_from_u8(status_u8);
        match validator_status {
            ValidatorStatus::PENDING_ACTIVE => {
                println!("   Validator status is PENDING_ACTIVE");
                println!("   Please wait for the next epoch to automatically become ACTIVE\n");
            }
            ValidatorStatus::ACTIVE => {
                println!("   Validator status is ACTIVE");
                println!("   Successfully joined the validator set\n");
            }
            _ => {
                println!("   Validator status is {validator_status:?}, unexpected status\n");
                return Err(anyhow::anyhow!("Unexpected validator status: {validator_status:?}"));
            }
        }
        Ok(())
    }
}
