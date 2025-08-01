// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_storage::{
        block_tree::BlockTree,
        pending_blocks::PendingBlocks,
        tracing::{observe_block, BlockStage},
        BlockReader,
    },
    payload_manager::TPayloadManager,
    persistent_liveness_storage::{PersistentLivenessStorage, RecoveryData, RootInfo},
    pipeline::execution_client::TExecutionClient,
    util::time_service::TimeService,
};
use anyhow::{bail, ensure, format_err, Context};
use gaptos::api_types::{
    compute_res::ComputeRes,
    u256_define::{BlockId, Random},
    ExternalBlock, ExternalBlockMeta,
};
use aptos_consensus_types::{
    block::Block,
    common::Round,
    pipelined_block::{ExecutionSummary, PipelinedBlock},
    quorum_cert::QuorumCert,
    sync_info::SyncInfo,
    timeout_2chain::TwoChainTimeoutCertificate,
    wrapped_ledger_info::WrappedLedgerInfo,
};
use gaptos::aptos_crypto::HashValue;
use aptos_executor_types::StateComputeResult;
use gaptos::aptos_infallible::{Mutex, RwLock};
use gaptos::aptos_logger::prelude::*;
use aptos_mempool::core_mempool::transaction::VerifiedTxn;
use gaptos::aptos_metrics_core::{register_int_gauge_vec, IntGaugeHelper, IntGaugeVec};
use gaptos::aptos_types::{
    aggregate_signature::AggregateSignature,
    ledger_info::{LedgerInfo, LedgerInfoWithSignatures},
};
use block_buffer_manager::{block_buffer_manager::BlockHashRef, get_block_buffer_manager};
use futures::executor::block_on;
use once_cell::sync::Lazy;
use sha3::digest::generic_array::typenum::Le;

#[cfg(test)]
use std::collections::VecDeque;
#[cfg(any(test, feature = "fuzzing"))]
use std::sync::atomic::AtomicBool;
#[cfg(any(test, feature = "fuzzing"))]
use std::sync::atomic::Ordering;
use std::{collections::BTreeMap, io::Read, sync::Arc, time::Duration};

use super::BlockRetriever;
use gaptos::aptos_consensus::counters as counters;

#[cfg(test)]
#[path = "block_store_test.rs"]
mod block_store_test;

#[path = "sync_manager.rs"]
pub mod sync_manager;

static CUR_RECOVER_BLOCK_NUMBER_GAUGE: Lazy<IntGaugeVec> = Lazy::new(|| {
     register_int_gauge_vec!(
         "aptos_current_recover_block_number",
         "Current reccover block number",
         &[]
     )
     .unwrap()
 });
 
 static RECOVERY_GAUGE: Lazy<IntGaugeVec> = Lazy::new(|| {
     register_int_gauge_vec!(
         "aptos_recovery",
         "is recovery or not",
         &[]
     )
     .unwrap()
 });

fn update_counters_for_ordered_blocks(ordered_blocks: &[Arc<PipelinedBlock>]) {
    for block in ordered_blocks {
        observe_block(block.block().timestamp_usecs(), BlockStage::ORDERED);
    }
}

/// Responsible for maintaining all the blocks of payload and the dependencies of those blocks
/// (parent and previous QC links).  It is expected to be accessed concurrently by multiple threads
/// and is thread-safe.
///
/// Example tree block structure based on parent links.
///                         ╭--> A3
/// Genesis--> B0--> B1--> B2--> B3
///             ╰--> C1--> C2
///                         ╰--> D3
///
/// Example corresponding tree block structure for the QC links (must follow QC constraints).
///                         ╭--> A3
/// Genesis--> B0--> B1--> B2--> B3
///             ├--> C1
///             ├--------> C2
///             ╰--------------> D3
pub struct BlockStore {
    inner: Arc<RwLock<BlockTree>>,
    execution_client: Arc<dyn TExecutionClient>,
    /// The persistent storage backing up the in-memory data structure, every write should go
    /// through this before in-memory tree.
    storage: Arc<dyn PersistentLivenessStorage>,
    /// Used to ensure that any block stored will have a timestamp < the local time
    time_service: Arc<dyn TimeService>,
    // consistent with round type
    vote_back_pressure_limit: Round,
    payload_manager: Arc<dyn TPayloadManager>,
    #[cfg(any(test, feature = "fuzzing"))]
    back_pressure_for_test: AtomicBool,
    order_vote_enabled: bool,
    pending_blocks: Arc<Mutex<PendingBlocks>>,
}

