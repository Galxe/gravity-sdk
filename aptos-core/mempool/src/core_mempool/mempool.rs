// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Mempool is used to track transactions which have been submitted but not yet
//! agreed upon.
use crate::{
    core_mempool::transaction::TimelineState,
    network::BroadcastPeerPriority,
    shared_mempool::types::{
        MempoolSenderBucket, MultiBucketTimelineIndexIds, TimelineIndexIdentifier,
    },
};
use gaptos::{
    api_types::{account::ExternalAccountAddress, u256_define::TxnHash},
    aptos_config::config::NodeConfig,
    aptos_crypto::HashValue,
    aptos_mempool::shared_mempool::types::CoreMempoolTrait,
    aptos_types::{
        account_address::AccountAddress,
        mempool_status::{MempoolStatus, MempoolStatusCode},
        transaction::{use_case::UseCaseKey, SignedTransaction, TransactionPayload},
        vm_status::DiscardedVMStatus,
    },
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::transaction::VerifiedTxn;
use block_buffer_manager::TxPool;

pub struct TxnCache {
    old_cache: HashSet<TxnHash>,
    cache: HashSet<TxnHash>,
    size: usize,
    last_rotation: Instant,
    ttl: Duration,
}

impl TxnCache {
    fn new(size: usize) -> Self {
        let ttl_secs = std::env::var("MEMPOOL_BROADCAST_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);
        Self {
            old_cache: HashSet::new(),
            cache: HashSet::new(),
            size,
            last_rotation: Instant::now(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    fn maybe_rotate(&mut self) {
        if self.cache.len() > self.size || self.last_rotation.elapsed() > self.ttl {
            self.old_cache = std::mem::take(&mut self.cache);
            self.last_rotation = Instant::now();
        }
    }

    fn insert(&mut self, txn_hash: TxnHash) {
        self.maybe_rotate();
        self.cache.insert(txn_hash);
    }

    pub fn is_contains(&mut self, txn_hash: &TxnHash) -> bool {
        self.maybe_rotate();
        self.cache.contains(txn_hash) || self.old_cache.contains(txn_hash)
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
        _sender_bucket: MempoolSenderBucket,
        _start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        vec![]
    }

    fn timeline_range_of_message(
        &self,
        _sender_start_end_pairs: HashMap<
            MempoolSenderBucket,
            HashMap<TimelineIndexIdentifier, (u64, u64)>,
        >,
    ) -> Vec<(SignedTransaction, u64)> {
        vec![]
    }

    fn get_parking_lot_addresses(&self) -> Vec<(AccountAddress, u64)> {
        // don't need to implement
        vec![]
    }

    fn read_timeline(
        &self,
        _sender_bucket: MempoolSenderBucket,
        _timeline_id: &MultiBucketTimelineIndexIds,
        _count: usize,
        _before: Option<Instant>,
        _priority_of_receiver: BroadcastPeerPriority,
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        let visited = self.txn_cache.clone();
        let filter = Box::new(move |txn: (ExternalAccountAddress, u64, TxnHash)| {
            !visited.lock().unwrap().is_contains(&txn.2)
        });
        let iter = self.pool.get_broadcast_txns(Some(filter));
        let mut broacasted_txns = vec![];
        let mut visited_cache = self.txn_cache.lock().unwrap();
        for txn in iter {
            visited_cache.insert(TxnHash::from_bytes(txn.committed_hash().as_slice()));
            broacasted_txns.push((VerifiedTxn::from(txn).into(), 0));
        }
        let len = broacasted_txns.len();

        (broacasted_txns, MultiBucketTimelineIndexIds { id_per_bucket: vec![0; len] })
    }

    fn gc(&mut self) {
        // don't need to implement
    }

    fn gen_snapshot(&self) -> gaptos::aptos_mempool::logging::TxnsLog {
        panic!("don't need to implement")
    }

    fn get_by_hash(&self, _hash: HashValue) -> Option<SignedTransaction> {
        panic!("don't need to implement")
    }

    fn add_txn(
        &mut self,
        txn: SignedTransaction,
        _ranking_score: u64,
        _sequence_info: u64,
        _timeline_state: gaptos::aptos_mempool::core_mempool::TimelineState,
        _client_submitted: bool,
        _ready_time_at_sender: Option<u64>,
        _priority: Option<BroadcastPeerPriority>,
    ) -> MempoolStatus {
        if !matches!(txn.payload(), TransactionPayload::GTxnBytes(_)) {
            return MempoolStatus::new(MempoolStatusCode::UnknownStatus);
        }

        self.txn_cache.lock().unwrap().insert(TxnHash::new(*txn.committed_hash()));
        let verfited_txn = crate::core_mempool::transaction::VerifiedTxn::from(txn);
        let res = self.pool.add_external_txn(verfited_txn.into());
        if res {
            MempoolStatus::new(MempoolStatusCode::Accepted)
        } else {
            MempoolStatus::new(MempoolStatusCode::UnknownStatus)
        }
    }

    fn gc_by_expiration_time(&mut self, _block_time: Duration) {
        // don't need to implement
    }

    fn get_batch(
        &self,
        max_txns: u64,
        max_bytes: u64,
        _return_non_full: bool,
        exclude_transactions: BTreeMap<
            gaptos::aptos_consensus_types::common::TransactionSummary,
            gaptos::aptos_consensus_types::common::TransactionInProgress,
        >,
    ) -> Vec<SignedTransaction> {
        self.get_batch_inner(max_txns, max_bytes, _return_non_full, exclude_transactions)
    }

    fn reject_transaction(
        &mut self,
        _sender: &AccountAddress,
        _sequence_number: u64,
        _hash: &HashValue,
        _reason: &DiscardedVMStatus,
    ) {
        // don't need to implement
    }

    fn commit_transaction(&mut self, sender: &AccountAddress, sequence_number: u64) {
        txn_metrics::TxnLifeTime::get_txn_life_time().record_committed(sender, sequence_number);
    }

    fn log_commit_transaction(
        &self,
        _sender: &AccountAddress,
        _sequence_number: u64,
        _tracked_use_case: Option<(UseCaseKey, &String)>,
        _block_timestamp: Duration,
    ) {
        // don't need to implement
    }
}

impl Mempool {
    pub fn new(_config: &NodeConfig, pool: Box<dyn TxPool>) -> Self {
        Self { pool, txn_cache: Arc::new(Mutex::new(TxnCache::new(100000))) }
    }

    /// This function will be called once the transaction has been stored.
    #[allow(dead_code)]
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
    /// Used to add a transaction to the Mempool.
    /// Performs basic validation: checks account's sequence number.
    #[allow(dead_code)]
    pub(crate) fn send_user_txn(
        &mut self,
        _txn: VerifiedTxn,
        _db_sequence_number: u64,
        _timeline_state: TimelineState,
        _client_submitted: bool,
        // The time at which the transaction was inserted into the mempool of the
        // downstream node (sender of the mempool transaction) in millis since epoch
        _ready_time_at_sender: Option<u64>,
        // The prority of this node for the peer that sent the transaction
        _priority: Option<BroadcastPeerPriority>,
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
        _return_non_full: bool,
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
        let best_txns = self.pool.best_txns(Some(filter), max_txns as usize);
        for txn in best_txns {
            let signed_txn = VerifiedTxn::from(txn).into();
            transactions.push(signed_txn);
            if transactions.len() >= max_txns as usize || transactions.len() >= max_bytes as usize {
                break;
            }
        }
        transactions
    }

    pub fn gen_snapshot(&self) -> Vec<SignedTransaction> {
        panic!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_txn_hash(id: u64) -> TxnHash {
        TxnHash::from_bytes(&[id as u8; 32])
    }

    #[test]
    fn test_txn_cache_size_rotation() {
        let cache_size = 2;
        let mut cache = TxnCache::new(cache_size);

        let h1 = mock_txn_hash(1);
        let h2 = mock_txn_hash(2);
        let h3 = mock_txn_hash(3);
        let h4 = mock_txn_hash(4);
        let h5 = mock_txn_hash(5);

        // 1. Insert within capacity — no rotation
        cache.insert(h1);
        cache.insert(h2);
        cache.insert(h3);
        // cache = {h1, h2, h3}, no rotation yet (maybe_rotate checks before insert)
        assert!(cache.is_contains(&h1));
        assert!(cache.is_contains(&h2));
        assert!(cache.is_contains(&h3));
        assert_eq!(cache.cache.len(), 3);
        assert_eq!(cache.old_cache.len(), 0);

        // 2. Next insert triggers rotation (cache.len()=3 > size=2)
        cache.insert(h4);
        // rotation moved {h1,h2,h3} to old_cache, then h4 inserted into cache
        assert!(cache.is_contains(&h1)); // in old_cache
        assert!(cache.is_contains(&h4)); // in cache
        assert_eq!(cache.cache.len(), 1);
        assert_eq!(cache.old_cache.len(), 3);

        // 3. One more rotation evicts old_cache
        cache.insert(h5);
        // cache = {h4, h5}, no rotation yet (len=2, not > 2)
        assert!(cache.is_contains(&h4));
        assert!(cache.is_contains(&h5));
        assert!(cache.is_contains(&h1)); // still in old_cache
    }

    #[test]
    fn test_txn_cache_ttl_rotation() {
        let cache_size = 100000; // large size so only TTL triggers rotation
        let mut cache = TxnCache::new(cache_size);
        cache.ttl = Duration::from_millis(50); // short TTL for testing

        let h1 = mock_txn_hash(1);
        let h2 = mock_txn_hash(2);

        cache.insert(h1);
        assert!(cache.is_contains(&h1));

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(60));

        // Next operation triggers rotation — h1 moves to old_cache
        cache.insert(h2);
        assert!(cache.is_contains(&h1)); // still in old_cache
        assert!(cache.is_contains(&h2));

        // Wait for TTL again
        std::thread::sleep(Duration::from_millis(60));

        // h1 should now be evicted (old_cache replaced)
        assert!(!cache.is_contains(&h1)); // evicted!
        assert!(cache.is_contains(&h2)); // moved to old_cache by the rotation in is_contains
    }
}
