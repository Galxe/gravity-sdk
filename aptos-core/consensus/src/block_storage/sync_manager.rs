// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_storage::{
        pending_blocks::PendingBlocks,
        tracing::{observe_block, BlockStage},
        BlockReader, BlockStore,
    },
    consensusdb::schema::{
        epoch_by_block_number::EpochByBlockNumberSchema, ledger_info::LedgerInfoSchema,
    },
    epoch_manager::LivenessStorageData,
    logging::{LogEvent, LogSchema},
    monitor,
    network::{IncomingBlockRetrievalRequest, NetworkSender},
    network_interface::ConsensusMsg,
    payload_manager::TPayloadManager,
    persistent_liveness_storage::{PersistentLivenessStorage, RecoveryData},
};
use anyhow::{anyhow, bail};
use aptos_consensus_types::{
    block::Block,
    block_retrieval::{
        BlockRetrievalRequest, BlockRetrievalResponse, BlockRetrievalStatus, NUM_PEERS_PER_RETRY,
        NUM_RETRIES, RETRY_INTERVAL_MSEC, RPC_TIMEOUT_MSEC,
    },
    common::Author,
    quorum_cert::QuorumCert,
    sync_info::SyncInfo,
    wrapped_ledger_info::WrappedLedgerInfo,
};
use gaptos::aptos_metrics_core::{register_int_gauge_vec, IntGaugeHelper, IntGaugeVec};
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_infallible::Mutex;
use gaptos::aptos_logger::prelude::*;
use gaptos::aptos_schemadb::batch::SchemaBatch;
use gaptos::aptos_types::{
    account_address::AccountAddress, epoch_change::EpochChangeProof,
    ledger_info::{self, LedgerInfoWithSignatures},
};
use fail::fail_point;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use futures_channel::oneshot;
use num_traits::ToPrimitive;
use once_cell::sync::Lazy;
use rand::{prelude::*, Rng};
use sha3::digest::generic_array::typenum::Le;
use std::{clone::Clone, cmp::min, hash::Hash, sync::Arc, time::Duration};
use tokio::{time, time::timeout};
use gaptos::aptos_consensus::counters::{
    BLOCKS_FETCHED_FROM_NETWORK_IN_BLOCK_RETRIEVER,
    BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC,
    BLOCKS_FETCHED_FROM_NETWORK_WHILE_INSERTING_QUORUM_CERT, LATE_EXECUTION_WITH_ORDER_VOTE_QC,
    SUCCESSFUL_EXECUTED_WITH_ORDER_VOTE_QC, SUCCESSFUL_EXECUTED_WITH_REGULAR_QC,
};

static CUR_BLOCK_SYNC_BLOCK_SUM_GAUGE: Lazy<IntGaugeVec> = Lazy::new(|| {
     register_int_gauge_vec!(
         "aptos_current_block_sync_block_sum",
         "Current block_sync block sum",
         &[]
     )
     .unwrap()
 });

 static BLOCK_SYNC_GAUGE: Lazy<IntGaugeVec> = Lazy::new(|| {
     register_int_gauge_vec!(
         "aptos_block_sync",
         "is block_sync or not",
         &[]
     )
     .unwrap()
 });

#[derive(Debug, PartialEq, Eq)]
/// Whether we need to do block retrieval if we want to insert a Quorum Cert.
pub enum NeedFetchResult {
    QCRoundBeforeRoot,
    QCAlreadyExist,
    QCBlockExist,
    NeedFetch,
}

impl BlockStore {
    /// Check if we're far away from this ledger info and need to sync.
    /// This ensures that the block referred by the ledger info is not in buffer manager.
    pub fn need_sync_for_ledger_info(&self, li: &LedgerInfoWithSignatures) -> bool {
        // TODO move min gap to fallback (30) to config.
        (self.ordered_root().round() < li.commit_info().round()
            && !self.block_exists(li.commit_info().id()))
            || self.commit_root().round() + 30.max(2 * self.vote_back_pressure_limit)
                < li.commit_info().round()
    }

    pub fn need_sync_to_highest_quorum_cert(&self, hqc: &QuorumCert) -> bool {
        (self.ordered_root().round() < hqc.certified_block().round()
            && !self.block_exists(hqc.certified_block().id()))
    }

    /// Checks if quorum certificate can be inserted in block store without RPC
    /// Returns the enum to indicate the detailed status.
    pub fn need_fetch_for_quorum_cert(&self, qc: &QuorumCert) -> NeedFetchResult {
        if qc.certified_block().round() < self.ordered_root().round() {
            return NeedFetchResult::QCRoundBeforeRoot;
        }
        if self.get_quorum_cert_for_block(qc.certified_block().id())
            .is_some() {
            return NeedFetchResult::QCAlreadyExist;
        }
        if self.block_exists(qc.certified_block().id()) {
            return NeedFetchResult::QCBlockExist;
        }
        NeedFetchResult::NeedFetch
    }

