// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    counters,
    counters::update_counters_for_committed_blocks,
    logging::{LogEvent, LogSchema},
    persistent_liveness_storage::PersistentLivenessStorage,
};
use anyhow::bail;
use aptos_consensus_types::common::Payload;
use aptos_consensus_types::{
    pipelined_block::PipelinedBlock, quorum_cert::QuorumCert,
    timeout_2chain::TwoChainTimeoutCertificate, wrapped_ledger_info::WrappedLedgerInfo,
};
use aptos_crypto::{hash::GENESIS_BLOCK_ID, HashValue};
use aptos_logger::prelude::*;
use aptos_types::{block_info::BlockInfo, ledger_info::LedgerInfoWithSignatures};
use futures::executor::block_on;
use mirai_annotations::{checked_verify_eq, precondition};
use std::{
    collections::{vec_deque::VecDeque, BTreeMap, HashMap, HashSet},
    fmt::{self, Display},
    sync::Arc,
};

/// This structure is a wrapper of [`ExecutedBlock`](aptos_consensus_types::pipelined_block::PipelinedBlock)
/// that adds `children` field to know the parent-child relationship between blocks.
struct LinkableBlock {
    /// Executed block that has raw block data and execution output.
    executed_block: Arc<PipelinedBlock>,
    /// The set of children for cascading pruning. Note: a block may have multiple children.
    children: HashSet<HashValue>,
}

impl LinkableBlock {
    pub fn new(block: PipelinedBlock) -> Self {
        Self { executed_block: Arc::new(block), children: HashSet::new() }
    }

    pub fn executed_block(&self) -> &Arc<PipelinedBlock> {
        &self.executed_block
    }

    pub fn children(&self) -> &HashSet<HashValue> {
        &self.children
    }

    pub fn add_child(&mut self, child_id: HashValue) {
        assert!(self.children.insert(child_id), "Block {:x} already existed.", child_id,);
    }
}

impl LinkableBlock {
    pub fn id(&self) -> HashValue {
        self.executed_block().id()
    }
}

/// This structure maintains a consistent block tree of parent and children links. Blocks contain
/// parent links and are immutable.  For all parent links, a child link exists. This structure
/// should only be used internally in BlockStore.
pub struct BlockTree {
    /// All the blocks known to this replica (with parent links)
    id_to_block: HashMap<HashValue, LinkableBlock>,
    block_number_to_id: BTreeMap<u64, HashValue>,
    /// Root of the tree. This is the root of ordering phase
    ordered_root_id: HashValue,
    /// Commit Root id: this is the root of commit phase
    commit_root_id: HashValue,
    /// A certified block id with highest round
    highest_certified_block_id: HashValue,
    // used in reth
    head_block_id: HashValue,
    safe_block_id: HashValue,
    finalized_block_id: HashValue,

    /// The quorum certificate of highest_certified_block
    highest_quorum_cert: Arc<QuorumCert>,
    /// The highest 2-chain timeout certificate (if any).
    highest_2chain_timeout_cert: Option<Arc<TwoChainTimeoutCertificate>>,
    /// The quorum certificate that has highest commit info.
    highest_ordered_cert: Arc<WrappedLedgerInfo>,
    /// The quorum certificate that has highest commit decision info.
    highest_commit_cert: Arc<WrappedLedgerInfo>,
    /// Map of block id to its completed quorum certificate (2f + 1 votes)
    id_to_quorum_cert: HashMap<HashValue, Arc<QuorumCert>>,
    /// To keep the IDs of the elements that have been pruned from the tree but not cleaned up yet.
    pruned_block_ids: VecDeque<(u64, HashValue)>,
    /// Num pruned blocks to keep in memory.
    max_pruned_blocks_in_mem: usize,
}

impl BlockTree {
    pub fn debug_string(&self) -> String {
        let mut result = String::new();
        result.push_str("BlockTree Information:\n");
        result.push_str(&format!("HighCommitQC:     {:?}\n", self.highest_commit_cert));
        result.push_str(&format!("HighOrderQC:      {:?}\n", self.highest_ordered_cert));
        result.push_str(&format!("HighQC:           {:?}\n", self.highest_quorum_cert));
        result.push_str(&format!("High2Chain:       {:?}\n", self.highest_2chain_timeout_cert));
        result.push_str(&format!("OrderRoot:        {:?}\n", self.ordered_root_id));
        result.push_str(&format!("CommitRoot:       {:?}\n", self.commit_root_id));
        result.push_str("\nTree Structure:\n");
        self.build_tree_string(&self.ordered_root_id, &mut result, "", true);
        result
    }

