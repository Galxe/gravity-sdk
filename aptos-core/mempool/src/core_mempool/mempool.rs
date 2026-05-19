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
    collections::{BTreeMap, HashMap},
    ops::Bound,
    sync::Mutex,
    time::{Duration, Instant},
};

use super::transaction::VerifiedTxn;
use block_buffer_manager::TxPool;

/// Per-txn metadata maintained for broadcast scheduling.
struct TxnMeta {
    sender_bucket: MempoolSenderBucket,
    timeline_id: u64,
    insertion_time: Instant,
}

/// In-memory index that mirrors the upstream reth pool's broadcastable set,
/// adding the per-(sender_bucket) ordering that the Aptos broadcast loop
/// expects. Each new txn is assigned a monotonic `timeline_id` within its
/// `sender_bucket` on first observation; entries are GC'd when the txn
/// disappears from the pool.
///
/// This replaces the previous global `TxnCache` (single-generation TTL
/// HashSet), which ignored sender_bucket / count / before / timeline_id and
/// caused one peer to drain the entire pool per tick.
struct BroadcastIndex {
    /// `timelines[sender_bucket]: BTreeMap<timeline_id, TxnHash>`
    timelines: Vec<BTreeMap<u64, TxnHash>>,
    /// Reverse index: txn hash -> metadata. Used for failover-delay lookup
    /// and for GC when the pool drops a txn.
    by_hash: HashMap<TxnHash, TxnMeta>,
    /// Next `timeline_id` to assign per sender_bucket. Starts at 1 so that
    /// the initial state of `id_per_bucket: vec![0; ..]` excludes nothing.
    next_ids: Vec<u64>,
}

impl BroadcastIndex {
    fn new(num_sender_buckets: usize) -> Self {
        Self {
            timelines: vec![BTreeMap::new(); num_sender_buckets],
            by_hash: HashMap::new(),
            next_ids: vec![1; num_sender_buckets],
        }
    }
}

pub struct Mempool {
    pool: Box<dyn TxPool>,
    index: Mutex<BroadcastIndex>,
    num_sender_buckets: MempoolSenderBucket,
}

