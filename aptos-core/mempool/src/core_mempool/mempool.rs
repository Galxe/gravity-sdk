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
    ops::Bound::{Excluded, Included, Unbounded},
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

/// Ordered log of broadcast-ready transactions for **one** `sender_bucket`.
///
/// Mirrors aptos `core_mempool::index::TimelineIndex`:
/// - aptos value: `(AccountAddress, seq, Instant)` pointing into the main table
/// - gravity value: `(TxnHash, Instant)` — body is looked up in [`TransactionStore::transactions`]
///   by hash (reth has no (addr, seq) main table)
///
/// `timeline_id` is a per-index monotonic counter starting at 1 (peer cursors start at 0).
/// The `Instant` is admit time into this log (Failover `before` filter only).
struct TimelineIndex {
    /// Next `timeline_id` to allocate on insert. Aptos field name: `timeline_id`.
    /// Starts at 1; never rewinds. Not the peer cursor (cursors are exclusive lower bounds).
    timeline_id: u64,
    /// Ordered log: `timeline_id` → `(txn hash, admit Instant)`.
    /// Aptos field name: `timeline`. Range reads use
    /// `(Excluded(cursor), Unbounded)` / `(Excluded(start), Included(end))`.
    timeline: BTreeMap<u64, (TxnHash, Instant)>,
}

impl TimelineIndex {
    fn new() -> Self {
        Self { timeline_id: 1, timeline: BTreeMap::new() }
    }
}

/// In-memory broadcast store: body table + timeline indexes + reverse hash index.
///
/// Mirrors the **broadcast-relevant** parts of aptos `TransactionStore`
/// (`transactions`, `timeline_index`, `hash_index`). Gravity does **not** host
/// parking-lot / priority / system-TTL indexes here — those live in reth pool.
///
/// Ground truth for membership is still `TxPool::get_broadcast_txns`; this store
/// is a poll-reconciled projection used by `read_timeline` / `timeline_range*`.
struct TransactionStore {
    /// Main body table keyed by committed txn hash.
    ///
    /// Aptos: `transactions: HashMap<AccountAddress, BTreeMap<seq, MempoolTransaction>>`
    /// (body + metadata under (sender, seq)).
    /// Gravity: reth owns canonical pool state; we only cache `SignedTransaction`
    /// by hash so range/read can materialize without another pool lookup.
    /// After each reconcile, the key set equals the current broadcastable set.
    transactions: HashMap<TxnHash, SignedTransaction>,

    /// Per-`sender_bucket` broadcast timelines.
    ///
    /// Aptos: `timeline_index: HashMap<MempoolSenderBucket, MultiBucketTimelineIndex>`
    /// (each sender bucket holds fee/ranking sub-timelines).
    /// Gravity v1: one [`TimelineIndex`] per sender bucket (no fee MultiBucket);
    /// returned peer cursors still have length `broadcast_buckets.len()` with
    /// real progress only in fee slot 0 (gaptos MessageId / PeerSyncState contract).
    timeline_index: HashMap<MempoolSenderBucket, TimelineIndex>,

    /// Reverse index: committed hash → `(sender_bucket, timeline_id)`.
    ///
    /// Aptos: `hash_index: HashMap<HashValue, (AccountAddress, seq)>` for main-table
    /// lookup; timeline id lives on `MempoolTransaction.timeline_state = Ready(id)`.
    /// Gravity: timeline entries are pointer-only `(TxnHash, Instant)`, so GC on
    /// leave needs this map for O(1) `timeline.remove(id)` without scanning the log.
    hash_index: HashMap<TxnHash, (MempoolSenderBucket, u64)>,

    /// Time of the last successful reconcile (poll against `get_broadcast_txns`).
    /// No aptos twin — aptos admits on insert/commit; we throttle full-pool polls.
    last_reconcile: Instant,

    /// Max age of a reconcile before `maybe_reconcile(false)` refreshes.
    /// Driven by `MEMPOOL_SNAPSHOT_MAX_AGE_MS` (default 20ms). Aptos has no
    /// equivalent poll interval on the timeline path.
    reconcile_max_age: Duration,

    /// `false` until the first reconcile completes.
    /// Distinguishes "never projected" from "projected empty pool".
    reconciled: bool,
}

