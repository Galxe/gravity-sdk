// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

/// This module provides various indexes used by Mempool.
use crate::core_mempool::transaction::{MempoolTransaction, TimelineState};
use crate::shared_mempool::types::{MultiBucketTimelineIndexIds, TimelineIndexIdentifier};
use aptos_consensus_types::common::TransactionSummary;
use aptos_crypto::HashValue;
use aptos_types::account_address::AccountAddress;
use std::{
    cmp::Ordering,
    collections::{btree_set::Iter, BTreeMap, BTreeSet, HashMap},
    hash::Hash,
    iter::Rev,
    ops::Bound,
    time::{Duration, Instant},
};

pub type AccountTransactions = BTreeMap<u64, MempoolTransaction>;

/// PriorityIndex represents the main Priority Queue in Mempool.
/// It's used to form the transaction block for Consensus.
/// Transactions are ordered by gas price. Second level ordering is done by expiration time.
///
/// We don't store the full content of transactions in the index.
/// Instead we use `OrderedQueueKey` - logical reference to the transaction in the main store.
pub struct PriorityIndex {
    data: BTreeSet<OrderedQueueKey>,
}

pub type PriorityQueueIter<'a> = Rev<Iter<'a, OrderedQueueKey>>;

impl PriorityIndex {
    pub(crate) fn new() -> Self {
        Self { data: BTreeSet::new() }
    }

    pub(crate) fn insert(&mut self, txn: &MempoolTransaction) {
        self.data.insert(self.make_key(txn));
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        self.data.remove(&self.make_key(txn));
    }

    pub(crate) fn contains(&self, txn: &MempoolTransaction) -> bool {
        self.data.contains(&self.make_key(txn))
    }

    fn make_key(&self, txn: &MempoolTransaction) -> OrderedQueueKey {
        OrderedQueueKey {
            address: txn.verified_txn().sender(),
            sequence_number: txn.verified_txn().sequence_number(),
            hash: txn.get_hash(),
        }
    }

    pub(crate) fn iter(&self) -> PriorityQueueIter {
        self.data.iter().rev()
    }

    pub(crate) fn size(&self) -> usize {
        self.data.len()
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Hash)]
pub struct OrderedQueueKey {
    pub address: AccountAddress,
    pub sequence_number: u64,
    pub hash: HashValue,
}

impl OrderedQueueKey {
    pub fn get_sequence_number(&self) -> u64 {
        self.sequence_number
    }
}