impl BlockStore {
    pub fn new(
        storage: Arc<dyn PersistentLivenessStorage>,
        initial_data: RecoveryData,
        execution_client: Arc<dyn TExecutionClient>,
        max_pruned_blocks_in_mem: usize,
        time_service: Arc<dyn TimeService>,
        vote_back_pressure_limit: Round,
        payload_manager: Arc<dyn TPayloadManager>,
        order_vote_enabled: bool,
        pending_blocks: Arc<Mutex<PendingBlocks>>,
    ) -> Self {
        let highest_2chain_tc = initial_data.highest_2chain_timeout_certificate();
        let (root, blocks, quorum_certs) = initial_data.take();
        info!("root {:?}", root);
        let block_store = block_on(Self::build(
            root,
            blocks,
            quorum_certs,
            highest_2chain_tc,
            execution_client,
            storage,
            max_pruned_blocks_in_mem,
            time_service,
            vote_back_pressure_limit,
            payload_manager,
            order_vote_enabled,
            pending_blocks,
        ));
        block_on(block_store.recover_blocks());
        block_store
    }

    pub async fn async_new(
        storage: Arc<dyn PersistentLivenessStorage>,
        initial_data: RecoveryData,
        execution_client: Arc<dyn TExecutionClient>,
        max_pruned_blocks_in_mem: usize,
        time_service: Arc<dyn TimeService>,
        vote_back_pressure_limit: Round,
        payload_manager: Arc<dyn TPayloadManager>,
        order_vote_enabled: bool,
        pending_blocks: Arc<Mutex<PendingBlocks>>,
    ) -> Self {
        let highest_2chain_tc = initial_data.highest_2chain_timeout_certificate();
        let (root, blocks, quorum_certs) = initial_data.take();
        info!("async_new root {:?}", root);
        let block_store = Self::build(
            root,
            blocks,
            quorum_certs,
            highest_2chain_tc,
            execution_client,
            storage,
            max_pruned_blocks_in_mem,
            time_service,
            vote_back_pressure_limit,
            payload_manager,
            order_vote_enabled,
            pending_blocks,
        )
        .await;
        block_store.recover_blocks().await;
        block_store
    }

    fn find_the_last_ledger_info(
        &self,
        cur_round: u64,
        round_to_ledger_infos: &BTreeMap<u64, LedgerInfoWithSignatures>,
    ) -> LedgerInfoWithSignatures {
        for i in (1..cur_round).rev() {
            if let Some(li) = round_to_ledger_infos.get(&i) {
                return li.clone();
            }
        }
        LedgerInfoWithSignatures::new(LedgerInfo::dummy(), AggregateSignature::empty())
    }

    async fn recover_blocks(&self) {
        RECOVERY_GAUGE.set_with(&[], 1);
        // reproduce the same batches (important for the commit phase)
        let mut certs = self.inner.read().get_all_quorum_certs_with_commit_info();
        certs.sort_unstable_by_key(|qc| qc.commit_info().round());
        for qc in certs {
            if qc.commit_info().round() > self.commit_root().round() {
                info!("sending qc {} to execution, current commit round {}", qc.commit_info().round(), self.commit_root().round());
                if let Err(e) = self.send_for_execution(qc.into_wrapped_ledger_info(), true).await {
                    error!("Error in try-committing blocks. {}", e.to_string());
                }
            }
        }
        RECOVERY_GAUGE.set_with(&[], 0);
    }

