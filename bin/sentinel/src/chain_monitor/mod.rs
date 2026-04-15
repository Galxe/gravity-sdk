//! Chain monitor module for monitoring bridge contract events on-chain.
//!
//! Each monitor runs as an independent tokio task, following the same pattern
//! as the existing Probe module.

pub mod abi;
pub mod bridge_timeout;
pub mod checkpoint;
pub mod config;
pub mod epoch_timeout;
pub mod large_withdrawal;
pub mod owner_activity;
pub mod provider;
pub mod timelock;
pub mod vault_balance;

use crate::notifier::Notifier;
use anyhow::{Context, Result};
use checkpoint::{Checkpoint, SharedCheckpoint};
use config::ChainMonitorConfig;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// Spawn all enabled chain monitor tasks.
pub async fn spawn_all(config: ChainMonitorConfig, notifier: Notifier) -> Result<()> {
    // Build providers
    let eth_provider = provider::build_provider(&config.ethereum_rpc_url)
        .context("Failed to build Ethereum provider")?;
    let gravity_provider = provider::build_provider(&config.gravity_rpc_url)
        .context("Failed to build Gravity provider")?;

    // Parse addresses
    let gbridge_sender = ChainMonitorConfig::parse_address(&config.gbridge_sender_address)?;
    let gravity_portal = ChainMonitorConfig::parse_address(&config.gravity_portal_address)?;
    let gbridge_receiver = ChainMonitorConfig::parse_address(&config.gbridge_receiver_address)?;
    let gtoken_address = ChainMonitorConfig::parse_address(&config.gtoken_address)?;

    // Load or create checkpoint
    let checkpoint: SharedCheckpoint =
        Arc::new(Mutex::new(Checkpoint::load_or_default(config.checkpoint_path.as_deref())?));

    let poll_interval = Duration::from_secs(config.poll_interval_seconds);
    let confirmation_blocks = config.confirmation_blocks;
    let max_failures = config.max_consecutive_failures;

    // Spawn checkpoint flusher if path is configured
    if let Some(ref path) = config.checkpoint_path {
        let ckpt = checkpoint.clone();
        let path = path.clone();
        tokio::spawn(async move {
            checkpoint::flush_loop(ckpt, path).await;
        });
    }

    // Rule 1: Large Withdrawal
    if config.large_withdrawal.enabled {
        let monitor = large_withdrawal::LargeWithdrawalMonitor::new(
            config.large_withdrawal.clone(),
            gbridge_sender,
            eth_provider.clone(),
            notifier.clone(),
            checkpoint.clone(),
            poll_interval,
            confirmation_blocks,
            max_failures,
        )?;
        println!(
            "Starting chain monitor: large withdrawal (threshold: {} wei)",
            config.large_withdrawal.threshold_wei
        );
        tokio::spawn(async move { monitor.run().await });
    }

    // Rule 2: Vault Balance
    if config.vault_balance.enabled {
        let monitor = vault_balance::VaultBalanceMonitor::new(
            config.vault_balance.clone(),
            gtoken_address,
            gbridge_sender,
            eth_provider.clone(),
            notifier.clone(),
            checkpoint.clone(),
            poll_interval,
            max_failures,
        )?;
        println!("Starting chain monitor: vault balance");
        tokio::spawn(async move { monitor.run().await });
    }

    // Rule 3: Bridge Timeout
    if config.bridge_timeout.enabled {
        let monitor = bridge_timeout::BridgeTimeoutMonitor::new(
            config.bridge_timeout.clone(),
            gbridge_sender,
            gbridge_receiver,
            eth_provider.clone(),
            gravity_provider.clone(),
            notifier.clone(),
            checkpoint.clone(),
            poll_interval,
            confirmation_blocks,
            max_failures,
        );
        println!(
            "Starting chain monitor: bridge timeout ({}s threshold)",
            config.bridge_timeout.timeout_seconds
        );
        tokio::spawn(async move { monitor.run().await });
    }

    // Rule 4: Owner Activity
    if config.owner_activity.enabled {
        let monitor = owner_activity::OwnerActivityMonitor::new(
            config.owner_activity.clone(),
            gbridge_sender,
            gravity_portal,
            eth_provider.clone(),
            notifier.clone(),
            checkpoint.clone(),
            poll_interval,
            confirmation_blocks,
            max_failures,
        );
        println!("Starting chain monitor: owner activity");
        tokio::spawn(async move { monitor.run().await });
    }

    // Rule 5: Epoch Timeout
    if config.epoch_timeout.enabled {
        let check_interval =
            Duration::from_secs(config.epoch_timeout.check_interval_seconds);
        let monitor = epoch_timeout::EpochTimeoutMonitor::new(
            config.epoch_timeout.clone(),
            gravity_provider.clone(),
            notifier.clone(),
            check_interval,
            max_failures,
        );
        println!(
            "Starting chain monitor: epoch timeout (overdue threshold: {}s, check interval: {}s)",
            config.epoch_timeout.overdue_threshold_seconds,
            config.epoch_timeout.check_interval_seconds,
        );
        tokio::spawn(async move { monitor.run().await });
    }

    // Rule 6: Timelock / Ownership
    if config.timelock.enabled {
        let expected_governance = config
            .timelock
            .expected_governance_address
            .as_ref()
            .map(|addr| ChainMonitorConfig::parse_address(addr))
            .transpose()?;

        let watched_contracts = vec![gbridge_sender, gravity_portal];

        let monitor = timelock::TimelockMonitor::new(
            config.timelock.clone(),
            watched_contracts,
            gbridge_sender,
            expected_governance,
            eth_provider.clone(),
            notifier.clone(),
            checkpoint.clone(),
            poll_interval,
            confirmation_blocks,
            max_failures,
        );
        println!("Starting chain monitor: timelock/ownership");
        tokio::spawn(async move { monitor.run().await });
    }

    Ok(())
}
