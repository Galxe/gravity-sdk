use alloy_primitives::{Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{SolCall, SolEvent};
use clap::Parser;

use crate::{
    command::Executable,
    contract::{Staking, STAKING_ADDRESS},
    util::{format_ether, parse_ether},
};

#[derive(Debug, Parser)]
pub struct CreateCommand {
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

    /// Stake amount in ETH
    #[clap(long)]
    pub stake_amount: String,

    /// Lockup duration in seconds (default 30 days)
    #[clap(long, default_value = "2592000")]
    pub lockup_duration: u64,
}

impl Executable for CreateCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl CreateCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        // 1. Initialize Provider and Wallet
        println!("Creating new StakePool...\n");
        println!("1. Initializing connection...");

        println!("   RPC URL: {}", self.rpc_url);
        let private_key_str = self.private_key.strip_prefix("0x").unwrap_or(&self.private_key);
        let private_key_bytes = hex::decode(private_key_str)?;
        let private_key = SigningKey::from_slice(private_key_bytes.as_slice())
            .map_err(|e| anyhow::anyhow!("Invalid private key: {e}"))?;
        let signer = PrivateKeySigner::from(private_key);
        let wallet_address = signer.address();
        println!("   Wallet address: {wallet_address:?}");
        println!("   Staking contract: {STAKING_ADDRESS:?}");

        // Create provider
        let provider = ProviderBuilder::new().wallet(signer).connect_http(self.rpc_url.parse()?);

        let chain_id = provider.get_chain_id().await?;
        println!("   Chain ID: {chain_id}");
        let balance = provider.get_balance(wallet_address).await?;
        println!("   Wallet balance: {} ETH\n", format_ether(balance));

        // 2. Create StakePool
        println!("2. Creating StakePool...");
        let stake_wei = parse_ether(&self.stake_amount)?;
        println!("   Stake amount: {} ETH", self.stake_amount);

        // Calculate lockup timestamp (current time + lockup duration in microseconds)
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await?
            .ok_or(anyhow::anyhow!("Failed to get latest block"))?;
        let current_timestamp = block.header.timestamp;
        println!("   Current timestamp: {current_timestamp}");
        println!("   Lockup duration: {} seconds", self.lockup_duration);
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
        let pending_tx = provider
            .send_transaction(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(STAKING_ADDRESS)),
                input: TransactionInput::new(input),
                value: Some(stake_wei),
                gas: Some(self.gas_limit),
                gas_price: Some(self.gas_price),
                ..Default::default()
            })
            .await?;
        let tx_hash = *pending_tx.tx_hash();
        println!("   Transaction hash: {tx_hash}");
        let _ = pending_tx
            .with_required_confirmations(2)
            .with_timeout(Some(std::time::Duration::from_secs(60)))
            .watch()
            .await?;

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

        // Parse PoolCreated event to get the new pool address
        let mut found_pool = None;
        for log in receipt.logs() {
            if let Ok(event) = Staking::PoolCreated::decode_log(&log.inner) {
                println!("\n✓ StakePool created successfully!");
                println!("   Pool address: {}", event.pool);
                println!("   Owner: {}", event.owner);
                println!("   Pool index: {}", event.poolIndex);
                found_pool = Some(event.pool);
                break;
            }
        }
        let stake_pool = found_pool.ok_or(anyhow::anyhow!("Failed to find PoolCreated event"))?;

        println!("\nUse this address with validator join:");
        println!("  --stake-pool {stake_pool}");

        Ok(())
    }
}
