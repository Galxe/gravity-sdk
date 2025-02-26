// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{consensusdb::ConsensusDB, epoch_manager::LivenessStorageData, error::DbError};
use anyhow::{format_err, Result};
use api_types::{ExecutionArgs, RecoveryApi};
use aptos_consensus_types::{
    block::Block, quorum_cert::QuorumCert, timeout_2chain::TwoChainTimeoutCertificate, vote::Vote,
    vote_data::VoteData, wrapped_ledger_info::WrappedLedgerInfo,
};
use aptos_crypto::{hash::{ACCUMULATOR_PLACEHOLDER_HASH, GENESIS_BLOCK_ID}, HashValue};
use aptos_logger::prelude::*;
use aptos_storage_interface::DbReader;
use aptos_types::{
    block_info::{BlockInfo, Round}, epoch_change::EpochChangeProof, ledger_info::LedgerInfoWithSignatures,
    on_chain_config::ValidatorSet, proof::TransactionAccumulatorSummary, transaction::Version,
};
use async_trait::async_trait;
use itertools::Itertools;
use std::{
    cmp::max,
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

/// PersistentLivenessStorage is essential for maintaining liveness when a node crashes.  Specifically,
/// upon a restart, a correct node will recover.  Even if all nodes crash, liveness is
/// guaranteed.
/// Blocks persisted are proposed but not yet committed.  The committed state is persisted
/// via StateComputer.
#[async_trait]
pub trait PersistentLivenessStorage: Send + Sync {
    /// Persist the blocks and quorum certs into storage atomically.
    fn save_tree(&self, blocks: Vec<Block>, quorum_certs: Vec<QuorumCert>) -> Result<()>;

    /// Delete the corresponding blocks and quorum certs atomically.
    fn prune_tree(&self, block_ids: Vec<HashValue>) -> Result<()>;

    /// Persist consensus' state
    fn save_vote(&self, vote: &Vote) -> Result<()>;

    /// Construct data that can be recovered from ledger
    fn recover_from_ledger(&self) -> LedgerRecoveryData;

    /// Construct necessary data to start consensus.
    async fn start(&self, order_vote_enabled: bool) -> LivenessStorageData;

    /// Persist the highest 2chain timeout certificate for improved liveness - proof for other replicas
    /// to jump to this round
    fn save_highest_2chain_timeout_cert(
        &self,
        highest_timeout_cert: &TwoChainTimeoutCertificate,
    ) -> Result<()>;

    /// Retrieve a epoch change proof for SafetyRules so it can instantiate its
    /// ValidatorVerifier.
    fn retrieve_epoch_change_proof(&self, version: u64) -> Result<EpochChangeProof>;

    /// Returns a handle of the aptosdb.
    fn aptos_db(&self) -> Arc<dyn DbReader>;

    // Returns a handle of the consensus db
    fn consensus_db(&self) -> Arc<ConsensusDB>;

    fn fetch_next_block_number(&self) -> u64;

    async fn latest_block_number(&self) -> u64;
}

#[derive(Clone)]
pub struct RootInfo(pub Box<Block>, pub QuorumCert, pub WrappedLedgerInfo, pub WrappedLedgerInfo);

impl Debug for RootInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RootInfo: [block: {}, quorum_cert: {}, ordered_cert: {}, commit_cert: {}]",
            self.0, self.1, self.2, self.3
        )
    }
}

/// LedgerRecoveryData is a subset of RecoveryData that we can get solely from ledger info.
#[derive(Clone)]
pub struct LedgerRecoveryData {
    storage_ledger: LedgerInfoWithSignatures,
}

impl LedgerRecoveryData {
    pub fn new(storage_ledger: LedgerInfoWithSignatures) -> Self {
        LedgerRecoveryData { storage_ledger }
    }

    pub fn committed_round(&self) -> Round {
        self.storage_ledger.commit_info().round()
    }