    async fn build(
        root: RootInfo,
        blocks: Vec<Block>,
        quorum_certs: Vec<QuorumCert>,
        highest_2chain_timeout_cert: Option<TwoChainTimeoutCertificate>,
        execution_client: Arc<dyn TExecutionClient>,
        storage: Arc<dyn PersistentLivenessStorage>,
        max_pruned_blocks_in_mem: usize,
        time_service: Arc<dyn TimeService>,
        vote_back_pressure_limit: Round,
        payload_manager: Arc<dyn TPayloadManager>,
        order_vote_enabled: bool,
        pending_blocks: Arc<Mutex<PendingBlocks>>,
    ) -> Self {
        let RootInfo(root_block, root_qc, root_ordered_cert, root_commit_cert) = root;

        let result = StateComputeResult::with_root_hash(root_block.id());

        let pipelined_root_block = PipelinedBlock::new(
            *root_block,
            vec![],
            // Create a dummy state_compute_result with necessary fields filled in.
            result,
        );

        let tree = BlockTree::new(
            pipelined_root_block,
            root_qc,
            root_ordered_cert,
            root_commit_cert,
            max_pruned_blocks_in_mem,
            highest_2chain_timeout_cert.map(Arc::new),
        );

        let block_store = Self {
            inner: Arc::new(RwLock::new(tree)),
            execution_client,
            storage,
            time_service,
            vote_back_pressure_limit,
            payload_manager,
            #[cfg(any(test, feature = "fuzzing"))]
            back_pressure_for_test: AtomicBool::new(false),
            order_vote_enabled,
            pending_blocks,
        };

        for block in blocks {
            block_store.insert_block(block, true).await.unwrap_or_else(|e| {
                panic!("[BlockStore] failed to insert block during build {:?}", e)
            });
        }
        for qc in quorum_certs {
            block_store.insert_single_quorum_cert(qc, true).unwrap_or_else(|e| {
                panic!("[BlockStore] failed to insert quorum during build{:?}", e)
            });
        }

        counters::LAST_COMMITTED_ROUND.set(block_store.ordered_root().round() as i64);
        block_store
    }

    pub fn init_block_number(&self, ordered_blocks: &Vec<Arc<PipelinedBlock>>) {
        let mut block_numbers = vec![];
        let mut block_number = 0;
        for p_block in ordered_blocks {
            if let Some(parent_block) = self.get_block(p_block.parent_id()) {
                block_number = parent_block.block().block_number().unwrap() + 1;
            } else if let Ok(Some(parent_block)) = self.storage.consensus_db().get_block(p_block.epoch(), p_block.parent_id()) {
                block_number = parent_block.block_number().unwrap() + 1;
            } else {
                panic!("Cannot find the parent_block id {}", p_block.parent_id());
            }
            if let Some(cur_block_number) = p_block.block().block_number() {
                assert_eq!(cur_block_number, block_number);
                continue;
            }
            p_block.block().set_block_number(block_number);
            block_numbers.push((p_block.block().epoch(), p_block.block().block_number().unwrap(), p_block.block().id()));
        }
        if block_numbers.len() != 0 {
            self.storage.save_tree(vec![], vec![], block_numbers).unwrap();
        }
    }

