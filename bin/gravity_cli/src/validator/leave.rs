use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::eth::{TransactionInput, TransactionRequest};
use alloy_sol_types::{SolCall, SolEvent, SolType, SolValue};
use clap::Parser;
use std::str::FromStr;

use crate::{
    command::Executable,
    contract::{
        status_from_u8, ValidatorManagement, ValidatorRecord, ValidatorStatus,
        VALIDATOR_MANAGER_ADDRESS,
    },
    signer::SignerArgs,
    util::format_ether,
};

#[derive(Debug, Parser)]
pub struct LeaveCommand {
    /// RPC URL for gravity node
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Gas limit for the transaction
    #[clap(long, env = "GRAVITY_GAS_LIMIT")]
    pub gas_limit: Option<u64>,

    /// Gas price in wei
    #[clap(long, env = "GRAVITY_GAS_PRICE")]
    pub gas_price: Option<u128>,

    /// StakePool address (validator identity)
    #[clap(long)]
    pub stake_pool: String,

    /// Signing-key source: defaults to interactive stdin prompt, or pass
    /// `--kms <resource>` to sign via Cloud KMS, or `--private-key-env <VAR>`
    /// to read the hex key from an env var.
    #[clap(flatten)]
    pub signer: SignerArgs,
}

impl Executable for LeaveCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl LeaveCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let rpc_url = self.rpc_url.ok_or_else(|| {
            anyhow::anyhow!(
                "--rpc-url is required. Set via CLI flag, GRAVITY_RPC_URL env var, or ~/.gravity/config.toml"
            )
        })?;
        let gas_limit = self.gas_limit.unwrap_or(2_000_000);
        let gas_price = self.gas_price.unwrap_or(20);

        // 1. Initialize Provider and Wallet
        println!("1. Initializing connection...");

        println!("   RPC URL: {rpc_url}");
        let resolved = self.signer.resolve().await?;
        let wallet_address = resolved.address;
        println!("   Wallet address: {wallet_address:?}");

        println!("   Contract address: {VALIDATOR_MANAGER_ADDRESS:?}");

        // Create provider
        let provider =
            ProviderBuilder::new().wallet(resolved.wallet).connect_http(rpc_url.parse()?);

        let chain_id = provider.get_chain_id().await?;
        println!("   Chain ID: {chain_id}\n");

        // 2. Check validator information
        println!("2. Checking validator information...");
        let stake_pool = Address::from_str(&self.stake_pool)?;

        // First check if it's a registered validator
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

        if !is_validator {
            return Err(anyhow::anyhow!("StakePool is not registered as a validator"));
        }

        // Get validator record
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

        // Check if validator status allows leaving
        match status {
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
                return Err(anyhow::anyhow!("Validator status {status:?} does not allow leaving"));
            }
        }

        // 3. Leave validator set
        println!("3. Leaving validator set...");
        let call = ValidatorManagement::leaveValidatorSetCall { stakePool: stake_pool };
        let input: Bytes = call.abi_encode().into();
        let pending_tx = provider
            .send_transaction(TransactionRequest {
                from: Some(wallet_address),
                to: Some(TxKind::Call(VALIDATOR_MANAGER_ADDRESS)),
                input: TransactionInput::new(input),
                gas: Some(gas_limit),
                gas_price: Some(gas_price),
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

        // Check leave event
        let mut found_leave_event = false;
        for log in receipt.logs() {
            if let Ok(event) = ValidatorManagement::ValidatorLeaveRequested::decode_log(&log.inner)
            {
                println!("   Leave request successful!");
                println!("   - StakePool: {}", event.stakePool);
                found_leave_event = true;
                break;
            }
        }

        if !found_leave_event {
            println!("   Leave event not found\n");
            return Err(anyhow::anyhow!("Failed to find ValidatorLeaveRequested event"));
        }
        println!();

        // 4. Final status check
        println!("4. Final status check...");
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
            ValidatorStatus::PENDING_INACTIVE => {
                println!("   Validator status is PENDING_INACTIVE");
                println!("   Will become INACTIVE in the next epoch\n");
            }
            ValidatorStatus::INACTIVE => {
                println!("   Validator status is INACTIVE");
                println!("   Successfully left the validator set\n");
            }
            _ => {
                println!("   Validator status is {validator_status:?}, unexpected status\n");
                return Err(anyhow::anyhow!("Unexpected validator status: {validator_status:?}"));
            }
        }
        Ok(())
    }
}
