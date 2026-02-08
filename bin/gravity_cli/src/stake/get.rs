use alloy_primitives::{Address, Bytes, TxKind, B256, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{BlockNumberOrTag, Filter, TransactionInput, TransactionRequest};
use alloy_sol_types::{SolCall, SolValue};
use clap::Parser;
use std::str::FromStr;

use crate::{
    command::Executable,
    contract::{Staking, STAKING_ADDRESS},
    util::format_ether,
};

// Event signature: PoolCreated(address indexed creator, address indexed pool, address indexed
// owner, address staker, uint256 poolIndex) keccak256("PoolCreated(address,address,address,address,
// uint256)") = 0x45d43f0d6767b37a70a442985866e6b596772c5a7f529f2b9f6798423b26a3e8
const POOL_CREATED_EVENT_SIGNATURE: &str =
    "0x45d43f0d6767b37a70a442985866e6b596772c5a7f529f2b9f6798423b26a3e8";

#[derive(Debug, Parser)]
pub struct GetCommand {
    /// RPC URL for gravity node
    #[clap(long)]
    pub rpc_url: String,

    /// Owner address to query
    #[clap(long)]
    pub owner: String,

    /// Starting block (default: auto, which queries the latest block and goes back up to 100000
    /// blocks to stay within reth's max block range limit)
    #[clap(long, default_value = "auto")]
    pub from_block: String,

    /// Ending block (default: latest)
    #[clap(long, default_value = "latest")]
    pub to_block: String,

    /// Query voting power for each pool
    #[clap(long, default_value = "true")]
    pub show_voting_power: bool,
}

/// Reth's default max block range for log queries
const MAX_BLOCK_RANGE: u64 = 90_000;

impl Executable for GetCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl GetCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        println!("Querying StakePools for owner: {}\n", self.owner);

        // Parse owner address and pad to 32 bytes for topic filtering
        let owner_addr = Address::from_str(&self.owner)?;
        let owner_topic = format!("0x{:0>64}", hex::encode(owner_addr.as_slice()));

        // Create provider
        let provider = ProviderBuilder::new().connect_http(self.rpc_url.parse()?);

        // Resolve to_block first (needed for auto from_block calculation)
        let to_block = if self.to_block == "earliest" {
            BlockNumberOrTag::Earliest
        } else if self.to_block == "latest" {
            BlockNumberOrTag::Latest
        } else {
            BlockNumberOrTag::Number(self.to_block.parse()?)
        };

        // Resolve from_block, handling "auto" and "earliest" by capping to MAX_BLOCK_RANGE
        let from_block = if self.from_block == "auto" || self.from_block == "earliest" {
            let latest = provider.get_block_number().await?;
            let start = latest.saturating_sub(MAX_BLOCK_RANGE);
            BlockNumberOrTag::Number(start)
        } else if self.from_block == "latest" {
            BlockNumberOrTag::Latest
        } else {
            BlockNumberOrTag::Number(self.from_block.parse()?)
        };

        // Construct filter for PoolCreated events
        // topics[0] = event signature
        // topics[1] = creator (any)
        // topics[2] = pool (any)
        // topics[3] = owner (filtered)
        let filter = Filter::new()
            .address(STAKING_ADDRESS)
            .from_block(from_block)
            .to_block(to_block)
            .event_signature(POOL_CREATED_EVENT_SIGNATURE.parse::<B256>()?)
            .topic3(owner_topic.parse::<B256>()?);

        println!("Searching for PoolCreated events...");
        println!("   Contract: {STAKING_ADDRESS:?}");
        println!("   Owner: {owner_addr:?}");
        println!("   Block range: {from_block} to {to_block}\n");

        let logs = provider.get_logs(&filter).await?;

        if logs.is_empty() {
            println!("No StakePools found for this owner.");
            return Ok(());
        }

        println!("Found {} StakePool(s):\n", logs.len());

        // Print header
        if self.show_voting_power {
            println!("{:<44} {:<16} {:<12}", "Pool Address", "Voting Power", "Block Number");
            println!("{}", "-".repeat(76));
        } else {
            println!("{:<44} {:<12}", "Pool Address", "Block Number");
            println!("{}", "-".repeat(58));
        }

        for log in logs {
            // Extract pool address from topics[2]
            let pool_topic = log.topics().get(2).ok_or(anyhow::anyhow!("Missing pool topic"))?;
            // Pool address is the last 20 bytes of the 32-byte topic
            let pool_bytes = &pool_topic.as_slice()[12..];
            let pool_address = Address::from_slice(pool_bytes);

            let block_number = log.block_number.ok_or(anyhow::anyhow!("Missing block number"))?;

            if self.show_voting_power {
                // Query voting power
                let call = Staking::getPoolVotingPowerNowCall { pool: pool_address };
                let input: Bytes = call.abi_encode().into();
                let result = provider
                    .call(TransactionRequest {
                        to: Some(TxKind::Call(STAKING_ADDRESS)),
                        input: TransactionInput::new(input),
                        ..Default::default()
                    })
                    .await;

                let voting_power = match result {
                    Ok(data) => {
                        let power = U256::abi_decode(&data)
                            .map_err(|e| anyhow::anyhow!("Failed to decode voting power: {e}"))?;
                        format!("{} ETH", format_ether(power))
                    }
                    Err(_) => "N/A".to_string(),
                };

                println!(
                    "{:<44} {:<16} {:<12}",
                    format!("{pool_address:?}"),
                    voting_power,
                    block_number
                );
            } else {
                println!("{:<44} {:<12}", format!("{pool_address:?}"), block_number);
            }
        }

        println!();
        Ok(())
    }
}