    /// Finds the root (last committed block) and returns the root block, the QC to the root block
    /// and the ledger info for the root block, return an error if it can not be found.
    ///
    /// We guarantee that the block corresponding to the storage's latest ledger info always exists.
    pub fn find_root(
        &self,
        blocks: &mut Vec<Block>,
        quorum_certs: &mut Vec<QuorumCert>,
        order_vote_enabled: bool,
    ) -> Result<RootInfo> {
        info!("The last committed block id as recorded in storage: {}", self.storage_ledger);

        // We start from the block that storage's latest ledger info, if storage has end-epoch
        // LedgerInfo, we generate the virtual genesis block
        let (root_id, latest_ledger_info_sig) = if self.storage_ledger.ledger_info().ends_epoch() {
            let genesis =
                Block::make_genesis_block_from_ledger_info(self.storage_ledger.ledger_info());
            let genesis_qc = QuorumCert::certificate_for_genesis_from_ledger_info(
                self.storage_ledger.ledger_info(),
                genesis.id(),
            );
            let genesis_ledger_info = genesis_qc.ledger_info().clone();
            let genesis_id = genesis.id();
            blocks.push(genesis);
            quorum_certs.push(genesis_qc);
            (genesis_id, genesis_ledger_info)
        } else {
            (self.storage_ledger.ledger_info().consensus_block_id(), self.storage_ledger.clone())
        };

        // sort by (epoch, round) to guarantee the topological order of parent <- child
        blocks.sort_by_key(|b| (b.epoch(), b.round()));

        let root_idx = blocks
            .iter()
            .position(|block| block.id() == root_id)
            .ok_or_else(|| format_err!("unable to find root: {}", root_id))?;
        let root_block = blocks.remove(root_idx);
        let root_quorum_cert = quorum_certs
            .iter()
            .find(|qc| qc.certified_block().id() == root_block.id())
            .ok_or_else(|| format_err!("No QC found for root: {}", root_id))?
            .clone();

        let (root_ordered_cert, root_commit_cert) = if order_vote_enabled {
            // We are setting ordered_root same as commit_root. As every committed block is also ordered, this is fine.
            // As the block store inserts all the fetched blocks and quorum certs and execute the blocks, the block store
            // updates highest_ordered_cert accordingly.
            let root_ordered_cert =
                WrappedLedgerInfo::new(VoteData::dummy(), latest_ledger_info_sig.clone());
            (root_ordered_cert.clone(), root_ordered_cert)
        } else {
            let root_ordered_cert = quorum_certs
                .iter()
                .find(|qc| qc.commit_info().id() == root_block.id())
                .ok_or_else(|| format_err!("No LI found for root: {}", root_id))?
                .clone()
                .into_wrapped_ledger_info();
            let root_commit_cert = root_ordered_cert
                .create_merged_with_executed_state(latest_ledger_info_sig)
                .expect("Inconsistent commit proof and evaluation decision, cannot commit block");
            (root_ordered_cert, root_commit_cert)
        };
        info!("Consensus root block is {}", root_block);

        Ok(RootInfo(Box::new(root_block), root_quorum_cert, root_ordered_cert, root_commit_cert))
    }
}

pub struct RootMetadata {
    pub accu_hash: HashValue,
    pub frozen_root_hashes: Vec<HashValue>,
    pub num_leaves: Version,
}

impl RootMetadata {
    pub fn version(&self) -> Version {
        max(self.num_leaves, 1) - 1
    }

    #[cfg(any(test, feature = "fuzzing"))]
    pub fn new_empty() -> Self {
        Self {
            accu_hash: *aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH,
            frozen_root_hashes: vec![],
            num_leaves: 0,
        }
    }
}

impl From<TransactionAccumulatorSummary> for RootMetadata {
    fn from(summary: TransactionAccumulatorSummary) -> Self {
        Self {
            accu_hash: summary.0.root_hash,
            frozen_root_hashes: summary.0.frozen_subtree_roots,
            num_leaves: summary.0.num_leaves,
        }
    }
}

/// The recovery data constructed from raw consensusdb data, it'll find the root value and
/// blocks that need cleanup or return error if the input data is inconsistent.
pub struct RecoveryData {
    // The last vote message sent by this validator.
    last_vote: Option<Vote>,
    root: RootInfo,
    // 1. the blocks guarantee the topological ordering - parent <- child.
    // 2. all blocks are children of the root.
    blocks: Vec<Block>,
    quorum_certs: Vec<QuorumCert>,
    blocks_to_prune: Option<Vec<HashValue>>,

    // Liveness data
    highest_2chain_timeout_certificate: Option<TwoChainTimeoutCertificate>,
}