    /// Send an ordered block id with the proof for execution, returns () on success or error
    pub async fn send_for_execution(
        &self,
        finality_proof: WrappedLedgerInfo,
        recovery: bool,
    ) -> anyhow::Result<()> {
        let block_id_to_commit = finality_proof.commit_info().id();
        let block_to_commit = self
            .get_block(block_id_to_commit)
            .ok_or_else(|| format_err!("Committed block id not found"))?;

        // First make sure that this commit is new.
        ensure!(
            block_to_commit.round() > self.ordered_root().round(),
            "Committed block round lower than root"
        );

        let blocks_to_commit = self.path_from_ordered_root(block_id_to_commit).unwrap_or_default();
        assert!(!blocks_to_commit.is_empty());
        counters::SEND_TO_EXECUTION_BLOCK_COUNTER.inc_by(blocks_to_commit.len() as u64);
        let block_tree = self.inner.clone();
        let storage = self.storage.clone();
        let finality_proof_clone = finality_proof.clone();
        self.pending_blocks.lock().gc(finality_proof.commit_info().round());
        // This callback is invoked synchronously with and could be used for multiple batches of blocks.
        self.init_block_number(&blocks_to_commit);
        if recovery {
            for p_block in &blocks_to_commit {
                let mut txns = vec![];
                loop {
                    match self.payload_manager.get_transactions(p_block.block()).await {
                        Ok((mut txns_, _)) => {
                            txns.append(&mut txns_);
                            break;
                        }
                        Err(e) => {
                            warn!("get transaction error {}", e);
                            if let Some(payload) = p_block.block().payload() {
                                self.payload_manager.prefetch_payload_data(
                                    payload,
                                    p_block.block().timestamp_usecs(),
                                );
                            }
                        }
                    }
                }
                info!("recover block {}, txn_size: {}", p_block.block(), txns.len());
                let verified_txns: Vec<VerifiedTxn> = txns.iter().map(|txn| txn.into()).collect();
                let txn_num = verified_txns.len() as u64;
                let verified_txns = verified_txns.into_iter().map(|txn| txn.into()).collect();
                let block_number = p_block.block().block_number().unwrap();
                CUR_RECOVER_BLOCK_NUMBER_GAUGE.with_label_values(&[]).set(block_number.try_into().unwrap());
                let maybe_block_hash = match self
                    .storage
                    .consensus_db()
                    .ledger_db
                    .metadata_db()
                    .get_block_hash(block_number)
                {
                    Some(block_hash) => Some(ComputeRes::new(*block_hash, txn_num, vec![], vec![])),
                    None => None,
                };
                let block = ExternalBlock {
                    txns: verified_txns,
                    block_meta: ExternalBlockMeta {
                        block_id: BlockId(*p_block.block().id()),
                        block_number,
                        usecs: p_block.block().timestamp_usecs(),
                        epoch: p_block.block().epoch(),
                        randomness: p_block
                            .randomness()
                            .map(|r| Random::from_bytes(r.randomness())),
                        block_hash: maybe_block_hash.clone(),
                    },
                };
                get_block_buffer_manager()
                    .set_ordered_blocks(BlockId(*p_block.parent_id()), block)
                    .await
                    .unwrap();
                let compute_res = get_block_buffer_manager().get_executed_res(
                    BlockId(*p_block.id()),
                    p_block.block().block_number().unwrap(),
                ).await.unwrap();
                let compute_res = compute_res.execution_output;
                if let Some(block_hash) = maybe_block_hash {
                    assert_eq!(block_hash.data, compute_res.data);
                }
                let commit_block = BlockHashRef {
                    block_id: BlockId(*p_block.id()),
                    num: p_block.block().block_number().unwrap(),
                    hash: Some(compute_res.data),
                    persist_notifier: None,
                };
                get_block_buffer_manager().set_commit_blocks(vec![commit_block]).await.unwrap();
            }
            let commit_decision = finality_proof.ledger_info().clone();
            block_tree.write().commit_callback(
                storage,
                &blocks_to_commit,
                finality_proof,
                commit_decision,
            );
        } else {
            info!("send the blocks to execution {:?}", blocks_to_commit);
            self.execution_client
                .finalize_order(
                    &blocks_to_commit,
                    finality_proof.ledger_info().clone(),
                    Box::new(
                        move |committed_blocks: &[Arc<PipelinedBlock>],
                              commit_decision: LedgerInfoWithSignatures| {
                            block_tree.write().commit_callback(
                                storage,
                                committed_blocks,
                                finality_proof,
                                commit_decision,
                            );
                        },
                    ),
                )
                .await
                .expect("Failed to persist commit");
        }

        self.inner.write().update_ordered_root(block_to_commit.id());
        self.inner.write().insert_ordered_cert(finality_proof_clone.clone());
        update_counters_for_ordered_blocks(&blocks_to_commit);
        Ok(())
    }

