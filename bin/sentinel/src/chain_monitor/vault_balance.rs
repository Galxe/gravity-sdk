use crate::{
    chain_monitor::{
        abi::balanceOfCall, checkpoint::SharedCheckpoint, config::VaultBalanceConfig,
        provider::HttpProvider,
    },
    config::Priority,
    notifier::Notifier,
};
use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use alloy_sol_types::SolCall;
use std::time::Duration;

pub struct VaultBalanceMonitor {
    config: VaultBalanceConfig,
    gtoken_address: Address,
    gbridge_sender: Address,
    drop_absolute_threshold: U256,
    provider: HttpProvider,
    notifier: Notifier,
    checkpoint: SharedCheckpoint,
    poll_interval: Duration,
    max_consecutive_failures: u32,
}

impl VaultBalanceMonitor {
    pub fn new(
        config: VaultBalanceConfig,
        gtoken_address: Address,
        gbridge_sender: Address,
        provider: HttpProvider,
        notifier: Notifier,
        checkpoint: SharedCheckpoint,
        poll_interval: Duration,
        max_consecutive_failures: u32,
    ) -> anyhow::Result<Self> {
        let drop_absolute_threshold = config
            .drop_absolute_threshold_wei
            .parse::<U256>()
            .map_err(|e| anyhow::anyhow!("Invalid drop_absolute_threshold_wei: {e}"))?;

        Ok(Self {
            config,
            gtoken_address,
            gbridge_sender,
            drop_absolute_threshold,
            provider,
            notifier,
            checkpoint,
            poll_interval,
            max_consecutive_failures,
        })
    }

    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.poll_interval);
        let mut consecutive_failures: u32 = 0;
        let mut backoff = Duration::from_secs(1);

        loop {
            interval.tick().await;

            match self.poll_once().await {
                Ok(()) => {
                    consecutive_failures = 0;
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    consecutive_failures += 1;
                    eprintln!(
                        "Vault balance monitor error (failures: {consecutive_failures}): {e:?}"
                    );

                    if consecutive_failures >= self.max_consecutive_failures {
                        let msg = format!(
                            "Vault balance monitor lost RPC connectivity ({consecutive_failures} failures). Last error: {e}"
                        );
                        if let Err(alert_err) =
                            self.notifier.alert(&msg, "CHAIN_MONITOR", Priority::P0).await
                        {
                            eprintln!("Failed to send connectivity alert: {alert_err:?}");
                        }
                        consecutive_failures = 0;
                    }

                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        // Call balanceOf(gbridge_sender) on gtoken contract
        let call = balanceOfCall { account: self.gbridge_sender };
        let input: Bytes = call.abi_encode().into();

        let tx = TransactionRequest {
            to: Some(TxKind::Call(self.gtoken_address)),
            input: TransactionInput::new(input),
            ..Default::default()
        };

        let result = self.provider.call(tx).await?;
        let current_balance = balanceOfCall::abi_decode_returns(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode balanceOf: {e}"))?;

        // Get previous balance from checkpoint
        let previous_balance = {
            let ckpt = self.checkpoint.lock().unwrap();
            ckpt.last_vault_balance.as_ref().and_then(|s| s.parse::<U256>().ok())
        };

        // Update checkpoint with current balance
        {
            let mut ckpt = self.checkpoint.lock().unwrap();
            ckpt.last_vault_balance = Some(current_balance.to_string());
        }

        // First run — no previous balance to compare against
        let Some(prev) = previous_balance else {
            println!("Vault balance baseline recorded: {} wei", current_balance);
            return Ok(());
        };

        // Only alert on decreases
        if current_balance >= prev {
            return Ok(());
        }

        let drop = prev - current_balance;

        // Check absolute threshold
        let absolute_triggered = drop >= self.drop_absolute_threshold;

        // Check percentage threshold
        let percentage_triggered = if !prev.is_zero() {
            // Calculate percentage: (drop * 10000) / prev gives basis points
            let basis_points = (drop * U256::from(10000)) / prev;
            let threshold_bp = (self.config.drop_percentage_threshold * 100.0) as u64;
            basis_points >= U256::from(threshold_bp)
        } else {
            false
        };

        if absolute_triggered || percentage_triggered {
            let drop_pct = if !prev.is_zero() {
                let bp = (drop * U256::from(10000)) / prev;
                format!("{:.2}%", bp.to::<u64>() as f64 / 100.0)
            } else {
                "N/A".to_string()
            };

            let msg = format!(
                "Vault balance alert!\nPrevious: {} wei\nCurrent: {} wei\nDrop: {} wei ({drop_pct})\nContract: {}",
                prev, current_balance, drop, self.gbridge_sender
            );
            if let Err(e) =
                self.notifier.alert(&msg, "BRIDGE_VAULT_BALANCE", self.config.priority).await
            {
                eprintln!("Failed to send vault balance alert: {e:?}");
            }
        }

        Ok(())
    }
}
