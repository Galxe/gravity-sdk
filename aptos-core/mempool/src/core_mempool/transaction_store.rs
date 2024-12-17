// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    core_mempool::{
        index::{
            AccountTransactions, TTLIndex,
        },
        mempool::Mempool,
        transaction::{InsertionInfo, MempoolTransaction, TimelineState},
    },
    counters::{self, BROADCAST_BATCHED_LABEL, BROADCAST_READY_LABEL, CONSENSUS_READY_LABEL},
    logging::{LogEntry, LogEvent, LogSchema, TxnsLog},
    network::BroadcastPeerPriority,
    shared_mempool::types::{
        MempoolSenderBucket, MultiBucketTimelineIndexIds, TimelineIndexIdentifier,
    },
};
use aptos_config::config::MempoolConfig;
use aptos_crypto::HashValue;
use aptos_logger::{prelude::*, Level};
use aptos_types::{
    account_address::AccountAddress,
    mempool_status::{MempoolStatus, MempoolStatusCode},
    transaction::SignedTransaction,
};
use std::{
    cmp::max,
    collections::HashMap,
    mem::size_of,
    ops::Bound,
    time::{Duration, Instant, SystemTime},
};

use super::index::{MultiBucketTimelineIndex, PriorityIndex, PriorityQueueIter};

/// Estimated per-txn overhead of indexes. Needs to be updated if additional indexes are added.
pub const TXN_INDEX_ESTIMATED_BYTES: usize = size_of::<crate::core_mempool::index::OrderedQueueKey>() // priority_index
    + size_of::<crate::core_mempool::index::TTLOrderingKey>() * 2 // expiration_time_index + system_ttl_index
    + (size_of::<u64>() * 3 + size_of::<AccountAddress>()) // timeline_index
    + (size_of::<HashValue>() + size_of::<u64>() + size_of::<AccountAddress>()); // hash_index

pub fn sender_bucket(
    address: &AccountAddress,
    num_sender_buckets: MempoolSenderBucket,
) -> MempoolSenderBucket {
    address.as_ref()[address.as_ref().len() - 1] as MempoolSenderBucket % num_sender_buckets
}

/// TransactionStore is in-memory storage for all transactions in mempool.
pub struct TransactionStore {
    // main DS
    transactions: HashMap<AccountAddress, AccountTransactions>,

    // Sequence numbers for accounts with transactions
    sequence_numbers: HashMap<AccountAddress, u64>,

    // indexes
    priority_index: PriorityIndex,
    timeline_index: HashMap<MempoolSenderBucket, MultiBucketTimelineIndex>,
    // We divide the senders into buckets and maintain a separate set of timelines for each sender bucket.
    // This is the number of sender buckets.
    num_sender_buckets: MempoolSenderBucket,
    hash_index: HashMap<HashValue, (AccountAddress, u64)>,
    // estimated size in bytes
    size_bytes: usize,

    // configuration
    capacity: usize,
    capacity_bytes: usize,
    capacity_per_user: usize,
    max_batch_bytes: u64,
}

impl TransactionStore {
    pub(crate) fn new(config: &MempoolConfig) -> Self {
        let mut timeline_index = HashMap::new();
        for sender_bucket in 0..config.num_sender_buckets {
            timeline_index.insert(
                sender_bucket,
                MultiBucketTimelineIndex::new(config.broadcast_buckets.clone()).unwrap(),
            );
        }
        Self {
            // main DS
            transactions: HashMap::new(),
            sequence_numbers: HashMap::new(),

            priority_index: PriorityIndex::new(),
            timeline_index,
            num_sender_buckets: config.num_sender_buckets,
            hash_index: HashMap::new(),
            // estimated size in bytes
            size_bytes: 0,

            // configuration
            capacity: config.capacity,
            capacity_bytes: config.capacity_bytes,
            capacity_per_user: config.capacity_per_user,
            max_batch_bytes: config.shared_mempool_max_batch_bytes,
        }
    }