    pub async fn rebuild(&self, root: RootInfo, blocks: Vec<Block>, quorum_certs: Vec<QuorumCert>) {
        info!(
            "Rebuilding block tree. root {:?}, blocks {:?}, qcs {:?}",
            root,
            blocks.iter().map(|b| b.id()).collect::<Vec<_>>(),
            quorum_certs.iter().map(|qc| qc.certified_block().id()).collect::<Vec<_>>()
        );
        let max_pruned_blocks_in_mem = self.inner.read().max_pruned_blocks_in_mem();
        // Rollover the previous highest TC from the old tree to the new one.
        let prev_2chain_htc = self.highest_2chain_timeout_cert().map(|tc| tc.as_ref().clone());
        let BlockStore { inner, .. } = Self::build(
            root,
            blocks,
            quorum_certs,
            prev_2chain_htc,
            self.execution_client.clone(),
            Arc::clone(&self.storage),
            max_pruned_blocks_in_mem,
            Arc::clone(&self.time_service),
            self.vote_back_pressure_limit,
            self.payload_manager.clone(),
            self.order_vote_enabled,
            self.pending_blocks.clone(),
        )
        .await;

        // Unwrap the new tree and replace the existing tree.
        *self.inner.write() = Arc::try_unwrap(inner)
            .unwrap_or_else(|_| panic!("New block tree is not shared"))
            .into_inner();
        self.recover_blocks().await;
    }

    /// Insert a block if it passes all validation tests.
    /// Returns the Arc to the block kept in the block store after persisting it to storage
    ///
    /// This function assumes that the ancestors are present (returns MissingParent otherwise).
    ///
    /// Duplicate inserts will return the previously inserted block (
    /// note that it is considered a valid non-error case, for example, it can happen if a validator
    /// receives a certificate for a block that is currently being added).
    pub async fn insert_block(
        &self,
        block: Block,
        rebuild: bool,
    ) -> anyhow::Result<Arc<PipelinedBlock>> {
        if let Some(existing_block) = self.get_block(block.id()) {
            return Ok(existing_block);
        }
        info!("insert block {}", block);
        if !rebuild {
            ensure!(
                self.inner.read().ordered_root().round() < block.round(),
                "Block with old round"
            );
        }

        let pipelined_block = PipelinedBlock::new_ordered(block.clone());
        // ensure local time past the block time
        let block_time = Duration::from_micros(pipelined_block.timestamp_usecs());
        let current_timestamp = self.time_service.get_current_timestamp();
        if let Some(t) = block_time.checked_sub(current_timestamp) {
            if t > Duration::from_secs(1) {
                warn!("Long wait time {}ms for block {}", t.as_millis(), pipelined_block.block());
            }
            self.time_service.wait_until(block_time).await;
        }
        if let Some(payload) = pipelined_block.block().payload() {
            self.payload_manager
                .prefetch_payload_data(payload, pipelined_block.block().timestamp_usecs());
        }
        if !rebuild {
            self.storage
                .save_tree(vec![pipelined_block.block().clone()], vec![], vec![])
                .context("Insert block failed when saving block")?;
        }
        self.inner.write().insert_block(pipelined_block)
    }

