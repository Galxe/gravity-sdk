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
    ops::Bound::{Excluded, Unbounded},
    sync::Mutex,
    time::{Duration, Instant},
};

use super::transaction::VerifiedTxn;
use block_buffer_manager::TxPool;

fn sender_to_bucket(
    sender: &ExternalAccountAddress,
    num_sender_buckets: u8,
) -> MempoolSenderBucket {
    let bytes = sender.bytes();
    let n = num_sender_buckets.max(1);
    bytes[31] % n
}

/// Per-`sender_bucket` monotonic timeline of broadcastable txn hashes.
struct TimelineIndex {
    /// Next id to allocate; starts at 1 and never rewinds.
    next_id: u64,
    entries: BTreeMap<u64, (TxnHash, Instant)>,
}

impl TimelineIndex {
    fn new() -> Self {
        Self { next_id: 1, entries: BTreeMap::new() }
    }
}

/// Poll-reconcile broadcast index: `get_broadcast_txns` is ground truth;
/// each pending hash is admitted once into a per-sender_bucket timeline.
struct BroadcastIndex {
    bodies: HashMap<TxnHash, SignedTransaction>,
    timelines: HashMap<MempoolSenderBucket, TimelineIndex>,
    hash_to_pos: HashMap<TxnHash, (MempoolSenderBucket, u64)>,
    last_refresh: Instant,
    max_age: Duration,
    initialized: bool,
}

impl BroadcastIndex {
    fn new(max_age: Duration) -> Self {
        Self {
            bodies: HashMap::new(),
            timelines: HashMap::new(),
            hash_to_pos: HashMap::new(),
            last_refresh: Instant::now(),
            max_age,
            initialized: false,
        }
    }
}

pub struct Mempool {
    pool: Box<dyn TxPool>,
    /// Interior mutability for `&self` trait methods (read_timeline / range).
    index: Mutex<BroadcastIndex>,
    num_sender_buckets: u8,
    /// Fee/ranking bucket count for cursor length (`broadcast_buckets.len()`).
    /// Logic in v1 only uses fee slot 0; length must still match gaptos.
    num_fee_slots: usize,
}

impl CoreMempoolTrait for Mempool {
    fn timeline_range(
        &self,
        _sender_bucket: MempoolSenderBucket,
        _start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        // Task 3 will implement real range; reconcile so index stays warm.
        self.maybe_reconcile(false);
        vec![]
    }