    #[inline]
    fn get_mempool_txn(
        &self,
        address: &AccountAddress,
        sequence_number: u64,
    ) -> Option<&MempoolTransaction> {
        self.transactions
            .get(address)
            .and_then(|txns| txns.get(&sequence_number))
    }

    /// Fetch transaction by account address + sequence_number.
    pub(crate) fn get(
        &self,
        address: &AccountAddress,
        sequence_number: u64,
    ) -> Option<SignedTransaction> {
        if let Some(txn) = self.get_mempool_txn(address, sequence_number) {
            return  Some(txn.verified_txn().clone());
        }
        None
    }

    /// Fetch transaction by account address + sequence_number, including ranking score
    pub(crate) fn get_with_ranking_score(
        &self,
        address: &AccountAddress,
        sequence_number: u64,
    ) -> Option<(SignedTransaction, u64)> {
        if let Some(txn) = self.get_mempool_txn(address, sequence_number) {
            // TODO: constructed signed txn from raw txn bytes
            return Some((txn.verified_txn().clone(), txn.ranking_score()));
        }
        None
    }

    pub(crate) fn get_by_hash(&self, hash: HashValue) -> Option<SignedTransaction> {
        match self.hash_index.get(&hash) {
            Some((address, seq)) => self.get(address, *seq),
            None => None,
        }
    }

    pub(crate) fn get_insertion_info_and_bucket(
        &self,
        address: &AccountAddress,
        sequence_number: u64,
    ) -> Option<(&InsertionInfo, String, String)> {
        if let Some(txn) = self.get_mempool_txn(address, sequence_number) {
            return Some((
                &txn.insertion_info(),
                self.get_bucket(txn.ranking_score(), address),
                txn.priority_of_sender()
                    .clone()
                    .map_or_else(|| "Unknown".to_string(), |priority| priority.to_string()),
            ));
        }
        None
    }

    pub(crate) fn get_ranking_score(
        &self,
        address: &AccountAddress,
        sequence_number: u64,
    ) -> Option<u64> {
        if let Some(txn) = self.get_mempool_txn(address, sequence_number) {
            return Some(txn.ranking_score());
        }
        None
    }

    #[inline]
    pub(crate) fn get_bucket(&self, ranking_score: u64, sender: &AccountAddress) -> String {
        let sender_bucket = sender_bucket(sender, self.num_sender_buckets);
        let bucket = self
            .timeline_index
            .get(&sender_bucket)
            .unwrap()
            .get_bucket(ranking_score)
            .to_string();
        format!("{}_{}", sender_bucket, bucket)
    }

    pub(crate) fn get_sequence_number(&self, address: &AccountAddress) -> Option<&u64> {
        self.sequence_numbers.get(address)
    }

