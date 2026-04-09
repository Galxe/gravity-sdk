use crate::{
    chain_monitor::{
        abi::{NativeMinted, TokensLocked},
        checkpoint::SharedCheckpoint,
        config::BridgeTimeoutConfig,
        provider::{self, HttpProvider},
    },
    config::Priority,
    notifier::Notifier,
};
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct BridgeTimeoutMonitor {
    config: BridgeTimeoutConfig,
    gbridge_sender: Address,
    gbridge_receiver: Address,
    eth_provider: HttpProvider,
    gravity_provider: HttpProvider,
    notifier: Notifier,
    checkpoint: SharedCheckpoint,
    eth_poll_interval: Duration,
    gravity_poll_interval: Duration,
    confirmation_blocks: u64,
    max_consecutive_failures: u32,
}

impl BridgeTimeoutMonitor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: BridgeTimeoutConfig,
        gbridge_sender: Address,
        gbridge_receiver: Address,
        eth_provider: HttpProvider,
        gravity_provider: HttpProvider,
        notifier: Notifier,
        checkpoint: SharedCheckpoint,
        eth_poll_interval: Duration,
        confirmation_blocks: u64,
        max_consecutive_failures: u32,
    ) -> Self {
        let gravity_poll_interval = Duration::from_secs(config.check_interval_seconds);
        Self {
            config,
            gbridge_sender,
            gbridge_receiver,
            eth_provider,
            gravity_provider,
            notifier,
            checkpoint,
            eth_poll_interval,
            gravity_poll_interval,
            confirmation_blocks,
            max_consecutive_failures,
        }
    }

    pub async fn run(self) {
        let mut eth_interval = tokio::time::interval(self.eth_poll_interval);
        let mut gravity_interval = tokio::time::interval(self.gravity_poll_interval);
        let mut timeout_interval =
            tokio::time::interval(Duration::from_secs(self.config.check_interval_seconds));

        let mut eth_failures: u32 = 0;
        let mut gravity_failures: u32 = 0;

        // Local cache of block timestamps to avoid repeated RPC calls
        let mut block_timestamps: HashMap<u64, u64> = HashMap::new();

        loop {
            tokio::select! {
                _ = eth_interval.tick() => {
                    match self.poll_ethereum(&mut block_timestamps).await {
                        Ok(()) => { eth_failures = 0; }
                        Err(e) => {
                            eth_failures += 1;
                            eprintln!("Bridge timeout: Ethereum poll error ({eth_failures}): {e:?}");
                            if eth_failures >= self.max_consecutive_failures {
                                let msg = format!("Bridge timeout monitor lost Ethereum RPC ({eth_failures} failures)");
                                let _ = self.notifier.alert(&msg, "CHAIN_MONITOR", Priority::P0).await;
                                eth_failures = 0;
                            }
                        }
                    }
                }
                _ = gravity_interval.tick() => {
                    match self.poll_gravity().await {
                        Ok(()) => { gravity_failures = 0; }
                        Err(e) => {
                            gravity_failures += 1;
                            eprintln!("Bridge timeout: Gravity poll error ({gravity_failures}): {e:?}");
                            if gravity_failures >= self.max_consecutive_failures {
                                let msg = format!("Bridge timeout monitor lost Gravity RPC ({gravity_failures} failures)");
                                let _ = self.notifier.alert(&msg, "CHAIN_MONITOR", Priority::P0).await;
                                gravity_failures = 0;
                            }
                        }
                    }
                }
                _ = timeout_interval.tick() => {
                    self.check_timeouts().await;
                }
            }
        }
    }

    /// Poll Ethereum for new TokensLocked events and record them as pending.
    async fn poll_ethereum(&self, block_timestamps: &mut HashMap<u64, u64>) -> anyhow::Result<()> {
        let safe_block =
            provider::get_safe_block_number(&self.eth_provider, self.confirmation_blocks).await?;
        let from_block = {
            let ckpt = self.checkpoint.lock().unwrap();
            if ckpt.ethereum_block == 0 {
                safe_block
            } else {
                ckpt.ethereum_block + 1
            }
        };

        if from_block > safe_block {
            return Ok(());
        }

        let filter = Filter::new()
            .address(self.gbridge_sender)
            .event_signature(TokensLocked::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(safe_block);

        let logs = provider::get_logs(&self.eth_provider, filter).await?;

        for log in &logs {
            if let Ok(event) = TokensLocked::decode_log_data(log.data()) {
                let block_number = log.block_number.unwrap_or(0);

                // Get block timestamp
                let timestamp = if let Some(&ts) = block_timestamps.get(&block_number) {
                    ts
                } else {
                    let ts = self.get_block_timestamp(block_number).await.unwrap_or_else(|_| {
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
                    });
                    block_timestamps.insert(block_number, ts);
                    ts
                };

                let nonce = event.nonce;
                let mut ckpt = self.checkpoint.lock().unwrap();
                ckpt.pending_nonces.insert(nonce, timestamp);
                println!(
                    "Bridge timeout: Tracking nonce {nonce} (block {block_number}, timestamp {timestamp})"
                );
            }
        }

        // Update ethereum block cursor
        {
            let mut ckpt = self.checkpoint.lock().unwrap();
            ckpt.ethereum_block = ckpt.ethereum_block.max(safe_block);
        }

        Ok(())
    }

    /// Poll Gravity for NativeMinted events and remove matched nonces from pending.
    async fn poll_gravity(&self) -> anyhow::Result<()> {
        // Gravity has finality, no confirmation blocks needed
        let latest_block = self.gravity_provider.get_block_number().await?;
        let from_block = {
            let ckpt = self.checkpoint.lock().unwrap();
            if ckpt.gravity_block == 0 {
                latest_block
            } else {
                ckpt.gravity_block + 1
            }
        };

        if from_block > latest_block {
            return Ok(());
        }

        let filter = Filter::new()
            .address(self.gbridge_receiver)
            .event_signature(NativeMinted::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(latest_block);

        let logs = provider::get_logs(&self.gravity_provider, filter).await?;

        for log in &logs {
            if let Ok(event) = NativeMinted::decode_log_data(log.data()) {
                let nonce = event.nonce;
                let mut ckpt = self.checkpoint.lock().unwrap();
                if ckpt.pending_nonces.remove(&nonce).is_some() {
                    println!("Bridge timeout: Nonce {nonce} confirmed on Gravity");
                }
            }
        }

        // Update gravity block cursor
        {
            let mut ckpt = self.checkpoint.lock().unwrap();
            ckpt.gravity_block = ckpt.gravity_block.max(latest_block);
        }

        Ok(())
    }

    /// Check for timed-out nonces and alert.
    async fn check_timeouts(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        let timed_out: Vec<(u128, u64)> = {
            let ckpt = self.checkpoint.lock().unwrap();
            ckpt.pending_nonces
                .iter()
                .filter(|(_, &ts)| now.saturating_sub(ts) > self.config.timeout_seconds)
                .map(|(&nonce, &ts)| (nonce, ts))
                .collect()
        };

        for (nonce, timestamp) in &timed_out {
            let elapsed = now.saturating_sub(*timestamp);
            let msg = format!(
                "Bridge transaction TIMEOUT!\nNonce: {nonce}\nLocked on Ethereum at: {} ({}s ago)\nNot yet confirmed on Gravity after {} seconds threshold\nInvestigate relay pipeline!",
                *timestamp, elapsed, self.config.timeout_seconds
            );
            if let Err(e) = self.notifier.alert(&msg, "BRIDGE_TIMEOUT", self.config.priority).await
            {
                eprintln!("Failed to send bridge timeout alert: {e:?}");
            }
        }

        // Remove alerted nonces to avoid repeated alerts
        // (they'll re-alert if still pending after another timeout_seconds)
        if !timed_out.is_empty() {
            let mut ckpt = self.checkpoint.lock().unwrap();
            for (nonce, _) in &timed_out {
                ckpt.pending_nonces.remove(nonce);
            }
        }
    }

    async fn get_block_timestamp(&self, block_number: u64) -> anyhow::Result<u64> {
        let block = self
            .eth_provider
            .get_block_by_number(block_number.into())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Block {block_number} not found"))?;
        Ok(block.header.timestamp)
    }
}
