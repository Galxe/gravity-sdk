//! Reth pending-body listener → timeline admit (via [`AdmitHandle`]).
//!
//! Drain policy: CAP + hard max-wait batch (`tokio::time::timeout` on recv).
//! Knobs (env, with defaults):
//! - `MEMPOOL_ADMIT_BATCH_CAP` (default 64)
//! - `MEMPOOL_ADMIT_BATCH_MAX_WAIT_MS` (default 5)
//!
//! Never locks outer `smp.mempool` — only `admit.admit_batch`.

use std::{sync::Arc, time::Duration};

use api::AdmitHandle;
use aptos_mempool::core_mempool::transaction::VerifiedTxn as CoreVerifiedTxn;
use futures::StreamExt;
use gaptos::aptos_types::transaction::SignedTransaction;
use greth::reth_transaction_pool::{
    EthPooledTransaction, NewTransactionEvent, TransactionPool, ValidPoolTransaction,
};
use tracing::{info, warn};

use crate::{mempool::to_verified_txn, reth_cli::RethTransactionPool};

/// Default batch size (`MEMPOOL_ADMIT_BATCH_CAP`).
pub const DEFAULT_ADMIT_BATCH_CAP: usize = 64;

/// Default max wait ms (`MEMPOOL_ADMIT_BATCH_MAX_WAIT_MS`).
pub const DEFAULT_ADMIT_BATCH_MAX_WAIT_MS: u64 = 5;

/// Runtime batching knobs for the pending-body → admit path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmitBatchConfig {
    /// Max txs to batch before flushing.
    pub cap: usize,
    /// Max time to wait after the first item before flushing even if under CAP.
    pub max_wait: Duration,
}

impl Default for AdmitBatchConfig {
    fn default() -> Self {
        Self {
            cap: DEFAULT_ADMIT_BATCH_CAP,
            max_wait: Duration::from_millis(DEFAULT_ADMIT_BATCH_MAX_WAIT_MS),
        }
    }
}

impl AdmitBatchConfig {
    /// Build from explicit values; zero cap is clamped to 1.
    pub fn new(cap: usize, max_wait_ms: u64) -> Self {
        Self { cap: cap.max(1), max_wait: Duration::from_millis(max_wait_ms) }
    }

    /// Read from process env; missing/invalid → defaults.
    ///
    /// - `MEMPOOL_ADMIT_BATCH_CAP` (usize, default 64)
    /// - `MEMPOOL_ADMIT_BATCH_MAX_WAIT_MS` (u64, default 5)
    pub fn from_env() -> Self {
        let cap = std::env::var("MEMPOOL_ADMIT_BATCH_CAP")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_ADMIT_BATCH_CAP);
        let max_wait_ms = std::env::var("MEMPOOL_ADMIT_BATCH_MAX_WAIT_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_ADMIT_BATCH_MAX_WAIT_MS);
        Self::new(cap, max_wait_ms)
    }
}

/// Drain decision: flush when CAP hit, max wait elapsed, or no more pending work.
///
/// Pure policy helper (unit-tested). The async drain loop enforces the same rules via
/// CAP and `tokio::time::timeout` on recv rather than calling this after a blocking recv.
#[cfg_attr(not(test), allow(dead_code))]
pub fn should_flush(
    len: usize,
    elapsed: Duration,
    more_pending: bool,
    cfg: AdmitBatchConfig,
) -> bool {
    len >= cfg.cap || elapsed >= cfg.max_wait || (!more_pending && len > 0)
}

/// Convert a reth pending pool event into a consensus `SignedTransaction`.
///
/// Path matches CoreMempool reconcile:
/// `api_types::VerifiedTxn` → `core_mempool::VerifiedTxn` → `SignedTransaction`.
/// On conversion failure: log warn and return `None` (caller flattens).
fn event_to_signed(
    ev: NewTransactionEvent<EthPooledTransaction>,
    chain_id: u64,
) -> Option<SignedTransaction> {
    event_pool_txn_to_signed(ev.transaction, chain_id)
}

fn event_pool_txn_to_signed(
    pool_txn: Arc<ValidPoolTransaction<EthPooledTransaction>>,
    chain_id: u64,
) -> Option<SignedTransaction> {
    // `to_verified_txn` is currently infallible; keep Option for skip-on-failure policy.
    let verified = to_verified_txn(pool_txn, chain_id);
    let signed: SignedTransaction = CoreVerifiedTxn::from(verified).into();
    Some(signed)
}

/// Subscribe to reth pending-body events and admit into the broadcast timeline.
///
/// Spawns a task on the current tokio runtime. On channel close, the task exits
/// (reconcile-only degrade). Only uses `admit.admit_batch` — never outer mempool lock.
pub fn spawn_broadcast_listener(
    pool: RethTransactionPool,
    admit: AdmitHandle,
    chain_id: u64,
    batch_cfg: AdmitBatchConfig,
) {
    let mut rx = pool.new_pending_pool_transactions_listener();
    info!(
        "spawned reth pending-body broadcast listener (cap={}, wait={}ms)",
        batch_cfg.cap,
        batch_cfg.max_wait.as_millis()
    );

    tokio::spawn(async move {
        loop {
            let first = match rx.next().await {
                Some(ev) => ev,
                None => {
                    warn!(
                        "reth pending-body listener channel closed; \
                         degrading to reconcile-only admit path"
                    );
                    break;
                }
            };

            let deadline = tokio::time::Instant::now() + batch_cfg.max_wait;
            let mut batch = Vec::with_capacity(batch_cfg.cap);
            if let Some(signed) = event_to_signed(first, chain_id) {
                batch.push(signed);
            } else {
                warn!("skipping pool event: conversion to SignedTransaction failed");
            }

            while batch.len() < batch_cfg.cap {
                let left = deadline.saturating_duration_since(tokio::time::Instant::now());
                if left.is_zero() {
                    break;
                }
                match tokio::time::timeout(left, rx.next()).await {
                    Ok(Some(ev)) => match event_to_signed(ev, chain_id) {
                        Some(signed) => batch.push(signed),
                        None => {
                            warn!("skipping pool event: conversion to SignedTransaction failed");
                        }
                    },
                    Ok(None) => {
                        // Channel closed after partial batch — flush what we have and exit.
                        if !batch.is_empty() {
                            admit.admit_batch(batch);
                        }
                        warn!(
                            "reth pending-body listener channel closed mid-batch; \
                             degrading to reconcile-only admit path"
                        );
                        return;
                    }
                    Err(_elapsed) => break, // hard time limit
                }
            }

            if !batch.is_empty() {
                admit.admit_batch(batch);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(cap: usize, wait_ms: u64) -> AdmitBatchConfig {
        AdmitBatchConfig::new(cap, wait_ms)
    }

    #[test]
    fn flush_on_cap() {
        assert!(should_flush(64, Duration::from_micros(10), true, cfg(64, 5)));
    }

    #[test]
    fn flush_on_max_wait_even_if_under_cap() {
        assert!(should_flush(1, Duration::from_millis(5), true, cfg(64, 5)));
    }

    #[test]
    fn no_flush_mid_batch_before_wait() {
        assert!(!should_flush(3, Duration::from_micros(100), true, cfg(64, 5)));
    }

    #[test]
    fn flush_when_no_more_pending() {
        assert!(should_flush(1, Duration::from_micros(1), false, cfg(64, 5)));
    }

    #[test]
    fn new_clamps_zero_cap() {
        assert_eq!(AdmitBatchConfig::new(0, 5).cap, 1);
    }
}