    /// Insert transaction into TransactionStore. Performs validation checks and updates indexes.
    pub(crate) fn insert(&mut self, txn: MempoolTransaction) -> MempoolStatus {
        let address = txn.verified_txn().sender();
        let txn_seq_num = txn.verified_txn().sequence_number();

        // If the transaction is already in Mempool, we just reject
        // TODO: how to replace the old txn with the new one?
        if let Some(txns) = self.transactions.get_mut(&address) {
            if let Some(current_version) = txns.get_mut(&txn_seq_num) {
                if current_version.verified_txn().gas_unit_price() < txn.verified_txn().gas_unit_price() {
                    // Update txn if gas unit price is a larger value than before
                    if let Some(txn) = txns.remove(&txn_seq_num) {
                        self.index_remove(&txn);
                    };
                    counters::CORE_MEMPOOL_GAS_UPGRADED_TXNS.inc();
                } else {
                    // check if txn is already in mempool
                    return MempoolStatus::new(MempoolStatusCode::InvalidSeqNumber).with_message(
                        format!(
                            "Transaction with account and sequence number already exists in mempool: {}:{}",
                            address, txn_seq_num,
                        ),
                    );
                }
            }
        }
        let acc_seq_num = txn.account_sequence_number();
        if self.check_is_full_after_eviction(&txn, acc_seq_num) {
            return MempoolStatus::new(MempoolStatusCode::MempoolIsFull).with_message(format!(
                "Mempool is full. Mempool size: {}, Capacity: {}",
                self.size_bytes,
                self.capacity,
            ));
        }

        self.clean_committed_transactions(&address, acc_seq_num);

        self.transactions.entry(address).or_default();

        if let Some(txns) = self.transactions.get_mut(&address) {
            // capacity check
            if txns.len() >= self.capacity_per_user {
                return MempoolStatus::new(MempoolStatusCode::TooManyTransactions).with_message(
                    format!(
                        "Mempool over capacity for account. Number of transactions from account: {} Capacity per account: {}",
                        txns.len(),
                        self.capacity_per_user,
                    ),
                );
            }

            // insert into storage and other indexes
            self.hash_index
                .insert(txn.get_hash(), (txn.verified_txn().sender(), txn_seq_num));
            self.sequence_numbers.insert(txn.verified_txn().sender(), acc_seq_num);
            self.size_bytes += txn.get_estimated_bytes();
            txns.insert(txn_seq_num, txn);
            self.track_indices();
        }
        self.process_ready_transactions(&address, acc_seq_num);
        MempoolStatus::new(MempoolStatusCode::Accepted)
    }

    fn track_indices(&self) {
        counters::core_mempool_index_size(
            counters::PRIORITY_INDEX_LABEL,
            self.priority_index.size(),
        );
        counters::core_mempool_index_size(
            counters::TIMELINE_INDEX_LABEL,
            self.timeline_index
                .values()
                .map(|timelines| timelines.size())
                .sum(),
        );

        let mut bucket_min_size_pairs = vec![];
        for (sender_bucket, timelines) in self.timeline_index.iter() {
            for (timeline_index_identifier, size) in timelines.get_sizes() {
                bucket_min_size_pairs.push((
                    format!("{}_{}", sender_bucket, timeline_index_identifier),
                    size,
                ));
            }
        }
        counters::core_mempool_timeline_index_size(bucket_min_size_pairs);
        counters::core_mempool_index_size(
            counters::TRANSACTION_HASH_INDEX_LABEL,
            self.hash_index.len(),
        );
        counters::core_mempool_index_size(counters::SIZE_BYTES_LABEL, self.size_bytes);
    }

    /// Checks if Mempool is full.
    /// If it's full, tries to free some space by evicting transactions from the ParkingLot.
    /// We only evict on attempt to insert a transaction that would be ready for broadcast upon insertion.
    fn check_is_full_after_eviction(
        &mut self,
        txn: &MempoolTransaction,
        curr_sequence_number: u64,
    ) -> bool {
        if self.is_full() && self.check_txn_ready(txn, curr_sequence_number) {
            // try to free some space in Mempool from ParkingLot by evicting a non-ready txn
        }
        self.is_full()
    }

    fn is_full(&self) -> bool {
        self.size_bytes >= self.capacity_bytes
    }

    /// Check if a transaction would be ready for broadcast in mempool upon insertion (without inserting it).
    /// Two ways this can happen:
    /// 1. txn sequence number == curr_sequence_number
    /// (this handles both cases where, (1) txn is first possible txn for an account and (2) the
    /// previous txn is committed).
    /// 2. The txn before this is ready for broadcast but not yet committed.
    fn check_txn_ready(&self, txn: &MempoolTransaction, curr_sequence_number: u64) -> bool {
        let tx_sequence_number = txn.verified_txn().sequence_number();
        if tx_sequence_number == curr_sequence_number {
            return true;
        } else if tx_sequence_number == 0 {
            // shouldn't really get here because filtering out old txn sequence numbers happens earlier in workflow
            unreachable!("[mempool] already committed txn detected, cannot be checked for readiness upon insertion");
        }

        // check previous txn in sequence is ready
        if let Some(account_txns) = self.transactions.get(&txn.verified_txn().sender()) {
            if let Some(prev_txn) = account_txns.get(&(tx_sequence_number - 1)) {
                if let TimelineState::Ready(_) = prev_txn.timeline_state {
                    return true;
                }
            }
        }
        false
    }