impl TransactionStore {
    fn new(reconcile_max_age: Duration) -> Self {
        Self {
            transactions: HashMap::new(),
            timeline_index: HashMap::new(),
            hash_index: HashMap::new(),
            last_reconcile: Instant::now(),
            reconcile_max_age,
            reconciled: false,
        }
    }
}

pub struct Mempool {
    /// Reth-backed pool: packing (`best_txns`) and broadcast ground truth
    /// (`get_broadcast_txns`). Aptos has no separate trait — body lives in store.
    pool: Box<dyn TxPool>,
    /// Broadcast projection (`TransactionStore`), under mutex because
    /// `CoreMempoolTrait::{read_timeline,timeline_range*}` take `&self` and must
    /// mutate. Aptos: `transactions: TransactionStore` with `&mut self` APIs.
    transactions: Mutex<TransactionStore>,
    /// Number of sender-address buckets (`addr last byte % n`). Aptos: same
    /// field on `TransactionStore` / mempool config `num_sender_buckets`.
    num_sender_buckets: u8,
    /// Length of returned `MultiBucketTimelineIndexIds.id_per_bucket`.
    /// Equals `config.mempool.broadcast_buckets.len()` (default 10). Aptos uses
    /// this many fee sub-timelines; Gravity v1 only advances fee slot 0 but must
    /// still emit this length so gaptos `PeerSyncState::update` is not a no-op.
    num_fee_slots: usize,
}

impl CoreMempoolTrait for Mempool {
    fn timeline_range(
        &self,
        sender_bucket: MempoolSenderBucket,
        start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        self.maybe_reconcile(false);
        let store = self.transactions.lock().unwrap();
        Self::timeline_range_with_store(&store, sender_bucket, start_end_pairs)
    }