    /// Fetches dependencies for given sync_info.quorum_cert
    /// If gap is large, performs state sync using sync_to_highest_ordered_cert
    /// Inserts sync_info.quorum_cert into block store as the last step
    pub async fn add_certs(
        &self,
        sync_info: &SyncInfo,
        mut retriever: BlockRetriever,
    ) -> anyhow::Result<()> {
        BLOCK_SYNC_GAUGE.set_with(&[], 1);
        self.sync_to_highest_commit_cert(
            sync_info.highest_commit_cert().clone(),
            &mut retriever,
        )
        .await?;

        // When the local ordered round is very old than the received sync_info, this function will
        // (1) resets the block store with highest commit cert = sync_info.highest_quorum_cert()
        // (2) insert all the blocks between (inclusive) highest_commit_cert.commit_info().id() to
        // highest_quorum_cert.certified_block().id() into the block store and storage
        // (3) insert the quorum cert for all the above blocks into the block store and storage
        // (4) executes all the blocks that are ordered while inserting the above quorum certs
        self.sync_to_highest_quorum_cert(
            sync_info.highest_quorum_cert().clone(),
            sync_info.highest_commit_cert().clone(),
            &mut retriever,
        )
        .await?;

        // The insert_ordered_cert(order_cert) function call expects that order_cert.commit_info().id() block
        // is already stored in block_store. So, we first call insert_quorum_cert(highest_quorum_cert).
        // This call will ensure that the highest ceritified block along with all its ancestors are inserted
        // into the block store.
        self.insert_quorum_cert(sync_info.highest_quorum_cert(), &mut retriever)
            .await?;

        // Even though we inserted the highest_quorum_cert (and its ancestors) in the above step,
        // we still need to insert ordered cert explicitly. This will send the highest ordered block
        // to execution.
        if self.order_vote_enabled {
            self.insert_ordered_cert(&sync_info.highest_ordered_cert())
                .await?;
        } else {
            // When order votes are disabled, the highest_ordered_cert().certified_block().id() need not be
            // one of the ancestors of highest_quorum_cert.certified_block().id() due to forks. So, we call
            // insert_quorum_cert instead of insert_ordered_cert as in the above case. This will ensure that
            // highest_ordered_cert().certified_block().id() is inserted the block store.
            self.insert_quorum_cert(
                &self
                    .highest_ordered_cert()
                    .as_ref()
                    .clone()
                    .into_quorum_cert(self.order_vote_enabled)?,
                &mut retriever,
            )
            .await?;
        }

        if let Some(tc) = sync_info.highest_2chain_timeout_cert() {
            self.insert_2chain_timeout_certificate(Arc::new(tc.clone()))?;
        }
        BLOCK_SYNC_GAUGE.set_with(&[], 0);
        Ok(())
    }

    pub async fn insert_quorum_cert(
        &self,
        qc: &QuorumCert,
        retriever: &mut BlockRetriever,
    ) -> anyhow::Result<()> {
        match self.need_fetch_for_quorum_cert(qc) {
            NeedFetchResult::NeedFetch => self.fetch_quorum_cert(qc.clone(), retriever).await?,
            NeedFetchResult::QCBlockExist => self.insert_single_quorum_cert(qc.clone(), false)?,
            NeedFetchResult::QCAlreadyExist => return Ok(()),
            _ => (),
        }
        if self.ordered_root().round() < qc.commit_info().round() {
            SUCCESSFUL_EXECUTED_WITH_REGULAR_QC.inc();
            self.send_for_execution(qc.into_wrapped_ledger_info(), false)
                .await?;
            if qc.ends_epoch() {
                retriever
                    .network
                    .broadcast_epoch_change(EpochChangeProof::new(
                        vec![qc.ledger_info().clone()],
                        /* more = */ false,
                    ))
                    .await;
            }
        }
        Ok(())
    }

    // Before calling this function, we need to maintain an invariant that ordered_cert.commit_info().id()
    // is already in the block store. So, currently insert_ordered_cert calls are preceded by insert_quorum_cert calls
    // to ensure this.
    pub async fn insert_ordered_cert(
        &self,
        ordered_cert: &WrappedLedgerInfo,
    ) -> anyhow::Result<()> {
        if self.ordered_root().round() < ordered_cert.ledger_info().ledger_info().round() {
            if let Some(ordered_block) = self.get_block(ordered_cert.commit_info().id()) {
                if !ordered_block.block().is_nil_block() {
                    observe_block(
                        ordered_block.block().timestamp_usecs(),
                        BlockStage::OC_ADDED,
                    );
                }
                SUCCESSFUL_EXECUTED_WITH_ORDER_VOTE_QC.inc();
                self.send_for_execution(ordered_cert.clone(), false).await?;
            } else {
                bail!("Ordered block not found in block store when inserting ordered cert");
            }
        } else {
            LATE_EXECUTION_WITH_ORDER_VOTE_QC.inc();
        }
        Ok(())
    }