    fn log_ready_transaction(
        ranking_score: u64,
        bucket: &str,
        insertion_info: &mut InsertionInfo,
        broadcast_ready: bool,
        priority: &str,
    ) {
        insertion_info.ready_time = SystemTime::now();
        if let Ok(time_delta) = SystemTime::now().duration_since(insertion_info.insertion_time) {
            let submitted_by = insertion_info.submitted_by_label();
            if broadcast_ready {
                counters::core_mempool_txn_commit_latency(
                    CONSENSUS_READY_LABEL,
                    submitted_by,
                    bucket,
                    time_delta,
                    priority,
                );
                counters::core_mempool_txn_commit_latency(
                    BROADCAST_READY_LABEL,
                    submitted_by,
                    bucket,
                    time_delta,
                    priority,
                );
            } else {
                counters::core_mempool_txn_commit_latency(
                    CONSENSUS_READY_LABEL,
                    submitted_by,
                    bucket,
                    time_delta,
                    priority,
                );
            }
        }

        if broadcast_ready {
            counters::core_mempool_txn_ranking_score(
                BROADCAST_READY_LABEL,
                BROADCAST_READY_LABEL,
                bucket,
                ranking_score,
            );
        }
        counters::core_mempool_txn_ranking_score(
            CONSENSUS_READY_LABEL,
            CONSENSUS_READY_LABEL,
            bucket,
            ranking_score,
        );
    }

    /// Maintains the following invariants:
    /// - All transactions of a given account that are sequential to the current sequence number
    ///   should be included in both the PriorityIndex (ordering for Consensus) and
    ///   TimelineIndex (txns for SharedMempool).
    /// - Other txns are considered to be "non-ready" and should be added to ParkingLotIndex.
    fn process_ready_transactions(&mut self, address: &AccountAddress, sequence_num: u64) {
        let sender_bucket = sender_bucket(address, self.num_sender_buckets);
        if let Some(txns) = self.transactions.get_mut(address) {
            let mut min_seq = sequence_num;

            while let Some(txn) = txns.get_mut(&min_seq) {
                let process_ready = !self.priority_index.contains(txn);

                self.priority_index.insert(txn);

                let process_broadcast_ready = txn.timeline_state == TimelineState::NotReady;
                if process_broadcast_ready {
                    self.timeline_index
                        .get_mut(&sender_bucket)
                        .unwrap()
                        .insert(txn);
                }

                if process_ready {
                    let bucket = self
                        .timeline_index
                        .get(&sender_bucket)
                        .unwrap()
                        .get_bucket(txn.ranking_score())
                        .to_string();
                    let bucket = format!("{}_{}", sender_bucket, bucket);
                    let priority = txn.priority_of_sender().clone();
                    Self::log_ready_transaction(
                        txn.ranking_score(),
                        bucket.as_str(),
                        &mut txn.get_mut_insertion_info(),
                        process_broadcast_ready,
                        priority
                            .clone()
                            .map_or_else(|| "Unknown".to_string(), |priority| priority.to_string())
                            .as_str(),
                    );
                }

                // Remove txn from parking lot after it has been promoted to
                // priority_index / timeline_index, i.e., txn status is ready.
                min_seq += 1;
            }

            let mut parking_lot_txns = 0;
            // for (_, txn) in txns.range_mut((Bound::Excluded(min_seq), Bound::Unbounded)) {
            //     match txn.timeline_state {
            //         TimelineState::Ready(_) => {},
            //         _ => panic!(),
            //     }
            // }

            trace!(
                LogSchema::new(LogEntry::ProcessReadyTxns).account(*address),
                first_ready_seq_num = sequence_num,
                last_ready_seq_num = min_seq,
                num_parked_txns = parking_lot_txns,
            );
            self.track_indices();
        }
    }

