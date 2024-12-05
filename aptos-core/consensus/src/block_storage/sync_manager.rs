// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_storage::{
        pending_blocks::PendingBlocks,
        tracing::{observe_block, BlockStage},
        BlockReader, BlockStore,
    },
    consensus_observer::ordered_blocks,
    counters::{
        BLOCKS_FETCHED_FROM_NETWORK_IN_BLOCK_RETRIEVER,
        BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC,
        BLOCKS_FETCHED_FROM_NETWORK_WHILE_INSERTING_QUORUM_CERT, LATE_EXECUTION_WITH_ORDER_VOTE_QC,
        SUCCESSFUL_EXECUTED_WITH_ORDER_VOTE_QC, SUCCESSFUL_EXECUTED_WITH_REGULAR_QC,
    },
    epoch_manager::LivenessStorageData,
    logging::{LogEvent, LogSchema},
    monitor,
    network::{IncomingBlockRetrievalRequest, NetworkSender},
    network_interface::ConsensusMsg,
    payload_manager::TPayloadManager,
    persistent_liveness_storage::{LedgerRecoveryData, PersistentLivenessStorage, RecoveryData},
    pipeline::execution_client::TExecutionClient,
};
use anyhow::{anyhow, bail, Context};
use api_types::{ExecutionApiV2, ExecutionBlocks, ExternalBlockMeta};
use aptos_consensus_types::{
    block::Block,
    block_retrieval::{
        BlockRetrievalRequest, BlockRetrievalResponse, BlockRetrievalStatus, SyncBlocks,
        NUM_PEERS_PER_RETRY, NUM_RETRIES, RETRY_INTERVAL_MSEC, RPC_TIMEOUT_MSEC,
    },
    common::Author,
    quorum_cert::QuorumCert,
    sync_info::SyncInfo,
    wrapped_ledger_info::WrappedLedgerInfo,
};
use aptos_crypto::{hash::GENESIS_BLOCK_ID, HashValue};
use aptos_infallible::Mutex;
use aptos_logger::prelude::*;
use aptos_types::{
    account_address::AccountAddress, epoch_change::EpochChangeProof,
    ledger_info::LedgerInfoWithSignatures,
};
use fail::fail_point;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use futures_channel::oneshot;
use rand::{prelude::*, Rng};
use std::{clone::Clone, cmp::min, sync::Arc, time::Duration};
use tokio::{time, time::timeout};

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

    /// Checks if quorum certificate can be inserted in block store without RPC
    /// Returns the enum to indicate the detailed status.
    pub fn need_fetch_for_quorum_cert(&self, qc: &QuorumCert) -> NeedFetchResult {
        if qc.certified_block().round() < self.ordered_root().round() {
            return NeedFetchResult::QCRoundBeforeRoot;
        }
        if self.get_quorum_cert_for_block(qc.certified_block().id()).is_some() {
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
        self.sync_to_highest_commit_cert(sync_info.highest_commit_cert().clone(), &mut retriever)
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
        self.insert_quorum_cert(sync_info.highest_quorum_cert(), &mut retriever).await?;

        // Even though we inserted the highest_quorum_cert (and its ancestors) in the above step,
        // we still need to insert ordered cert explicitly. This will send the highest ordered block
        // to execution.
        if self.order_vote_enabled {
            self.insert_ordered_cert(&sync_info.highest_ordered_cert()).await?;
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
            self.send_for_execution(qc.into_wrapped_ledger_info(), false).await?;
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
                    observe_block(ordered_block.block().timestamp_usecs(), BlockStage::OC_ADDED);
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
            let mut blocks = retriever
                .retrieve_blocks_in_range(
                    retrieve_qc.certified_block().id(),
                    1,
                    retrieve_qc.certified_block().id(),
                    qc.ledger_info().get_voters(&retriever.validator_addresses()),
                )
                .await?;
            // retrieve_blocks_in_range guarantees that blocks has exactly 1 element
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
        if !self.need_sync_for_ledger_info(highest_commit_cert.ledger_info()) {
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
        self.rebuild(root, blocks, quorum_certs).await;

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

    pub async fn fast_forward_sync_by_block_number<'a>(
        retriever: &'a mut BlockRetriever,
        execution_api: Arc<dyn ExecutionApiV2>,
        start_block_number: u64,
    ) -> anyhow::Result<()> {
        info!(
            LogSchema::new(LogEvent::StateSync).remote_peer(retriever.preferred_peer),
            "Start block sync from start_block_number {}", start_block_number,
        );

        let blocks_batch = retriever
            .retrieve_block_by_block_number(retriever.validator_addresses(), start_block_number)
            .await?;
        for blocks in blocks_batch {
            let commit_block_hash = blocks.latest_block_hash;
            let commit_block_number = blocks.latest_block_number;
            execution_api.recover_execution_blocks(blocks).await;
            // TODO(gravity_lightman): error handle
            execution_api
                .commit_block(ExternalBlockMeta {
                    block_id: commit_block_hash,
                    block_number: commit_block_number,
                })
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
        let mut blocks = retriever
            .retrieve_blocks_in_range(
                highest_quorum_cert.certified_block().id(),
                num_blocks,
                highest_commit_cert.commit_info().id(),
                highest_quorum_cert.ledger_info().get_voters(&retriever.validator_addresses()),
            )
            .await?;

        assert_eq!(
            blocks.first().expect("blocks are empty").id(),
            highest_quorum_cert.certified_block().id(),
            "Expecting in the retrieval response, first block should be {}, but got {}",
            highest_quorum_cert.certified_block().id(),
            blocks.first().expect("blocks are empty").id(),
        );

        if highest_commit_cert.commit_info().id() != *GENESIS_BLOCK_ID {
            // Confirm retrieval ended when it hit the last block we care about, even if it didn't reach all num_blocks blocks.
            assert_eq!(
                blocks.last().expect("blocks are empty").id(),
                highest_commit_cert.commit_info().id()
            );
        }

        let mut quorum_certs = vec![highest_quorum_cert.clone()];
        quorum_certs
            .extend(blocks.iter().take(blocks.len() - 1).map(|block| block.quorum_cert().clone()));
        if !order_vote_enabled && highest_commit_cert.commit_info().id() != *GENESIS_BLOCK_ID {
            // check if highest_commit_cert comes from a fork
            // if so, we need to fetch it's block as well, to have a proof of commit.
            let highest_commit_certified_block_id =
                highest_commit_cert.certified_block(order_vote_enabled)?.id();
            if !blocks.iter().any(|block| block.id() == highest_commit_certified_block_id) {
                info!("Found forked QC {}, fetching it as well", highest_commit_cert);
                BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC.inc_by(1);
                let mut additional_blocks = retriever
                    .retrieve_blocks_in_range(
                        highest_commit_certified_block_id,
                        1,
                        highest_commit_certified_block_id,
                        highest_commit_cert
                            .ledger_info()
                            .get_voters(&retriever.validator_addresses()),
                    )
                    .await?;

                assert_eq!(additional_blocks.len(), 1);
                let block = additional_blocks.pop().expect("blocks are empty");
                assert_eq!(
                    block.id(),
                    highest_commit_certified_block_id,
                    "Expecting in the retrieval response, for commit certificate fork, first block should be {}, but got {}",
                    highest_commit_certified_block_id,
                    block.id(),
                );
                blocks.push(block);
                quorum_certs
                    .push(highest_commit_cert.clone().into_quorum_cert(order_vote_enabled)?);
            }
        }
        assert_eq!(blocks.len(), quorum_certs.len());
        info!("[FastForwardSync] Fetched {} blocks. Requested num_blocks {}. Initial block hash {:?}, target block hash {:?}",
            blocks.len(), num_blocks, highest_quorum_cert.certified_block().id(), highest_commit_cert.commit_info().id()
        );
        for (i, block) in blocks.iter().enumerate() {
            assert_eq!(block.id(), quorum_certs[i].certified_block().id());
            if let Some(payload) = block.payload() {
                payload_manager.prefetch_payload_data(payload, block.timestamp_usecs());
            }
        }
        // Check early that recovery will succeed, and return before corrupting our state in case it will not.
        // TODO(gravity_lightman):
        // LedgerRecoveryData::new(highest_commit_cert.ledger_info().clone())
        //     .find_root(
        //         &mut blocks.clone(),
        //         &mut quorum_certs.clone(),
        //         order_vote_enabled,
        //     )
        //     .with_context(|| {
        //         // for better readability
        //         quorum_certs.sort_by_key(|qc| qc.certified_block().round());
        //         format!(
        //             "\nRoot: {:?}\nBlocks in db: {}\nQuorum Certs in db: {}\n",
        //             highest_commit_cert.commit_info(),
        //             blocks
        //                 .iter()
        //                 .map(|b| format!("\n\t{}", b))
        //                 .collect::<Vec<String>>()
        //                 .concat(),
        //             quorum_certs
        //                 .iter()
        //                 .map(|qc| format!("\n\t{}", qc))
        //                 .collect::<Vec<String>>()
        //                 .concat(),
        //         )
        //     })?;
        storage.save_tree(blocks.clone(), quorum_certs.clone())?;
        // we do not need to update block_tree.highest_commit_decision_ledger_info here
        // because the block_tree is going to rebuild itself.

        let recovery_data = match storage.start(order_vote_enabled) {
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
            let proof = ledger_info.clone();
            let network = retriever.network.clone();
            tokio::spawn(async move { network.send_commit_proof(proof).await });
            return Ok(());
        } else if self.ordered_root().round() < ledger_info.commit_info().round()
            && !self.block_exists(ledger_info.commit_info().id())
        {
            // if the block doesnt exist after ordered root
            let start_block_number = match self.ordered_root().block().is_genesis_block() {
                true => 1,
                false => {
                    let block_number = self.ordered_root().block().block_number();
                    assert!(block_number.is_some());
                    block_number.unwrap() + 1
                }
            };
            return Self::fast_forward_sync_by_block_number(
                retriever,
                self.execution_api.as_ref().unwrap().clone(),
                start_block_number,
            )
            .await;
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
        let mut response_bytes = vec![];
        if let Some(start_block_number) = request.req.start_block_number() {
            assert!(self.commit_root().block().block_number().is_some());
            let mut status = BlockRetrievalStatus::Succeeded;
            let target_block_number = self
                .get_block(self.highest_commit_cert().commit_info().id())
                .unwrap()
                .block()
                .block_number()
                .unwrap();
            let end_block_number =
                target_block_number.min(start_block_number + request.req.num_blocks());
            info!(
                "sync blocks by block number from {} to {}, target is {}",
                start_block_number, end_block_number, target_block_number
            );
            let blocks = self
                .execution_api
                .as_ref()
                .unwrap()
                .get_blocks_by_range(start_block_number, end_block_number);
            if end_block_number == target_block_number {
                status = BlockRetrievalStatus::SucceededWithTarget;
            }
            let response =
                Box::new(BlockRetrievalResponse::new(status, SyncBlocks::Execution(blocks)));
            response_bytes =
                request.protocol.to_bytes(&ConsensusMsg::BlockRetrievalResponse(response))?;
        } else {
            info!(
                "process_block_retrieval origin_block_id {}, target_block_id {}",
                request.req.block_id(),
                request.req.target_block_id().unwrap(),
            );
            let mut blocks = vec![];
            let mut status = BlockRetrievalStatus::Succeeded;
            let mut id = request.req.block_id();

            while (blocks.len() as u64) < request.req.num_blocks() {
                if id == *GENESIS_BLOCK_ID {
                    assert!(request.req.target_block_id().unwrap() == *GENESIS_BLOCK_ID);
                    status = BlockRetrievalStatus::SucceededWithTarget;
                    break;
                }
                if let Some(executed_block) = self.get_block(id) {
                    if executed_block.block().is_genesis_block() {
                        continue;
                    }
                    blocks.push(executed_block.block().clone());
                    if request.req.match_target_id(id) {
                        status = BlockRetrievalStatus::SucceededWithTarget;
                        break;
                    }
                    id = executed_block.parent_id();
                } else {
                    info!("Cannot find the block id {}", id);
                    status = BlockRetrievalStatus::NotEnoughBlocks;
                    break;
                }
            }

            if blocks.is_empty() {
                status = BlockRetrievalStatus::IdNotFound;
            }
            info!("process block retrieval done. status={:?}, block size={}", status, blocks.len());
            let response =
                Box::new(BlockRetrievalResponse::new(status, SyncBlocks::Consensus(blocks)));
            response_bytes =
                request.protocol.to_bytes(&ConsensusMsg::BlockRetrievalResponse(response))?;
        }
        assert!(!response_bytes.is_empty());
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
        Self { network, preferred_peer, validator_addresses, max_blocks_to_request, pending_blocks }
    }

    pub fn validator_addresses(&self) -> Vec<AccountAddress> {
        self.validator_addresses.clone()
    }

    async fn retrieve_blocks(
        &mut self,
        block_id: HashValue,
        target_block_id: HashValue,
        retrieve_batch_size: u64,
        mut peers: Vec<AccountAddress>,
        start_block_number: Option<u64>,
    ) -> anyhow::Result<BlockRetrievalResponse> {
        let mut failed_attempt = 0_u32;
        let mut cur_retry = 0;

        let num_retries = NUM_RETRIES;
        let request_num_peers = NUM_PEERS_PER_RETRY;
        let retry_interval = Duration::from_millis(RETRY_INTERVAL_MSEC);
        let rpc_timeout = Duration::from_millis(RPC_TIMEOUT_MSEC);

        monitor!("retrieve_blocks", {
            let mut interval = time::interval(retry_interval);
            let mut futures = FuturesUnordered::new();
            if retrieve_batch_size == 1 {
                let (tx, rx) = oneshot::channel();
                self.pending_blocks.lock().insert_request(target_block_id, tx);
                let author = self.network.author();
                futures.push(
                    async move {
                        let response = match timeout(rpc_timeout, rx).await {
                            Ok(Ok(block)) => Ok(BlockRetrievalResponse::new(
                                BlockRetrievalStatus::SucceededWithTarget,
                                SyncBlocks::Consensus(vec![block]),
                            )),
                            Ok(Err(_)) => Err(anyhow!("self retrieval cancelled")),
                            Err(_) => Err(anyhow!("self retrieval timeout")),
                        };
                        (author, response)
                    }
                    .boxed(),
                )
            }
            let request: BlockRetrievalRequest;
            match start_block_number {
                Some(start_block_number) => {
                    request = BlockRetrievalRequest::new_with_block_number(
                        retrieve_batch_size,
                        start_block_number,
                    );
                }
                None => {
                    request = BlockRetrievalRequest::new_with_target_block_id(
                        block_id,
                        retrieve_batch_size,
                        target_block_id,
                    );
                }
            }
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

    async fn retrieve_block_by_block_number(
        &mut self,
        peers: Vec<AccountAddress>,
        start_block_number: u64,
    ) -> anyhow::Result<Vec<ExecutionBlocks>> {
        let mut start_block_number = start_block_number;
        info!("Retrieving blocks starting from {}", start_block_number);
        let mut result_blocks = vec![];
        if peers.is_empty() {
            bail!("Failed to fetch block number {}: no peers available", start_block_number);
        }
        loop {
            let response = self
                .retrieve_blocks(
                    HashValue::zero(),
                    HashValue::zero(),
                    self.max_blocks_to_request,
                    peers.clone(),
                    Some(start_block_number),
                )
                .await;
            match response {
                Ok(result) if matches!(result.status(), BlockRetrievalStatus::Succeeded) => {
                    // extend the result blocks
                    let batch = result.execution_blocks().clone();
                    start_block_number = batch.latest_block_number + 1;
                    result_blocks.push(batch);
                }
                Ok(result)
                    if matches!(result.status(), BlockRetrievalStatus::SucceededWithTarget) =>
                {
                    // if we found the target, end the loop
                    let batch = result.execution_blocks().clone();
                    result_blocks.push(batch);
                    break;
                }
                _e => {
                    bail!("Failed to fetch block number {}", start_block_number,);
                }
            }
        }
        Ok(result_blocks)
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
    ) -> anyhow::Result<Vec<Block>> {
        info!("Retrieving blocks starting from {}, the total number is {}", block_id, num_blocks);
        let mut progress = 0;
        let mut last_block_id = block_id;
        let mut result_blocks: Vec<Block> = vec![];
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
                .retrieve_blocks(
                    last_block_id,
                    target_block_id,
                    retrieve_batch_size,
                    peers.clone(),
                    None,
                )
                .await;
            match response {
                Ok(result) if matches!(result.status(), BlockRetrievalStatus::Succeeded) => {
                    // extend the result blocks
                    let batch = result.consensus_blocks().clone();
                    progress += batch.len() as u64;
                    last_block_id = batch.last().expect("Batch should not be empty").parent_id();
                    result_blocks.extend(batch);
                }
                Ok(result)
                    if matches!(result.status(), BlockRetrievalStatus::SucceededWithTarget) =>
                {
                    // if we found the target, end the loop
                    let batch = result.consensus_blocks().clone();
                    result_blocks.extend(batch);
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
        if target_block_id != *GENESIS_BLOCK_ID {
            assert_eq!(
                result_blocks.last().expect("Expected at least a result_block").id(),
                target_block_id
            );
        }
        Ok(result_blocks)
    }

    /// Retrieve chain of n blocks for given QC
    async fn retrieve_blocks_in_range(
        &mut self,
        initial_block_id: HashValue,
        num_blocks: u64,
        target_block_id: HashValue,
        peers: Vec<AccountAddress>,
    ) -> anyhow::Result<Vec<Block>> {
        BLOCKS_FETCHED_FROM_NETWORK_IN_BLOCK_RETRIEVER.inc_by(num_blocks);
        self.retrieve_block_for_id(initial_block_id, target_block_id, peers, num_blocks).await
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
