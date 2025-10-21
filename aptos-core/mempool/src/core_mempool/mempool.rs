// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Mempool is used to track transactions which have been submitted but not yet
//! agreed upon.
use crate::{
    core_mempool::{
        index::TxnPointer,
        transaction::{InsertionInfo, MempoolTransaction, TimelineState},
        transaction_store::TransactionStore,
    },
    logging::{LogEntry, LogSchema, TxnsLog},
    network::BroadcastPeerPriority,
    shared_mempool::types::{
        MempoolSenderBucket, MultiBucketTimelineIndexIds, TimelineIndexIdentifier,
    },
};
use aptos_consensus_types::common::{TransactionInProgress, TransactionSummary};
use gaptos::aptos_config::config::NodeConfig;
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::prelude::*;
use gaptos::aptos_types::{
    account_address::AccountAddress,
    mempool_status::{MempoolStatus, MempoolStatusCode},
    transaction::{use_case::UseCaseKey, SignedTransaction},
    vm_status::DiscardedVMStatus,
};
use gaptos::{
    api_types::{
        account::ExternalAccountAddress, u256_define::TxnHash, VerifiedTxnWithAccountSeqNum,
    },
    aptos_mempool::shared_mempool::types::CoreMempoolTrait,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use super::transaction::VerifiedTxn;
use block_buffer_manager::TxPool;
use gaptos::aptos_mempool::counters;

pub struct TxnCache {
    old_cache: HashSet<TxnHash>,
    cache: HashSet<TxnHash>,
    size: usize
}

impl TxnCache {
    fn new(size: usize) -> Self {
        Self {
            old_cache: HashSet::new(),
            cache: HashSet::new(),
            size,
        }
    }

    fn insert(&mut self, txn_hash: TxnHash) {
        self.cache.insert(txn_hash);
        if self.cache.len() > self.size {
            self.old_cache = self.cache.clone();
            self.cache.clear();
        }
    }

    fn is_contains(&self, txn_hash: &TxnHash) -> bool {
        self.cache.contains(txn_hash)
    }

}

pub struct Mempool {
    // Stores the metadata of all transactions in mempool (of all states).
    pool: Box<dyn TxPool>,
    txn_cache: Arc<Mutex<TxnCache>>,
}


impl CoreMempoolTrait for Mempool {
    fn timeline_range(
        &self,
        sender_bucket: MempoolSenderBucket,
        start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        let visited = self.txn_cache.clone();
        let filter = Box::new(move |txn: (ExternalAccountAddress, u64, TxnHash)| {
            !visited.lock().unwrap().is_contains(&txn.2)
        });
        let iter = self.pool.get_broadcast_txns(Some(filter));
        let mut broacasted_txns = vec![];
        let mut visited_cache = self.txn_cache.lock().unwrap();
        for txn in iter {   
            visited_cache.insert(TxnHash::from_bytes(txn.committed_hash().as_slice()));
            broacasted_txns.push((
                VerifiedTxn::from(txn).into(),
                0,
            ));
        }
        broacasted_txns
    }

    fn timeline_range_of_message(
        &self,
        sender_start_end_pairs: HashMap<
            MempoolSenderBucket,
            HashMap<TimelineIndexIdentifier, (u64, u64)>,
        >,
    ) -> Vec<(SignedTransaction, u64)> {
        let visited = self.txn_cache.clone();
        let filter = Box::new(move |txn: (ExternalAccountAddress, u64, TxnHash)| {
            !visited.lock().unwrap().is_contains(&txn.2)
        });
        let iter = self.pool.get_broadcast_txns(Some(filter));
        let mut broacasted_txns = vec![];
        let mut visited_cache = self.txn_cache.lock().unwrap();
        for txn in iter {   
            visited_cache.insert(TxnHash::from_bytes(txn.committed_hash().as_slice()));
            broacasted_txns.push((
                VerifiedTxn::from(txn).into(),
                0,
            ));
        }
        broacasted_txns
    }

    fn get_parking_lot_addresses(&self) -> Vec<(AccountAddress, u64)> {
        // don't need to implement
        vec![]
    }

    fn read_timeline(
        &self,
        sender_bucket: MempoolSenderBucket,
        timeline_id: &MultiBucketTimelineIndexIds,
        count: usize,
        before: Option<Instant>,
        priority_of_receiver: BroadcastPeerPriority,
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        let txns = self.timeline_range(sender_bucket, HashMap::new());
        let txns_len = txns.len();
        info!("read_timeline txns_len: {}", txns_len);
        (txns, MultiBucketTimelineIndexIds { id_per_bucket: vec![0; txns_len] })
    }

    fn gc(&mut self) {
        // don't need to implement
    }

    fn gen_snapshot(&self) -> gaptos::aptos_mempool::logging::TxnsLog {
        panic!("don't need to implement")
    }

    fn get_by_hash(&self, hash: HashValue) -> Option<SignedTransaction> {
        panic!("don't need to implement")
    }

    fn add_txn(
        &mut self,
        txn: SignedTransaction,
        ranking_score: u64,
        sequence_info: u64,
        timeline_state: gaptos::aptos_mempool::core_mempool::TimelineState,
        client_submitted: bool,
        ready_time_at_sender: Option<u64>,
        priority: Option<BroadcastPeerPriority>,
    ) -> MempoolStatus {
        info!("add_txn sequence_info: {}", txn.sequence_number());
        let verfited_txn = crate::core_mempool::transaction::VerifiedTxn::from(txn);
        let res = self.pool.add_external_txn(
            verfited_txn.into(),
        );
        if res {
            MempoolStatus::new(MempoolStatusCode::Accepted)
        } else {
            MempoolStatus::new(MempoolStatusCode::UnknownStatus)
        }
    }

    fn gc_by_expiration_time(&mut self, block_time: Duration) {
        // don't need to implement
    }

    fn get_batch(
        &self,
        max_txns: u64,
        max_bytes: u64,
        return_non_full: bool,
        exclude_transactions: BTreeMap<
            gaptos::aptos_consensus_types::common::TransactionSummary,
            gaptos::aptos_consensus_types::common::TransactionInProgress,
        >,
    ) -> Vec<SignedTransaction> {
        self.get_batch_inner(max_txns, max_bytes, return_non_full, exclude_transactions)
    }

    fn reject_transaction(
        &mut self,
        sender: &AccountAddress,
        sequence_number: u64,
        hash: &HashValue,
        reason: &DiscardedVMStatus,
    ) {
        // don't need to implement
    }

    fn commit_transaction(&mut self, sender: &AccountAddress, sequence_number: u64) {
        // don't need to implement
    }

    fn log_commit_transaction(
        &self,
        sender: &AccountAddress,
        sequence_number: u64,
        tracked_use_case: Option<(UseCaseKey, &String)>,
        block_timestamp: Duration,
    ) {
        // don't need to implement
    }
}

impl Mempool {
    pub fn new(_config: &NodeConfig, pool: Box<dyn TxPool>) -> Self {
        Self {
            pool,
            txn_cache: Arc::new(Mutex::new(TxnCache::new(100000))),
        }
    }

    /// This function will be called once the transaction has been stored.
    pub(crate) fn commit_transaction(&mut self, _sender: &AccountAddress, _sequence_number: u64) {
        // debug!(
        //     "commit txn {} {}",
        //     sender,
        //     sequence_number
        // );
        // counters::MEMPOOL_TXN_COMMIT_COUNT.inc();
        // self.transactions
        //     .commit_transaction(sender, sequence_number);
    }

    pub(crate) fn reject_transaction(
        &mut self,
        _sender: &AccountAddress,
        _sequence_number: u64,
        _hash: &HashValue,
        _reason: &DiscardedVMStatus,
    ) {
    }

    pub(crate) fn log_txn_latency(
        _insertion_info: &InsertionInfo,
        _bucket: &str,
        _stage: &'static str,
        _priority: &str,
    ) {
    }

    pub(crate) fn log_commit_transaction(
        &self,
        _sender: &AccountAddress,
        _sequence_number: u64,
        _tracked_use_case: Option<(UseCaseKey, &String)>,
        _block_timestamp: Duration,
    ) {
    }

    pub(crate) fn add_user_txns_batch(
        &mut self,
        _txns_with_numbers: Vec<VerifiedTxnWithAccountSeqNum>,
        _client_submitted: bool,
        _timeline_state: TimelineState,
        _priority: Option<BroadcastPeerPriority>,
    ) -> Vec<MempoolStatus> {
        panic!()
    }

    /// Used to add a transaction to the Mempool.
    /// Performs basic validation: checks account's sequence number.
    pub(crate) fn send_user_txn(
        &mut self,
        txn: VerifiedTxn,
        db_sequence_number: u64,
        timeline_state: TimelineState,
        client_submitted: bool,
        // The time at which the transaction was inserted into the mempool of the
        // downstream node (sender of the mempool transaction) in millis since epoch
        ready_time_at_sender: Option<u64>,
        // The prority of this node for the peer that sent the transaction
        priority: Option<BroadcastPeerPriority>,
    ) -> MempoolStatus {
        panic!()
    }

    /// Fetches next block of transactions for consensus.
    /// `return_non_full` - if false, only return transactions when max_txns or max_bytes is reached
    ///                     Should always be true for Quorum Store.
    /// `include_gas_upgraded` - Return transactions that had gas upgraded, even if they are in
    ///                          exclude_transactions. Should only be true for Quorum Store.
    /// `exclude_transactions` - transactions that were sent to Consensus but were not committed yet
    ///  mempool should filter out such transactions.
    #[allow(clippy::explicit_counter_loop)]
    pub(crate) fn get_batch_inner(
        &self,
        max_txns: u64,
        max_bytes: u64,
        return_non_full: bool,
        exclude_transactions: BTreeMap<
            gaptos::aptos_consensus_types::common::TransactionSummary,
            gaptos::aptos_consensus_types::common::TransactionInProgress,
        >,
    ) -> Vec<SignedTransaction> {
        let filter = Box::new(move |txn: (ExternalAccountAddress, u64, TxnHash)| {
            let summary = gaptos::aptos_consensus_types::common::TransactionSummary {
                sender: AccountAddress::new(txn.0.bytes()),
                sequence_number: txn.1,
                hash: HashValue::new(txn.2 .0),
            };
            !exclude_transactions.contains_key(&summary)
        });
        let mut transactions = vec![];
        let best_txns = self.pool.best_txns(Some(filter));
        for txn in best_txns {
            let signed_txn = VerifiedTxn::from(txn).into();
            transactions.push(signed_txn);
            if transactions.len() >= max_txns as usize || transactions.len() >= max_bytes as usize {
                break;
            }
        }
        transactions
    }

    pub(crate) fn get_txn_count(&self) -> usize {
        // self.transactions.txn_size()
        todo!()
    }

    pub(crate) fn priority_index_size(&self) -> usize {
        // self.transactions.priority_index_size()
        todo!()
    }

    #[cfg(test)]
    pub fn get_transaction_store(&self) -> &TransactionStore {
        // &self.transactions
        panic!()
    }

    pub fn gen_snapshot(&self) -> Vec<SignedTransaction> {
        panic!()
    }
}