    /// Validates quorum certificates and inserts it into block tree assuming dependencies exist.
    pub fn insert_single_quorum_cert(&self, qc: QuorumCert, rebuild: bool) -> anyhow::Result<()> {
        info!("insert qc {}", qc);
        // If the parent block is not the root block (i.e not None), ensure the executed state
        // of a block is consistent with its QuorumCert, otherwise persist the QuorumCert's
        // state and on restart, a new execution will agree with it.  A new execution will match
        // the QuorumCert's state on the next restart will work if there is a memory
        // corruption, for example.
        match self.get_block(qc.certified_block().id()) {
            Some(pipelined_block) => {
                ensure!(
                    // decoupled execution allows dummy block infos
                    pipelined_block.block_info().match_ordered_only(qc.certified_block()),
                    "QC for block {} has different {:?} than local {:?}",
                    qc.certified_block().id(),
                    qc.certified_block(),
                    pipelined_block.block_info()
                );
                observe_block(pipelined_block.block().timestamp_usecs(), BlockStage::QC_ADDED);
            }
            None => bail!("Insert {} without having the block in store first", qc),
        };
        if !rebuild {
            self.storage
                .save_tree(vec![], vec![qc.clone()], vec![])
                .context("Insert block failed when saving quorum")?;
        }
        self.inner.write().insert_quorum_cert(qc)
    }

    /// Replace the highest 2chain timeout certificate in case the given one has a higher round.
    /// In case a timeout certificate is updated, persist it to storage.
    pub fn insert_2chain_timeout_certificate(
        &self,
        tc: Arc<TwoChainTimeoutCertificate>,
    ) -> anyhow::Result<()> {
        let cur_tc_round = self.highest_2chain_timeout_cert().map_or(0, |tc| tc.round());
        if tc.round() <= cur_tc_round {
            return Ok(());
        }
        self.storage
            .save_highest_2chain_timeout_cert(tc.as_ref())
            .context("Timeout certificate insert failed when persisting to DB")?;
        self.inner.write().replace_2chain_timeout_cert(tc);
        Ok(())
    }

    /// Prune the tree up to next_root_id (keep next_root_id's block).  Any branches not part of
    /// the next_root_id's tree should be removed as well.
    ///
    /// For example, root = B0
    /// B0--> B1--> B2
    ///        ╰--> B3--> B4
    ///
    /// prune_tree(B3) should be left with
    /// B3--> B4, root = B3
    ///
    /// Returns the block ids of the blocks removed.
    #[cfg(test)]
    fn prune_tree(&self, next_root_id: HashValue) -> VecDeque<(u64, HashValue)> {
        let id_to_remove = self.inner.read().find_blocks_to_prune(next_root_id);
        if let Err(e) = self.storage.prune_tree(id_to_remove.clone().into_iter().collect()) {
            // it's fine to fail here, as long as the commit succeeds, the next restart will clean
            // up dangling blocks, and we need to prune the tree to keep the root consistent with
            // executor.
            warn!(error = ?e, "fail to delete block");
        }
        // synchronously update both root_id and commit_root_id
        let mut wlock = self.inner.write();
        wlock.update_ordered_root(next_root_id);
        wlock.update_commit_and_finalized_root(next_root_id);
        wlock.process_pruned_blocks(id_to_remove.clone());
        id_to_remove
    }

    #[cfg(any(test, feature = "fuzzing"))]
    pub fn set_back_pressure_for_test(&self, back_pressure: bool) {
        self.back_pressure_for_test.store(back_pressure, Ordering::Relaxed)
    }

    pub fn pending_blocks(&self) -> Arc<Mutex<PendingBlocks>> {
        self.pending_blocks.clone()
    }

    pub async fn wait_for_payload(&self, block: &Block) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(1), self.payload_manager.get_transactions(block))
            .await??;
        Ok(())
    }

    pub fn check_payload(&self, proposal: &Block) -> bool {
        self.payload_manager.check_payload_availability(proposal)
    }
}

