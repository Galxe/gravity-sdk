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
pub(crate) const FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_ENV: &str =
    "FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC";
/// Default client-side Prepare timeout.
///
/// Sized from testnet serving measurements: cold index builds for ~29k-block epochs took
/// ~4.5–5.7s on a loaded VFN. 30s (~5× that peak) leaves headroom for larger epochs, slower
/// disks, and concurrent handler load without immediately falling back to legacy reverse sync.
pub(crate) const FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT: u64 = 30_000;

/// Opt-in switch for the block-number anchored epoch sync path. Nodes use the legacy reverse sync
/// path unless operators explicitly set `ENABLE_FORWARD_EPOCH_SYNC=true`.
pub(crate) fn forward_epoch_sync_enabled() -> bool {
    std::env::var(ENABLE_FORWARD_EPOCH_SYNC_ENV)
        .ok()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(false)
}

/// Client-side timeout for a single forward-epoch-sync Prepare RPC attempt.
///
/// Operators can override via `FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC`. Unset, unparsable, or
/// values `< 1` fall back to [`FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT`] (30000).
pub(crate) fn forward_epoch_sync_prepare_timeout_msec() -> u64 {
    match std::env::var(FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_ENV) {
        Err(_) => FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT,
        Ok(value) => match value.parse::<u64>() {
            Ok(n) if n >= 1 => n,
            Ok(n) => {
                gaptos::aptos_logger::warn!(
                    env = FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_ENV,
                    value = n,
                    default = FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT,
                    "Invalid FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC (must be >= 1); using default"
                );
                FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT
            }
            Err(_) => {
                gaptos::aptos_logger::warn!(
                    env = FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_ENV,
                    value = %value,
                    default = FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT,
                    "Unparsable FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC; using default"
                );
                FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC_DEFAULT
            }
        },
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
