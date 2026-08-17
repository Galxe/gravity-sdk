// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Block-number anchored, forward epoch synchronization.
//!
//! This module owns the ephemeral server index, versioned RPC handling, authenticated client
//! verification, and batch persistence/replay. The legacy reverse-sync implementation remains in
//! the parent module as the rolling-upgrade fallback.

use super::{BlockReader, BlockRetriever, BlockStore};
use crate::{
    consensusdb::schema::{
        block::BlockNumberSchema, epoch_by_block_number::EpochByBlockNumberSchema,
        ledger_info::LedgerInfoSchema,
    },
    network::IncomingForwardEpochSyncRequest,
    network_interface::ConsensusMsg,
};
use anyhow::{anyhow, bail, ensure};
use aptos_consensus_types::{
    block_retrieval::{NUM_RETRIES, RETRY_INTERVAL_MSEC, RPC_TIMEOUT_MSEC},
    forward_epoch_sync::{
        ForwardEpochSyncBatch, ForwardEpochSyncBatchStatus, ForwardEpochSyncError,
        ForwardEpochSyncFetchRequest, ForwardEpochSyncManifest, ForwardEpochSyncPrepareRequest,
        ForwardEpochSyncRecord, ForwardEpochSyncRequest, ForwardEpochSyncRequestV1,
        ForwardEpochSyncResponse, ForwardEpochSyncResponseV1,
    },
};
use gaptos::{
    aptos_config::network_id::PeerNetworkId,
    aptos_consensus::counters::BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC,
    aptos_crypto::{hash::CryptoHash, HashValue},
    aptos_logger::prelude::*,
    aptos_schemadb::batch::SchemaBatch,
    aptos_types::{account_address::AccountAddress, ledger_info::LedgerInfoWithSignatures},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::time;

#[derive(Clone)]
struct ForwardEpochSyncIndexEntry {
    block_number: Option<u64>,
    /// Highest execution block number reached at this consensus block. Unnumbered certifying
    /// suffix blocks retain the preceding value so they can still be used as resumable cursors.
    anchor_block_number: u64,
    block_id: HashValue,
    parent_id: HashValue,
}

#[derive(Clone)]
struct ForwardEpochSyncBoundary {
    certifying_position: usize,
    target_block_number: u64,
    target_block_id: HashValue,
    ledger_info: LedgerInfoWithSignatures,
}

/// Immutable metadata snapshot for one epoch. Blocks, payloads, QCs, and randomness stay in the
/// existing databases and are loaded only for the requested batch.
pub(in crate::block_storage::block_store) struct ForwardEpochSyncIndex {
    manifest: ForwardEpochSyncManifest,
    entries: Vec<ForwardEpochSyncIndexEntry>,
    positions: HashMap<HashValue, usize>,
    boundaries: Vec<ForwardEpochSyncBoundary>,
}

fn select_forward_batch_end(
    start: usize,
    requested: usize,
    total: usize,
    epoch_boundary_position: usize,
) -> Option<usize> {
    let mut end = start.saturating_add(requested).min(total);
    // The epoch boundary and suffix must be fetched by one final batch. A non-final batch stops
    // immediately before the boundary; if the final unit itself is too large, no valid end exists.
    if start <= epoch_boundary_position && end > epoch_boundary_position && end < total {
        end = epoch_boundary_position;
    }
    (end > start).then_some(end)
}

impl BlockStore {
    fn build_forward_epoch_sync_index(
        &self,
        epoch: u64,
    ) -> Result<ForwardEpochSyncIndex, ForwardEpochSyncError> {
        let db = self.storage.consensus_db();
        let epoch_end_block_number = db
            .get_all::<EpochByBlockNumberSchema>()
            .map_err(|error| {
                error!(epoch = epoch, error = ?error, "Failed to scan epoch boundaries for forward sync");
                ForwardEpochSyncError::Internal
            })?
            .into_iter()
            .filter_map(|(block_number, stored_epoch)| {
                (stored_epoch == epoch).then_some(block_number)
            })
            .max()
            .ok_or(ForwardEpochSyncError::EpochNotFound)?;
        let target_ledger_info = db
            .get::<LedgerInfoSchema>(&epoch_end_block_number)
            .map_err(|error| {
                error!(epoch = epoch, error = ?error, "Failed to read epoch-ending ledger info");
                ForwardEpochSyncError::Internal
            })?
            .ok_or(ForwardEpochSyncError::EpochNotFound)?;

        let start_key = (epoch, HashValue::zero());
        let end_key = (epoch, HashValue::new([u8::MAX; HashValue::LENGTH]));
        let quorum_certs = db.get_qc_range(&start_key, &end_key).map_err(|error| {
            error!(epoch = epoch, error = ?error, "Failed to read QCs for forward sync");
            ForwardEpochSyncError::Internal
        })?;
        let qcs_by_certified_id = quorum_certs
            .into_iter()
            .map(|qc| (qc.certified_block().id(), qc))
            .collect::<HashMap<_, _>>();
        let terminal_qc = qcs_by_certified_id
            .values()
            .filter(|qc| {
                qc.commit_info().id() == target_ledger_info.ledger_info().consensus_block_id()
            })
            .max_by_key(|qc| qc.certified_block().round())
            .ok_or_else(|| {
                error!(
                    epoch = epoch,
                    target = %target_ledger_info.ledger_info().consensus_block_id(),
                    "Epoch-ending commit has no certifying QC"
                );
                ForwardEpochSyncError::Internal
            })?;

        let mut reverse_entries = Vec::new();
        let mut visited = HashSet::new();
        let mut cursor = terminal_qc.certified_block().id();
        loop {
            if !visited.insert(cursor) {
                error!(epoch = epoch, block_id = %cursor, "Cycle in persisted consensus block chain");
                return Err(ForwardEpochSyncError::Internal);
            }
            let block = db.get_block(epoch, cursor).map_err(|error| {
                error!(epoch = epoch, block_id = %cursor, error = ?error, "Failed to read block");
                ForwardEpochSyncError::Internal
            })?;
            let Some(block) = block else { break };
            let block_number = match block.block_number() {
                Some(block_number) => Some(block_number),
                None => db.get::<BlockNumberSchema>(&(epoch, block.id())).map_err(|error| {
                    error!(
                        epoch = epoch,
                        block_id = %block.id(),
                        error = ?error,
                        "Failed to read forward-sync block number"
                    );
                    ForwardEpochSyncError::Internal
                })?,
            };
            if !qcs_by_certified_id.contains_key(&block.id()) {
                error!(epoch = epoch, block_id = %block.id(), "Forward-sync block has no QC");
                return Err(ForwardEpochSyncError::Internal);
            }
            reverse_entries.push(ForwardEpochSyncIndexEntry {
                block_number,
                anchor_block_number: 0,
                block_id: block.id(),
                parent_id: block.parent_id(),
            });
            cursor = block.parent_id();
        }
        reverse_entries.reverse();
        if reverse_entries.is_empty() {
            return Err(ForwardEpochSyncError::EpochNotFound);
        }
        for pair in reverse_entries.windows(2) {
            if pair[1].parent_id != pair[0].block_id {
                error!(
                    epoch = epoch,
                    parent_id = %pair[0].block_id,
                    child_id = %pair[1].block_id,
                    "Persisted epoch path is not contiguous"
                );
                return Err(ForwardEpochSyncError::Internal);
            }
        }
        let first_block_number =
            reverse_entries.iter().find_map(|entry| entry.block_number).ok_or_else(|| {
                error!(epoch = epoch, "Forward-sync epoch path has no numbered blocks");
                ForwardEpochSyncError::Internal
            })?;
        let mut anchor_block_number = first_block_number.checked_sub(1).ok_or_else(|| {
            error!(epoch = epoch, "Forward-sync epoch path starts at block number zero");
            ForwardEpochSyncError::Internal
        })?;
        for entry in &mut reverse_entries {
            if let Some(block_number) = entry.block_number {
                if block_number != anchor_block_number.saturating_add(1) {
                    error!(
                        epoch = epoch,
                        block_id = %entry.block_id,
                        previous_number = anchor_block_number,
                        block_number = block_number,
                        "Persisted numbered epoch path is not contiguous"
                    );
                    return Err(ForwardEpochSyncError::Internal);
                }
                anchor_block_number = block_number;
            }
            entry.anchor_block_number = anchor_block_number;
        }

        let positions = reverse_entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.block_id, position))
            .collect::<HashMap<_, _>>();
        let target_epoch_info = target_ledger_info.ledger_info().commit_info().epoch_block_info();
        let target_block_id = target_epoch_info
            .map(|info| info.block_id)
            .unwrap_or_else(|| target_ledger_info.ledger_info().consensus_block_id());
        let target_block_number = target_epoch_info
            .map(|info| info.block_number)
            .or_else(|| {
                positions
                    .get(&target_block_id)
                    .and_then(|pos| reverse_entries[*pos].block_number)
            })
            .ok_or_else(|| {
                error!(epoch = epoch, target = %target_block_id, "Epoch target is not on canonical path");
                ForwardEpochSyncError::Internal
            })?;

        let persisted_ledger_infos = db.get_all::<LedgerInfoSchema>().map_err(|error| {
            error!(epoch = epoch, error = ?error, "Failed to scan ledger infos for forward sync");
            ForwardEpochSyncError::Internal
        })?;
        let mut boundaries = Vec::new();
        for (stored_block_number, ledger_info) in persisted_ledger_infos {
            if ledger_info.ledger_info().epoch() != epoch {
                continue;
            }
            let Some(certifying_qc) = qcs_by_certified_id
                .values()
                .filter(|qc| {
                    qc.commit_info().id() == ledger_info.ledger_info().consensus_block_id()
                })
                .max_by_key(|qc| qc.certified_block().round())
            else {
                continue;
            };
            let Some(certifying_position) =
                positions.get(&certifying_qc.certified_block().id()).copied()
            else {
                continue;
            };
            let epoch_info = ledger_info.ledger_info().commit_info().epoch_block_info();
            let boundary_id = epoch_info
                .map(|info| info.block_id)
                .unwrap_or_else(|| ledger_info.ledger_info().consensus_block_id());
            let Some(target_position) = positions.get(&boundary_id).copied() else {
                continue;
            };
            if target_position > certifying_position {
                continue;
            }
            let boundary_number =
                epoch_info.map(|info| info.block_number).unwrap_or(stored_block_number);
            boundaries.push(ForwardEpochSyncBoundary {
                certifying_position,
                target_block_number: boundary_number,
                target_block_id: boundary_id,
                ledger_info,
            });
        }

        let terminal = reverse_entries.last().expect("non-empty checked above");
        let manifest_bytes = bcs::to_bytes(&(
            epoch,
            first_block_number,
            terminal.anchor_block_number,
            terminal.block_id,
            target_block_number,
            target_block_id,
            &target_ledger_info,
        ))
        .map_err(|error| {
            error!(epoch = epoch, error = ?error, "Failed to hash forward-sync manifest");
            ForwardEpochSyncError::Internal
        })?;
        let manifest = ForwardEpochSyncManifest {
            epoch,
            manifest_id: HashValue::sha3_256_of(&manifest_bytes),
            first_block_number,
            terminal_block_number: terminal.anchor_block_number,
            terminal_block_id: terminal.block_id,
            target_block_number,
            target_block_id,
            target_ledger_info,
        };
        Ok(ForwardEpochSyncIndex { manifest, entries: reverse_entries, positions, boundaries })
    }

    fn forward_epoch_sync_index(
        &self,
        epoch: u64,
    ) -> Result<Arc<ForwardEpochSyncIndex>, ForwardEpochSyncError> {
        let mut indexes = self.forward_epoch_sync_indexes.lock();
        if let Some(index) = indexes.get(&epoch) {
            return Ok(index.clone());
        }
        let index = Arc::new(self.build_forward_epoch_sync_index(epoch)?);
        // A BlockStore only needs to serve the epoch it currently owns. Bounding this map avoids
        // retaining historical path metadata after unusual cross-epoch requests.
        indexes.clear();
        indexes.insert(epoch, index.clone());
        Ok(index)
    }

    fn validate_forward_anchor(
        index: &ForwardEpochSyncIndex,
        block_number: u64,
        block_id: HashValue,
    ) -> Result<usize, ForwardEpochSyncError> {
        if let Some(position) = index.positions.get(&block_id).copied() {
            return (index.entries[position].anchor_block_number == block_number)
                .then_some(position.saturating_add(1))
                .ok_or(ForwardEpochSyncError::AnchorMismatch);
        }
        let first = index.entries.first().ok_or(ForwardEpochSyncError::EpochNotFound)?;
        let first_follows_anchor = match first.block_number {
            Some(first_number) => first_number == block_number.saturating_add(1),
            None => first.anchor_block_number == block_number,
        };
        if first.parent_id == block_id && first_follows_anchor {
            Ok(0)
        } else {
            Err(ForwardEpochSyncError::AnchorMismatch)
        }
    }

    fn prepare_forward_epoch_sync(
        &self,
        request: ForwardEpochSyncPrepareRequest,
    ) -> ForwardEpochSyncResponseV1 {
        let index = match self.forward_epoch_sync_index(request.epoch) {
            Ok(index) => index,
            Err(error) => return ForwardEpochSyncResponseV1::Error(error),
        };
        match Self::validate_forward_anchor(
            &index,
            request.anchor_block_number,
            request.anchor_block_id,
        ) {
            Ok(_) => ForwardEpochSyncResponseV1::Prepared(index.manifest.clone()),
            Err(error) => ForwardEpochSyncResponseV1::Error(error),
        }
    }

    fn fetch_forward_epoch_sync(
        &self,
        request: ForwardEpochSyncFetchRequest,
        max_blocks_allowed: u64,
    ) -> ForwardEpochSyncResponseV1 {
        if request.batch_size_blocks == 0 || request.batch_size_blocks > max_blocks_allowed {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::InvalidBatchSize);
        }
        let index = match self.forward_epoch_sync_index(request.epoch) {
            Ok(index) => index,
            Err(error) => return ForwardEpochSyncResponseV1::Error(error),
        };
        if request.manifest_id != index.manifest.manifest_id {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::ManifestMismatch);
        }
        let start = match Self::validate_forward_anchor(
            &index,
            request.anchor_block_number,
            request.anchor_block_id,
        ) {
            Ok(start) => start,
            Err(error) => return ForwardEpochSyncResponseV1::Error(error),
        };
        if Self::validate_forward_anchor(
            &index,
            request.replay_anchor_block_number,
            request.replay_anchor_block_id,
        )
        .is_err()
        {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::AnchorMismatch);
        }
        if start >= index.entries.len() {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound);
        }
        let requested = usize::try_from(request.batch_size_blocks).unwrap_or(usize::MAX);
        let target_position = index
            .positions
            .get(&index.manifest.target_block_id)
            .copied()
            .expect("manifest target checked while building forward index");
        let Some(end) =
            select_forward_batch_end(start, requested, index.entries.len(), target_position)
        else {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound);
        };
        let boundary = index
            .boundaries
            .iter()
            .filter(|boundary| {
                boundary.certifying_position >= start &&
                    boundary.certifying_position < end &&
                    index.positions[&boundary.target_block_id] >= start &&
                    boundary.target_block_number > request.replay_anchor_block_number
            })
            .max_by_key(|boundary| (boundary.target_block_number, boundary.certifying_position));
        let Some(boundary) = boundary else {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound);
        };

        let db = self.storage.consensus_db();
        let mut records = Vec::with_capacity(end - start);
        for entry in &index.entries[start..end] {
            let block = match db.get_block(request.epoch, entry.block_id) {
                Ok(Some(block)) => block,
                Ok(None) => {
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal)
                }
                Err(error) => {
                    error!(epoch = request.epoch, block_id = %entry.block_id, error = ?error, "Failed to read forward-sync block");
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal);
                }
            };
            let quorum_cert = match db.get_qc(request.epoch, entry.block_id) {
                Ok(Some(qc)) => qc,
                Ok(None) => {
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal)
                }
                Err(error) => {
                    error!(epoch = request.epoch, block_id = %entry.block_id, error = ?error, "Failed to read forward-sync QC");
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal);
                }
            };
            let randomness = match entry.block_number {
                Some(block_number) => match db.get_randomness(block_number) {
                    Ok(randomness) => randomness,
                    Err(error) => {
                        error!(epoch = request.epoch, block_number = block_number, error = ?error, "Failed to read forward-sync randomness");
                        return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal);
                    }
                },
                None => None,
            };
            records.push(ForwardEpochSyncRecord {
                block,
                block_number: entry.block_number,
                randomness,
                quorum_cert,
            });
        }
        let tail = index.entries.get(end - 1).expect("non-empty batch");
        ForwardEpochSyncResponseV1::Batch(ForwardEpochSyncBatch {
            epoch: request.epoch,
            manifest_id: request.manifest_id,
            anchor_block_number: request.anchor_block_number,
            anchor_block_id: request.anchor_block_id,
            records,
            ledger_infos: vec![boundary.ledger_info.clone()],
            replay_target_block_number: boundary.target_block_number,
            replay_target_block_id: boundary.target_block_id,
            next_anchor_block_number: tail.anchor_block_number,
            next_anchor_block_id: tail.block_id,
            status: if end == index.entries.len() {
                ForwardEpochSyncBatchStatus::Complete
            } else {
                ForwardEpochSyncBatchStatus::More
            },
        })
    }

    pub async fn process_forward_epoch_sync(
        &self,
        request: IncomingForwardEpochSyncRequest,
        max_blocks_allowed: u64,
    ) -> anyhow::Result<()> {
        let response = match request.req {
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Prepare(prepare)) => {
                self.prepare_forward_epoch_sync(prepare)
            }
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Fetch(fetch)) => {
                self.fetch_forward_epoch_sync(fetch, max_blocks_allowed)
            }
        };
        let response = ConsensusMsg::ForwardEpochSyncResponse(Box::new(
            ForwardEpochSyncResponse::V1(response),
        ));
        let response_bytes = request.protocol.to_bytes(&response)?;
        request
            .response_sender
            .send(Ok(response_bytes.into()))
            .map_err(|_| anyhow::anyhow!("Failed to send forward epoch sync response"))
    }
}

