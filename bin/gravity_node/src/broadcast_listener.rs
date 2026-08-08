//! Reth pending-body listener → timeline admit (via [`AdmitHandle`]).
//!
//! Drain policy: CAP=128 + hard 1ms batch wait (`tokio::time::timeout` on recv).
//! Never locks outer `smp.mempool` — only `admit.admit_batch`.

use std::sync::Arc;

use api::AdmitHandle;
use aptos_mempool::core_mempool::transaction::VerifiedTxn as CoreVerifiedTxn;
use futures::StreamExt;
use gaptos::aptos_types::transaction::SignedTransaction;
use greth::reth_transaction_pool::{
    EthPooledTransaction, NewTransactionEvent, TransactionPool, ValidPoolTransaction,
};
use tracing::{info, warn};

use crate::{mempool::to_verified_txn, reth_cli::RethTransactionPool};

/// Max number of txs to batch before flushing to admit.
pub const ADMIT_BATCH_CAP: usize = 128;

/// Max time to wait after the first item before flushing even if under CAP.
pub const ADMIT_BATCH_MAX_WAIT: std::time::Duration = std::time::Duration::from_millis(1);

/// Drain decision: flush when CAP hit, max wait elapsed, or no more pending work.
///
/// Pure policy helper (unit-tested). The async drain loop enforces the same rules via
/// CAP and `tokio::time::timeout` on recv rather than calling this after a blocking recv.
#[cfg_attr(not(test), allow(dead_code))]
pub fn should_flush(len: usize, elapsed: std::time::Duration, more_pending: bool) -> bool {
    len >= ADMIT_BATCH_CAP || elapsed >= ADMIT_BATCH_MAX_WAIT || (!more_pending && len > 0)
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
pub fn spawn_broadcast_listener(pool: RethTransactionPool, admit: AdmitHandle, chain_id: u64) {
    let mut rx = pool.new_pending_pool_transactions_listener();
    info!("spawned reth pending-body broadcast listener (cap={ADMIT_BATCH_CAP}, wait=1ms)");

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

            let deadline = tokio::time::Instant::now() + ADMIT_BATCH_MAX_WAIT;
            let mut batch = Vec::with_capacity(ADMIT_BATCH_CAP);
            if let Some(signed) = event_to_signed(first, chain_id) {
                batch.push(signed);
            } else {
                warn!("skipping pool event: conversion to SignedTransaction failed");
            }

            while batch.len() < ADMIT_BATCH_CAP {
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

    #[test]
    fn flush_on_cap() {
        assert!(should_flush(128, std::time::Duration::from_micros(10), true));
    }

    #[test]
    fn flush_on_max_wait_even_if_under_cap() {
        assert!(should_flush(1, std::time::Duration::from_millis(1), true));
    }

    #[test]
    fn no_flush_mid_batch_before_wait() {
        assert!(!should_flush(3, std::time::Duration::from_micros(100), true));
    }

    #[test]
    fn flush_when_no_more_pending() {
        assert!(should_flush(1, std::time::Duration::from_micros(1), false));
    }
}