impl PartialOrd for OrderedQueueKey {
    fn partial_cmp(&self, other: &OrderedQueueKey) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedQueueKey {
    fn cmp(&self, other: &OrderedQueueKey) -> Ordering {
        match self.address.cmp(&other.address) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.sequence_number.cmp(&other.sequence_number).reverse() {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        self.hash.cmp(&other.hash)
    }
}

/// TTLIndex is used to perform garbage collection of old transactions in Mempool.
/// Periodically separate GC-like job queries this index to find out transactions that have to be
/// removed. Index is represented as `BTreeSet<TTLOrderingKey>`, where `TTLOrderingKey`
/// is a logical reference to TxnInfo.
/// Index is ordered by `TTLOrderingKey::expiration_time`.
pub struct TTLIndex {
    data: BTreeSet<TTLOrderingKey>,
    get_expiration_time: Box<dyn Fn(&MempoolTransaction) -> Duration + Send + Sync>,
}

impl TTLIndex {
    pub(crate) fn new<F>(get_expiration_time: Box<F>) -> Self
    where
        F: Fn(&MempoolTransaction) -> Duration + 'static + Send + Sync,
    {
        Self { data: BTreeSet::new(), get_expiration_time }
    }

    pub(crate) fn insert(&mut self, txn: &MempoolTransaction) {
        self.data.insert(self.make_key(txn));
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        self.data.remove(&self.make_key(txn));
    }

    /// Garbage collect all old transactions.
    pub(crate) fn gc(&mut self, now: Duration) -> Vec<TTLOrderingKey> {
        let ttl_key = TTLOrderingKey {
            expiration_time: now,
            address: AccountAddress::ZERO,
            sequence_number: 0,
        };

        let mut active = self.data.split_off(&ttl_key);
        let ttl_transactions = self.data.iter().cloned().collect();
        self.data.clear();
        self.data.append(&mut active);
        ttl_transactions
    }

    fn make_key(&self, txn: &MempoolTransaction) -> TTLOrderingKey {
        TTLOrderingKey {
            expiration_time: (self.get_expiration_time)(txn),
            address: txn.verified_txn().sender(),
            sequence_number: txn.verified_txn().sequence_number(),
        }
    }

    pub(crate) fn iter(&self) -> Iter<TTLOrderingKey> {
        self.data.iter()
    }

    pub(crate) fn size(&self) -> usize {
        self.data.len()
    }
}

#[allow(clippy::derive_ord_xor_partial_ord)]
#[derive(Eq, PartialEq, PartialOrd, Clone, Debug)]
pub struct TTLOrderingKey {
    pub expiration_time: Duration,
    pub address: AccountAddress,
    pub sequence_number: u64,
}

/// Be very careful with this, to not break the partial ordering.
/// See:  https://rust-lang.github.io/rust-clippy/master/index.html#derive_ord_xor_partial_ord
#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for TTLOrderingKey {
    fn cmp(&self, other: &TTLOrderingKey) -> Ordering {
        match self.expiration_time.cmp(&other.expiration_time) {
            Ordering::Equal => {
                (&self.address, self.sequence_number).cmp(&(&other.address, other.sequence_number))
            }
            ordering => ordering,
        }
    }
}

/// TimelineIndex is an ordered log of all transactions that are "ready" for broadcast.
/// We only add a transaction to the index if it has a chance to be included in the next consensus
/// block (which means its status is != NotReady or its sequential to another "ready" transaction).
///
/// It's represented as Map <timeline_id, (Address, sequence_number)>, where timeline_id is auto
/// increment unique id of "ready" transaction in local Mempool. (Address, sequence_number) is a
/// logical reference to transaction content in main storage.
pub struct TimelineIndex {
    timeline_id: u64,
    timeline: BTreeMap<u64, (AccountAddress, u64, Instant)>,
}

impl TimelineIndex {
    pub(crate) fn new() -> Self {
        Self { timeline_id: 1, timeline: BTreeMap::new() }
    }

    /// Read all transactions from the timeline since <timeline_id>.
    /// At most `count` transactions will be returned.
    /// If `before` is set, only transactions inserted before this time will be returned.
    pub(crate) fn read_timeline(
        &self,
        timeline_id: u64,
        count: usize,
        before: Option<Instant>,
    ) -> Vec<(AccountAddress, u64)> {
        let mut batch = vec![];
        for (_id, &(address, sequence_number, insertion_time)) in
            self.timeline.range((Bound::Excluded(timeline_id), Bound::Unbounded))
        {
            if let Some(before) = before {
                if insertion_time >= before {
                    break;
                }
            }
            if batch.len() == count {
                break;
            }
            batch.push((address, sequence_number));
        }
        batch
    }

    /// Read transactions from the timeline from `start_id` (exclusive) to `end_id` (inclusive).
    pub(crate) fn timeline_range(&self, start_id: u64, end_id: u64) -> Vec<(AccountAddress, u64)> {
        self.timeline
            .range((Bound::Excluded(start_id), Bound::Included(end_id)))
            .map(|(_idx, &(address, sequence, _))| (address, sequence))
            .collect()
    }