impl BlockReader for BlockStore {
    fn block_exists(&self, block_id: HashValue) -> bool {
        self.inner.read().block_exists(&block_id)
    }

    fn get_block(&self, block_id: HashValue) -> Option<Arc<PipelinedBlock>> {
        self.inner.read().get_block(&block_id)
    }

    fn ordered_root(&self) -> Arc<PipelinedBlock> {
        self.inner.read().ordered_root()
    }

    fn commit_root(&self) -> Arc<PipelinedBlock> {
        self.inner.read().commit_root()
    }

    fn get_quorum_cert_for_block(&self, block_id: HashValue) -> Option<Arc<QuorumCert>> {
        self.inner.read().get_quorum_cert_for_block(&block_id)
    }

    fn path_from_ordered_root(&self, block_id: HashValue) -> Option<Vec<Arc<PipelinedBlock>>> {
        self.inner.read().path_from_ordered_root(block_id)
    }

    fn path_from_commit_root(&self, block_id: HashValue) -> Option<Vec<Arc<PipelinedBlock>>> {
        self.inner.read().path_from_commit_root(block_id)
    }

    #[cfg(test)]
    fn highest_certified_block(&self) -> Arc<PipelinedBlock> {
        self.inner.read().highest_certified_block()
    }

    fn highest_quorum_cert(&self) -> Arc<QuorumCert> {
        self.inner.read().highest_quorum_cert()
    }

    fn highest_ordered_cert(&self) -> Arc<WrappedLedgerInfo> {
        self.inner.read().highest_ordered_cert()
    }

    fn highest_commit_cert(&self) -> Arc<WrappedLedgerInfo> {
        self.inner.read().highest_commit_cert()
    }

    fn highest_2chain_timeout_cert(&self) -> Option<Arc<TwoChainTimeoutCertificate>> {
        self.inner.read().highest_2chain_timeout_cert()
    }

    fn sync_info(&self) -> SyncInfo {
        SyncInfo::new_decoupled(
            self.highest_quorum_cert().as_ref().clone(),
            self.highest_ordered_cert().as_ref().clone(),
            self.highest_commit_cert().as_ref().clone(),
            self.highest_2chain_timeout_cert().map(|tc| tc.as_ref().clone()),
        )
    }

    /// Return if the consensus is backpressured
    fn vote_back_pressure(&self) -> bool {
        #[cfg(any(test, feature = "fuzzing"))]
        {
            if self.back_pressure_for_test.load(Ordering::Relaxed) {
                return true;
            }
        }
        let commit_round = self.commit_root().round();
        let ordered_round = self.ordered_root().round();
        counters::OP_COUNTERS.gauge("back_pressure").set((ordered_round - commit_round) as i64);
        ordered_round > self.vote_back_pressure_limit + commit_round
    }