    /// Insert the quorum certificate separately from the block, used to split the processing of
    /// updating the consensus state(with qc) and deciding whether to vote(with block)
    /// The missing ancestors are going to be retrieved from the given peer. If a given peer
    /// fails to provide the missing ancestors, the qc is not going to be added.
    async fn fetch_quorum_cert(
        &self,
        qc: QuorumCert,
        retriever: &mut BlockRetriever,
    ) -> anyhow::Result<()> {
        let mut pending = vec![];
        let mut retrieve_qc = qc.clone();
        loop {
            if self.block_exists(retrieve_qc.certified_block().id()) {
                break;
            }
            BLOCKS_FETCHED_FROM_NETWORK_WHILE_INSERTING_QUORUM_CERT.inc_by(1);
            let (mut blocks, _, _) = retriever
                .retrieve_blocks_in_range(
                    retrieve_qc.certified_block().id(),
                    1,
                    retrieve_qc.certified_block().id(),
                    qc.ledger_info()
                        .get_voters(&retriever.validator_addresses()),
                    self.payload_manager.clone(),
                )
                .await?;
            if blocks.is_empty() {
                break;
            }
            let block = blocks.remove(0);
            retrieve_qc = block.quorum_cert().clone();
            pending.push(block);
        }
        // insert the qc <- block pair
        while let Some(block) = pending.pop() {
            let block_qc = block.quorum_cert().clone();
            self.insert_single_quorum_cert(block_qc, false)?;
            self.insert_block(block, false).await?;
        }
        self.insert_single_quorum_cert(qc, false)
    }

    /// Check the highest ordered cert sent by peer to see if we're behind and start a fast
    /// forward sync if the committed block doesn't exist in our tree.
    /// It works as follows:
    /// 1. request the gap blocks from the peer (from highest_ledger_info to highest_ordered_cert)
    /// 2. We persist the gap blocks to storage before start sync to ensure we could restart if we
    /// crash in the middle of the sync.
    /// 3. We prune the old tree and replace with a new tree built with the 3-chain.
    async fn sync_to_highest_quorum_cert(
        &self,
        highest_quorum_cert: QuorumCert,
        highest_commit_cert: WrappedLedgerInfo,
        retriever: &mut BlockRetriever,
    ) -> anyhow::Result<()> {
        if !self.need_sync_to_highest_quorum_cert(&highest_quorum_cert) {
            return Ok(());
        }
        let (root, blocks, quorum_certs) = Self::fast_forward_sync(
            &highest_quorum_cert,
            &highest_commit_cert,
            retriever,
            self.storage.clone(),
            self.payload_manager.clone(),
            self.order_vote_enabled,
        )
        .await?
        .take();
        info!(
            LogSchema::new(LogEvent::CommitViaSync).round(self.ordered_root().round()),
            committed_round = root.0.round(),
            block_id = root.0.id(),
        );
        self.rebuild(root, blocks, quorum_certs)
            .await;

        if highest_commit_cert.ledger_info().ledger_info().ends_epoch() {
            retriever
                .network
                .send_epoch_change(EpochChangeProof::new(
                    vec![highest_quorum_cert.ledger_info().clone()],
                    /* more = */ false,
                ))
                .await;
        }
        Ok(())
    }