    pub fn block_size(&self) -> usize {
        self.id_to_block.len()
    }

    pub fn is_safe_block_payload_none(&self) -> bool {
        // Genesis block is a nil block.
        // And when the safe block is nil, it means it is genesis.
        self.get_block(&self.safe_block_id)
            .expect("Highest certified block must exist")
            .block()
            .payload()
            .is_none()
    }

    pub fn is_finalized_block_payload_none(&self) -> bool {
        // Genesis block is a nil block.
        // And when the finalized block is nil, it means it is genesis.
        self.get_block(&self.finalized_block_id)
            .expect("Highest certified block must exist")
            .block()
            .payload()
            .is_none()
    }

    pub fn is_head_block_payload_none(&self) -> bool {
        // Genesis block is a nil block.
        // And when the head block is nil, it means it is genesis.
        self.get_block(&self.head_block_id)
            .expect("Highest certified block must exist")
            .block()
            .payload()
            .is_none()
    }

    pub fn is_head_block_qc(&self) -> bool {
        info!("is_last_block_qc {:?}", self.id_to_quorum_cert.get(&self.head_block_id));
        self.id_to_quorum_cert.get(&self.head_block_id).is_some()
    }

    fn build_node_string(&self, block_id: &HashValue) -> String {
        let block = self
            .id_to_block
            .get(block_id)
            .expect(&format!("failed to get block for id {:?}", block_id));
        let transactions = block.executed_block.input_transactions();
        format!("input {:?}\n, payload is {:?}", transactions, block.executed_block.payload())
    }
    fn build_tree_string(
        &self,
        block_id: &HashValue,
        result: &mut String,
        prefix: &str,
        is_last: bool,
    ) {
        if let Some(block) = self.id_to_block.get(block_id) {
            let node_info = self.build_node_string(block_id);
            result.push_str(&format!(
                "{}{}── Block ID: {:?}\n",
                prefix,
                if is_last { "└" } else { "├" },
                block_id
            ));
            let transaction_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });
            for line in node_info.lines() {
                result.push_str(&format!("{}│  {}\n", transaction_prefix, line));
            }
            let new_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });
            let children: Vec<_> = block.children.iter().collect();
            for (i, child_id) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;
                self.build_tree_string(child_id, result, &new_prefix, is_last_child);
            }
        }
    }
}
impl Display for BlockTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.debug_string())
    }
}

impl BlockTree {
    pub(super) fn new(
        root: PipelinedBlock,
        root_quorum_cert: QuorumCert,
        root_ordered_cert: WrappedLedgerInfo,
        root_commit_cert: WrappedLedgerInfo,
        max_pruned_blocks_in_mem: usize,
        highest_2chain_timeout_cert: Option<Arc<TwoChainTimeoutCertificate>>,
    ) -> Self {
        assert_eq!(
            root.id(),
            root_ordered_cert.commit_info().id(),
            "inconsistent root and ledger info"
        );
        let root_id = root.id();
        info!("insert root block id {}", root_id);
        let mut id_to_block = HashMap::new();
        id_to_block.insert(root_id, LinkableBlock::new(root));
        counters::NUM_BLOCKS_IN_TREE.set(1);

        let root_quorum_cert = Arc::new(root_quorum_cert);
        let mut id_to_quorum_cert = HashMap::new();
        id_to_quorum_cert
            .insert(root_quorum_cert.certified_block().id(), Arc::clone(&root_quorum_cert));

        let pruned_block_ids = VecDeque::with_capacity(max_pruned_blocks_in_mem);

        BlockTree {
            id_to_block,
            block_number_to_id: BTreeMap::new(),
            ordered_root_id: root_id,
            commit_root_id: root_id, // initially we set commit_root_id = root_id
            highest_certified_block_id: root_id,
            highest_quorum_cert: Arc::clone(&root_quorum_cert),
            highest_ordered_cert: Arc::new(root_ordered_cert),
            highest_commit_cert: Arc::new(root_commit_cert),
            id_to_quorum_cert,
            pruned_block_ids,
            max_pruned_blocks_in_mem,
            highest_2chain_timeout_cert,
            head_block_id: root_id,
            safe_block_id: root_id,
            finalized_block_id: root_id,
        }
    }

    pub fn safe_block_hash(&self) -> HashValue {
        todo!()
    }

    // This method will only be used in this module.
    fn get_linkable_block(&self, block_id: &HashValue) -> Option<&LinkableBlock> {
        self.id_to_block.get(block_id)
    }

    // This method will only be used in this module.
    fn get_linkable_block_mut(&mut self, block_id: &HashValue) -> Option<&mut LinkableBlock> {
        self.id_to_block.get_mut(block_id)
    }