impl RecoveryData {
    pub fn find_root_by_block_number(
        execution_latest_block_num: u64,
        blocks: &mut Vec<Block>,
        quorum_certs: &mut Vec<QuorumCert>,
        order_vote_enabled: bool,
    ) -> Result<RootInfo> {
        // sort by (epoch, round) to guarantee the topological order of parent <- child
        blocks.sort_by_key(|b| (b.epoch(), b.round()));
        quorum_certs.sort_by_key(|q| (q.certified_block().epoch(), q.certified_block().round()));
        let root_idx = blocks
            .iter()
            .position(|block| match block.block_number() {
                Some(block_number) => block_number == execution_latest_block_num,
                None => false,
            })
            .ok_or_else(|| {
                format_err!("unable to find block_number: {}", execution_latest_block_num)
            })?;
        let root_block = blocks.remove(root_idx);
        let root_quorum_cert = quorum_certs
            .iter()
            .find(|qc| qc.certified_block().id() == root_block.id())
            .ok_or_else(|| format_err!("No QC found for root: {}", root_block.id()))?
            .clone();
        let (root_ordered_cert, root_commit_cert) = if order_vote_enabled {
            // We are setting ordered_root same as commit_root. As every committed block is also ordered, this is fine.
            // As the block store inserts all the fetched blocks and quorum certs and execute the blocks, the block store
            // updates highest_ordered_cert accordingly.
            let root_ordered_cert =
                WrappedLedgerInfo::new(VoteData::dummy(), root_quorum_cert.ledger_info().clone());
            (root_ordered_cert.clone(), root_ordered_cert)
        } else {
            let root_ordered_cert = quorum_certs
                .iter()
                .find(|qc| qc.certified_block().round() > root_block.round() && !qc.commit_info().is_empty())
                .ok_or_else(|| format_err!("No LI found for root: {}", root_block.id()))?
                .clone()
                .into_wrapped_ledger_info();
            let root_commit_cert = root_ordered_cert
                .create_merged_with_executed_state(root_ordered_cert.ledger_info().clone())
                .expect("Inconsistent commit proof and evaluation decision, cannot commit block");
            (root_ordered_cert, root_commit_cert)
        };
        info!("Consensus root block is {}", root_block);
        Ok(RootInfo(Box::new(root_block), root_quorum_cert, root_ordered_cert, root_commit_cert))
    }

    pub fn new(
        last_vote: Option<Vote>,
        ledger_recovery_data: LedgerRecoveryData,
        execution_latest_block_num: u64,
        mut blocks: Vec<Block>,
        mut quorum_certs: Vec<QuorumCert>,
        highest_2chain_timeout_cert: Option<TwoChainTimeoutCertificate>,
        order_vote_enabled: bool,
    ) -> Result<Self> {
        info!("blocks in db: {:?}", blocks.len());
        info!("quorum certs in db: {:?}", quorum_certs.len());
        let root;
        if !blocks.is_empty() && execution_latest_block_num != 0 {
            root = Self::find_root_by_block_number(
                execution_latest_block_num,
                &mut blocks,
                &mut quorum_certs,
                order_vote_enabled,
            )?;
        } else {
            root = ledger_recovery_data.find_root(
                &mut blocks,
                &mut quorum_certs,
                order_vote_enabled,
            )?;
        }
        println!("root info: {:?}", root);
        let blocks_to_prune = Some(vec![]);
        let epoch = root.0.epoch();
        Ok(RecoveryData {
            last_vote: match last_vote {
                Some(v) if v.epoch() == epoch => Some(v),
                _ => None,
            },
            root,
            blocks,
            quorum_certs,
            blocks_to_prune,
            highest_2chain_timeout_certificate: match highest_2chain_timeout_cert {
                Some(tc) if tc.epoch() == epoch => Some(tc),
                _ => None,
            },
        })
    }

    pub fn root_block(&self) -> &Block {
        &self.root.0
    }

    pub fn last_vote(&self) -> Option<Vote> {
        self.last_vote.clone()
    }

    pub fn take(self) -> (RootInfo, Vec<Block>, Vec<QuorumCert>) {
        (self.root, self.blocks, self.quorum_certs)
    }

    pub fn take_blocks_to_prune(&mut self) -> Vec<HashValue> {
        self.blocks_to_prune.take().expect("blocks_to_prune already taken")
    }

    pub fn highest_2chain_timeout_certificate(&self) -> Option<TwoChainTimeoutCertificate> {
        self.highest_2chain_timeout_certificate.clone()
    }