    fn clean_committed_transactions(&mut self, address: &AccountAddress, sequence_number: u64) {
        // Remove all previous seq number transactions for this account.
        // This can happen if transactions are sent to multiple nodes and one of the
        // nodes has sent the transaction to consensus but this node still has the
        // transaction sitting in mempool.
        if let Some(txns) = self.transactions.get_mut(address) {
            let mut active = txns.split_off(&sequence_number);
            let txns_for_removal = txns.clone();
            txns.clear();
            txns.append(&mut active);

            let mut rm_txns = match aptos_logger::enabled!(Level::Trace) {
                true => TxnsLog::new(),
                false => TxnsLog::new_with_max(10),
            };
            for transaction in txns_for_removal.values() {
                rm_txns.add(
                    transaction.verified_txn().sender(),
                    transaction.verified_txn().sequence_number(),
                );
                self.index_remove(transaction);
            }
            trace!(
                LogSchema::new(LogEntry::CleanCommittedTxn).txns(rm_txns),
                "txns cleaned with committing tx {}:{}",
                address,
                sequence_number
            );
        }
    }

    /// Handles transaction commit.
    /// It includes deletion of all transactions with sequence number <= `account_sequence_number`
    /// and potential promotion of sequential txns to PriorityIndex/TimelineIndex.
    pub fn commit_transaction(&mut self, account: &AccountAddress, sequence_number: u64) {
        let current_seq_number = self.get_sequence_number(account).map_or(0, |v| *v);
        let new_seq_number = max(current_seq_number, sequence_number + 1);
        self.sequence_numbers.insert(*account, new_seq_number);
        self.clean_committed_transactions(account, new_seq_number);
        self.process_ready_transactions(account, new_seq_number);
    }

    pub fn reject_transaction(
        &mut self,
        account: &AccountAddress,
        sequence_number: u64,
        hash: &HashValue,
    ) {
        let mut txn_to_remove = None;
        if let Some((indexed_account, indexed_sequence_number)) = self.hash_index.get(hash) {
            if account == indexed_account && sequence_number == *indexed_sequence_number {
                txn_to_remove = self.get_mempool_txn(account, sequence_number).cloned();
            }
        }
        if let Some(txn_to_remove) = txn_to_remove {
            if let Some(txns) = self.transactions.get_mut(account) {
                txns.remove(&sequence_number);
            }
            self.index_remove(&txn_to_remove);
            println!("remove {:?}", txn_to_remove.verified_txn());
            if aptos_logger::enabled!(Level::Trace) {
                let mut txns_log = TxnsLog::new();
                txns_log.add(
                    txn_to_remove.verified_txn().sender(),
                    txn_to_remove.verified_txn().sequence_number(),
                );
                trace!(LogSchema::new(LogEntry::CleanRejectedTxn).txns(txns_log));
            }
        }
    }

    /// Removes transaction from all indexes. Only call after removing from main transactions DS.
    fn index_remove(&mut self, txn: &MempoolTransaction) {
        // panic!("index remove");
        counters::CORE_MEMPOOL_REMOVED_TXNS.inc();
        self.priority_index.remove(txn);
        let sender_bucket = sender_bucket(&txn.verified_txn().sender(), self.num_sender_buckets);
        self.timeline_index
            .get_mut(&sender_bucket)
            .unwrap_or_else(|| {
                panic!(
                    "Unable to get the timeline index for the sender bucket {}",
                    sender_bucket
                )
            })
            .remove(txn);
        self.size_bytes -= txn.get_estimated_bytes();

        // Remove account datastructures if there are no more transactions for the account.
        let address = &txn.verified_txn().sender();
        if let Some(txns) = self.transactions.get(address) {
            if txns.is_empty() {
                self.transactions.remove(address);
                self.sequence_numbers.remove(address);
            }
        }

        self.track_indices();
    }

