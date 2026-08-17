// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Versioned messages for forward, block-number anchored epoch synchronization.
//!
//! The block number is only a lookup/cursor hint. Block IDs, parent links, quorum certificates,
//! and signed ledger infos remain the authenticated source of truth.

use crate::{block::Block, quorum_cert::QuorumCert};
use gaptos::{aptos_crypto::HashValue, aptos_types::ledger_info::LedgerInfoWithSignatures};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ForwardEpochSyncRequest {
    V1(ForwardEpochSyncRequestV1),
}

impl ForwardEpochSyncRequest {
    pub fn epoch(&self) -> u64 {
        match self {
            Self::V1(request) => request.epoch(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ForwardEpochSyncRequestV1 {
    Prepare(ForwardEpochSyncPrepareRequest),
    Fetch(ForwardEpochSyncFetchRequest),
}

impl ForwardEpochSyncRequestV1 {
    pub fn epoch(&self) -> u64 {
        match self {
            Self::Prepare(request) => request.epoch,
            Self::Fetch(request) => request.epoch,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForwardEpochSyncPrepareRequest {
    pub epoch: u64,
    pub anchor_block_number: u64,
    pub anchor_block_id: HashValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForwardEpochSyncFetchRequest {
    pub epoch: u64,
    pub manifest_id: HashValue,
    pub anchor_block_number: u64,
    pub anchor_block_id: HashValue,
    /// Last boundary durably replayed by the client. It can lag the fetch anchor.
    pub replay_anchor_block_number: u64,
    pub replay_anchor_block_id: HashValue,
    pub batch_size_blocks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ForwardEpochSyncResponse {
    V1(ForwardEpochSyncResponseV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ForwardEpochSyncResponseV1 {
    Prepared(ForwardEpochSyncManifest),
    Batch(ForwardEpochSyncBatch),
    Error(ForwardEpochSyncError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForwardEpochSyncManifest {
    pub epoch: u64,
    pub manifest_id: HashValue,
    /// First block available for this epoch on the canonical path.
    pub first_block_number: u64,
    /// Tail block included in the epoch snapshot. This can be later than the committed target
    /// when an epoch-change suffix is present.
    pub terminal_block_number: u64,
    pub terminal_block_id: HashValue,
    /// Signed epoch-ending commit target.
    pub target_block_number: u64,
    pub target_block_id: HashValue,
    pub target_ledger_info: LedgerInfoWithSignatures,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForwardEpochSyncRecord {
    pub block: Block,
    /// Execution block number, when this consensus block was ordered. A certifying suffix after
    /// a non-blocking epoch boundary can legitimately have no execution block number.
    pub block_number: Option<u64>,
    pub randomness: Option<Vec<u8>>,
    pub quorum_cert: QuorumCert,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ForwardEpochSyncBatchStatus {
    More,
    Complete,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForwardEpochSyncBatch {
    pub epoch: u64,
    pub manifest_id: HashValue,
    /// Echo of the fetch cursor. The first returned block must be its direct child.
    pub anchor_block_number: u64,
    pub anchor_block_id: HashValue,
    pub records: Vec<ForwardEpochSyncRecord>,
    pub ledger_infos: Vec<LedgerInfoWithSignatures>,
    /// Authenticated boundary that can be replayed after this batch is persisted. It can lag the
    /// response tail because the tail QC commits an ancestor.
    pub replay_target_block_number: u64,
    pub replay_target_block_id: HashValue,
    /// Cursor for the next fetch. This is always the response tail, not the replay target.
    pub next_anchor_block_number: u64,
    pub next_anchor_block_id: HashValue,
    pub status: ForwardEpochSyncBatchStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ForwardEpochSyncError {
    EpochNotFound,
    AnchorMismatch,
    ManifestMismatch,
    InvalidBatchSize,
    BatchBoundaryNotFound,
    Busy,
    Internal,
}