impl BlockStore {
    /// Fast-forwards the local consensus state by synchronizing blocks and ledger infos for a given
    /// epoch.
    ///
    /// This function retrieves all blocks, quorum certificates, and ledger infos for the specified
    /// epoch from a remote retriever. It then prefetches payload data for each block, saves the
    /// blocks and certificates to local storage, and updates the ledger info in the database.
    /// After updating storage, it attempts to recover the consensus state from the latest
    /// ledger info and rebuilds the in-memory state. If the epoch ends, it sends an epoch
    /// change proof to the network.
    ///
    /// # Arguments
    /// * `retriever` - The block retriever used to fetch blocks and related data.
    /// * `epoch` - The epoch to fast-forward to.
    ///
    /// # Returns
    /// * `Ok(())` if the synchronization and state rebuild succeed.
    /// * `Err` if any step fails.
    pub async fn fast_forward_sync_by_epoch(
        &self,
        mut retriever: BlockRetriever,
        epoch: u64,
        batch_size_blocks: u64,
    ) -> anyhow::Result<()> {
        ensure!(batch_size_blocks > 0, "Forward epoch sync batch size must be positive");
        match self
            .fast_forward_sync_by_epoch_forward(&mut retriever, epoch, batch_size_blocks)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => {
                info!(epoch = epoch, "Falling back to legacy reverse epoch sync");
                self.fast_forward_sync_by_epoch_legacy(retriever, epoch).await
            }
            Err(error) => Err(error),
        }
    }

    async fn fast_forward_sync_by_epoch_forward(
        &self,
        retriever: &mut BlockRetriever,
        epoch: u64,
        batch_size_blocks: u64,
    ) -> anyhow::Result<bool> {
        let fetch_root = self.ordered_root();
        let replay_root = self.commit_root();
        let mut fetch_anchor_block_number = fetch_root
            .block()
            .block_number()
            .ok_or_else(|| anyhow!("Ordered root has no block number"))?;
        let mut fetch_anchor_block_id = fetch_root.id();
        let mut replay_anchor_block_number = replay_root
            .block()
            .block_number()
            .ok_or_else(|| anyhow!("Commit root has no block number"))?;
        let mut replay_anchor_block_id = replay_root.id();
        ensure!(
            fetch_anchor_block_number >= replay_anchor_block_number,
            "Ordered root is behind commit root by block number"
        );

        let Some((manifest, serving_peer)) = retriever
            .try_prepare_forward_epoch_sync(epoch, fetch_anchor_block_number, fetch_anchor_block_id)
            .await?
        else {
            return Ok(false);
        };
        info!(
            epoch = epoch,
            manifest_id = manifest.manifest_id,
            first_block_number = manifest.first_block_number,
            terminal_block_number = manifest.terminal_block_number,
            target_block_number = manifest.target_block_number,
            batch_size_blocks = batch_size_blocks,
            "Prepared forward epoch sync"
        );

        if replay_anchor_block_number >= manifest.target_block_number &&
            fetch_anchor_block_number >= manifest.terminal_block_number
        {
            ensure!(
                replay_anchor_block_number > manifest.target_block_number ||
                    replay_anchor_block_id == manifest.target_block_id,
                "Local replay root conflicts with forward manifest target"
            );
            // The blocks and epoch-ending LI are durable, but the self-directed epoch-change
            // message is not. Re-send it after restart before reporting the sync as complete.
            self.send_committed_epoch_change(retriever, &manifest.target_ledger_info).await?;
            return Ok(true);
        }

        loop {
            let request = ForwardEpochSyncFetchRequest {
                epoch,
                manifest_id: manifest.manifest_id,
                anchor_block_number: fetch_anchor_block_number,
                anchor_block_id: fetch_anchor_block_id,
                replay_anchor_block_number,
                replay_anchor_block_id,
                batch_size_blocks,
            };
            let batch =
                retriever.fetch_forward_epoch_sync_batch(request.clone(), serving_peer).await?;
            ensure!(
                batch.next_anchor_block_number <= manifest.terminal_block_number,
                "Forward batch passes manifest terminal"
            );
            match batch.status {
                ForwardEpochSyncBatchStatus::More => ensure!(
                    batch.next_anchor_block_id != manifest.terminal_block_id,
                    "Non-final forward batch reaches manifest terminal ID"
                ),
                ForwardEpochSyncBatchStatus::Complete => ensure!(
                    batch.next_anchor_block_number == manifest.terminal_block_number &&
                        batch.next_anchor_block_id == manifest.terminal_block_id &&
                        batch.replay_target_block_number == manifest.target_block_number &&
                        batch.replay_target_block_id == manifest.target_block_id,
                    "Final forward batch does not match manifest terminal/target"
                ),
            }

            BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC.inc_by(batch.records.len() as u64);
            self.persist_and_replay_forward_epoch_sync_batch(&batch).await?;

            let committed = self.commit_root();
            let committed_number = committed
                .block()
                .block_number()
                .ok_or_else(|| anyhow!("Commit root has no block number after forward replay"))?;
            ensure!(
                committed_number > batch.replay_target_block_number ||
                    (committed_number == batch.replay_target_block_number &&
                        committed.id() == batch.replay_target_block_id),
                "Forward batch replay was not durably confirmed: expected {}:{}, got {}:{}",
                batch.replay_target_block_number,
                batch.replay_target_block_id,
                committed_number,
                committed.id()
            );

            fetch_anchor_block_number = batch.next_anchor_block_number;
            fetch_anchor_block_id = batch.next_anchor_block_id;
            replay_anchor_block_number = batch.replay_target_block_number;
            replay_anchor_block_id = batch.replay_target_block_id;

            info!(
                epoch = epoch,
                fetch_anchor_block_number = fetch_anchor_block_number,
                replay_anchor_block_number = replay_anchor_block_number,
                status = ?batch.status,
                "Forward epoch sync batch persisted and replayed"
            );
            if batch.status == ForwardEpochSyncBatchStatus::Complete {
                self.send_committed_epoch_change(retriever, &manifest.target_ledger_info).await?;
                return Ok(true);
            }
        }
    }

    async fn persist_and_replay_forward_epoch_sync_batch(
        &self,
        batch: &ForwardEpochSyncBatch,
    ) -> anyhow::Result<()> {
        for record in &batch.records {
            if let Some(payload) = record.block.payload() {
                self.payload_manager.prefetch_payload_data(payload, record.block.timestamp_usecs());
            }
        }
        let blocks = batch.records.iter().map(|record| record.block.clone()).collect::<Vec<_>>();
        let quorum_certs =
            batch.records.iter().map(|record| record.quorum_cert.clone()).collect::<Vec<_>>();
        let block_numbers = batch
            .records
            .iter()
            .filter_map(|record| {
                record
                    .block_number
                    .map(|block_number| (batch.epoch, block_number, record.block.id()))
            })
            .collect::<Vec<_>>();
        self.storage.save_tree(blocks, quorum_certs.clone(), block_numbers)?;
        self.storage.consensus_db().put_randomness(
            &batch
                .records
                .iter()
                .filter_map(|record| record.block_number.zip(record.randomness.clone()))
                .collect::<Vec<_>>(),
        )?;

        let mut ledger_info_batch = SchemaBatch::new();
        for ledger_info in &batch.ledger_infos {
            self.storage
                .consensus_db()
                .ledger_db
                .metadata_db()
                .put_ledger_info(ledger_info, &mut ledger_info_batch)?;
        }
        self.storage.consensus_db().ledger_db.metadata_db().write_schemas(ledger_info_batch)?;
        self.storage.consensus_db().ledger_db.metadata_db().update_latest_ledger_info()?;

        let sync_blocks = batch
            .records
            .iter()
            .map(|record| (record.block.clone(), record.block_number, record.randomness.clone()))
            .collect();
        self.append_blocks_for_sync_checked(sync_blocks, quorum_certs).await
    }
}

