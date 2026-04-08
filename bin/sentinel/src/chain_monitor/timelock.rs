use crate::{
    chain_monitor::{
        abi::{EmergencyWithdraw, OwnershipTransferStarted, OwnershipTransferred},
        checkpoint::SharedCheckpoint,
        config::TimelockConfig,
        provider::{self, HttpProvider},
    },
    config::Priority,
    notifier::Notifier,
};
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use alloy_sol_types::SolEvent;
use std::time::Duration;

pub struct TimelockMonitor {
    config: TimelockConfig,
    /// All bridge contract addresses to monitor for ownership events
    watched_contracts: Vec<Address>,
    gbridge_sender: Address,
    expected_governance: Option<Address>,
    provider: HttpProvider,
    notifier: Notifier,
    checkpoint: SharedCheckpoint,
    poll_interval: Duration,
    confirmation_blocks: u64,
    max_consecutive_failures: u32,
}

impl TimelockMonitor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: TimelockConfig,
        watched_contracts: Vec<Address>,
        gbridge_sender: Address,
        expected_governance: Option<Address>,
        provider: HttpProvider,
        notifier: Notifier,
        checkpoint: SharedCheckpoint,
        poll_interval: Duration,
        confirmation_blocks: u64,
        max_consecutive_failures: u32,
    ) -> Self {
        Self {
            config,
            watched_contracts,
            gbridge_sender,
            expected_governance,
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
                        "Timelock monitor error (failures: {consecutive_failures}): {e:?}"
                    );

                    if consecutive_failures >= self.max_consecutive_failures {
                        let msg = format!(
                            "Timelock monitor lost RPC connectivity ({consecutive_failures} failures)"
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

        self.check_ownership_transfer_started(from_block, safe_block)
            .await?;
        self.check_ownership_transferred(from_block, safe_block)
            .await?;
        self.check_governance_bypass(from_block, safe_block).await?;

        Ok(())
    }

    /// Monitor OwnershipTransferStarted events on all watched contracts.
    async fn check_ownership_transfer_started(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        for &contract in &self.watched_contracts {
            let filter = Filter::new()
                .address(contract)
                .event_signature(OwnershipTransferStarted::SIGNATURE_HASH)
                .from_block(from_block)
                .to_block(to_block);

            let logs = provider::get_logs(&self.provider, filter).await?;
            for log in &logs {
                if let Ok(event) = OwnershipTransferStarted::decode_log_data(log.data()) {
                    let msg = format!(
                        "OWNERSHIP ALERT: Transfer initiated!\nContract: {contract}\nPrevious Owner: {}\nNew Owner: {}\nThis is step 1 of Ownable2Step - pending acceptance.",
                        event.previousOwner, event.newOwner
                    );
                    let _ = self
                        .notifier
                        .alert(&msg, "TIMELOCK", self.config.priority)
                        .await;
                }
            }
        }
        Ok(())
    }

    /// Monitor OwnershipTransferred events on all watched contracts.
    async fn check_ownership_transferred(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        for &contract in &self.watched_contracts {
            let filter = Filter::new()
                .address(contract)
                .event_signature(OwnershipTransferred::SIGNATURE_HASH)
                .from_block(from_block)
                .to_block(to_block);

            let logs = provider::get_logs(&self.provider, filter).await?;
            for log in &logs {
                if let Ok(event) = OwnershipTransferred::decode_log_data(log.data()) {
                    let msg = format!(
                        "CRITICAL OWNERSHIP CHANGE: Transfer completed!\nContract: {contract}\nPrevious Owner: {}\nNew Owner: {}\nOwnership has been transferred - verify this was authorized!",
                        event.previousOwner, event.newOwner
                    );
                    let _ = self
                        .notifier
                        .alert(&msg, "TIMELOCK", Priority::P0)
                        .await;
                }
            }
        }
        Ok(())
    }

    /// If expected_governance_address is set, check if EmergencyWithdraw was called
    /// by a transaction sender that is NOT the expected governance address.
    async fn check_governance_bypass(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> anyhow::Result<()> {
        let Some(expected_gov) = self.expected_governance else {
            return Ok(());
        };

        let filter = Filter::new()
            .address(self.gbridge_sender)
            .event_signature(EmergencyWithdraw::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider::get_logs(&self.provider, filter).await?;

        for log in &logs {
            if EmergencyWithdraw::decode_log_data(log.data()).is_ok() {
                // Get the transaction to check who sent it
                if let Some(tx_hash) = log.transaction_hash {
                    if let Ok(Some(tx)) = self.provider.get_transaction_by_hash(tx_hash).await {
                        let sender = tx.inner.signer();
                        if sender != expected_gov {
                            let msg = format!(
                                "GOVERNANCE BYPASS DETECTED!\nEmergencyWithdraw called by {} instead of expected governance address {}\nTransaction: {:#x}\nThis may indicate a compromised owner key!",
                                sender, expected_gov, tx_hash
                            );
                            let _ = self
                                .notifier
                                .alert(&msg, "GOVERNANCE_BYPASS", Priority::P0)
                                .await;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
