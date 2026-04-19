use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::TransactionRequest;
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::PrivateKeySigner;
use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use std::str::FromStr;

use crate::{
    command::Executable,
    localnet::common::{
        derive_rpc_url, load_cluster_toml, resolve_cluster_dir, resolve_config,
        DEFAULT_ANVIL_FAUCET_KEY,
    },
    output::OutputFormat,
    util::{format_ether, parse_ether},
};

#[derive(Debug, Parser)]
pub struct FaucetCommand {
    /// Destination address (0x-prefixed)
    #[clap(long)]
    pub to: String,

    /// Amount to send. Decimal ETH by default; pass --wei to treat as wei.
    #[clap(long)]
    pub amount: String,

    /// Treat --amount as a wei-denominated integer instead of decimal ETH
    #[clap(long)]
    pub wei: bool,

    /// RPC URL. If unset, derived from cluster.toml (first node).
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Funding private key (hex). Defaults to the anvil test key (matches
    /// cluster/faucet.sh) unless GRAVITY_LOCALNET_FAUCET_KEY is set or this
    /// flag is passed.
    #[clap(long, env = "GRAVITY_LOCALNET_FAUCET_KEY")]
    pub from_key: Option<String>,

    #[clap(long, env = "GRAVITY_CLUSTER_DIR")]
    pub cluster_dir: Option<String>,

    #[clap(long)]
    pub config: Option<String>,

    /// Output format
    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Serialize)]
struct FaucetReceipt {
    tx_hash: String,
    from: String,
    to: String,
    amount_wei: String,
    block_number: Option<u64>,
    status: bool,
    gas_used: Option<u64>,
}

impl Executable for FaucetCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl FaucetCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let to = Address::from_str(&self.to)
            .map_err(|e| anyhow::anyhow!("invalid --to address: {e}"))?;
        let amount_wei = if self.wei {
            U256::from_str(&self.amount)
                .map_err(|e| anyhow::anyhow!("invalid --amount wei: {e}"))?
        } else {
            parse_ether(&self.amount)?
        };

        // Resolve RPC URL: CLI/env first, then cluster.toml fallback.
        let rpc_url = match self.rpc_url.clone() {
            Some(u) => u,
            None => {
                let cluster_dir = resolve_cluster_dir(self.cluster_dir.as_deref())?;
                let config = resolve_config(&cluster_dir, self.config.as_deref());
                let cfg = load_cluster_toml(&config)?;
                derive_rpc_url(&cfg)?
            }
        };

        // Resolve faucet key. The default is hardcoded to the well-known anvil
        // test account so `gravity-cli localnet faucet --to 0x... --amount 1`
        // just works on a fresh cluster. Env override is intentional per design.
        let key_hex_owned = self.from_key.clone().unwrap_or_else(|| DEFAULT_ANVIL_FAUCET_KEY.to_string());
        let key_hex = key_hex_owned.trim().strip_prefix("0x").unwrap_or(key_hex_owned.trim());
        let key_bytes = hex::decode(key_hex)
            .map_err(|e| anyhow::anyhow!("faucet key is not valid hex: {e}"))?;
        let signing_key = SigningKey::from_slice(&key_bytes)
            .map_err(|e| anyhow::anyhow!("invalid faucet private key: {e}"))?;
        let signer = PrivateKeySigner::from(signing_key);
        let from = signer.address();

        let is_json = matches!(self.output_format, OutputFormat::Json);
        if !is_json {
            println!("{} {}", "[localnet faucet]".cyan(), format!("rpc: {rpc_url}"));
            println!(
                "{} sending {} ETH from {from:?} → {to:?}",
                "[localnet faucet]".cyan(),
                format_ether(amount_wei)
            );
        }

        let provider = ProviderBuilder::new()
            .wallet(signer)
            .connect_http(rpc_url.parse()?);

        let balance = provider.get_balance(from).await?;
        if balance < amount_wei {
            return Err(anyhow::anyhow!(
                "faucet balance {} ETH is less than requested {} ETH",
                format_ether(balance),
                format_ether(amount_wei)
            ));
        }

        let tx = TransactionRequest::default().to(to).value(amount_wei);
        let pending = provider.send_transaction(tx).await?;
        let tx_hash = *pending.tx_hash();

        if !is_json {
            println!(
                "{} tx submitted: {tx_hash:?} — waiting for receipt…",
                "[localnet faucet]".cyan()
            );
        }
        let receipt = pending.get_receipt().await?;
        let result = FaucetReceipt {
            tx_hash: format!("{tx_hash:?}"),
            from: format!("{from:?}"),
            to: format!("{to:?}"),
            amount_wei: amount_wei.to_string(),
            block_number: receipt.block_number,
            status: receipt.status(),
            gas_used: Some(receipt.gas_used),
        };

        match self.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Plain => {
                let status = if result.status { "success".green() } else { "FAILED".red() };
                println!(
                    "{} {} at block {} (gas_used={})",
                    "[localnet faucet]".cyan(),
                    status,
                    result.block_number.map(|b| b.to_string()).unwrap_or_else(|| "?".into()),
                    result.gas_used.map(|g| g.to_string()).unwrap_or_else(|| "?".into())
                );
            }
        }

        if !result.status {
            return Err(anyhow::anyhow!("faucet tx reverted"));
        }
        Ok(())
    }
}