    /// fetch all the quorum certs with non-empty commit info
    pub fn get_all_quorum_certs_with_commit_info(&self) -> Vec<QuorumCert> {
        return self
            .id_to_quorum_cert
            .values()
            .filter(|qc| qc.commit_info() != &BlockInfo::empty())
            .map(|qc| (**qc).clone())
            .collect::<Vec<QuorumCert>>();
    }

    // This method will only be used in this module.
    // This method is used in pruning and length query,
    // to reflect the actual root, we use commit root
    fn linkable_root(&self) -> &LinkableBlock {
        self.get_linkable_block(&self.commit_root_id).expect("Root must exist")
    }

    fn remove_block(&mut self, block_number: u64, block_id: HashValue) {
        // Remove the block from the store
        self.id_to_block.remove(&block_id);
        self.id_to_quorum_cert.remove(&block_id);
        self.block_number_to_id.remove(&block_number);
    }

    pub(super) fn block_exists(&self, block_id: &HashValue) -> bool {
        self.id_to_block.contains_key(block_id)
    }

    pub(super) fn get_block(&self, block_id: &HashValue) -> Option<Arc<PipelinedBlock>> {
        self.get_linkable_block(block_id).map(|lb| lb.executed_block().clone())
    }

    pub(super) fn ordered_root(&self) -> Arc<PipelinedBlock> {
        self.get_block(&self.ordered_root_id).expect("Root must exist")
    }

    pub(super) fn commit_root(&self) -> Arc<PipelinedBlock> {
        self.get_block(&self.commit_root_id).expect("Commit root must exist")
    }

    pub(super) fn highest_certified_block(&self) -> Arc<PipelinedBlock> {
        self.get_block(&self.highest_certified_block_id)
            .expect("Highest cerfified block must exist")
    }

    pub(super) fn highest_quorum_cert(&self) -> Arc<QuorumCert> {
        Arc::clone(&self.highest_quorum_cert)
    }

    pub(super) fn highest_2chain_timeout_cert(&self) -> Option<Arc<TwoChainTimeoutCertificate>> {
        self.highest_2chain_timeout_cert.clone()
    }

    /// Replace highest timeout cert with the given value.
    pub(super) fn replace_2chain_timeout_cert(&mut self, tc: Arc<TwoChainTimeoutCertificate>) {
        self.highest_2chain_timeout_cert.replace(tc);
    }

    pub(super) fn highest_ordered_cert(&self) -> Arc<WrappedLedgerInfo> {
        Arc::clone(&self.highest_ordered_cert)
    }

    pub(super) fn highest_commit_cert(&self) -> Arc<WrappedLedgerInfo> {
        Arc::clone(&self.highest_commit_cert)
    }

    pub(super) fn get_quorum_cert_for_block(
        &self,
        block_id: &HashValue,
    ) -> Option<Arc<QuorumCert>> {
        self.id_to_quorum_cert.get(block_id).cloned()
    }

    pub(super) fn insert_block(
        &mut self,
        block: PipelinedBlock,
    ) -> anyhow::Result<Arc<PipelinedBlock>> {
        let block_id = block.id();
        if !block.block().is_nil_block() {
            self.head_block_id = block_id;
        }
        if let Some(existing_block) = self.get_block(&block_id) {
            debug!("Already had block {:?} for id {:?} when trying to add another block {:?} for the same id",
                       existing_block,
                       block_id,
                       block);
            checked_verify_eq!(existing_block.compute_result(), block.compute_result());
            Ok(existing_block)
        } else {
            match self.get_linkable_block_mut(&block.parent_id()) {
                Some(parent_block) => parent_block.add_child(block_id),
                None => {
                    if block.parent_id() != *GENESIS_BLOCK_ID {
                        bail!("Parent block {} not found", block.parent_id())
                    }
                }
            };
            let linkable_block = LinkableBlock::new(block);
            let arc_block = Arc::clone(linkable_block.executed_block());
            assert!(self.id_to_block.insert(block_id, linkable_block).is_none());
            counters::NUM_BLOCKS_IN_TREE.inc();
            Ok(arc_block)
        }
    }

    fn update_highest_commit_cert(&mut self, new_commit_cert: WrappedLedgerInfo) {
        if new_commit_cert.commit_info().round() > self.highest_commit_cert.commit_info().round() {
            self.highest_commit_cert = Arc::new(new_commit_cert);
            self.update_commit_and_finalized_root(self.highest_commit_cert.commit_info().id());
        }
    }