    fn timeline_range_of_message(
        &self,
        _sender_start_end_pairs: HashMap<
            MempoolSenderBucket,
            HashMap<TimelineIndexIdentifier, (u64, u64)>,
        >,
    ) -> Vec<(SignedTransaction, u64)> {
        self.maybe_reconcile(false);
        vec![]
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
        _priority_of_receiver: BroadcastPeerPriority, // no content filter (upstream parity)
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        self.maybe_reconcile(false);
        let idx = self.index.lock().unwrap();

        let cursor0 = timeline_id.id_per_bucket.first().copied().unwrap_or(0);
        let mut out = Vec::new();
        let mut last_included = None;

        let Some(tl) = idx.timelines.get(&sender_bucket) else {
            return (out, self.cursor_from(cursor0, last_included));
        };

        for (&id, (hash, admit_at)) in tl.entries.range((Excluded(cursor0), Unbounded)) {
            // Failover before: stop when admit Instant is too new; later ids are newer.
            if let Some(t) = before {
                if *admit_at >= t {
                    break;
                }
            }
            // At most `count` successful body joins.
            if out.len() >= count {
                break;
            }
            let Some(txn) = idx.bodies.get(hash) else {
                continue;
            };
            out.push((txn.clone(), 0)); // ready_time_ms = 0
            last_included = Some(id);
        }

        (out, self.cursor_from(cursor0, last_included))
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
        let max_age = Duration::from_millis(
            std::env::var("MEMPOOL_SNAPSHOT_MAX_AGE_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(20),
        );
        let num_sender_buckets = config.mempool.num_sender_buckets.max(1);
        let num_fee_slots = config.mempool.broadcast_buckets.len().max(1);

        Self {
            pool,
            index: Mutex::new(BroadcastIndex::new(max_age)),
            num_sender_buckets,
            num_fee_slots,
        }
    }

    /// Build fee-slot-shaped cursor: progress only in slot 0; rest stay 0.
    /// Empty batch keeps `cursor0` (does not advance).
    fn cursor_from(&self, cursor0: u64, last: Option<u64>) -> MultiBucketTimelineIndexIds {
        let mut id_per_bucket = vec![0u64; self.num_fee_slots];
        id_per_bucket[0] = last.unwrap_or(cursor0);
        MultiBucketTimelineIndexIds { id_per_bucket }
    }

    /// Throttled reconcile against `pool.get_broadcast_txns`.
    /// `force=true` ignores `max_age` (used by tests and any urgent refresh).
    fn maybe_reconcile(&self, force: bool) {
        let mut idx = self.index.lock().unwrap();
        if !force && idx.initialized && idx.last_refresh.elapsed() < idx.max_age {
            return;
        }
        self.reconcile_locked(&mut idx);
    }

    /// Design §4: remove left hashes, admit new in `get_broadcast_txns` order.
    fn reconcile_locked(&self, idx: &mut BroadcastIndex) {
        let pending: Vec<_> = self.pool.get_broadcast_txns(None).collect();
        let mut pending_hashes: HashSet<TxnHash> = HashSet::with_capacity(pending.len());
        // Materialize (hash, bucket, signed) while preserving iteration order for admit.
        let mut pending_pairs: Vec<(TxnHash, MempoolSenderBucket, SignedTransaction)> =
            Vec::with_capacity(pending.len());
        for txn in pending {
            let hash = TxnHash::from_bytes(txn.committed_hash().as_slice());
            let bucket = sender_to_bucket(txn.sender(), self.num_sender_buckets);
            pending_hashes.insert(hash);
            let signed: SignedTransaction = VerifiedTxn::from(txn).into();
            pending_pairs.push((hash, bucket, signed));
        }

        // --- remove hashes no longer in pending ---
        let to_remove: Vec<TxnHash> =
            idx.bodies.keys().filter(|h| !pending_hashes.contains(h)).copied().collect();
        for h in to_remove {
            if let Some((bucket, id)) = idx.hash_to_pos.remove(&h) {
                if let Some(timeline) = idx.timelines.get_mut(&bucket) {
                    timeline.entries.remove(&id);
                }
            }
            idx.bodies.remove(&h);
        }

        // --- admit new hashes in iteration order ---
        for (hash, bucket, signed) in pending_pairs {
            if idx.bodies.contains_key(&hash) {
                // Still present: optional body overwrite; timeline id/Instant stay.
                idx.bodies.insert(hash, signed);
                continue;
            }
            let timeline = idx.timelines.entry(bucket).or_insert_with(TimelineIndex::new);
            let id = timeline.next_id;
            timeline.next_id = timeline.next_id.saturating_add(1);
            timeline.entries.insert(id, (hash, Instant::now()));
            idx.hash_to_pos.insert(hash, (bucket, id));
            idx.bodies.insert(hash, signed);
        }

        idx.initialized = true;
        idx.last_refresh = Instant::now();
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
        let mut total_bytes: u64 = 0;
        let best_txns = self.pool.best_txns(Some(filter), max_txns as usize, max_bytes);
        for txn in best_txns {
            let signed_txn: SignedTransaction = VerifiedTxn::from(txn).into();
            let txn_bytes = signed_txn.raw_txn_bytes_len() as u64;
            // Authoritatively enforce the byte budget so proposers never build a
            // payload that exceeds max_receiving_block_bytes, which receivers reject
            // in round_manager::process_proposal. max_bytes may already have been
            // reduced by validator transactions (MixedPayloadClient), so it can be
            // smaller than a single txn; in that case we return fewer (possibly zero)
            // user txns this round rather than overflow the block — the remainder is
            // pulled in a later round.
            if total_bytes + txn_bytes > max_bytes {
                break;
            }
            total_bytes += txn_bytes;
            transactions.push(signed_txn);
            if transactions.len() >= max_txns as usize {
                break;
            }
        }
        transactions
    }

    pub fn gen_snapshot(&self) -> Vec<SignedTransaction> {
        panic!()
    }

    // --- test-only helpers (Task 1 reconcile inspection) ---

    #[cfg(test)]
    fn force_reconcile_for_test(&self) {
        self.maybe_reconcile(true);
    }

    #[cfg(test)]
    fn debug_timeline_len(&self, bucket: MempoolSenderBucket) -> usize {
        let idx = self.index.lock().unwrap();
        idx.timelines.get(&bucket).map(|t| t.entries.len()).unwrap_or(0)
    }

    #[cfg(test)]
    fn debug_next_id(&self, bucket: MempoolSenderBucket) -> u64 {
        let idx = self.index.lock().unwrap();
        idx.timelines.get(&bucket).map(|t| t.next_id).unwrap_or(1)
    }

    #[cfg(test)]
    fn debug_only_id(&self, bucket: MempoolSenderBucket) -> u64 {
        let idx = self.index.lock().unwrap();
        let t = idx.timelines.get(&bucket).expect("timeline for bucket");
        assert_eq!(t.entries.len(), 1, "debug_only_id requires exactly one entry");
        *t.entries.keys().next().unwrap()
    }

    #[cfg(test)]
    fn debug_bodies_len(&self) -> usize {
        self.index.lock().unwrap().bodies.len()
    }

    /// Admit Instant for the broadcast body whose sequence number equals `nonce`.
    /// Panics if no matching body is currently indexed (test helper only).
    #[cfg(test)]
    fn debug_admit_instant_for_nonce(&self, nonce: u64) -> Instant {
        let idx = self.index.lock().unwrap();
        for (hash, txn) in &idx.bodies {
            if txn.sequence_number() == nonce {
                let (bucket, id) = idx.hash_to_pos.get(hash).expect("hash_to_pos entry for body");
                let (_h, admit_at) = idx
                    .timelines
                    .get(bucket)
                    .and_then(|t| t.entries.get(id))
                    .expect("timeline entry for body");
                return *admit_at;
            }
        }
        panic!("no indexed body with sequence_number={nonce}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaptos::api_types::{
        account::ExternalChainId, VerifiedTxn as ApiVerifiedTxn, GLOBAL_CRYPTO_TXN_HASHER,
    };
    use std::sync::{Arc, Mutex as StdMutex};

    fn install_hasher() {
        // Identity-ish hasher for tests: hash = first 32 bytes of payload,
        // zero-padded. Sufficient to produce distinct hashes for our tests.
        let _ = GLOBAL_CRYPTO_TXN_HASHER.set(Box::new(|bytes: &Vec<u8>| {
            let mut out = [0u8; 32];
            for (i, b) in bytes.iter().take(32).enumerate() {
                out[i] = *b;
            }
            out
        }));
    }

    fn mk_addr(last_byte: u8) -> ExternalAccountAddress {
        let mut a = [0u8; 32];
        a[31] = last_byte;
        ExternalAccountAddress::new(a)
    }

    fn mk_txn(addr_last: u8, seq: u64, body_seed: u8) -> ApiVerifiedTxn {
        // Distinct body_seed values produce distinct hashes via install_hasher().
        let bytes = vec![body_seed; 32];
        ApiVerifiedTxn::new(bytes, mk_addr(addr_last), seq, ExternalChainId::new(1))
    }

    /// Test constructor: `(txns, max_age, num_sender_buckets, num_fee_slots)`.
    fn mempool_with(
        txns: Arc<StdMutex<Vec<ApiVerifiedTxn>>>,
        max_age: Duration,
        num_buckets: u8,
        fee_slots: usize,
    ) -> Mempool {
        install_hasher();
        struct Shared(Arc<StdMutex<Vec<ApiVerifiedTxn>>>);
        impl TxPool for Shared {
            fn best_txns(
                &self,
                _f: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
                _l: usize,
                _max_bytes: u64,
            ) -> Box<dyn Iterator<Item = ApiVerifiedTxn>> {
                Box::new(std::iter::empty())
            }
            fn get_broadcast_txns(
                &self,
                _f: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
            ) -> Box<dyn Iterator<Item = ApiVerifiedTxn>> {
                Box::new(self.0.lock().unwrap().clone().into_iter())
            }
            fn add_external_txn(&self, _t: ApiVerifiedTxn) -> bool {
                false
            }
            fn remove_txns(&self, _t: Vec<ApiVerifiedTxn>) {}
        }
        Mempool {
            pool: Box::new(Shared(txns)),
            index: Mutex::new(BroadcastIndex::new(max_age)),
            num_sender_buckets: num_buckets.max(1),
            num_fee_slots: fee_slots.max(1),
        }
    }

    #[test]
    fn reconcile_admits_new_hashes_monotonic_ids() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1), mk_txn(0, 1, 2)]));
        let m = mempool_with(txns, Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        assert_eq!(m.debug_timeline_len(0), 2);
        assert_eq!(m.debug_next_id(0), 3);
    }

    #[test]
    fn reconcile_removes_left_hashes() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)]));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        assert_eq!(m.debug_timeline_len(0), 1);
        txns.lock().unwrap().clear();
        m.force_reconcile_for_test();
        assert_eq!(m.debug_timeline_len(0), 0);
        assert_eq!(m.debug_bodies_len(), 0);
    }

    #[test]
    fn reconcile_stable_id_while_hash_stays() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)]));
        let m = mempool_with(txns, Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        let id1 = m.debug_only_id(0);
        m.force_reconcile_for_test();
        assert_eq!(m.debug_only_id(0), id1);
        assert_eq!(m.debug_next_id(0), id1 + 1);
    }

    #[test]
    fn reconcile_reenter_gets_new_id() {
        let t = mk_txn(0, 0, 1);
        let txns = Arc::new(StdMutex::new(vec![t.clone()]));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        let id1 = m.debug_only_id(0);
        txns.lock().unwrap().clear();
        m.force_reconcile_for_test();
        txns.lock().unwrap().push(t);
        m.force_reconcile_for_test();
        let id2 = m.debug_only_id(0);
        assert!(id2 > id1);
    }

    fn empty_cursor(fee_slots: usize) -> MultiBucketTimelineIndexIds {
        MultiBucketTimelineIndexIds { id_per_bucket: vec![0; fee_slots] }
    }

    #[test]
    fn read_timeline_returns_fee_slot_shaped_cursor() {
        let m = mempool_with(Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)])), Duration::ZERO, 1, 10);
        let (out, cur) =
            m.read_timeline(0, &empty_cursor(10), 16, None, BroadcastPeerPriority::Primary);
        assert_eq!(out.len(), 1);
        assert_eq!(cur.id_per_bucket.len(), 10);
        assert_eq!(cur.id_per_bucket[0], 1);
        assert!(cur.id_per_bucket[1..].iter().all(|&x| x == 0));
        assert_eq!(out[0].1, 0); // ready_time_ms
    }

    #[test]
    fn read_timeline_respects_cursor_incremental() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1), mk_txn(0, 1, 2), mk_txn(0, 2, 3)]));
        let m = mempool_with(txns, Duration::ZERO, 1, 10);
        let (out1, c1) =
            m.read_timeline(0, &empty_cursor(10), 2, None, BroadcastPeerPriority::Primary);
        assert_eq!(out1.len(), 2);
        assert_eq!(c1.id_per_bucket[0], 2);
        let (out2, c2) = m.read_timeline(0, &c1, 16, None, BroadcastPeerPriority::Primary);
        assert_eq!(out2.len(), 1);
        assert_eq!(c2.id_per_bucket[0], 3);
    }

    #[test]
    fn read_timeline_count_truncation() {
        let txns = Arc::new(StdMutex::new((0..5).map(|n| mk_txn(0, n, 50 + n as u8)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, 10);
        let (out, c) =
            m.read_timeline(0, &empty_cursor(10), 3, None, BroadcastPeerPriority::Primary);
        assert_eq!(out.len(), 3);
        assert_eq!(c.id_per_bucket[0], 3);
        let (rest, c2) = m.read_timeline(0, &c, 16, None, BroadcastPeerPriority::Primary);
        assert_eq!(rest.len(), 2);
        assert_eq!(c2.id_per_bucket[0], 5);
    }

    #[test]
    fn read_timeline_empty_batch_does_not_advance() {
        let m = mempool_with(Arc::new(StdMutex::new(vec![])), Duration::ZERO, 1, 10);
        let old = MultiBucketTimelineIndexIds { id_per_bucket: vec![7, 0, 0, 0, 0, 0, 0, 0, 0, 0] };
        let (out, cur) = m.read_timeline(0, &old, 16, None, BroadcastPeerPriority::Primary);
        assert!(out.is_empty());
        assert_eq!(cur.id_per_bucket[0], 7);
        assert_eq!(cur.id_per_bucket.len(), 10);
    }

    #[test]
    fn read_timeline_before_filters_new_admits() {
        // 1) Admit A alone.
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)]));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        // 2) Sleep so B's admit Instant is strictly later than A's.
        std::thread::sleep(Duration::from_millis(5));
        // 3) Admit B.
        txns.lock().unwrap().push(mk_txn(0, 1, 2));
        m.force_reconcile_for_test();
        // 4) before = B's admit Instant → range breaks on Instant >= before, so B excluded.
        let b_instant = m.debug_admit_instant_for_nonce(1);
        let (out, _) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(b_instant),
            BroadcastPeerPriority::Primary,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0.sequence_number(), 0);
    }

    #[test]
    fn read_timeline_bucket_isolation() {
        let txns = Arc::new(StdMutex::new(vec![
            mk_txn(0, 0, 10),
            mk_txn(1, 0, 11),
            mk_txn(2, 0, 12),
            mk_txn(3, 0, 13),
        ]));
        let m = mempool_with(txns, Duration::ZERO, 4, 10);
        for k in 0u8..4 {
            let (out, _) =
                m.read_timeline(k, &empty_cursor(10), 16, None, BroadcastPeerPriority::Primary);
            assert_eq!(out.len(), 1, "bucket {k}");
        }
    }

    // A TxPool that hands back a fixed set of txns, honoring the `limit` argument
    // (like the real reth pool) so get_batch_inner's own capping can be exercised.
    fn batch_mempool(txns: Vec<ApiVerifiedTxn>) -> Mempool {
        install_hasher();
        struct BatchPool(Vec<ApiVerifiedTxn>);
        impl TxPool for BatchPool {
            fn best_txns(
                &self,
                _f: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
                l: usize,
                _max_bytes: u64,
            ) -> Box<dyn Iterator<Item = ApiVerifiedTxn>> {
                // Ignore max_bytes here (it's a prefetch hint); return all
                // candidates so get_batch_inner's authoritative cap is exercised.
                Box::new(self.0.clone().into_iter().take(l))
            }
            fn get_broadcast_txns(
                &self,
                _f: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
            ) -> Box<dyn Iterator<Item = ApiVerifiedTxn>> {
                Box::new(std::iter::empty())
            }
            fn add_external_txn(&self, _t: ApiVerifiedTxn) -> bool {
                false
            }
            fn remove_txns(&self, _t: Vec<ApiVerifiedTxn>) {}
        }
        Mempool {
            pool: Box::new(BatchPool(txns)),
            index: Mutex::new(BroadcastIndex::new(Duration::from_millis(20))),
            num_sender_buckets: 1,
            num_fee_slots: 10,
        }
    }

    #[test]
    fn get_batch_enforces_byte_budget() {
        let txns: Vec<_> = (0..10u8).map(|i| mk_txn(0, i as u64, i + 40)).collect();
        let m = batch_mempool(txns);

        // Baseline: with generous limits all txns come back; every txn here has an
        // identical serialized size, so per_txn is a clean unit for the budget math.
        let all = m.get_batch_inner(100, u64::MAX, true, BTreeMap::new());
        assert_eq!(all.len(), 10);
        let per_txn = all[0].raw_txn_bytes_len() as u64;
        assert!(per_txn > 0);

        // A byte budget sized for exactly 3 txns must cap the batch at 3 — this is
        // the behavior that was dead code before (max_bytes compared to txn count).
        let batch = m.get_batch_inner(100, per_txn * 3, true, BTreeMap::new());
        assert_eq!(batch.len(), 3, "byte budget must cap the batch");

        // max_txns still caps independently of the byte budget.
        let capped = m.get_batch_inner(2, u64::MAX, true, BTreeMap::new());
        assert_eq!(capped.len(), 2, "max_txns must still cap the batch");

        // When the remaining budget is too small for even one txn (e.g. validator
        // txns already consumed most of the block via MixedPayloadClient), no user
        // txn is admitted — the batch must never overflow the byte budget.
        let tiny = m.get_batch_inner(100, 1, true, BTreeMap::new());
        assert!(tiny.is_empty(), "a txn exceeding the budget must not be admitted");
    }
}