    fn pipeline_pending_latency(&self, proposal_timestamp: Duration) -> Duration {
        let ordered_root = self.ordered_root();
        let commit_root = self.commit_root();
        let pending_path = self.path_from_commit_root(self.ordered_root().id()).unwrap_or_default();
        let pending_rounds = pending_path.len();
        let oldest_not_committed = pending_path.into_iter().min_by_key(|b| b.round());

        let oldest_not_committed_spent_in_pipeline = oldest_not_committed
            .as_ref()
            .and_then(|b| b.elapsed_in_pipeline())
            .unwrap_or(Duration::ZERO);

        let ordered_round = ordered_root.round();
        let oldest_not_committed_round = oldest_not_committed.as_ref().map_or(0, |b| b.round());
        let commit_round = commit_root.round();
        let ordered_timestamp = Duration::from_micros(ordered_root.timestamp_usecs());
        let oldest_not_committed_timestamp = oldest_not_committed
            .as_ref()
            .map(|b| Duration::from_micros(b.timestamp_usecs()))
            .unwrap_or(Duration::ZERO);
        let committed_timestamp = Duration::from_micros(commit_root.timestamp_usecs());
        let commit_cert_timestamp =
            Duration::from_micros(self.highest_commit_cert().commit_info().timestamp_usecs());

        fn latency_from_proposal(proposal_timestamp: Duration, timestamp: Duration) -> Duration {
            if timestamp.is_zero() {
                // latency not known without non-genesis blocks
                Duration::ZERO
            } else {
                proposal_timestamp.saturating_sub(timestamp)
            }
        }

        let latency_to_committed = latency_from_proposal(proposal_timestamp, committed_timestamp);
        let latency_to_oldest_not_committed =
            latency_from_proposal(proposal_timestamp, oldest_not_committed_timestamp);
        let latency_to_ordered = latency_from_proposal(proposal_timestamp, ordered_timestamp);

        info!(
            pending_rounds = pending_rounds,
            ordered_round = ordered_round,
            oldest_not_committed_round = oldest_not_committed_round,
            commit_round = commit_round,
            oldest_not_committed_spent_in_pipeline =
                oldest_not_committed_spent_in_pipeline.as_millis() as u64,
            latency_to_ordered_ms = latency_to_ordered.as_millis() as u64,
            latency_to_oldest_not_committed = latency_to_oldest_not_committed.as_millis() as u64,
            latency_to_committed_ms = latency_to_committed.as_millis() as u64,
            latency_to_commit_cert_ms =
                latency_from_proposal(proposal_timestamp, commit_cert_timestamp).as_millis() as u64,
            "Pipeline pending latency on proposal creation",
        );

        counters::CONSENSUS_PROPOSAL_PENDING_ROUNDS.observe(pending_rounds as f64);
        counters::CONSENSUS_PROPOSAL_PENDING_DURATION
            .observe_duration(oldest_not_committed_spent_in_pipeline);

        if pending_rounds > 1 {
            // TODO cleanup
            // previous logic was using difference between committed and ordered.
            // keeping it until we test out the new logic.
            // latency_to_oldest_not_committed
            //     .saturating_sub(latency_to_ordered.min(MAX_ORDERING_PIPELINE_LATENCY_REDUCTION))

            oldest_not_committed_spent_in_pipeline
        } else {
            Duration::ZERO
        }
    }

    fn get_recent_block_execution_times(&self, num_blocks: usize) -> Vec<ExecutionSummary> {
        let mut res = vec![];
        let mut cur_block = Some(self.ordered_root());
        loop {
            match cur_block {
                Some(block) => {
                    if let Some(execution_time_and_size) = block.get_execution_summary() {
                        info!(
                            "Found execution time for {}, {:?}",
                            block.id(),
                            execution_time_and_size
                        );
                        res.push(execution_time_and_size);
                        if res.len() >= num_blocks {
                            return res;
                        }
                    } else {
                        info!("Couldn't find execution time for {}", block.id());
                    }
                    cur_block = self.get_block(block.parent_id());
                }
                None => return res,
            }
        }
    }
}

#[cfg(any(test, feature = "fuzzing"))]
impl BlockStore {
    /// Returns the number of blocks in the tree
    pub(crate) fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns the number of child links in the tree
    pub(crate) fn child_links(&self) -> usize {
        self.inner.read().child_links()
    }

    /// The number of pruned blocks that are still available in memory
    pub(super) fn pruned_blocks_in_mem(&self) -> usize {
        self.inner.read().pruned_blocks_in_mem()
    }

    /// Helper function to insert the block with the qc together
    pub async fn insert_block_with_qc(&self, block: Block) -> anyhow::Result<Arc<PipelinedBlock>> {
        self.insert_single_quorum_cert(block.quorum_cert().clone(), false)?;
        if self.ordered_root().round() < block.quorum_cert().commit_info().round() {
            self.send_for_execution(block.quorum_cert().into_wrapped_ledger_info(), false).await?;
        }
        self.insert_block(block, false).await
    }
}