    pub(super) fn insert_quorum_cert(&mut self, qc: QuorumCert) -> anyhow::Result<()> {
        let block_id = qc.certified_block().id();
        let qc = Arc::new(qc);

        // Safety invariant: For any two quorum certificates qc1, qc2 in the block store,
        // qc1 == qc2 || qc1.round != qc2.round
        // The invariant is quadratic but can be maintained in linear time by the check
        // below.
        precondition!({
            let qc_round = qc.certified_block().round();
            self.id_to_quorum_cert.values().all(|x| {
                (*(*x).ledger_info()).ledger_info().consensus_data_hash()
                    == (*(*qc).ledger_info()).ledger_info().consensus_data_hash()
                    || x.certified_block().round() != qc_round
            })
        });

        match self.get_block(&block_id) {
            Some(block) => {
                if block.round() > self.highest_certified_block().round() {
                    self.highest_certified_block_id = block.id();
                    if !block.block().is_nil_block() {
                        self.safe_block_id = block.id();
                    }
                    self.highest_quorum_cert = Arc::clone(&qc);
                }
            }
            None => bail!("Block {} not found", block_id),
        }

        self.id_to_quorum_cert.entry(block_id).or_insert_with(|| Arc::clone(&qc));

        if self.highest_ordered_cert.commit_info().round() < qc.commit_info().round() {
            // Question: We are updating highest_ordered_cert but not highest_ordered_root. Is that fine?
            self.highest_ordered_cert = Arc::new(qc.into_wrapped_ledger_info());
        }

        Ok(())
    }

    pub fn insert_ordered_cert(&mut self, ordered_cert: WrappedLedgerInfo) {
        if ordered_cert.commit_info().round() > self.highest_ordered_cert.commit_info().round() {
            self.highest_ordered_cert = Arc::new(ordered_cert);
        }
    }

    pub(super) fn find_blocks_to_prune(&self, prune_block_number: u64) -> VecDeque<(u64, HashValue)> {
        let mut blocks_pruned = VecDeque::new();
        for (block_number, block_id) in &self.block_number_to_id {
            if *block_number < prune_block_number {
                blocks_pruned.push_back((*block_number, *block_id));
            }
        }
        blocks_pruned
    }

    pub(super) fn update_ordered_root(&mut self, root_id: HashValue) {
        assert!(self.block_exists(&root_id));
        self.ordered_root_id = root_id;
    }

    pub(super) fn update_commit_and_finalized_root(&mut self, root_id: HashValue) {
        assert!(self.block_exists(&root_id));
        let block = self.get_block(&root_id).expect("Block must exist");
        if !block.block().is_nil_block() {
            self.finalized_block_id = root_id;
        }
        self.commit_root_id = root_id;
    }

    /// Process the data returned by the prune_tree, they're separated because caller might
    /// be interested in doing extra work e.g. delete from persistent storage.
    /// Note that we do not necessarily remove the pruned blocks: they're kept in a separate buffer
    /// for some time in order to enable other peers to retrieve the blocks even after they've
    /// been committed.
    pub(super) fn process_pruned_blocks(&mut self, mut newly_pruned_blocks: VecDeque<(u64, HashValue)>) {
        counters::NUM_BLOCKS_IN_TREE.sub(newly_pruned_blocks.len() as i64);
        // The newly pruned blocks are pushed back to the deque pruned_block_ids.
        // In case the overall number of the elements is greater than the predefined threshold,
        // the oldest elements (in the front of the deque) are removed from the tree.
        self.pruned_block_ids.append(&mut newly_pruned_blocks);
        if self.pruned_block_ids.len() > self.max_pruned_blocks_in_mem {
            let num_blocks_to_remove = self.pruned_block_ids.len() - self.max_pruned_blocks_in_mem;
            for _ in 0..num_blocks_to_remove {
                if let Some((number, id)) = self.pruned_block_ids.pop_front() {
                    self.remove_block(number, id);
                }
            }
        }
    }

    /// Returns all the blocks between the commit root and the given block, including the given block
    /// but excluding the root.
    /// In case a given block is not the successor of the root, return None.
    /// While generally the provided blocks should always belong to the active tree, there might be
    /// a race, in which the root of the tree is propagated forward between retrieving the block
    /// and getting its path from root (e.g., at proposal generator). Hence, we don't want to panic
    /// and prefer to return None instead.
    pub(super) fn path_from_root_to_block(
        &self,
        block_id: HashValue,
        root_id: HashValue,
        root_round: u64,
    ) -> Option<Vec<Arc<PipelinedBlock>>> {
        let mut res = vec![];
        let mut cur_block_id = block_id;
        loop {
            match self.get_block(&cur_block_id) {
                Some(ref block) if block.round() <= root_round => {
                    break;
                }
                Some(block) => {
                    cur_block_id = block.parent_id();
                    res.push(block);
                }
                None => return None,
            }
        }
        // At this point cur_block.round() <= self.root.round()
        if cur_block_id != root_id {
            return None;
        }
        // Called `.reverse()` to get the chronically increased order.
        res.reverse();
        Some(res)
    }

