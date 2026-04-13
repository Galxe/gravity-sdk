use crate::{
    chain_monitor::{
        abi::{EmergencyWithdraw, TokensLocked},
        checkpoint::SharedCheckpoint,
        config::LargeWithdrawalConfig,
        provider::{self, HttpProvider},
    },
    config::Priority,
    notifier::Notifier,
};
use alloy_primitives::{Address, U256};
use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use std::time::Duration;

pub struct LargeWithdrawalMonitor {
    config: LargeWithdrawalConfig,
    gbridge_sender: Address,
    threshold: U256,
    provider: HttpProvider,
    notifier: Notifier,
    checkpoint: SharedCheckpoint,
    poll_interval: Duration,
    confirmation_blocks: u64,
    max_consecutive_failures: u32,
}

impl LargeWithdrawalMonitor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: LargeWithdrawalConfig,
        gbridge_sender: Address,
        provider: HttpProvider,
        notifier: Notifier,
        checkpoint: SharedCheckpoint,
        poll_interval: Duration,
        confirmation_blocks: u64,
        max_consecutive_failures: u32,
    ) -> anyhow::Result<Self> {
        let threshold = config
            .threshold_wei
            .parse::<U256>()
            .map_err(|e| anyhow::anyhow!("Invalid threshold_wei: {e}"))?;

        Ok(Self {
            config,
            gbridge_sender,
            threshold,
            provider,
            notifier,
            checkpoint,
            poll_interval,
            confirmation_blocks,
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
                        "Large withdrawal monitor error (failures: {consecutive_failures}): {e:?}"
                    );

                    if consecutive_failures >= self.max_consecutive_failures {
                        let msg = format!(
                            "Chain monitor lost connectivity to Ethereum RPC ({consecutive_failures} consecutive failures). Last error: {e}"
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
        let safe_block =
            provider::get_safe_block_number(&self.provider, self.confirmation_blocks).await?;
        let from_block = {
            let ckpt = self.checkpoint.lock().unwrap();
            if ckpt.ethereum_block == 0 {
                safe_block // Start from current block on first run
            } else {
                ckpt.ethereum_block + 1
            }
        };

        if from_block > safe_block {
            return Ok(()); // No new blocks
        }

        // Fetch TokensLocked events
        let tokens_locked_filter = Filter::new()
            .address(self.gbridge_sender)
            .event_signature(TokensLocked::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(safe_block);

        let locked_logs = provider::get_logs(&self.provider, tokens_locked_filter).await?;

        for log in &locked_logs {
            if let Ok(event) = TokensLocked::decode_log_data(log.data()) {
                if event.amount >= self.threshold {
                    let amount_eth = format_wei_to_eth(event.amount);
                    let msg = format!(
                        "Large withdrawal detected!\nFrom: {}\nRecipient: {}\nAmount: {} tokens ({} wei)\nNonce: {}",
                        event.from, event.recipient, amount_eth, event.amount, event.nonce
                    );
                    if let Err(e) = self
                        .notifier
                        .alert(&msg, "BRIDGE_LARGE_WITHDRAWAL", self.config.priority)
                        .await
                    {
                        eprintln!("Failed to send large withdrawal alert: {e:?}");
                    }
                }
            }
        }

        // Fetch EmergencyWithdraw events - ALWAYS P0 regardless of amount
        let emergency_filter = Filter::new()
            .address(self.gbridge_sender)
            .event_signature(EmergencyWithdraw::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(safe_block);

        let emergency_logs = provider::get_logs(&self.provider, emergency_filter).await?;

        for log in &emergency_logs {
            if let Ok(event) = EmergencyWithdraw::decode_log_data(log.data()) {
                let amount_eth = format_wei_to_eth(event.amount);
                let msg = format!(
                    "CRITICAL: EmergencyWithdraw called on GBridgeSender!\nRecipient: {}\nAmount: {} tokens ({} wei)\nThis drains the vault - investigate immediately!",
                    event.recipient, amount_eth, event.amount
                );
                if let Err(e) =
                    self.notifier.alert(&msg, "BRIDGE_EMERGENCY_WITHDRAW", Priority::P0).await
                {
                    eprintln!("Failed to send emergency withdraw alert: {e:?}");
                }
            }
        }

        // Update checkpoint
        {
            let mut ckpt = self.checkpoint.lock().unwrap();
            ckpt.ethereum_block = ckpt.ethereum_block.max(safe_block);
        }

        Ok(())
    }
}

/// Format a U256 wei value to a human-readable ETH-like string.
fn format_wei_to_eth(wei: U256) -> String {
    let divisor = U256::from(10u64.pow(18));
    let whole = wei / divisor;
    let remainder = wei % divisor;
    format!("{whole}.{remainder:0>18}").trim_end_matches('0').trim_end_matches('.').to_string()
}