    fn find_blocks_to_prune(
        root_id: HashValue,
        blocks: &mut Vec<Block>,
        quorum_certs: &mut Vec<QuorumCert>,
    ) -> Vec<HashValue> {
        // prune all the blocks that don't have root as ancestor
        let mut tree = HashSet::new();
        let mut to_remove = HashSet::new();
        tree.insert(root_id);
        // assume blocks are sorted by round already
        blocks.retain(|block| {
            if tree.contains(&block.parent_id()) {
                tree.insert(block.id());
                true
            } else {
                to_remove.insert(block.id());
                false
            }
        });
        quorum_certs.retain(|qc| {
            if tree.contains(&qc.certified_block().id()) {
                true
            } else {
                to_remove.insert(qc.certified_block().id());
                false
            }
        });
        to_remove.into_iter().collect()
    }
}

/// The proxy we use to persist data in db storage service via grpc.
pub struct StorageWriteProxy {
    db: Arc<ConsensusDB>,
    aptos_db: Arc<dyn DbReader>,
    next_block_number: AtomicU64,
    recovery_api: Option<Arc<dyn RecoveryApi>>,
}

impl StorageWriteProxy {
    pub fn new(
        db: Arc<ConsensusDB>,
        aptos_db: Arc<dyn DbReader>,
        recovery_api: Option<Arc<dyn RecoveryApi>>,
    ) -> Self {
        // let db = Arc::new(ConsensusDB::new(config.storage.dir()));
        StorageWriteProxy { db, aptos_db, next_block_number: AtomicU64::new(0), recovery_api }
    }

    pub fn init_next_block_number(&self, blocks: &Vec<Block>) {
        if blocks.len() == 0 {
            self.next_block_number.store(1, Ordering::SeqCst);
            return;
        };

        let mut max_block_number = 0;
        for block in blocks {
            if let Some(bn) = block.block_number() {
                max_block_number = max_block_number.max(bn);
            }
        }
        self.next_block_number.store(max_block_number + 1, Ordering::SeqCst);
    }

    async fn register_execution_args(&self, blocks: &Vec<Block>, latest_block_number: u64) {
        let mut block_number_to_block_id: BTreeMap<u64, HashValue> = blocks
            .iter()
            .filter(|block| {
                block.block_number().is_some()
                    && block.block_number().unwrap() <= latest_block_number
            })
            .map(|block| (block.block_number().unwrap(), block.id()))
            .sorted_by(|a, b| Ord::cmp(&b.0, &a.0))
            .take(256)
            .collect();
        if latest_block_number == 0 {
            block_number_to_block_id.insert(0u64, *GENESIS_BLOCK_ID);
        }
        let args = ExecutionArgs { block_number_to_block_id };
        self.recovery_api.as_ref().unwrap().register_execution_args(args).await;
    }
}

#[async_trait]
impl PersistentLivenessStorage for StorageWriteProxy {
    fn save_tree(&self, blocks: Vec<Block>, quorum_certs: Vec<QuorumCert>) -> Result<()> {
        Ok(self.db.save_blocks_and_quorum_certificates(blocks, quorum_certs)?)
    }

    fn prune_tree(&self, block_ids: Vec<HashValue>) -> Result<()> {
        panic!("Can't delete blocks");
        if !block_ids.is_empty() {
            // quorum certs that certified the block_ids will get removed
            self.db.delete_blocks_and_quorum_certificates(block_ids)?;
        }
        Ok(())
    }

    fn save_vote(&self, vote: &Vote) -> Result<()> {
        Ok(self.db.save_vote(bcs::to_bytes(vote)?)?)
    }

    fn recover_from_ledger(&self) -> LedgerRecoveryData {
        let latest_ledger_info =
            self.aptos_db.get_latest_ledger_info().expect("Failed to get latest ledger info.");
        LedgerRecoveryData::new(latest_ledger_info)
    }

