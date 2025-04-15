// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use gaptos::aptos_infallible::duration_since_epoch;
use gaptos::aptos_metrics_core::HistogramVec;
use std::time::Duration;
use gaptos::aptos_consensus::dag::observability::counters as counters;

#[derive(strum_macros::AsRefStr)]
pub enum NodeStage {
    NodeReceived,
    CertAggregated,
    CertifiedNodeReceived,
    AnchorOrdered,
    NodeOrdered,
}

#[derive(strum_macros::AsRefStr)]
pub enum RoundStage {
    NodeBroadcasted,
    CertifiedNodeBroadcasted,
    StrongLinkReceived,
    Finished,
}

fn observe(counter: &HistogramVec, timestamp: u64, name: &str) {
    if let Some(t) = duration_since_epoch().checked_sub(Duration::from_micros(timestamp)) {
        counter.with_label_values(&[name]).observe(t.as_secs_f64());
    }
}

/// Record the time during each stage of a node.
pub fn observe_node(timestamp: u64, stage: NodeStage) {
    observe(&counters::NODE_TRACING, timestamp, stage.as_ref());
}

/// Record the time during each stage of a round.
pub fn observe_round(timestamp: u64, stage: RoundStage) {
    observe(&counters::ROUND_TRACING, timestamp, stage.as_ref());
}
