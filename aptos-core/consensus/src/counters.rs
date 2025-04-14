// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

use crate::{
    block_storage::tracing::{observe_block, BlockStage},
    quorum_store,
};
use api_types::compute_res::ComputeRes;
use aptos_consensus_types::{block::Block, pipelined_block::PipelinedBlock};
use gaptos::aptos_crypto::HashValue;
use aptos_executor_types::{StateComputeResult, ExecutorError};
use gaptos::aptos_logger::prelude::{error, warn};
use gaptos::aptos_metrics_core::{
    exponential_buckets, op_counters::DurationHistogram, register_avg_counter, register_counter,
    register_gauge, register_gauge_vec, register_histogram, register_histogram_vec,
    register_int_counter, register_int_counter_vec, register_int_gauge, register_int_gauge_vec,
    Counter, Gauge, GaugeVec, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use gaptos::aptos_consensus::counters::{
    COMMITTED_FAILED_ROUNDS_COUNT, COMMITTED_BLOCKS_COUNT, NUM_BYTES_PER_BLOCK,
    TXN_COMMIT_SUCCESS_LABEL, COMMITTED_TXNS_COUNT, NUM_TXNS_PER_BLOCK, LAST_COMMITTED_VERSION,
    LAST_COMMITTED_ROUND,
};

pub fn log_executor_error_occurred(
    e: ExecutorError,
    counter: &Lazy<IntCounterVec>,
    block_id: HashValue,
) {
    match e {
        ExecutorError::CouldNotGetData => {
            counter.with_label_values(&["CouldNotGetData"]).inc();
            warn!(
                block_id = block_id,
                "Execution error - CouldNotGetData {}", block_id
            );
        },
        ExecutorError::BlockNotFound(block_id) => {
            counter.with_label_values(&["BlockNotFound"]).inc();
            warn!(
                block_id = block_id,
                "Execution error BlockNotFound {}", block_id
            );
        },
        e => {
            counter.with_label_values(&["UnexpectedError"]).inc();
            error!(
                block_id = block_id,
                "Execution error {:?} for {}", e, block_id
            );
        },
    }
}

pub fn update_counters_for_block(block: &Block) {
    observe_block(block.timestamp_usecs(), BlockStage::COMMITTED);
    NUM_BYTES_PER_BLOCK.observe(block.payload().map_or(0, |payload| payload.size()) as f64);
    COMMITTED_BLOCKS_COUNT.inc();
    LAST_COMMITTED_ROUND.set(block.round() as i64);
    let failed_rounds = block
        .block_data()
        .failed_authors()
        .map(|v| v.len())
        .unwrap_or(0);
    if failed_rounds > 0 {
        COMMITTED_FAILED_ROUNDS_COUNT.inc_by(failed_rounds as u64);
    }
    gaptos::aptos_consensus::quorum_store::counters::NUM_BATCH_PER_BLOCK.observe(block.payload_size() as f64);
}

pub fn update_counters_for_compute_result(compute_result: &StateComputeResult) {
    // let txn_status = compute_result.compute_status_for_input_txns();
    // LAST_COMMITTED_VERSION.set(compute_result.last_version_or_0() as i64);
    // NUM_TXNS_PER_BLOCK.observe(txn_status.len() as f64);
    // for status in txn_status.iter() {
    //     let commit_status = match status {
    //         TransactionStatus::Keep(_) => TXN_COMMIT_SUCCESS_LABEL,
    //         TransactionStatus::Discard(reason) => {
    //             /// TODO(gravity_byteyue): rethink do we need txn status
    //             todo!();
    //             if *reason == DiscardedVMStatus::SEQUENCE_NUMBER_TOO_NEW {
    //                 TXN_COMMIT_RETRY_LABEL
    //             } else if *reason == DiscardedVMStatus::SEQUENCE_NUMBER_TOO_OLD {
    //                 TXN_COMMIT_FAILED_DUPLICATE_LABEL
    //             } else if *reason == DiscardedVMStatus::TRANSACTION_EXPIRED {
    //                 TXN_COMMIT_FAILED_EXPIRED_LABEL
    //             } else {
    //                 TXN_COMMIT_FAILED_LABEL
    //             }
    //         },
    //         TransactionStatus::Retry => TXN_COMMIT_RETRY_LABEL,
    //     };
    //     COMMITTED_TXNS_COUNT
    //         .with_label_values(&[commit_status])
    //         .inc();
    // }
}


/// Update various counters for committed blocks
pub fn update_counters_for_committed_blocks(blocks_to_commit: &[Arc<PipelinedBlock>]) {
    for block in blocks_to_commit {
        update_counters_for_block(block.block());
        update_counters_for_compute_result(block.compute_result());
    }
}

// TODO(gravity_byteyue): Refactor this when compute res can carry txn status
pub fn update_counters_for_compute_res(compute_res: &ComputeRes) {
    let num = compute_res.txn_num(); 
    LAST_COMMITTED_VERSION.add(num as i64);
    NUM_TXNS_PER_BLOCK.observe(num as f64);
    COMMITTED_TXNS_COUNT
        .with_label_values(&[TXN_COMMIT_SUCCESS_LABEL])
        .inc_by(num);
}