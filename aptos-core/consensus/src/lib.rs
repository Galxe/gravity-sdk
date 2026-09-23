// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]
#![allow(unreachable_code)]
#![allow(clippy::all)]
#![allow(unexpected_cfgs)]
#![forbid(unsafe_code)]

//! Consensus for the Aptos Core blockchain
//!
//! The consensus protocol implemented is AptosBFT (based on
//! [DiemBFT](https://developers.diem.com/papers/diem-consensus-state-machine-replication-in-the-diem-blockchain/2021-08-17.pdf)).

#![cfg_attr(feature = "fuzzing", allow(dead_code))]
#![recursion_limit = "512"]

#[macro_use(defer)]
extern crate scopeguard;

extern crate core;

mod block_storage;
#[cfg(feature = "byzantine-test")]
mod byzantine_test;
pub mod consensusdb;
mod dag;
mod epoch_manager;
mod error;
mod liveness;
mod logging;
mod metrics_safety_rules;
mod network;
#[cfg(test)]
mod network_tests;
pub mod payload_client;
mod pending_order_votes;
mod pending_votes;
pub mod persistent_liveness_storage;
mod pipeline;
pub mod quorum_store;
mod rand;
mod recovery_manager;
mod round_manager;
mod state_computer;
#[cfg(test)]
mod state_computer_tests;
mod state_replication;
#[cfg(any(test, feature = "fuzzing"))]
pub mod test_utils;
#[cfg(test)]
mod twins;
mod txn_notifier;
pub mod util;

mod block_preparer;
pub mod consensus_observer;
/// AptosBFT implementation
pub mod consensus_provider;
/// Required by the telemetry service
pub mod counters;
mod execution_pipeline;
pub mod gravity_state_computer;
/// AptosNet interface.
pub mod network_interface;
mod payload_manager;
mod qc_aggregator;
mod transaction_deduper;
mod transaction_filter;
mod transaction_shuffler;
mod txn_hash_and_authenticator_deduper;

pub use consensusdb::create_checkpoint;
/// Required by the smoke tests
pub use consensusdb::CONSENSUS_DB_NAME;
use gaptos::aptos_metrics_core::IntGauge;
pub use quorum_store::quorum_store_db::QUORUM_STORE_DB_NAME;
#[cfg(feature = "fuzzing")]
pub use round_manager::round_manager_fuzzing;

pub(crate) const ENABLE_FORWARD_EPOCH_SYNC_ENV: &str = "ENABLE_FORWARD_EPOCH_SYNC";
pub(crate) const FORWARD_EPOCH_SYNC_COLD_BUILD_QUOTA_ENV: &str =
    "FORWARD_EPOCH_SYNC_COLD_BUILD_QUOTA";
pub(crate) const FORWARD_EPOCH_SYNC_FETCH_QUOTA_ENV: &str = "FORWARD_EPOCH_SYNC_FETCH_QUOTA";
/// Default for both serving quotas: how many cold index builds may run at once, and how many
/// Fetch handlers may run at once. The pools are separate so a burst of cold builds cannot starve
/// the Fetch pages of a sync already in progress, and vice versa.
pub(crate) const FORWARD_EPOCH_SYNC_QUOTA_DEFAULT: usize = 4;

/// Opt-out switch for the block-number anchored epoch sync path. Nodes use it by default and fall
/// back to the legacy reverse sync path only when operators explicitly set
/// `ENABLE_FORWARD_EPOCH_SYNC=false`; unset or unparsable values keep it enabled.
pub(crate) fn forward_epoch_sync_enabled() -> bool {
    std::env::var(ENABLE_FORWARD_EPOCH_SYNC_ENV)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(true)
}

/// Serving-side cap on concurrent cold index builds (`FORWARD_EPOCH_SYNC_COLD_BUILD_QUOTA`).
pub(crate) fn forward_epoch_sync_cold_build_quota() -> usize {
    forward_epoch_sync_quota(FORWARD_EPOCH_SYNC_COLD_BUILD_QUOTA_ENV)
}

/// Serving-side cap on concurrent Fetch handlers (`FORWARD_EPOCH_SYNC_FETCH_QUOTA`).
pub(crate) fn forward_epoch_sync_fetch_quota() -> usize {
    forward_epoch_sync_quota(FORWARD_EPOCH_SYNC_FETCH_QUOTA_ENV)
}

/// Unset, unparsable, or out-of-range values (`< 1`, or more permits than a tokio semaphore can
/// hold) fall back to [`FORWARD_EPOCH_SYNC_QUOTA_DEFAULT`] with a warning.
fn forward_epoch_sync_quota(env: &str) -> usize {
    let Ok(value) = std::env::var(env) else {
        return FORWARD_EPOCH_SYNC_QUOTA_DEFAULT;
    };
    match value.parse::<usize>() {
        Ok(n) if (1..=tokio::sync::Semaphore::MAX_PERMITS).contains(&n) => n,
        _ => {
            gaptos::aptos_logger::warn!(
                env = env,
                value = %value,
                default = FORWARD_EPOCH_SYNC_QUOTA_DEFAULT,
                "Invalid forward epoch sync quota (must be a positive integer); using default"
            );
            FORWARD_EPOCH_SYNC_QUOTA_DEFAULT
        }
    }
}

struct IntGaugeGuard {
    gauge: IntGauge,
}

impl IntGaugeGuard {
    fn new(gauge: IntGauge) -> Self {
        gauge.inc();
        Self { gauge }
    }
}

impl Drop for IntGaugeGuard {
    fn drop(&mut self) {
        self.gauge.dec();
    }
}

/// Helper function to record metrics for external calls.
/// Include call counts, time, and whether it's inside or not (1 or 0).
/// It assumes a OpMetrics defined as OP_COUNTERS in crate::counters;
#[macro_export]
macro_rules! monitor {
    ($name:literal, $fn:expr) => {{
        use gaptos::aptos_consensus::counters::OP_COUNTERS;
        use $crate::IntGaugeGuard;
        let _timer = OP_COUNTERS.timer($name);
        let _guard = IntGaugeGuard::new(OP_COUNTERS.gauge(concat!($name, "_running")));
        $fn
    }};
}
