use alloy_primitives::{Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_sol_types::{SolCall, SolEvent};
use clap::Parser;

use crate::{
    command::Executable,
    contract::{Staking, STAKING_ADDRESS},
    output::OutputFormat,
    signer::SignerArgs,
    util::{format_ether, parse_ether},
};

#[derive(Debug, Parser)]
pub struct CreateCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Gas limit for the transaction
    #[clap(long, env = "GRAVITY_GAS_LIMIT")]
    pub gas_limit: Option<u64>,

    /// Gas price in wei
    #[clap(long, env = "GRAVITY_GAS_PRICE")]
    pub gas_price: Option<u128>,

    /// Stake amount in ETH
    #[clap(long)]
    pub stake_amount: String,

    /// Lockup duration in seconds (default 30 days)
    #[clap(long, default_value = "2592000")]
    pub lockup_duration: u64,

    /// Output format (injected from global flag)
    #[clap(skip)]
    pub output_format: OutputFormat,

    #[clap(flatten)]
    pub signer: SignerArgs,
}

impl Executable for CreateCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl CreateCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let is_json = matches!(self.output_format, OutputFormat::Json);

        // 1. Initialize Provider and Wallet
        if !is_json {
            println!("Creating new StakePool...\n");
            println!("1. Initializing connection...");
        }

        let rpc_url = self.rpc_url.ok_or_else(|| {
            anyhow::anyhow!(
                "--rpc-url is required. Set via CLI flag, GRAVITY_RPC_URL env var, or ~/.gravity/config.toml"
            )
        })?;
        let gas_limit = self.gas_limit.unwrap_or(2_000_000);
        let gas_price = self.gas_price.unwrap_or(20);

        if !is_json {
            println!("   RPC URL: {rpc_url}");
        }
        let resolved = self.signer.resolve().await?;
        let wallet_address = resolved.address;
        if !is_json {
            println!("   Wallet address: {wallet_address:?}");
            println!("   Staking contract: {STAKING_ADDRESS:?}");
        }

        // Create provider
        let provider =
            ProviderBuilder::new().wallet(resolved.wallet).connect_http(rpc_url.parse()?);

        let chain_id = provider.get_chain_id().await?;
        if !is_json {
            println!("   Chain ID: {chain_id}");
        }
        let balance = provider.get_balance(wallet_address).await?;
        if !is_json {
            println!("   Wallet balance: {} ETH\n", format_ether(balance));
        }

        // 2. Create StakePool
        if !is_json {
            println!("2. Creating StakePool...");
        }
        let stake_wei = parse_ether(&self.stake_amount)?;
        if !is_json {
            println!("   Stake amount: {} ETH", self.stake_amount);
        }

        // Calculate lockup expiration timestamp.
        //
        // Unit handling (do not "fix" the `* 1_000_000` without reading this):
        //   - `block.header.timestamp` is in **seconds** (EVM standard).
        //   - `self.lockup_duration` is in **seconds** (see the CLI flag doc above).
        //   - The Staking contract's `lockedUntil` field is in **microseconds**, matching
        //     `initial_locked_until_micros` in cluster genesis files and the
        //     `get_current_time_micros()`-based callers in `gravity_e2e/.../staking/`.
        //
        // So we add two second-valued quantities and convert the sum to microseconds
        // exactly once at the end.
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await?
            .ok_or(anyhow::anyhow!("Failed to get latest block"))?;
        let current_timestamp = block.header.timestamp;
        if !is_json {
            println!("   Current timestamp: {current_timestamp} (seconds)");
            println!("   Lockup duration: {} seconds", self.lockup_duration);
        }
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
                gas: Some(gas_limit),
                gas_price: Some(gas_price),
                ..Default::default()
            })
            .await?;
        let tx_hash = *pending_tx.tx_hash();
        if !is_json {
            println!("   Transaction hash: {tx_hash}");
        }
        let _ = pending_tx
            .with_required_confirmations(2)
            .with_timeout(Some(std::time::Duration::from_secs(60)))
            .watch()
            .await?;

        let receipt = provider
            .get_transaction_receipt(tx_hash)
            .await?
            .ok_or(anyhow::anyhow!("Failed to get transaction receipt"))?;
        let block_number =
            receipt.block_number.ok_or(anyhow::anyhow!("Failed to get block number"))?;
        if !is_json {
            println!("   Transaction confirmed, block number: {block_number}");
            println!("   Gas used: {}", receipt.gas_used);
            println!(
                "   Transaction cost: {} ETH",
                format_ether(
                    U256::from(receipt.effective_gas_price) * U256::from(receipt.gas_used)
                )
            );
        }

        // Parse PoolCreated event to get the new pool address
        let mut found_pool = None;
        for log in receipt.logs() {
            if let Ok(event) = Staking::PoolCreated::decode_log(&log.inner) {
                found_pool = Some((event.pool, event.owner, event.poolIndex));
                break;
            }
        }
        let (stake_pool, owner, pool_index) =
            found_pool.ok_or(anyhow::anyhow!("Failed to find PoolCreated event"))?;

        if is_json {
            let result = serde_json::json!({
                "pool_address": format!("{stake_pool}"),
                "owner": format!("{owner}"),
                "pool_index": pool_index,
                "tx_hash": format!("{tx_hash}"),
                "block_number": block_number,
                "gas_used": receipt.gas_used,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("\n✓ StakePool created successfully!");
            println!("   Pool address: {stake_pool}");
            println!("   Owner: {owner}");
            println!("   Pool index: {pool_index}");
            println!("\nUse this address with validator join:");
            println!("  --stake-pool {stake_pool}");
        }

        Ok(())
    }
}