    /// Read at most `count` transactions from timeline since `timeline_id`.
    /// This method takes into account the max number of bytes per transaction batch.
    /// Returns block of transactions along with their transaction ready times
    /// and new last_timeline_id.
    pub(crate) fn read_timeline(
        &self,
        sender_bucket: MempoolSenderBucket,
        timeline_id: &MultiBucketTimelineIndexIds,
        count: usize,
        before: Option<Instant>,
        // The priority of the receipient of the transactions
        priority_of_receiver: BroadcastPeerPriority,
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        let mut batch = vec![];
        let mut batch_total_bytes: u64 = 0;
        let mut last_timeline_id = timeline_id.id_per_bucket.clone();

        // Add as many transactions to the batch as possible
        for (i, bucket) in self
            .timeline_index
            .get(&sender_bucket)
            .unwrap_or_else(|| {
                panic!(
                    "Unable to get the timeline index for the sender bucket {}",
                    sender_bucket
                )
            })
            .read_timeline(timeline_id, count, before)
            .iter()
            .enumerate()
            .rev()
        {
            for (address, sequence_number) in bucket {
                if let Some(txn) = self.get_mempool_txn(address, *sequence_number) {
                    let transaction_bytes = txn.get_estimated_bytes() as u64;
                    if batch_total_bytes.saturating_add(transaction_bytes) > self.max_batch_bytes {
                        break; // The batch is full
                    } else {
                        // reconstruct signed transaction from raw transaction bytes
                        batch.push((
                            (txn.verified_txn().clone()),
                            aptos_infallible::duration_since_epoch_at(
                                &txn.insertion_info().ready_time,
                            )
                            .as_millis() as u64,
                        ));
                        batch_total_bytes = batch_total_bytes.saturating_add(transaction_bytes);
                        if let TimelineState::Ready(timeline_id) = txn.timeline_state {
                            last_timeline_id[i] = timeline_id;
                        }
                        let bucket = self.get_bucket(txn.ranking_score(), &txn.verified_txn().sender());
                        Mempool::log_txn_latency(
                            &txn.insertion_info(),
                            bucket.as_str(),
                            BROADCAST_BATCHED_LABEL,
                            priority_of_receiver.to_string().as_str(),
                        );
                        counters::core_mempool_txn_ranking_score(
                            BROADCAST_BATCHED_LABEL,
                            BROADCAST_BATCHED_LABEL,
                            bucket.as_str(),
                            txn.ranking_score(),
                        );
                    }
                }
            }
        }

        (batch, last_timeline_id.into())
    }

    pub(crate) fn timeline_range(
        &self,
        sender_bucket: MempoolSenderBucket,
        start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        self.timeline_index
            .get(&sender_bucket)
            .unwrap_or_else(|| {
                panic!(
                    "Unable to get the timeline index for the sender bucket {}",
                    sender_bucket
                )
            })
            .timeline_range(start_end_pairs)
            .iter()
            .filter_map(|(account, sequence_number)| {
                self.transactions
                    .get(account)
                    .and_then(|txns| txns.get(sequence_number))
                    .map(|txn| {
                        (
                            txn.verified_txn().clone(),
                            aptos_infallible::duration_since_epoch_at(
                                &txn.insertion_info().ready_time,
                            )
                            .as_millis() as u64,
                        )
                    })
            })
            .collect()
    }

    pub(crate) fn iter_queue(&self) -> PriorityQueueIter {
        self.priority_index.iter()
    }

    // #[cfg(test)]
    pub(crate) fn get_transactions(&self) -> &HashMap<AccountAddress, AccountTransactions> {
        &self.transactions
    }
}