    /// Fast-forwards the local consensus state by synchronizing blocks and ledger infos for a given epoch.
    /// 
    /// This function retrieves all blocks, quorum certificates, and ledger infos for the specified epoch
    /// from a remote retriever. It then prefetches payload data for each block, saves the blocks and
    /// certificates to local storage, and updates the ledger info in the database. After updating storage,
    /// it attempts to recover the consensus state from the latest ledger info and rebuilds the in-memory
    /// state. If the epoch ends, it sends an epoch change proof to the network.
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
    ) -> anyhow::Result<()> {
        info!("[Fast_Forward_sync] epoch {}", epoch);
        let highest_commit_cert = self.highest_commit_cert();
        let payload_manager = self.payload_manager.clone();
        let storage = self.storage.clone();
        let (blocks, mut quorum_certs, mut ledger_infos) = retriever
            .retrieve_block_by_epoch(
                epoch,
                highest_commit_cert.commit_info().id(),
                retriever.validator_addresses(),
                payload_manager.clone(),
            )
            .await?;

        for (i, block) in blocks.iter().enumerate() {
            assert_eq!(block.id(), quorum_certs[i].certified_block().id());
            if let Some(payload) = block.payload() {
                payload_manager.prefetch_payload_data(payload, block.timestamp_usecs());
            }
        }
        let block_numbers = blocks
            .iter()
            .filter(|block| block.block_number().is_some())
            .map(|block| (block.epoch(), block.block_number().unwrap(), block.id()))
            .collect::<Vec<(u64, u64, HashValue)>>();
        storage.save_tree(blocks, quorum_certs, block_numbers)?;
        if !ledger_infos.is_empty() {
            ledger_infos.reverse();
            let mut ledger_info_batch = SchemaBatch::new();
            for ledger_info in &ledger_infos {
                storage
                    .consensus_db()
                    .ledger_db
                    .metadata_db()
                    .put_ledger_info(ledger_info, &mut ledger_info_batch)?;
            }
            storage.consensus_db().ledger_db.metadata_db().write_schemas(ledger_info_batch)?;
        }
        let (root, blocks, quorum_certs) =
            match storage.start(false, ledger_infos.last().unwrap().ledger_info().epoch()).await {
                LivenessStorageData::FullRecoveryData(recovery_data) => recovery_data,
                _ => panic!("Failed to construct recovery data after fast forward sync"),
            }
            .take();
        self.rebuild(root, blocks, quorum_certs).await;
        storage.consensus_db().ledger_db.metadata_db().set_latest_ledger_info(ledger_infos.last().unwrap().clone());
        info!("lightman0814 send_epoch_change {} {:?}", ledger_infos.len(), ledger_infos.last().unwrap().ledger_info());

        if ledger_infos.last().unwrap().ledger_info().ends_epoch() {
            info!("lightman0814 send_epoch_change");
            retriever
                .network
                .send_epoch_change(EpochChangeProof::new(
                    vec![ledger_infos.last().unwrap().clone()],
                    /* more = */ false,
                ))
                .await;
        }
        Ok(())
    }

    pub async fn fast_forward_sync<'a>(
        highest_quorum_cert: &'a QuorumCert,
        highest_commit_cert: &'a WrappedLedgerInfo,
        retriever: &'a mut BlockRetriever,
        storage: Arc<dyn PersistentLivenessStorage>,
        payload_manager: Arc<dyn TPayloadManager>,
        order_vote_enabled: bool,
    ) -> anyhow::Result<RecoveryData> {
        info!(
            LogSchema::new(LogEvent::StateSync).remote_peer(retriever.preferred_peer),
            "Start block sync to commit cert: {}, quorum cert: {}",
            highest_commit_cert,
            highest_quorum_cert,
        );

        // we fetch the blocks from
        let num_blocks = highest_quorum_cert.certified_block().round()
            - highest_commit_cert.ledger_info().ledger_info().round()
            + 1;

        // although unlikely, we might wrap num_blocks around on a 32-bit machine
        assert!(num_blocks < std::usize::MAX as u64);

        BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC.inc_by(num_blocks);
        let (mut blocks, _, ledger_infos) = retriever
            .retrieve_blocks_in_range(
                highest_quorum_cert.certified_block().id(),
                num_blocks,
                highest_commit_cert.commit_info().id(),
                highest_quorum_cert
                    .ledger_info()
                    .get_voters(&retriever.validator_addresses()),
                payload_manager.clone(),
            )
            .await?;

        assert_eq!(
            blocks.first().expect("blocks are empty").id(),
            highest_quorum_cert.certified_block().id(),
            "Expecting in the retrieval response, first block should be {}, but got {}",
            highest_quorum_cert.certified_block().id(),
            blocks.first().expect("blocks are empty").id(),
        );

        let mut quorum_certs = vec![highest_quorum_cert.clone()];
        quorum_certs.extend(
            blocks
                .iter()
                .take(blocks.len() - 1)
                .map(|block| block.quorum_cert().clone()),
        );
        assert_eq!(blocks.len(), quorum_certs.len());
        info!("[FastForwardSync] Fetched {} blocks. Requested num_blocks {}. Initial block hash {:?}, target block hash {:?}",
            blocks.len(), num_blocks, highest_quorum_cert.certified_block().id(), highest_commit_cert.commit_info().id()
        );
        for (i, block) in blocks.iter().enumerate() {
            assert_eq!(block.id(), quorum_certs[i].certified_block().id());
        }
        let block_numbers = blocks
            .iter()
            .filter(|block| block.block_number().is_some())
            .map(|block| (block.epoch(), block.block_number().unwrap(), block.id()))
            .collect::<Vec<(u64, u64, HashValue)>>();
        storage.save_tree(blocks.clone(), quorum_certs.clone(), block_numbers)?;
        if !ledger_infos.is_empty() {
            let mut ledger_info_batch = SchemaBatch::new();
            for ledger_info in ledger_infos {
                storage.consensus_db().ledger_db.metadata_db().put_ledger_info(&ledger_info, &mut ledger_info_batch);
            }
            storage.consensus_db().ledger_db.metadata_db().write_schemas(ledger_info_batch);
        }
        // we do not need to update block_tree.highest_commit_decision_ledger_info here
        // because the block_tree is going to rebuild itself.

        let recovery_data = match storage.start(order_vote_enabled, highest_quorum_cert.certified_block().epoch()).await {
            LivenessStorageData::FullRecoveryData(recovery_data) => recovery_data,
            _ => panic!("Failed to construct recovery data after fast forward sync"),
        };
        Ok(recovery_data)
    }

    /// Fast forward in the decoupled-execution pipeline if the block exists there
    async fn sync_to_highest_commit_cert(
        &self,
        highest_commit_cert: WrappedLedgerInfo,
        retriever: &mut BlockRetriever,
    ) -> anyhow::Result<()> {
        let ledger_info = highest_commit_cert.ledger_info();
        // if the block exists between commit root and ordered root
        if self.commit_root().round() < ledger_info.commit_info().round()
            && self.block_exists(ledger_info.commit_info().id())
            && self.ordered_root().round() >= ledger_info.commit_info().round()
        {
            info!("sync_to_highest_commit_cert: block exists between commit root and ordered root {:?}, {:?}", self.commit_root().round(), ledger_info.commit_info().round());
            let proof = ledger_info.clone();
            let network = retriever.network.clone();
            tokio::spawn(async move { network.send_commit_proof(proof).await });
            return Ok(());
        } else if self.ordered_root().round() < ledger_info.commit_info().round() 
            && !self.block_exists(ledger_info.commit_info().id()) {
            // if the block doesnt exist after ordered root
            let highest_commit_cert = highest_commit_cert.into_quorum_cert(self.order_vote_enabled).unwrap();
            let (root, blocks, quorum_certs) = Self::fast_forward_sync(
                &highest_commit_cert,
                &self.highest_ordered_cert(),
                retriever,
                self.storage.clone(),
                self.payload_manager.clone(),
                self.order_vote_enabled,
            )
            .await?
            .take();
            info!(
                LogSchema::new(LogEvent::CommitViaSync).round(self.ordered_root().round()),
                committed_round = root.0.round(),
                block_id = root.0.id(),
            );
            self.rebuild(root, blocks, quorum_certs)
                .await;
            return Ok(());
        }
        Ok(())
    }

    /// Retrieve a n chained blocks from the block store starting from
    /// an initial parent id, returning with <n (as many as possible) if
    /// id or its ancestors can not be found.
    ///
    /// The current version of the function is not really async, but keeping it this way for
    /// future possible changes.
    pub async fn process_block_retrieval(
        &self,
        request: IncomingBlockRetrievalRequest,
    ) -> anyhow::Result<()> {
        fail_point!("consensus::process_block_retrieval", |_| {
            Err(anyhow::anyhow!("Injected error in process_block_retrieval"))
        });
        let mut blocks = vec![];
        let mut quorum_certs = vec![];
        let mut status = BlockRetrievalStatus::Succeeded;
        let (retrieval_epoch, mut id) = if let Some(epoch) = request.req.epoch() {
            if request.req.block_id() != HashValue::zero() {
                (epoch, request.req.block_id())
            } else {
                let target_block_number = self
                    .storage
                    .consensus_db()
                    .get_all::<EpochByBlockNumberSchema>()
                    .unwrap()
                    .into_iter()
                    .find(|(_, eppch_)| *eppch_ == epoch)
                    .map(|(block_number, _)| block_number)
                    .unwrap();
                let end_block_id = self
                    .storage
                    .consensus_db()
                    .get::<LedgerInfoSchema>(&target_block_number)
                    .unwrap()
                    .unwrap()
                    .ledger_info()
                    .consensus_block_id();

                let start_key = (epoch, HashValue::zero());
                let end_key = (epoch, HashValue::new([u8::MAX; HashValue::LENGTH]));
                let qc = self
                    .storage
                    .consensus_db()
                    .get_qc_range(&start_key, &end_key)
                    .unwrap()
                    .into_iter()
                    .find(|qc| qc.commit_info().id() == end_block_id)
                    .unwrap()
                    .clone();
                let block = self
                    .storage
                    .consensus_db()
                    .get_block(epoch, qc.certified_block().id())
                    .unwrap()
                    .unwrap();
                quorum_certs.push(qc);
                blocks.push(block);

                (
                    epoch,
                    self.storage
                        .consensus_db()
                        .get::<LedgerInfoSchema>(&target_block_number)
                        .unwrap()
                        .unwrap()
                        .ledger_info()
                        .consensus_block_id(),
                )
            }
        } else {
            (self.ordered_root().epoch(), request.req.block_id())
        };
        info!("process_block_retrieval origin_block_id {}, target_block_id {}, retrieval_epoch {}",
                request.req.block_id(), request.req.target_block_id().unwrap(), retrieval_epoch);

        while (blocks.len() as u64) < request.req.num_blocks() {
            if request.req.match_target_id(id) {
                status = BlockRetrievalStatus::SucceededWithTarget;
                break;
            }
            if let Some(executed_block) = self.get_block(id) {
                quorum_certs.push((*self.get_quorum_cert_for_block(id).unwrap()).clone());
                blocks.push(executed_block.block().clone());
                id = executed_block.parent_id();
            } else if let Ok(Some(executed_block)) =
                self.storage.consensus_db().get_block(retrieval_epoch, id)
            {
                quorum_certs.push(
                    self.storage.consensus_db().get_qc(retrieval_epoch, id).unwrap().unwrap(),
                );
                blocks.push(executed_block.clone());
                id = executed_block.parent_id();
            } else {
                info!("Cannot find the block id {}", id);
                status = BlockRetrievalStatus::NotEnoughBlocks;
                break;
            }
        }

        let mut lower = 0;
        let mut upper = 0;
        for block in &blocks {
            if block.block_number().is_none() {
                continue;
            }
            if upper == 0 {
                upper = block.block_number().unwrap() + 1;
            }
            lower = block.block_number().unwrap();
        }
        let mut ledger_infos = vec![];
        if upper != 0 {
            ledger_infos = self.storage.consensus_db().ledger_db.metadata_db().get_ledger_infos_by_range((lower, upper));
            ledger_infos.reverse();
        }
        info!("process block retrieval done. status={:?}, block size={}", status, blocks.len());
        let response = Box::new(BlockRetrievalResponse::new(status, blocks, quorum_certs, ledger_infos));
        let response_bytes = request
            .protocol
            .to_bytes(&ConsensusMsg::BlockRetrievalResponse(response))?;
        request
            .response_sender
            .send(Ok(response_bytes.into()))
            .map_err(|_| anyhow::anyhow!("Failed to send block retrieval response"))
    }
}

