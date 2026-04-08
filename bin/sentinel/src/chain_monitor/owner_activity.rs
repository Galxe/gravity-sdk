use crate::{
    chain_monitor::{
        abi::{
            ERC20Recovered, EmergencyWithdraw, FeeConfigUpdated, FeeRecipientUpdated,
            FeesWithdrawn,
        },
        checkpoint::SharedCheckpoint,
        config::OwnerActivityConfig,
        provider::{self, HttpProvider},
    },
    config::Priority,
    notifier::Notifier,
};
use alloy_primitives::Address;
use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use std::time::Duration;

pub struct OwnerActivityMonitor {
    config: OwnerActivityConfig,
    gbridge_sender: Address,
    gravity_portal: Address,
    provider: HttpProvider,
    notifier: Notifier,
    checkpoint: SharedCheckpoint,
    poll_interval: Duration,
    confirmation_blocks: u64,
    max_consecutive_failures: u32,
}

impl OwnerActivityMonitor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: OwnerActivityConfig,
        gbridge_sender: Address,
        gravity_portal: Address,
        provider: HttpProvider,
        notifier: Notifier,
        checkpoint: SharedCheckpoint,
        poll_interval: Duration,
        confirmation_blocks: u64,
        max_consecutive_failures: u32,
    ) -> Self {
        Self {
            config,
            gbridge_sender,
            gravity_portal,
            provider,
            notifier,
            checkpoint,
            poll_interval,
            confirmation_blocks,
            max_consecutive_failures,
        }
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
                        "Owner activity monitor error (failures: {consecutive_failures}): {e:?}"
                    );

                    if consecutive_failures >= self.max_consecutive_failures {
                        let msg = format!(
                            "Owner activity monitor lost RPC connectivity ({consecutive_failures} failures)"
                        );
                        let _ = self
                            .notifier
                            .alert(&msg, "CHAIN_MONITOR", Priority::P0)
                            .await;
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
                safe_block
            } else {
                ckpt.ethereum_block + 1
            }
        };

        if from_block > safe_block {
            return Ok(());
        }

        // Monitor GBridgeSender privileged events
        self.check_emergency_withdraw(from_block, safe_block).await?;
        self.check_erc20_recovered(from_block, safe_block).await?;

        // Monitor GravityPortal privileged events
        self.check_fee_config_updated(from_block, safe_block).await?;
        self.check_fee_recipient_updated(from_block, safe_block).await?;
        self.check_fees_withdrawn(from_block, safe_block).await?;

        Ok(())
    }

    async fn check_emergency_withdraw(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        let filter = Filter::new()
            .address(self.gbridge_sender)
            .event_signature(EmergencyWithdraw::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider::get_logs(&self.provider, filter).await?;
        for log in &logs {
            if let Ok(event) = EmergencyWithdraw::decode_log_data(log.data()) {
                let msg = format!(
                    "OWNER ACTIVITY: EmergencyWithdraw called!\nRecipient: {}\nAmount: {} wei\nContract: GBridgeSender ({})",
                    event.recipient, event.amount, self.gbridge_sender
                );
                let _ = self
                    .notifier
                    .alert(&msg, "OWNER_ACTIVITY", self.config.priority)
                    .await;
            }
        }
        Ok(())
    }

    async fn check_erc20_recovered(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        let filter = Filter::new()
            .address(self.gbridge_sender)
            .event_signature(ERC20Recovered::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider::get_logs(&self.provider, filter).await?;
        for log in &logs {
            if let Ok(event) = ERC20Recovered::decode_log_data(log.data()) {
                let msg = format!(
                    "OWNER ACTIVITY: ERC20Recovered called!\nToken: {}\nRecipient: {}\nAmount: {} wei\nContract: GBridgeSender ({})",
                    event.token, event.recipient, event.amount, self.gbridge_sender
                );
                let _ = self
                    .notifier
                    .alert(&msg, "OWNER_ACTIVITY", self.config.priority)
                    .await;
            }
        }
        Ok(())
    }

    async fn check_fee_config_updated(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        let filter = Filter::new()
            .address(self.gravity_portal)
            .event_signature(FeeConfigUpdated::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider::get_logs(&self.provider, filter).await?;
        for log in &logs {
            if let Ok(event) = FeeConfigUpdated::decode_log_data(log.data()) {
                let msg = format!(
                    "OWNER ACTIVITY: FeeConfigUpdated!\nBaseFee: {} wei\nFeePerByte: {} wei\nContract: GravityPortal ({})",
                    event.baseFee, event.feePerByte, self.gravity_portal
                );
                let _ = self
                    .notifier
                    .alert(&msg, "OWNER_ACTIVITY", self.config.priority)
                    .await;
            }
        }
        Ok(())
    }

    async fn check_fee_recipient_updated(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        let filter = Filter::new()
            .address(self.gravity_portal)
            .event_signature(FeeRecipientUpdated::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider::get_logs(&self.provider, filter).await?;
        for log in &logs {
            if let Ok(event) = FeeRecipientUpdated::decode_log_data(log.data()) {
                let msg = format!(
                    "OWNER ACTIVITY: FeeRecipientUpdated!\nOld: {}\nNew: {}\nContract: GravityPortal ({})",
                    event.oldRecipient, event.newRecipient, self.gravity_portal
                );
                let _ = self
                    .notifier
                    .alert(&msg, "OWNER_ACTIVITY", self.config.priority)
                    .await;
            }
        }
        Ok(())
    }

    async fn check_fees_withdrawn(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        let filter = Filter::new()
            .address(self.gravity_portal)
            .event_signature(FeesWithdrawn::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider::get_logs(&self.provider, filter).await?;
        for log in &logs {
            if let Ok(event) = FeesWithdrawn::decode_log_data(log.data()) {
                let msg = format!(
                    "OWNER ACTIVITY: FeesWithdrawn!\nRecipient: {}\nAmount: {} wei\nContract: GravityPortal ({})",
                    event.recipient, event.amount, self.gravity_portal
                );
                let _ = self
                    .notifier
                    .alert(&msg, "OWNER_ACTIVITY", self.config.priority)
                    .await;
            }
        }
        Ok(())
    }
}