    async fn start(&self, order_vote_enabled: bool) -> LivenessStorageData {
        info!("Start consensus recovery.");
        let raw_data = self.db.get_data().expect("unable to recover consensus data");

        let last_vote = raw_data
            .0
            .map(|bytes| bcs::from_bytes(&bytes[..]).expect("unable to deserialize last vote"));

        let highest_2chain_timeout_cert = raw_data.1.map(|b| {
            bcs::from_bytes(&b).expect("unable to deserialize highest 2-chain timeout cert")
        });
        let blocks = raw_data.2;
        self.init_next_block_number(&blocks);
        let quorum_certs: Vec<_> = raw_data.3;
        let blocks_repr: Vec<String> = blocks.iter().map(|b| format!("\n\t{}", b)).collect();
        info!("The following blocks were restored from ConsensusDB : {}", blocks_repr.concat());
        let qc_repr: Vec<String> = quorum_certs.iter().map(|qc| format!("\n\t{}", qc)).collect();
        info!("The following quorum certs were restored from ConsensusDB: {}", qc_repr.concat());
        let latest_block_number = self.latest_block_number().await;
        info!("The execution_latest_block_number is {}", latest_block_number);
        // only use when latest_block_number is zero
        let mut latest_ledger_info = LedgerInfoWithSignatures::genesis(
            *ACCUMULATOR_PLACEHOLDER_HASH,
            ValidatorSet::new(self.consensus_db().mock_validators()),
        );
        if latest_block_number != 0 {
            latest_ledger_info = self.aptos_db().get_latest_ledger_info().unwrap();
        }
        self.register_execution_args(&blocks, latest_block_number).await;
        let ledger_recovery_data = LedgerRecoveryData::new(latest_ledger_info);
        match RecoveryData::new(
            last_vote,
            ledger_recovery_data,
            latest_block_number,
            blocks,
            quorum_certs,
            highest_2chain_timeout_cert,
            order_vote_enabled,
        ) {
            Ok(initial_data) => {
                // TODO(gravity_lightman)
                // (self as &dyn PersistentLivenessStorage)
                //     .prune_tree(initial_data.take_blocks_to_prune())
                //     .expect("unable to prune dangling blocks during restart");
                if initial_data.last_vote.is_none() {
                    self.db.delete_last_vote_msg().expect("unable to cleanup last vote");
                }
                if initial_data.highest_2chain_timeout_certificate.is_none() {
                    self.db
                        .delete_highest_2chain_timeout_certificate()
                        .expect("unable to cleanup highest 2-chain timeout cert");
                }
                println!(
                    "Starting up the consensus state machine with recovery data - [root block {:?}] [last_vote {}], [highest timeout certificate: {}]",
                    initial_data.root_block(),
                    initial_data.last_vote.as_ref().map_or("None".to_string(), |v| v.to_string()),
                    initial_data.highest_2chain_timeout_certificate().as_ref().map_or("None".to_string(), |v| v.to_string()),
                );
                info!(
                    "Starting up the consensus state machine with recovery data - [last_vote {}], [highest timeout certificate: {}]",
                    initial_data.last_vote.as_ref().map_or("None".to_string(), |v| v.to_string()),
                    initial_data.highest_2chain_timeout_certificate().as_ref().map_or("None".to_string(), |v| v.to_string()),
                );

                LivenessStorageData::FullRecoveryData(initial_data)
            }
            Err(e) => {
                error!(error = ?e, "Failed to construct recovery data");
                panic!(""); // TODO(gravity_lightman)
                            // LivenessStorageData::PartialRecoveryData(ledger_recovery_data)
            }
        }
    }

    fn save_highest_2chain_timeout_cert(
        &self,
        highest_timeout_cert: &TwoChainTimeoutCertificate,
    ) -> Result<()> {
        Ok(self.db.save_highest_2chain_timeout_certificate(bcs::to_bytes(highest_timeout_cert)?)?)
    }

    fn retrieve_epoch_change_proof(&self, version: u64) -> Result<EpochChangeProof> {
        let (_, proofs) =
            self.aptos_db.get_state_proof(version).map_err(DbError::from)?.into_inner();
        Ok(proofs)
    }

    fn aptos_db(&self) -> Arc<dyn DbReader> {
        self.aptos_db.clone()
    }

    fn consensus_db(&self) -> Arc<ConsensusDB> {
        self.db.clone()
    }

    fn fetch_next_block_number(&self) -> u64 {
        let next_block_number = self.next_block_number.fetch_add(1, Ordering::SeqCst);
        next_block_number
    }

    async fn latest_block_number(&self) -> u64 {
        self.recovery_api.as_ref().unwrap().latest_block_number().await
    }
}