/// BlockRetriever is used internally to retrieve blocks
pub struct BlockRetriever {
    network: Arc<NetworkSender>,
    preferred_peer: Author,
    validator_addresses: Vec<AccountAddress>,
    max_blocks_to_request: u64,
    pending_blocks: Arc<Mutex<PendingBlocks>>,
}

impl BlockRetriever {
    pub fn new(
        network: Arc<NetworkSender>,
        preferred_peer: Author,
        validator_addresses: Vec<AccountAddress>,
        max_blocks_to_request: u64,
        pending_blocks: Arc<Mutex<PendingBlocks>>,
    ) -> Self {
        Self {
            network,
            preferred_peer,
            validator_addresses,
            max_blocks_to_request,
            pending_blocks,
        }
    }

    pub fn validator_addresses(&self) -> Vec<AccountAddress> {
        self.validator_addresses.clone()
    }

    async fn retrieve_block_for_id_chunk(
        &mut self,
        block_id: HashValue,
        target_block_id: HashValue,
        retrieve_batch_size: u64,
        mut peers: Vec<AccountAddress>,
        epoch: Option<u64>,
    ) -> anyhow::Result<BlockRetrievalResponse> {
        let mut failed_attempt = 0_u32;
        let mut cur_retry = 0;

        let num_retries = NUM_RETRIES;
        let request_num_peers = NUM_PEERS_PER_RETRY;
        let retry_interval = Duration::from_millis(RETRY_INTERVAL_MSEC);
        let rpc_timeout = Duration::from_millis(RPC_TIMEOUT_MSEC);

        monitor!("retrieve_block_for_id_chunk", {
            let mut interval = time::interval(retry_interval);
            let mut futures = FuturesUnordered::new();
            let request = if let Some(epoch) = epoch {
                BlockRetrievalRequest::new_with_epoch(
                    block_id,
                    retrieve_batch_size,
                    target_block_id,
                    epoch,
                )
            } else {
                BlockRetrievalRequest::new_with_target_block_id(
                    block_id,
                    retrieve_batch_size,
                    target_block_id,
                )
            };
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // send batch request to a set of peers of size request_num_peers (or 1 for the first time)
                        let next_peers = if cur_retry < num_retries {
                            let first_atempt = cur_retry == 0;
                            cur_retry += 1;
                            self.pick_peers(
                                first_atempt,
                                &mut peers,
                                if first_atempt { 1 } else {request_num_peers}
                            )
                        } else {
                            Vec::new()
                        };

                        if next_peers.is_empty() && futures.is_empty() {
                            bail!("Couldn't fetch block")
                        }

                        for peer in next_peers {
                            info!(
                                LogSchema::new(LogEvent::RetrieveBlock).remote_peer(peer),
                                block_id = block_id,
                                "Fetching {} blocks, retry {}, failed attempts {}",
                                retrieve_batch_size,
                                cur_retry,
                                failed_attempt
                            );
                            let remote_peer = peer;
                            let future = self.network.request_block(
                                request.clone(),
                                peer,
                                rpc_timeout,
                            );
                            futures.push(async move { (remote_peer, future.await) }.boxed());
                        }
                    }
                    Some((peer, response)) = futures.next() => {
                        match response {
                            Ok(result) => return Ok(result),
                            e => {
                                warn!(
                                    remote_peer = peer,
                                    block_id = block_id,
                                    "{:?}, Failed to fetch block",
                                    e,
                                );
                                failed_attempt += 1;
                            },
                        }
                    },
                }
            }
        })
    }

    /// Retrieve n blocks for given block_id from peers
    ///
    /// Returns Result with Vec that if succeeded. This method will
    /// continue until the quorum certificate members all fail to return the missing chain.
    ///
    /// The first attempt of block retrieval will always be sent to preferred_peer to allow the
    /// leader to drive quorum certificate creation The other peers from the quorum certificate
    /// will be randomly tried next.  If all members of the quorum certificate are exhausted, an
    /// error is returned
    async fn retrieve_block_for_id(
        &mut self,
        block_id: HashValue,
        target_block_id: HashValue,
        peers: Vec<AccountAddress>,
        num_blocks: u64,
        payload_manager: Arc<dyn TPayloadManager>,
        epoch: Option<u64>,
    ) -> anyhow::Result<(Vec<Block>, Vec<QuorumCert>, Vec<LedgerInfoWithSignatures>)> {
        info!(
            "Retrieving blocks starting from {}, the total number is {}",
            block_id, num_blocks
        );
        let mut progress = 0;
        let mut last_block_id = block_id;
        let mut result_blocks: Vec<Block> = vec![];
        let mut ledger_infos = vec![];
        let mut quorum_certs = vec![];
        let mut retrieve_batch_size = self.max_blocks_to_request;
        if peers.is_empty() {
            bail!("Failed to fetch block {}: no peers available", block_id);
        }
        while progress < num_blocks {
            // in case this is the last retrieval
            retrieve_batch_size = min(retrieve_batch_size, num_blocks - progress);

            info!(
                "Retrieving chunk: {} blocks starting from {}, original start {}",
                retrieve_batch_size, last_block_id, block_id
            );

            let response = self
                .retrieve_block_for_id_chunk(
                    last_block_id,
                    target_block_id,
                    retrieve_batch_size,
                    peers.clone(),
                    epoch,
                )
                .await;
            match response {
                Ok(result) if matches!(result.status(), BlockRetrievalStatus::Succeeded) => {
                    // extend the result blocks
                    let batch = result.blocks().clone();
                    for block in batch.iter() {
                        if let Some(payload) = block.payload() {
                            payload_manager.prefetch_payload_data(payload, block.timestamp_usecs());
                        }
                    }
                    progress += batch.len() as u64;
                    last_block_id = batch.last().expect("Batch should not be empty").parent_id();
                    CUR_BLOCK_SYNC_BLOCK_SUM_GAUGE.with_label_values(&[]).add(batch.len() as i64);
                    result_blocks.extend(batch);
                    ledger_infos.extend(result.ledger_infos().clone());
                    quorum_certs.extend(result.quorum_certs().clone());
                }
                Ok(result)
                    if matches!(result.status(), BlockRetrievalStatus::SucceededWithTarget) =>
                {
                    // if we found the target, end the loop
                    let batch = result.blocks().clone();
                    for block in batch.iter() {
                        if let Some(payload) = block.payload() {
                            payload_manager.prefetch_payload_data(payload, block.timestamp_usecs());
                        }
                    }
                    CUR_BLOCK_SYNC_BLOCK_SUM_GAUGE.with_label_values(&[]).add(batch.len() as i64);
                    result_blocks.extend(batch);
                    ledger_infos.extend(result.ledger_infos().clone());
                    quorum_certs.extend(result.quorum_certs().clone());
                    break;
                }
                _e => {
                    bail!(
                        "Failed to fetch block {}, for original start {}",
                        last_block_id,
                        block_id,
                    );
                }
            }
        }
        Ok((result_blocks, quorum_certs, ledger_infos))
    }

    /// Retrieves all blocks, quorum certificates, and ledger infos for a given epoch from peers.
    ///
    /// This function first attempts to retrieve a batch of blocks for the specified epoch using
    /// `retrieve_block_for_id_chunk`. If more blocks are needed, it continues to fetch the remaining
    /// chain using `retrieve_block_for_id`. For each block, it prefetches the payload data if present.
    /// The function accumulates all blocks, quorum certificates, and ledger infos into vectors and
    /// returns them as a tuple.
    ///
    /// # Arguments
    /// * `epoch` - The epoch to retrieve blocks for.
    /// * `target_block_id` - The target block id to stop retrieval.
    /// * `peers` - The list of peer addresses to fetch blocks from.
    /// * `payload_manager` - The payload manager used to prefetch payload data.
    ///
    /// # Returns
    /// * `Ok((blocks, quorum_certs, ledger_infos))` on success, containing all retrieved data.
    /// * `Err` if the retrieval fails at any step.
    async fn retrieve_block_by_epoch(
        &mut self,
        epoch: u64,
        target_block_id: HashValue,
        peers: Vec<AccountAddress>,
        payload_manager: Arc<dyn TPayloadManager>,
    ) -> anyhow::Result<(Vec<Block>, Vec<QuorumCert>, Vec<LedgerInfoWithSignatures>)> {
        let mut result_blocks = vec![];
        let mut ledger_infos = vec![];
        let mut quorum_certs = vec![];
        let response = self
            .retrieve_block_for_id_chunk(
                HashValue::zero(),
                target_block_id,
                self.max_blocks_to_request,
                peers.clone(),
                Some(epoch),
            )
            .await;
        match response {
            Ok(result) if matches!(result.status(), BlockRetrievalStatus::Succeeded) => {
                let batch = result.blocks().clone();
                for block in batch.iter() {
                    if let Some(payload) = block.payload() {
                        payload_manager.prefetch_payload_data(payload, block.timestamp_usecs());
                    }
                }
                let last_block_id = batch.last().expect("Batch should not be empty").parent_id();
                result_blocks.extend(batch);
                ledger_infos.extend(result.ledger_infos().clone());
                quorum_certs.extend(result.quorum_certs().clone());
                let (mut other_blocks, mut other_quorum_certs, mut other_ledger_infos) = self
                    .retrieve_block_for_id(
                        last_block_id,
                        target_block_id,
                        peers,
                        u64::MAX,
                        payload_manager,
                        Some(epoch),
                    )
                    .await?;

                result_blocks.append(&mut other_blocks);
                ledger_infos.append(&mut other_ledger_infos);
                quorum_certs.append(&mut other_quorum_certs);
            }
            Ok(result) if matches!(result.status(), BlockRetrievalStatus::SucceededWithTarget) => {
                // if we found the target, end the loop
                let batch = result.blocks().clone();
                for block in batch.iter() {
                    if let Some(payload) = block.payload() {
                        payload_manager.prefetch_payload_data(payload, block.timestamp_usecs());
                    }
                }
                result_blocks.extend(batch);
                ledger_infos.extend(result.ledger_infos().clone());
                quorum_certs.extend(result.quorum_certs().clone());
            }
            e => {
                bail!("Failed to fetch epoch {} {:?}", epoch, e);
            }
        }
        Ok((result_blocks, quorum_certs, ledger_infos))
    }

    /// Retrieve chain of n blocks for given QC
    async fn retrieve_blocks_in_range(
        &mut self,
        initial_block_id: HashValue,
        num_blocks: u64,
        target_block_id: HashValue,
        peers: Vec<AccountAddress>,
        payload_manager: Arc<dyn TPayloadManager>,
    ) -> anyhow::Result<(Vec<Block>, Vec<QuorumCert>, Vec<LedgerInfoWithSignatures>)> {
        BLOCKS_FETCHED_FROM_NETWORK_IN_BLOCK_RETRIEVER.inc_by(num_blocks);
        self.retrieve_block_for_id(initial_block_id, target_block_id, peers, num_blocks, payload_manager, None)
            .await
    }

    fn pick_peer(&self, first_atempt: bool, peers: &mut Vec<AccountAddress>) -> AccountAddress {
        assert!(!peers.is_empty(), "pick_peer on empty peer list");

        if first_atempt {
            // remove preferred_peer if its in list of peers
            // (strictly speaking it is not required to be there)
            for i in 0..peers.len() {
                if peers[i] == self.preferred_peer {
                    peers.remove(i);
                    break;
                }
            }
            return self.preferred_peer;
        }

        let peer_idx = thread_rng().gen_range(0, peers.len());
        peers.remove(peer_idx)
    }

    fn pick_peers(
        &self,
        first_atempt: bool,
        peers: &mut Vec<AccountAddress>,
        request_num_peers: usize,
    ) -> Vec<AccountAddress> {
        let mut result = Vec::new();
        while !peers.is_empty() && result.len() < request_num_peers {
            result.push(self.pick_peer(first_atempt && result.is_empty(), peers));
        }
        result
    }
}