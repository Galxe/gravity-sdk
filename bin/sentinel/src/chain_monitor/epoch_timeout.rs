use crate::{
    chain_monitor::{
        abi::{currentEpochCall, epochIntervalMicrosCall, lastReconfigurationTimeCall},
        config::EpochTimeoutConfig,
        provider::HttpProvider,
    },
    config::Priority,
    notifier::Notifier,
};
use alloy_primitives::{address, Address, Bytes, TxKind};
use alloy_provider::Provider;
use alloy_rpc_types::eth::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_sol_types::SolCall;
use std::time::Duration;

/// Reconfiguration contract address (SystemAddresses.RECONFIGURATION)
const RECONFIGURATION_ADDRESS: Address = address!("00000000000000000000000000000001625F2003");

/// EpochConfig contract address (SystemAddresses.EPOCH_CONFIG)
const EPOCH_CONFIG_ADDRESS: Address = address!("00000000000000000000000000000001625F1005");

pub struct EpochTimeoutMonitor {
    config: EpochTimeoutConfig,
    provider: HttpProvider,
    notifier: Notifier,
    check_interval: Duration,
    max_consecutive_failures: u32,
}

impl EpochTimeoutMonitor {
    pub fn new(
        config: EpochTimeoutConfig,
        provider: HttpProvider,
        notifier: Notifier,
        check_interval: Duration,
        max_consecutive_failures: u32,
    ) -> Self {
        Self { config, provider, notifier, check_interval, max_consecutive_failures }
    }

    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.check_interval);
        let mut consecutive_failures: u32 = 0;
        let mut backoff = Duration::from_secs(1);
        let mut alerted_epoch: Option<u64> = None;

        loop {
            interval.tick().await;

            match self.poll_once().await {
                Ok(Some((epoch, overdue_secs))) => {
                    consecutive_failures = 0;
                    backoff = Duration::from_secs(1);

                    // Only alert once per epoch to avoid spam
                    if alerted_epoch != Some(epoch) {
                        let msg = format!(
                            "Epoch change timeout!\n\
                             Current Epoch: {epoch}\n\
                             Overdue: {overdue_secs}s (threshold: {}s)\n\
                             Epoch transition should have occurred but hasn't.",
                            self.config.overdue_threshold_seconds
                        );
                        if let Err(e) = self
                            .notifier
                            .alert(&msg, "EPOCH_TIMEOUT", self.config.priority)
                            .await
                        {
                            eprintln!("Failed to send epoch timeout alert: {e:?}");
                        }
                        alerted_epoch = Some(epoch);
                    }
                }
                Ok(None) => {
                    consecutive_failures = 0;
                    backoff = Duration::from_secs(1);
                    // Epoch is on time — reset alert state so we can alert again next epoch
                    alerted_epoch = None;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    eprintln!(
                        "Epoch timeout monitor error (failures: {consecutive_failures}): {e:?}"
                    );

                    if consecutive_failures >= self.max_consecutive_failures {
                        let msg = format!(
                            "Epoch timeout monitor lost RPC connectivity \
                             ({consecutive_failures} failures). Last error: {e}"
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

    /// Returns `Ok(Some((epoch, overdue_secs)))` if overdue, `Ok(None)` if on time.
    async fn poll_once(&self) -> anyhow::Result<Option<(u64, u64)>> {
        // Query current epoch
        let input: Bytes = currentEpochCall {}.abi_encode().into();
        let result = self
            .provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let current_epoch = currentEpochCall::abi_decode_returns(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode currentEpoch: {e}"))?;

        // Query last reconfiguration time (microseconds)
        let input: Bytes = lastReconfigurationTimeCall {}.abi_encode().into();
        let result = self
            .provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(RECONFIGURATION_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let last_reconfig_time_us =
            lastReconfigurationTimeCall::abi_decode_returns(&result)
                .map_err(|e| anyhow::anyhow!("Failed to decode lastReconfigurationTime: {e}"))?;

        // Query epoch interval (microseconds)
        let input: Bytes = epochIntervalMicrosCall {}.abi_encode().into();
        let result = self
            .provider
            .call(TransactionRequest {
                to: Some(TxKind::Call(EPOCH_CONFIG_ADDRESS)),
                input: TransactionInput::new(input),
                ..Default::default()
            })
            .await?;
        let interval_us = epochIntervalMicrosCall::abi_decode_returns(&result)
            .map_err(|e| anyhow::anyhow!("Failed to decode epochIntervalMicros: {e}"))?;

        // Get latest block timestamp
        let latest_block = self
            .provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch latest block"))?;

        let block_timestamp_us = latest_block.header.timestamp * 1_000_000;
        let running_us = block_timestamp_us.saturating_sub(last_reconfig_time_us);
        let running_secs = running_us / 1_000_000;
        let expected_secs = interval_us / 1_000_000;

        if running_secs > expected_secs {
            let overdue_secs = running_secs - expected_secs;
            if overdue_secs >= self.config.overdue_threshold_seconds {
                return Ok(Some((current_epoch, overdue_secs)));
            }
        }

        Ok(None)
    }
}