    pub(super) fn path_from_ordered_root(
        &self,
        block_id: HashValue,
    ) -> Option<Vec<Arc<PipelinedBlock>>> {
        self.path_from_root_to_block(block_id, self.ordered_root_id, self.ordered_root().round())
    }

    pub(super) fn path_from_commit_root(
        &self,
        block_id: HashValue,
    ) -> Option<Vec<Arc<PipelinedBlock>>> {
        self.path_from_root_to_block(block_id, self.commit_root_id, self.commit_root().round())
    }

    fn get_block_reth_hash(&self, block_id: HashValue) -> HashValue {
        let block = self.id_to_block.get(&block_id);
        match block {
            Some(hash) => match hash.executed_block.payload().expect(" payload not found") {
                Payload::DirectMempool((id, _txns)) => id.clone(),
                _ => {
                    unreachable!("unexpected payload type")
                }
            },
            None => panic!("block not found"),
        }
    }

    pub fn get_safe_block_hash(&self) -> HashValue {
        self.get_block_reth_hash(self.safe_block_id)
    }

    pub fn get_head_block_hash(&self) -> HashValue {
        self.get_block_reth_hash(self.head_block_id)
    }

    pub fn get_head_block(&self) -> Arc<PipelinedBlock> {
        self.get_block(&self.head_block_id).expect("Head block must exist")
    }

    pub fn get_finalized_block_hash(&self) -> HashValue {
        self.get_block_reth_hash(self.finalized_block_id)
    }

    pub fn insert_block_number(&mut self, block_number: u64, block_id: HashValue) {
        self.block_number_to_id.insert(block_number, block_id);
    }

    pub(super) fn max_pruned_blocks_in_mem(&self) -> usize {
        self.max_pruned_blocks_in_mem
    }

    /// Update the counters for committed blocks and prune them from the in-memory and persisted store.
    pub fn commit_callback(
        &mut self,
        storage: Arc<dyn PersistentLivenessStorage>,
        blocks_to_commit: &[Arc<PipelinedBlock>],
        finality_proof: WrappedLedgerInfo,
        commit_decision: LedgerInfoWithSignatures,
        prune_block_number: u64,
    ) {
        let commit_proof = finality_proof
            .create_merged_with_executed_state(commit_decision)
            .expect("Inconsistent commit proof and evaluation decision, cannot commit block");
        let block_to_commit = blocks_to_commit.last().expect("pipeline is empty").clone();
        update_counters_for_committed_blocks(blocks_to_commit);
        let current_round = self.commit_root().round();
        let committed_round = block_to_commit.round();
        debug!(
            LogSchema::new(LogEvent::CommitViaBlock).round(current_round),
            committed_round = committed_round,
            block_id = block_to_commit.id(),
        );
        // TODO(gravity_lightman)
        info!("the prune block block number {}", prune_block_number);
        let ids_to_remove = self.find_blocks_to_prune(prune_block_number);
        if let Err(e) = storage.prune_tree(ids_to_remove.clone().into_iter().map(
            |(_, id)| id
        ).collect()) {
            // it's fine to fail here, as long as the commit succeeds, the next restart will clean
            // up dangling blocks, and we need to prune the tree to keep the root consistent with
            // executor.
            warn!(error = ?e, "fail to delete block");
        }
        self.process_pruned_blocks(ids_to_remove);

        self.update_highest_commit_cert(commit_proof);
    }
}

#[cfg(any(test, feature = "fuzzing"))]
impl BlockTree {
    /// Returns the number of blocks in the tree
    pub(super) fn len(&self) -> usize {
        // BFS over the tree to find the number of blocks in the tree.
        let mut res = 0;
        let mut to_visit = vec![self.linkable_root()];
        while let Some(block) = to_visit.pop() {
            res += 1;
            for child_id in block.children() {
                to_visit
                    .push(self.get_linkable_block(child_id).expect("Child must exist in the tree"));
            }
        }
        res
    }

    /// Returns the number of child links in the tree
    pub(super) fn child_links(&self) -> usize {
        self.len() - 1
    }

    /// The number of pruned blocks that are still available in memory
    pub(super) fn pruned_blocks_in_mem(&self) -> usize {
        self.pruned_block_ids.len()
    }
}