    pub(crate) fn insert(&mut self, txn: &mut MempoolTransaction) {
        self.timeline.insert(
            self.timeline_id,
            (txn.verified_txn().sender(), txn.verified_txn().sequence_number(), Instant::now()),
        );
        txn.timeline_state = TimelineState::Ready(self.timeline_id);
        self.timeline_id += 1;
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        if let TimelineState::Ready(timeline_id) = txn.timeline_state {
            self.timeline.remove(&timeline_id);
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.timeline.len()
    }
}

/// Logical pointer to `MempoolTransaction`.
/// Includes Account's address and transaction sequence number.
pub type TxnPointer = TransactionSummary;

impl From<&MempoolTransaction> for TxnPointer {
    fn from(txn: &MempoolTransaction) -> Self {
        Self {
            sender: txn.verified_txn().sender(),
            sequence_number: txn.verified_txn().sequence_number(),
            hash: txn.get_hash(),
        }
    }
}

impl From<&OrderedQueueKey> for TxnPointer {
    fn from(key: &OrderedQueueKey) -> Self {
        Self { sender: key.address, sequence_number: key.sequence_number, hash: key.hash }
    }
}

pub struct MultiBucketTimelineIndex {
    timelines: Vec<TimelineIndex>,
    bucket_mins: Vec<u64>,
    bucket_mins_to_string: Vec<String>,
}

impl MultiBucketTimelineIndex {
    pub(crate) fn new(bucket_mins: Vec<u64>) -> anyhow::Result<Self> {
        anyhow::ensure!(!bucket_mins.is_empty(), "Must not be empty");
        anyhow::ensure!(bucket_mins[0] == 0, "First bucket must start at 0");

        let mut prev = None;
        let mut timelines = vec![];
        for entry in bucket_mins.clone() {
            if let Some(prev) = prev {
                anyhow::ensure!(prev < entry, "Values must be sorted and not repeat");
            }
            prev = Some(entry);
            timelines.push(TimelineIndex::new());
        }

        let bucket_mins_to_string: Vec<_> =
            bucket_mins.iter().map(|bucket_min| bucket_min.to_string()).collect();

        Ok(Self { timelines, bucket_mins, bucket_mins_to_string })
    }

    /// Read all transactions from the timeline since <timeline_id>.
    /// At most `count` transactions will be returned.
    pub(crate) fn read_timeline(
        &self,
        timeline_id: &MultiBucketTimelineIndexIds,
        count: usize,
        before: Option<Instant>,
    ) -> Vec<Vec<(AccountAddress, u64)>> {
        assert!(timeline_id.id_per_bucket.len() == self.bucket_mins.len());

        let mut added = 0;
        let mut returned = vec![];
        for (timeline, &timeline_id) in
            self.timelines.iter().zip(timeline_id.id_per_bucket.iter()).rev()
        {
            let txns = timeline.read_timeline(timeline_id, count - added, before);
            added += txns.len();
            returned.push(txns);

            if added == count {
                break;
            }
        }
        while returned.len() < self.timelines.len() {
            returned.push(vec![]);
        }
        returned.iter().rev().cloned().collect()
    }

    /// Read transactions from the timeline from `start_id` (exclusive) to `end_id` (inclusive).
    pub(crate) fn timeline_range(
        &self,
        start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(AccountAddress, u64)> {
        assert_eq!(start_end_pairs.len(), self.timelines.len());

        let mut all_txns = vec![];
        for (timeline_index_identifier, (start_id, end_id)) in start_end_pairs {
            let mut txns = self
                .timelines
                .get(timeline_index_identifier as usize)
                .map_or_else(Vec::new, |timeline| timeline.timeline_range(start_id, end_id));
            all_txns.append(&mut txns);
        }
        all_txns
    }

    #[inline]
    fn get_timeline(&mut self, ranking_score: u64) -> &mut TimelineIndex {
        let index = self.bucket_mins.binary_search(&ranking_score).unwrap_or_else(|i| i - 1);
        self.timelines.get_mut(index).unwrap()
    }

    pub(crate) fn insert(&mut self, txn: &mut MempoolTransaction) {
        self.get_timeline(txn.ranking_score()).insert(txn);
    }

    pub(crate) fn remove(&mut self, txn: &MempoolTransaction) {
        self.get_timeline(txn.ranking_score()).remove(txn);
    }

    pub(crate) fn size(&self) -> usize {
        let mut size = 0;
        for timeline in &self.timelines {
            size += timeline.size()
        }
        size
    }

    pub(crate) fn get_sizes(&self) -> Vec<(&str, usize)> {
        self.bucket_mins_to_string
            .iter()
            .zip(self.timelines.iter())
            .map(|(bucket_min, timeline)| (bucket_min.as_str(), timeline.size()))
            .collect()
    }

    #[inline]
    pub(crate) fn get_bucket(&self, ranking_score: u64) -> &str {
        let index = self.bucket_mins.binary_search(&ranking_score).unwrap_or_else(|i| i - 1);
        self.bucket_mins_to_string[index].as_str()
    }
}