impl BlockRetriever {
    async fn request_forward_epoch_sync(
        &mut self,
        request: ForwardEpochSyncRequest,
        peers: Vec<AccountAddress>,
        rpc_timeout: Duration,
        max_attempts: usize,
    ) -> anyhow::Result<(ForwardEpochSyncResponse, AccountAddress)> {
        ensure!(!peers.is_empty(), "No peers available for forward epoch sync");
        let mut candidates = peers;
        let attempts = max_attempts.max(1);
        let mut last_error = None;
        for attempt in 0..attempts {
            if candidates.is_empty() && attempt > 0 {
                break;
            }
            let peer = self.pick_peer(attempt == 0, &mut candidates);
            match self
                .network
                .request_forward_epoch_sync(
                    request.clone(),
                    PeerNetworkId::new(self.network_id, peer),
                    rpc_timeout,
                )
                .await
            {
                Ok(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Error(
                    ForwardEpochSyncError::Busy,
                ))) => {
                    last_error = Some(anyhow!("Forward epoch sync peer {peer} is busy"));
                    time::sleep(Duration::from_millis(RETRY_INTERVAL_MSEC)).await;
                }
                Ok(response) => return Ok((response, peer)),
                Err(error) => {
                    warn!(remote_peer = peer, error = ?error, "Forward epoch sync RPC failed");
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("No forward epoch sync peer available")))
    }