/// Map a sender address to its `sender_bucket`, matching upstream Aptos
/// (`transaction_store.rs::sender_bucket`): the last byte of the address
/// modulo `num_sender_buckets`.
fn sender_bucket_of(sender: &ExternalAccountAddress, num_sender_buckets: MempoolSenderBucket) -> MempoolSenderBucket {
    let bytes = sender.bytes();
    bytes[bytes.len() - 1] % num_sender_buckets
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

    /// Honors all four routing parameters from upstream Aptos:
    /// - `sender_bucket`: only return txns whose sender hashes into this bucket.
    /// - `timeline_id`: resume from the per-peer cursor (max position taken
    ///   as the single id; we don't subdivide by fee bucket).
    /// - `count`: cap on the returned batch.
    /// - `before`: failover-delay gate; skip txns first observed after this
    ///   instant. Breaks the scan since `timeline_id` is monotonic with
    ///   observation time.
    ///
    /// `priority_of_receiver` is unused — the caller selects `before` based
    /// on it. The returned `MultiBucketTimelineIndexIds` length matches the
    /// input length (so `state.timelines[sender_bucket]` stays well-formed);
    /// we write the single resume id into every position.
    fn read_timeline(
        &self,
        sender_bucket: MempoolSenderBucket,
        timeline_id: &MultiBucketTimelineIndexIds,
        count: usize,
        before: Option<Instant>,
        _priority_of_receiver: BroadcastPeerPriority,
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        let num_fee_buckets = timeline_id.id_per_bucket.len();
        if num_fee_buckets == 0 || count == 0 {
            return (vec![], timeline_id.clone());
        }
        let bucket_idx = sender_bucket as usize;
        if bucket_idx >= self.num_sender_buckets as usize {
            return (
                vec![],
                MultiBucketTimelineIndexIds { id_per_bucket: vec![0; num_fee_buckets] },
            );
        }

        // Snapshot the reth pool's broadcastable set. We pass `None` for the
        // filter so we see every pending txn and can keep our index in sync.
        let snapshot: Vec<gaptos::api_types::VerifiedTxn> =
            self.pool.get_broadcast_txns(None).collect();

        // Build a hash -> txn lookup once for the snapshot, used both for GC
        // detection and for materializing the output.
        let mut snapshot_by_hash: HashMap<TxnHash, gaptos::api_types::VerifiedTxn> =
            HashMap::with_capacity(snapshot.len());
        for txn in snapshot {
            let hash = TxnHash::from_bytes(txn.committed_hash().as_slice());
            snapshot_by_hash.insert(hash, txn);
        }

        let now = Instant::now();
        let num_sender_buckets = self.num_sender_buckets;
        let mut idx = self.index.lock().unwrap();

        // GC entries whose txns are no longer in the pool (committed,
        // expired, or evicted upstream).
        let stale: Vec<TxnHash> = idx
            .by_hash
            .keys()
            .filter(|h| !snapshot_by_hash.contains_key(*h))
            .copied()
            .collect();
        for h in &stale {
            if let Some(meta) = idx.by_hash.remove(h) {
                if let Some(tl) = idx.timelines.get_mut(meta.sender_bucket as usize) {
                    tl.remove(&meta.timeline_id);
                }
            }
        }

        // Assign a timeline_id to txns we've never seen, regardless of which
        // sender_bucket this call targets — other ticks will ask about the
        // remaining buckets.
        for (hash, txn) in &snapshot_by_hash {
            if idx.by_hash.contains_key(hash) {
                continue;
            }
            let sb = sender_bucket_of(txn.sender(), num_sender_buckets);
            let sb_idx = sb as usize;
            if sb_idx >= idx.timelines.len() {
                // Defensive: skip if config indicates more buckets than we
                // allocated. Should not happen since both come from the same
                // config at Mempool construction time.
                continue;
            }
            let tid = idx.next_ids[sb_idx];
            idx.next_ids[sb_idx] = idx.next_ids[sb_idx].saturating_add(1);
            idx.timelines[sb_idx].insert(tid, *hash);
            idx.by_hash.insert(
                *hash,
                TxnMeta { sender_bucket: sb, timeline_id: tid, insertion_time: now },
            );
        }

        // Resume from the max position in the input. We don't subdivide by
        // fee bucket internally, so all positions encode the same id; taking
        // max is robust even if a caller hands us heterogenous values.
        let resume_id = *timeline_id.id_per_bucket.iter().max().unwrap_or(&0);
        let mut last_seen_id = resume_id;
        let mut output_txns: Vec<(SignedTransaction, u64)> = Vec::new();

        for (&tid, hash) in idx.timelines[bucket_idx]
            .range((Bound::Excluded(resume_id), Bound::Unbounded))
        {
            if output_txns.len() >= count {
                break;
            }
            let meta = match idx.by_hash.get(hash) {
                Some(m) => m,
                None => continue,
            };
            if let Some(before) = before {
                if meta.insertion_time >= before {
                    // timeline_id is monotonic with insertion_time, so every
                    // subsequent entry is also too fresh; bail out.
                    break;
                }
            }
            let verified_gaptos = match snapshot_by_hash.get(hash) {
                Some(t) => t,
                None => continue,
            };
            let local: VerifiedTxn = verified_gaptos.clone().into();
            let signed: SignedTransaction = local.into();
            // ready_time (second tuple element) is left as 0, matching the
            // previous behavior. Wiring it to the txn's real insertion time
            // is a follow-up — needs SystemTime tracking, not just Instant.
            output_txns.push((signed, 0));
            last_seen_id = tid;
        }

        let new_ids = MultiBucketTimelineIndexIds {
            id_per_bucket: vec![last_seen_id; num_fee_buckets],
        };
        (output_txns, new_ids)
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
    pub fn new(config: &NodeConfig, pool: Box<dyn TxPool>) -> Self {
        let num_sender_buckets = config.mempool.num_sender_buckets.max(1);
        Self {
            pool,
            index: Mutex::new(BroadcastIndex::new(num_sender_buckets as usize)),
            num_sender_buckets,
        }
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
    //! Tests for the broadcast index. They exercise `BroadcastIndex` directly
    //! rather than going through `Mempool::read_timeline`, because the latter
    //! needs a `TxPool` (which requires the full reth pool wiring). The end-
    //! to-end behavior is covered by integration tests in the broadcast loop.

    use super::*;

    fn hash_of(id: u8) -> TxnHash {
        TxnHash::from_bytes(&[id; 32])
    }

    #[test]
    fn sender_bucket_takes_last_byte_mod_n() {
        let mut addr = [0u8; 32];
        addr[31] = 7;
        let a = ExternalAccountAddress::new(addr);
        assert_eq!(sender_bucket_of(&a, 4), 3);
        assert_eq!(sender_bucket_of(&a, 8), 7);
        assert_eq!(sender_bucket_of(&a, 1), 0);
    }

    #[test]
    fn broadcast_index_partitions_by_sender_bucket() {
        // Two txns landing in different sender buckets should advance
        // independent timeline_ids.
        let mut idx = BroadcastIndex::new(4);
        let now = Instant::now();
        let h1 = hash_of(1);
        let h2 = hash_of(2);
        idx.timelines[0].insert(1, h1);
        idx.by_hash.insert(h1, TxnMeta { sender_bucket: 0, timeline_id: 1, insertion_time: now });
        idx.next_ids[0] = 2;
        idx.timelines[2].insert(1, h2);
        idx.by_hash.insert(h2, TxnMeta { sender_bucket: 2, timeline_id: 1, insertion_time: now });
        idx.next_ids[2] = 2;

        assert_eq!(idx.timelines[0].len(), 1);
        assert_eq!(idx.timelines[1].len(), 0);
        assert_eq!(idx.timelines[2].len(), 1);
        assert_eq!(idx.timelines[3].len(), 0);
    }

    #[test]
    fn range_query_respects_resume_id() {
        // BTreeMap::range with Excluded lower-bound is what makes the resume
        // semantics work — guard against accidental Bound::Included changes.
        let mut idx = BroadcastIndex::new(1);
        let now = Instant::now();
        for i in 1u64..=5 {
            let h = hash_of(i as u8);
            idx.timelines[0].insert(i, h);
            idx.by_hash.insert(h, TxnMeta { sender_bucket: 0, timeline_id: i, insertion_time: now });
        }
        let after_two: Vec<u64> = idx.timelines[0]
            .range((Bound::Excluded(2u64), Bound::Unbounded))
            .map(|(&k, _)| k)
            .collect();
        assert_eq!(after_two, vec![3, 4, 5]);
    }

    #[test]
    fn failover_before_filter_breaks_scan() {
        // Once we hit a txn that's too fresh, the whole tail is too fresh
        // (timeline_ids are assigned in arrival order). The scan must break,
        // not continue — otherwise `last_seen_id` would advance past txns
        // that should still be re-presented on the next tick.
        let mut idx = BroadcastIndex::new(1);
        let t0 = Instant::now() - Duration::from_secs(10);
        let t_recent = Instant::now();
        idx.timelines[0].insert(1, hash_of(1));
        idx.by_hash.insert(hash_of(1), TxnMeta { sender_bucket: 0, timeline_id: 1, insertion_time: t0 });
        idx.timelines[0].insert(2, hash_of(2));
        idx.by_hash.insert(hash_of(2), TxnMeta { sender_bucket: 0, timeline_id: 2, insertion_time: t_recent });
        idx.timelines[0].insert(3, hash_of(3));
        idx.by_hash.insert(hash_of(3), TxnMeta { sender_bucket: 0, timeline_id: 3, insertion_time: t_recent });

        let before = Instant::now() - Duration::from_secs(1);
        let mut last_seen = 0u64;
        let mut out = vec![];
        for (&tid, _h) in idx.timelines[0].range((Bound::Excluded(0u64), Bound::Unbounded)) {
            let meta = idx.by_hash.get(&hash_of(tid as u8)).unwrap();
            if meta.insertion_time >= before {
                break;
            }
            out.push(tid);
            last_seen = tid;
        }
        assert_eq!(out, vec![1]);
        assert_eq!(last_seen, 1);
    }
}