    fn timeline_range_of_message(
        &self,
        sender_start_end_pairs: HashMap<
            MempoolSenderBucket,
            HashMap<TimelineIndexIdentifier, (u64, u64)>,
        >,
    ) -> Vec<(SignedTransaction, u64)> {
        // Lock once; do not call timeline_range (std Mutex is not reentrant).
        self.maybe_reconcile(false);
        let store = self.transactions.lock().unwrap();
        let mut out = Vec::new();
        for (bucket, pairs) in sender_start_end_pairs {
            out.extend(Self::timeline_range_with_store(&store, bucket, pairs));
        }
        out
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
        let store = self.transactions.lock().unwrap();

        let cursor0 = timeline_id.id_per_bucket.first().copied().unwrap_or(0);
        let mut out = Vec::new();
        let mut last_included = None;

        let Some(tl) = store.timeline_index.get(&sender_bucket) else {
            return (out, self.cursor_from(cursor0, last_included));
        };

        for (&id, (hash, admit_at)) in tl.timeline.range((Excluded(cursor0), Unbounded)) {
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
            let Some(txn) = store.transactions.get(hash) else {
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
            transactions: Mutex::new(TransactionStore::new(max_age)),
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

    /// Materialize `(Excluded(start), Included(end))` for fee slot 0 only.
    /// Takes `&TransactionStore` so callers can lock once (std `Mutex` is not reentrant).
    fn timeline_range_with_store(
        store: &TransactionStore,
        sender_bucket: MempoolSenderBucket,
        start_end_pairs: HashMap<TimelineIndexIdentifier, (u64, u64)>,
    ) -> Vec<(SignedTransaction, u64)> {
        // Only fee slot 0 is used in v1; ignore other keys. Missing key → empty window.
        let (start, end) = start_end_pairs.get(&0).copied().unwrap_or((0, 0));
        let Some(tl) = store.timeline_index.get(&sender_bucket) else {
            return vec![];
        };
        let mut out = Vec::new();
        for (_id, (hash, _)) in tl.timeline.range((Excluded(start), Included(end))) {
            if let Some(txn) = store.transactions.get(hash) {
                out.push((txn.clone(), 0)); // ready_time_ms = 0
            }
        }
        out
    }

    /// Throttled reconcile against `pool.get_broadcast_txns`.
    /// `force=true` ignores `reconcile_max_age` (tests / urgent refresh).
    fn maybe_reconcile(&self, force: bool) {
        let mut store = self.transactions.lock().unwrap();
        if !force && store.reconciled && store.last_reconcile.elapsed() < store.reconcile_max_age {
            return;
        }
        self.reconcile_locked(&mut store);
    }

    /// Remove left hashes, admit new in `get_broadcast_txns` iteration order.
    fn reconcile_locked(&self, store: &mut TransactionStore) {
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
            store.transactions.keys().filter(|h| !pending_hashes.contains(h)).copied().collect();
        for h in to_remove {
            if let Some((bucket, id)) = store.hash_index.remove(&h) {
                if let Some(tl) = store.timeline_index.get_mut(&bucket) {
                    tl.timeline.remove(&id);
                }
            }
            store.transactions.remove(&h);
        }

        // --- admit new hashes in iteration order ---
        for (hash, bucket, signed) in pending_pairs {
            if store.transactions.contains_key(&hash) {
                // Still present: optional body overwrite; timeline id/Instant stay.
                store.transactions.insert(hash, signed);
                continue;
            }
            let tl = store.timeline_index.entry(bucket).or_insert_with(TimelineIndex::new);
            let id = tl.timeline_id;
            tl.timeline_id = tl.timeline_id.saturating_add(1);
            tl.timeline.insert(id, (hash, Instant::now()));
            store.hash_index.insert(hash, (bucket, id));
            store.transactions.insert(hash, signed);
        }

        store.reconciled = true;
        store.last_reconcile = Instant::now();
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
        let store = self.transactions.lock().unwrap();
        store.timeline_index.get(&bucket).map(|t| t.timeline.len()).unwrap_or(0)
    }

    #[cfg(test)]
    fn debug_next_id(&self, bucket: MempoolSenderBucket) -> u64 {
        let store = self.transactions.lock().unwrap();
        store.timeline_index.get(&bucket).map(|t| t.timeline_id).unwrap_or(1)
    }

    #[cfg(test)]
    fn debug_only_id(&self, bucket: MempoolSenderBucket) -> u64 {
        let store = self.transactions.lock().unwrap();
        let t = store.timeline_index.get(&bucket).expect("timeline for bucket");
        assert_eq!(t.timeline.len(), 1, "debug_only_id requires exactly one entry");
        *t.timeline.keys().next().unwrap()
    }

    #[cfg(test)]
    fn debug_bodies_len(&self) -> usize {
        self.transactions.lock().unwrap().transactions.len()
    }

    /// Admit Instant for the broadcast body whose sequence number equals `nonce`.
    /// Panics if no matching body is currently indexed (test helper only).
    #[cfg(test)]
    fn debug_admit_instant_for_nonce(&self, nonce: u64) -> Instant {
        let store = self.transactions.lock().unwrap();
        for (hash, txn) in &store.transactions {
            if txn.sequence_number() == nonce {
                let (bucket, id) = store.hash_index.get(hash).expect("hash_index entry for body");
                let (_h, admit_at) = store
                    .timeline_index
                    .get(bucket)
                    .and_then(|t| t.timeline.get(id))
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
            transactions: Mutex::new(TransactionStore::new(max_age)),
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

    #[test]
    fn timeline_range_returns_window() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1), mk_txn(0, 1, 2), mk_txn(0, 2, 3)]));
        let m = mempool_with(txns, Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        // ids 1..=3 in bucket 0
        let mut pairs = HashMap::new();
        pairs.insert(0u8, (0u64, 2u64)); // (Excluded(0), Included(2)) → id 1,2
        let out = m.timeline_range(0, pairs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, 0); // ready_time_ms
        assert_eq!(out[1].1, 0);
    }

    #[test]
    fn timeline_range_skips_removed_bodies() {
        let t = mk_txn(0, 0, 1);
        let txns = Arc::new(StdMutex::new(vec![t, mk_txn(0, 1, 2)]));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        txns.lock().unwrap().remove(0); // drop first
        m.force_reconcile_for_test();
        let mut pairs = HashMap::new();
        pairs.insert(0u8, (0u64, 10u64));
        let out = m.timeline_range(0, pairs);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn timeline_range_of_message_flattens_buckets() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1), mk_txn(1, 0, 2)]));
        let m = mempool_with(txns, Duration::ZERO, 2, 10);
        m.force_reconcile_for_test();
        let mut outer = HashMap::new();
        for b in 0u8..2 {
            let mut inner = HashMap::new();
            inner.insert(0u8, (0u64, 100u64));
            outer.insert(b, inner);
        }
        let out = m.timeline_range_of_message(outer);
        assert_eq!(out.len(), 2);
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
            transactions: Mutex::new(TransactionStore::new(Duration::from_millis(20))),
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