    async fn request_forward_epoch_sync_from_peer(
        &self,
        request: ForwardEpochSyncRequest,
        peer: AccountAddress,
        rpc_timeout: Duration,
        max_attempts: usize,
    ) -> anyhow::Result<ForwardEpochSyncResponse> {
        let attempts = max_attempts.max(1);
        let mut last_error = None;
        for attempt in 0..attempts {
            match self
                .network
                .request_forward_epoch_sync(
                    request.clone(),
                    PeerNetworkId::new(self.network_id, peer),
                    rpc_timeout,
                )
                .await
            {
                Ok(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Error(
                    ForwardEpochSyncError::Busy,
                ))) => {
                    last_error = Some(anyhow!("Forward epoch sync peer {peer} is busy"));
                }
                Ok(response) => return Ok(response),
                Err(error) => {
                    warn!(remote_peer = peer, error = ?error, "Forward epoch sync RPC failed");
                    last_error = Some(error);
                }
            }
            if attempt + 1 < attempts {
                time::sleep(Duration::from_millis(RETRY_INTERVAL_MSEC)).await;
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("Forward epoch sync peer {peer} unavailable")))
    }

    async fn try_prepare_forward_epoch_sync(
        &mut self,
        epoch: u64,
        anchor_block_number: u64,
        anchor_block_id: HashValue,
    ) -> anyhow::Result<Option<(ForwardEpochSyncManifest, AccountAddress)>> {
        let request = ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Prepare(
            ForwardEpochSyncPrepareRequest { epoch, anchor_block_number, anchor_block_id },
        ));
        // Capability probing is deliberately short and bounded. During a rolling upgrade an old
        // peer cannot decode the appended enum variant, so the caller must quickly fall back to
        // the legacy reverse retrieval path.
        let (response, serving_peer) = match self
            .request_forward_epoch_sync(
                request,
                self.available_peers.clone(),
                Duration::from_millis(1_000),
                2,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                info!(epoch = epoch, error = ?error, "Forward epoch sync unavailable; use legacy fallback");
                return Ok(None);
            }
        };
        let ForwardEpochSyncResponse::V1(response) = response;
        match response {
            ForwardEpochSyncResponseV1::Prepared(manifest) => {
                ensure!(manifest.epoch == epoch, "Forward manifest epoch mismatch");
                ensure!(
                    manifest.target_ledger_info.ledger_info().epoch() == epoch,
                    "Forward manifest target LI epoch mismatch"
                );
                ensure!(
                    manifest.target_ledger_info.ledger_info().ends_epoch(),
                    "Forward manifest target does not end epoch"
                );
                manifest.target_ledger_info.verify_signatures(self.network.validators())?;
                let epoch_info =
                    manifest.target_ledger_info.ledger_info().commit_info().epoch_block_info();
                let expected_target_id =
                    epoch_info.map(|info| info.block_id).unwrap_or_else(|| {
                        manifest.target_ledger_info.ledger_info().consensus_block_id()
                    });
                let expected_target_number = epoch_info
                    .map(|info| info.block_number)
                    .unwrap_or_else(|| manifest.target_ledger_info.ledger_info().block_number());
                ensure!(
                    manifest.target_block_id == expected_target_id &&
                        manifest.target_block_number == expected_target_number,
                    "Forward manifest target mismatch"
                );
                ensure!(
                    manifest.first_block_number <= manifest.terminal_block_number &&
                        manifest.target_block_number <= manifest.terminal_block_number,
                    "Forward manifest block range is invalid"
                );
                Ok(Some((manifest, serving_peer)))
            }
            ForwardEpochSyncResponseV1::Error(error) => {
                info!(epoch = epoch, error = ?error, "Forward epoch sync prepare rejected; use legacy fallback");
                Ok(None)
            }
            ForwardEpochSyncResponseV1::Batch(_) => {
                bail!("Forward epoch sync prepare returned a batch")
            }
        }
    }

    async fn fetch_forward_epoch_sync_batch(
        &self,
        request: ForwardEpochSyncFetchRequest,
        serving_peer: AccountAddress,
    ) -> anyhow::Result<ForwardEpochSyncBatch> {
        let response = self
            .request_forward_epoch_sync_from_peer(
                ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Fetch(request.clone())),
                serving_peer,
                Duration::from_millis(RPC_TIMEOUT_MSEC),
                NUM_RETRIES,
            )
            .await?;
        let ForwardEpochSyncResponse::V1(response) = response;
        let batch = match response {
            ForwardEpochSyncResponseV1::Batch(batch) => batch,
            ForwardEpochSyncResponseV1::Error(error) => {
                bail!("Forward epoch sync fetch rejected: {error:?}")
            }
            ForwardEpochSyncResponseV1::Prepared(_) => {
                bail!("Forward epoch sync fetch returned a manifest")
            }
        };
        self.verify_forward_epoch_sync_batch(&request, &batch)?;
        Ok(batch)
    }

    fn verify_forward_epoch_sync_batch(
        &self,
        request: &ForwardEpochSyncFetchRequest,
        batch: &ForwardEpochSyncBatch,
    ) -> anyhow::Result<()> {
        ensure!(batch.epoch == request.epoch, "Forward batch epoch mismatch");
        ensure!(batch.manifest_id == request.manifest_id, "Forward batch manifest mismatch");
        ensure!(
            batch.anchor_block_number == request.anchor_block_number &&
                batch.anchor_block_id == request.anchor_block_id,
            "Forward batch anchor echo mismatch"
        );
        ensure!(!batch.records.is_empty(), "Forward batch is empty");
        ensure!(
            batch.records.len() as u64 <= request.batch_size_blocks,
            "Forward batch exceeds requested size"
        );

        let mut expected_parent = request.anchor_block_id;
        let mut anchor_block_number = request.anchor_block_number;
        for record in &batch.records {
            ensure!(record.block.epoch() == request.epoch, "Forward block epoch mismatch");
            ensure!(
                record.block.id() == record.block.block_data().hash(),
                "Forward block ID does not match its contents"
            );
            ensure!(record.block.parent_id() == expected_parent, "Forward blocks are not chained");
            if let Some(block_number) = record.block_number {
                ensure!(
                    block_number == anchor_block_number.saturating_add(1),
                    "Forward block number gap"
                );
                anchor_block_number = block_number;
            } else {
                ensure!(record.randomness.is_none(), "Unnumbered forward block carries randomness");
            }
            if let Some(embedded_number) = record.block.block_number() {
                ensure!(
                    Some(embedded_number) == record.block_number,
                    "Forward block carries conflicting block number"
                );
            }
            record.block.validate_signature(self.network.validators())?;
            record.block.verify_well_formed()?;
            ensure!(
                record.quorum_cert.certified_block().id() == record.block.id(),
                "Forward QC certifies a different block"
            );
            record.quorum_cert.verify(self.network.validators())?;
            expected_parent = record.block.id();
        }
        let tail = batch.records.last().expect("non-empty checked above");
        ensure!(
            batch.next_anchor_block_id == tail.block.id() &&
                batch.next_anchor_block_number == anchor_block_number,
            "Forward batch next anchor is not its tail"
        );
        ensure!(
            batch.replay_target_block_number > request.replay_anchor_block_number,
            "Forward batch does not advance replay progress"
        );
        ensure!(
            batch.replay_target_block_number <= batch.next_anchor_block_number,
            "Forward replay target is after the response tail"
        );
        ensure!(batch.ledger_infos.len() == 1, "Forward batch must carry exactly one replay proof");
        let replay_proof = &batch.ledger_infos[0];
        replay_proof.verify_signatures(self.network.validators())?;
        ensure!(
            batch.records.iter().any(|record| {
                record.quorum_cert.commit_info().id() ==
                    replay_proof.ledger_info().consensus_block_id()
            }),
            "Forward replay proof has no certifying QC in its batch"
        );
        let epoch_info = replay_proof.ledger_info().commit_info().epoch_block_info();
        let expected_replay_id = epoch_info
            .map(|info| info.block_id)
            .unwrap_or_else(|| replay_proof.ledger_info().consensus_block_id());
        let replay_record = batch
            .records
            .iter()
            .find(|record| record.block.id() == expected_replay_id)
            .ok_or_else(|| anyhow!("Forward replay target block is not in its batch"))?;
        ensure!(
            batch.replay_target_block_id == expected_replay_id &&
                replay_record.block_number == Some(batch.replay_target_block_number),
            "Forward replay target does not match its signed proof"
        );
        if let Some(epoch_info) = epoch_info {
            ensure!(
                batch.replay_target_block_number == epoch_info.block_number,
                "Forward epoch-change target number does not match its signed proof"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod forward_epoch_sync_tests {
    use super::select_forward_batch_end;

    #[test]
    fn forward_batch_stays_before_epoch_boundary() {
        assert_eq!(select_forward_batch_end(0, 3, 10, 8), Some(3));
        assert_eq!(select_forward_batch_end(3, 4, 10, 5), Some(5));
    }

    #[test]
    fn final_batch_contains_boundary_and_suffix() {
        assert_eq!(select_forward_batch_end(4, 6, 10, 5), Some(10));
    }

    #[test]
    fn rejects_batch_too_small_for_boundary_suffix_unit() {
        assert_eq!(select_forward_batch_end(5, 2, 10, 5), None);
    }
}
