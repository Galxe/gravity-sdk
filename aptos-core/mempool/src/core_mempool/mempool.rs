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
            use std::collections::hash_map::Entry;
            match store.transactions.entry(hash) {
                Entry::Occupied(mut e) => {
                    // Still present: optional body overwrite; timeline id/Instant stay.
                    e.insert(signed);
                    continue;
                }
                Entry::Vacant(e) => {
                    let tl = store.timeline_index.entry(bucket).or_insert_with(TimelineIndex::new);
                    let id = tl.timeline_id;
                    tl.timeline_id = tl.timeline_id.saturating_add(1);
                    tl.timeline.insert(id, (hash, Instant::now()));
                    store.hash_index.insert(hash, (bucket, id));
                    e.insert(signed);
                }
            }
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

    // =========================================================================
    // Risk coverage: R-alt (T1/T2), R6 (T3), Failover before (T5 unit)
    // Design: _local/wiki/mempool-broadcast/test-design-r-alt-r6-wan-failover.md
    // Does NOT drive gaptos network.rs; simulates filter/pending/expired against
    // Gravity CoreMempoolTrait (timeline_range* / read_timeline).
    // =========================================================================

    /// Production-shaped MessageId window for fee-slot cursors (slot 0 carries
    /// progress; other slots zip as (0,0) and are ignored by v1 range).
    /// Mirrors gaptos `MempoolMessageId::from_timeline_ids` + `decode` shape for
    /// a single sender_bucket without depending on `pub(crate)` gaptos APIs.
    fn message_window_from_cursors(
        sender_bucket: MempoolSenderBucket,
        old: &MultiBucketTimelineIndexIds,
        new: &MultiBucketTimelineIndexIds,
    ) -> HashMap<MempoolSenderBucket, HashMap<TimelineIndexIdentifier, (u64, u64)>> {
        assert_eq!(old.id_per_bucket.len(), new.id_per_bucket.len());
        let mut inner = HashMap::new();
        for (i, (&o, &n)) in old.id_per_bucket.iter().zip(new.id_per_bucket.iter()).enumerate() {
            inner.insert(i as TimelineIndexIdentifier, (o, n));
        }
        let mut outer = HashMap::new();
        outer.insert(sender_bucket, inner);
        outer
    }

    /// Slot-0 only id for tracking sent_messages in the filter simulation.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct SimMessageId {
        sender_bucket: MempoolSenderBucket,
        /// Exclusive lower bound (cursor before Fresh).
        start: u64,
        /// Inclusive upper bound (cursor after Fresh).
        end: u64,
    }

    impl SimMessageId {
        fn from_cursors(
            bucket: MempoolSenderBucket,
            old: &MultiBucketTimelineIndexIds,
            new: &MultiBucketTimelineIndexIds,
        ) -> Self {
            Self {
                sender_bucket: bucket,
                start: old.id_per_bucket.first().copied().unwrap_or(0),
                end: new.id_per_bucket.first().copied().unwrap_or(0),
            }
        }

        fn to_range_args(
            &self,
        ) -> HashMap<MempoolSenderBucket, HashMap<TimelineIndexIdentifier, (u64, u64)>> {
            let mut inner = HashMap::new();
            inner.insert(0, (self.start, self.end));
            let mut outer = HashMap::new();
            outer.insert(self.sender_bucket, inner);
            outer
        }
    }

    /// Outcome of one simulated `determine_broadcast_batch` tick (filter + pending
    /// + Expired/Retry/Fresh + optional backoff). See gaptos `network.rs`
    /// determine_broadcast_batch / process_broadcast_ack.
    #[derive(Debug)]
    enum SimBatchOutcome {
        TooManyPendingBroadcasts {
            pending: usize,
        },
        /// ACK timeout retransmit via timeline_range (does not advance cursor).
        Expired {
            id: SimMessageId,
            bodies: Vec<SignedTransaction>,
        },
        /// Peer-full / retry=true path: re-fetch via timeline_range, no cursor advance.
        Retry {
            id: SimMessageId,
            bodies: Vec<SignedTransaction>,
        },
        Fresh {
            id: SimMessageId,
            bodies: Vec<SignedTransaction>,
            new_cursor: MultiBucketTimelineIndexIds,
        },
        NoTransactions,
        /// backoff_mode set and this tick is not a scheduled_backoff tick.
        PeerNotScheduled,
    }

    /// Filter `sent` the way gaptos does: drop MessageIds whose
    /// `timeline_range_of_message` is empty (bodies committed / left pool).
    fn filter_sent_keepalive(
        m: &Mempool,
        sent: BTreeMap<SimMessageId, Instant>,
    ) -> BTreeMap<SimMessageId, Instant> {
        sent.into_iter()
            .filter(|(id, _)| !m.timeline_range_of_message(id.to_range_args()).is_empty())
            .collect()
    }

    /// Optional inputs for the extended determine_broadcast_batch sim.
    #[derive(Default)]
    struct SimBatchOpts<'a> {
        /// MessageIds awaiting retry retransmit (peer full / retry=true ACK path).
        retry: Option<&'a mut BTreeSet<SimMessageId>>,
        /// After backoff=true ACK, non-scheduled ticks must not broadcast.
        backoff_mode: bool,
        scheduled_backoff: bool,
    }

    /// Minimal reimplementation of determine_broadcast_batch branches used by T1-B/C.
    fn sim_determine_broadcast_batch(
        m: &Mempool,
        sent: &mut BTreeMap<SimMessageId, Instant>,
        peer_cursor: &MultiBucketTimelineIndexIds,
        sender_bucket: MempoolSenderBucket,
        max_broadcasts_per_peer: usize,
        ack_timeout: Duration,
        now: Instant,
        batch_size: usize,
    ) -> SimBatchOutcome {
        sim_determine_broadcast_batch_ex(
            m,
            sent,
            peer_cursor,
            sender_bucket,
            max_broadcasts_per_peer,
            ack_timeout,
            now,
            batch_size,
            SimBatchOpts::default(),
        )
    }

    /// Extended sim: backoff gate + optional retry set (gaptos order approximated).
    fn sim_determine_broadcast_batch_ex(
        m: &Mempool,
        sent: &mut BTreeMap<SimMessageId, Instant>,
        peer_cursor: &MultiBucketTimelineIndexIds,
        sender_bucket: MempoolSenderBucket,
        max_broadcasts_per_peer: usize,
        ack_timeout: Duration,
        now: Instant,
        batch_size: usize,
        mut opts: SimBatchOpts<'_>,
    ) -> SimBatchOutcome {
        // gaptos: backoff_mode without scheduled_backoff → PeerNotScheduled.
        if opts.backoff_mode && !opts.scheduled_backoff {
            return SimBatchOutcome::PeerNotScheduled;
        }

        *sent = filter_sent_keepalive(m, std::mem::take(sent));
        // Drop retry ids whose range is empty (same GC rule as sent).
        if let Some(retry) = opts.retry.as_mut() {
            **retry = std::mem::take(*retry)
                .into_iter()
                .filter(|id| !m.timeline_range_of_message(id.to_range_args()).is_empty())
                .collect();
        }

        let mut pending = 0usize;
        let mut expired_id: Option<SimMessageId> = None;
        for (id, sent_at) in sent.iter() {
            if now.duration_since(*sent_at) > ack_timeout {
                // Keep earliest expired by iteration order; any expired is enough.
                if expired_id.is_none() {
                    expired_id = Some(*id);
                }
            } else {
                pending += 1;
            }
            if pending >= max_broadcasts_per_peer {
                return SimBatchOutcome::TooManyPendingBroadcasts { pending };
            }
        }

        if let Some(id) = expired_id {
            let bodies: Vec<_> = m
                .timeline_range_of_message(id.to_range_args())
                .into_iter()
                .map(|(t, _)| t)
                .collect();
            return SimBatchOutcome::Expired { id, bodies };
        }

        // Prefer Retry over Fresh when a retry MessageId is tracked.
        if let Some(retry) = opts.retry.as_mut() {
            if let Some(&id) = retry.iter().next() {
                let bodies: Vec<_> = m
                    .timeline_range_of_message(id.to_range_args())
                    .into_iter()
                    .map(|(t, _)| t)
                    .collect();
                retry.remove(&id);
                return SimBatchOutcome::Retry { id, bodies };
            }
        }

        let (out, new_cursor) = m.read_timeline(
            sender_bucket,
            peer_cursor,
            batch_size,
            None,
            BroadcastPeerPriority::Primary,
        );
        if out.is_empty() {
            return SimBatchOutcome::NoTransactions;
        }
        let id = SimMessageId::from_cursors(sender_bucket, peer_cursor, &new_cursor);
        let bodies: Vec<_> = out.into_iter().map(|(t, _)| t).collect();
        SimBatchOutcome::Fresh { id, bodies, new_cursor }
    }

    /// T1-A: After Fresh, MessageId window yields non-empty `timeline_range_of_message`
    /// so the gaptos sent_messages filter would **not** drop the in-flight id.
    /// (Architecture A stub range always empty → cleared every tick → permanent Fresh.)
    #[test]
    fn t1a_timeline_range_of_message_keeps_sent_window_nonempty() {
        let fee_slots = 10usize;
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1), mk_txn(0, 1, 2)]));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let old = empty_cursor(fee_slots);
        let (batch, new_ids) = m.read_timeline(0, &old, 16, None, BroadcastPeerPriority::Primary);
        assert!(!batch.is_empty(), "need ≥1 broadcastable txn");
        assert_eq!(new_ids.id_per_bucket.len(), fee_slots);
        assert!(new_ids.id_per_bucket[0] > 0);

        // Production-shaped window: zip(old, new) fee slots for sender_bucket 0.
        let window = message_window_from_cursors(0, &old, &new_ids);
        let range_bodies = m.timeline_range_of_message(window);
        assert!(
            !range_bodies.is_empty(),
            "non-empty range proves sent_messages filter keeps this MessageId \
             (empty range would clear sent every tick — Arch A failure mode)"
        );
        assert_eq!(range_bodies.len(), batch.len());

        // Slot-0 pair alone is sufficient for v1 (same pass criterion).
        let sim = SimMessageId::from_cursors(0, &old, &new_ids);
        let again = m.timeline_range_of_message(sim.to_range_args());
        assert!(!again.is_empty());
        assert_eq!(again.len(), batch.len());
    }

    /// T1-B: Withheld ACKs → pending grows; at max_broadcasts_per_peer refuse Fresh.
    #[test]
    fn t1b_max_broadcasts_per_peer_backpressure() {
        let fee_slots = 10usize;
        let max_broadcasts = 2usize;
        let ack_timeout = Duration::from_secs(60); // do not expire
        let batch_size = 2usize;
        // Enough txns for several Fresh batches without draining early.
        let txns =
            Arc::new(StdMutex::new((0..12u8).map(|i| mk_txn(0, i as u64, 100 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut cursor = empty_cursor(fee_slots);
        let t0 = Instant::now();

        // Two successful Fresh sends (no ACK) → pending = 2.
        for tick in 0..max_broadcasts {
            match sim_determine_broadcast_batch(
                &m,
                &mut sent,
                &cursor,
                0,
                max_broadcasts,
                ack_timeout,
                t0,
                batch_size,
            ) {
                SimBatchOutcome::Fresh { id, bodies, new_cursor } => {
                    assert!(!bodies.is_empty(), "tick {tick} Fresh empty");
                    sent.insert(id, t0);
                    cursor = new_cursor;
                }
                other => panic!("tick {tick}: expected Fresh, got {other:?}"),
            }
        }
        assert_eq!(sent.len(), max_broadcasts);

        // Third tick: both still pending → TooManyPendingBroadcasts (no Fresh).
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
        ) {
            SimBatchOutcome::TooManyPendingBroadcasts { pending } => {
                assert!(pending >= max_broadcasts);
            }
            other => panic!("expected TooManyPendingBroadcasts, got {other:?}"),
        }
        // In-flight windows still keepalive (bodies still in pool).
        assert_eq!(filter_sent_keepalive(&m, sent.clone()).len(), max_broadcasts);

        // Inject one ACK (remove one sent id) → Fresh allowed again.
        let ack_id = *sent.keys().next().expect("sent non-empty");
        sent.remove(&ack_id);
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
        ) {
            SimBatchOutcome::Fresh { .. } => {}
            other => panic!("after ACK expected Fresh, got {other:?}"),
        }
    }

    /// T1-C: ACK timeout → Expired branch re-fetches body via timeline_range (not Fresh).
    #[test]
    fn t1c_ack_timeout_expired_uses_timeline_range() {
        let fee_slots = 10usize;
        let max_broadcasts = 20usize;
        let ack_timeout = Duration::from_millis(50);
        let batch_size = 4usize;
        let txns = Arc::new(StdMutex::new((0..6u8).map(|i| mk_txn(0, i as u64, 50 + i)).collect()));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut cursor = empty_cursor(fee_slots);
        let send_at = Instant::now();

        let (fresh_id, fresh_hashes) = match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            send_at,
            batch_size,
        ) {
            SimBatchOutcome::Fresh { id, bodies, new_cursor } => {
                let hashes: HashSet<u64> = bodies.iter().map(|t| t.sequence_number()).collect();
                sent.insert(id, send_at);
                cursor = new_cursor;
                (id, hashes)
            }
            other => panic!("expected Fresh, got {other:?}"),
        };
        assert!(!fresh_hashes.is_empty());

        // Advance past ack_timeout without ACK.
        let later = send_at + ack_timeout + Duration::from_millis(1);
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            later,
            batch_size,
        ) {
            SimBatchOutcome::Expired { id, bodies } => {
                assert_eq!(id, fresh_id, "Expired must retransmit the same MessageId window");
                let expired_hashes: HashSet<u64> =
                    bodies.iter().map(|t| t.sequence_number()).collect();
                assert_eq!(
                    expired_hashes, fresh_hashes,
                    "Expired body set must match original Fresh window (via timeline_range)"
                );
                // Cursor must NOT advance on Expired path (Fresh would change cursor).
                // peer_cursor is unchanged in our sim when Expired is selected.
            }
            other => panic!("expected Expired after ack_timeout, got {other:?}"),
        }

        // Remove all bodies from pool → range empty → filter drops tracking (GC).
        txns.lock().unwrap().clear();
        m.force_reconcile_for_test();
        let kept = filter_sent_keepalive(&m, sent.clone());
        assert!(
            kept.is_empty(),
            "empty timeline_range_of_message must drop sent_messages entry (correct GC)"
        );
    }

    /// T2 (lite): After cursor drain, further Fresh with same cursor is empty —
    /// no periodic re-emission of already-scanned ids without leave/re-admit.
    #[test]
    fn t2_no_periodic_fresh_rebroadcast_after_drain() {
        let fee_slots = 10usize;
        let n = 20usize;
        let count = 5usize;
        let txns =
            Arc::new(StdMutex::new((0..n as u8).map(|i| mk_txn(0, i as u64, 40 + i)).collect()));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut cursor = empty_cursor(fee_slots);
        let mut fresh_count: HashMap<u64, usize> = HashMap::new(); // seq → inclusions
        let mut ticks = 0usize;
        loop {
            let (batch, new_cur) =
                m.read_timeline(0, &cursor, count, None, BroadcastPeerPriority::Primary);
            ticks += 1;
            if batch.is_empty() {
                break;
            }
            for (txn, _) in &batch {
                *fresh_count.entry(txn.sequence_number()).or_insert(0) += 1;
            }
            // Cursor must advance while draining.
            assert!(
                new_cur.id_per_bucket[0] > cursor.id_per_bucket[0],
                "cursor must monotonically advance during drain"
            );
            cursor = new_cur;
            assert!(ticks <= n + 2, "drain should finish in O(n/count) ticks");
        }

        // Each admitted seq appeared exactly once in Fresh during drain.
        assert_eq!(fresh_count.len(), n);
        for seq in 0..n as u64 {
            assert_eq!(
                fresh_count.get(&seq).copied().unwrap_or(0),
                1,
                "seq {seq} must appear exactly once in Fresh during drain"
            );
        }

        // Many more ticks with fixed pool + same cursor: no re-emission.
        let drained_cursor = cursor.clone();
        for _ in 0..30 {
            let (batch, cur) =
                m.read_timeline(0, &drained_cursor, count, None, BroadcastPeerPriority::Primary);
            assert!(batch.is_empty(), "post-drain Fresh must stay empty (no TTL rebroadcast)");
            assert_eq!(
                cur.id_per_bucket[0], drained_cursor.id_per_bucket[0],
                "empty batch must not advance cursor"
            );
        }

        // Leave one hash and re-admit: new timeline id, can appear again in Fresh.
        let reenter = mk_txn(0, 0, 40); // same body as first
        {
            let mut guard = txns.lock().unwrap();
            guard.retain(|t| t.seq_number() != 0);
        }
        m.force_reconcile_for_test();
        txns.lock().unwrap().push(reenter);
        m.force_reconcile_for_test();
        // From drained cursor: only re-admitted id (new, > old max) should show.
        let (batch, _) =
            m.read_timeline(0, &drained_cursor, count, None, BroadcastPeerPriority::Primary);
        assert_eq!(batch.len(), 1, "re-admitted hash gets new id past drained cursor");
        assert_eq!(batch[0].0.sequence_number(), 0);
    }

    /// T3 / R6: same sender_bucket, many senders, small count — cursor drain covers
    /// every sender in ≤ ceil(S/count)+1 ticks (no permanent table-head starvation).
    #[test]
    fn t3_multi_sender_same_bucket_cover_in_ceil_s_over_count_ticks() {
        let fee_slots = 10usize;
        let s = 30usize;
        let count = 5usize;
        // num_sender_buckets=1 → all addresses share bucket 0 regardless of last byte.
        // Distinct last bytes = distinct senders for inclusion accounting.
        let txns = Arc::new(StdMutex::new((0..s as u8).map(|i| mk_txn(i, 0, 200 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut cursor = empty_cursor(fee_slots);
        let mut inclusion: HashMap<u8, usize> = HashMap::new(); // last byte of sender
        let mut ticks = 0usize;
        let mut first_batch_len = None;

        loop {
            let (batch, new_cur) =
                m.read_timeline(0, &cursor, count, None, BroadcastPeerPriority::Primary);
            if batch.is_empty() {
                break;
            }
            if first_batch_len.is_none() {
                first_batch_len = Some(batch.len());
            }
            // No duplicate seq/sender within one tick for single-cursor peer.
            let mut seen_this_tick = HashSet::new();
            for (txn, _) in &batch {
                let last = txn.sender().into_bytes()[31];
                assert!(seen_this_tick.insert(last), "duplicate sender {last} in same Fresh batch");
                *inclusion.entry(last).or_insert(0) += 1;
            }
            assert!(
                new_cur.id_per_bucket[0] > cursor.id_per_bucket[0],
                "cursor must step each non-empty tick"
            );
            cursor = new_cur;
            ticks += 1;
            // Safety: avoid infinite loop on broken cursor.
            assert!(ticks <= s + 2);
        }

        assert_eq!(first_batch_len, Some(count), "first batch must be full when S > count");

        let max_ticks = (s + count - 1) / count + 1; // ceil(S/count)+1
        assert!(ticks <= max_ticks, "full cover ticks {ticks} > ceil(S/count)+1 = {max_ticks}");

        for i in 0..s as u8 {
            assert!(
                inclusion.get(&i).copied().unwrap_or(0) >= 1,
                "sender last_byte={i} never appeared (table-head starvation / no cursor)"
            );
        }
        // Weak fairness: after cover, each sender once (1 txn each).
        for i in 0..s as u8 {
            assert_eq!(inclusion[&i], 1, "sender {i} inclusion count");
        }
    }

    /// T5 unit strengthen: Failover-style `before = now - 500ms` suppresses freshly
    /// admitted txns; Primary (before=None) still returns them. Documents alignment
    /// with `shared_mempool_failover_delay_ms` default 500 (wall-clock first-alt SLA
    /// remains e2e — this is Instant filter semantics only).
    #[test]
    fn t5_failover_before_500ms_filters_fresh_admits() {
        const FAILOVER_DELAY_MS: u64 = 500; // shared_mempool_failover_delay_ms default

        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)]));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        // Separate admit Instant for B (existing filter boundary).
        std::thread::sleep(Duration::from_millis(5));
        txns.lock().unwrap().push(mk_txn(0, 1, 2));
        m.force_reconcile_for_test();

        let a_admit = m.debug_admit_instant_for_nonce(0);
        let b_admit = m.debug_admit_instant_for_nonce(1);
        assert!(b_admit > a_admit, "B must admit strictly after A");

        // Primary: no before → both A and B.
        let (primary, _) =
            m.read_timeline(0, &empty_cursor(10), 16, None, BroadcastPeerPriority::Primary);
        assert_eq!(primary.len(), 2, "Primary sees all admits");

        // Boundary: before = B's admit Instant → only A (admit_at < before).
        let (only_a, _) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(b_admit),
            BroadcastPeerPriority::Primary,
        );
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].0.sequence_number(), 0);

        // before = A's admit → empty (A also excluded as >=).
        let (none, _) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(a_admit),
            BroadcastPeerPriority::Primary,
        );
        assert!(none.is_empty(), "before at A's admit excludes A and later");

        // Failover production formula: before = now - failover_delay_ms.
        // Both A and B were admitted within the last few ms ≪ 500ms, so both
        // are "too new" for Failover — batch empty. Proves delay gate without a
        // flaky 500ms sleep (deterministic relative to Instant::now()).
        let failover_before = Instant::now() - Duration::from_millis(FAILOVER_DELAY_MS);
        assert!(
            a_admit > failover_before && b_admit > failover_before,
            "test assumption: admits are younger than failover delay window"
        );
        let (failover_out, failover_cur) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(failover_before),
            BroadcastPeerPriority::Failover,
        );
        assert!(
            failover_out.is_empty(),
            "Failover before=now-{FAILOVER_DELAY_MS}ms must not emit sub-delay admits"
        );
        // Empty batch does not advance cursor (same invariant as Primary).
        assert_eq!(failover_cur.id_per_bucket[0], 0);
    }

    // -------------------------------------------------------------------------
    // T4 — delayed-ACK inject (app-layer Instant clock, no netem / wall sleep)
    // -------------------------------------------------------------------------

    /// T4-A: RTT D < ack_timeout → no Expired storm; delayed ACKs free pending;
    /// each seq appears exactly once in Fresh over the drain.
    #[test]
    fn t4a_delayed_ack_rtt_under_timeout_no_expired_storm() {
        let fee_slots = 10usize;
        let max_broadcasts = 2usize;
        let ack_timeout = Duration::from_millis(200);
        let batch_size = 2usize;
        let n = 8usize;
        let d = Duration::from_millis(50); // D < ack_timeout
        let tick_step = Duration::from_millis(20);

        let txns =
            Arc::new(StdMutex::new((0..n as u8).map(|i| mk_txn(0, i as u64, 10 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        // Queued ACKs: (due_time, message_id)
        let mut pending_acks: Vec<(Instant, SimMessageId)> = Vec::new();
        let mut cursor = empty_cursor(fee_slots);
        let mut now = Instant::now();
        let mut expired_count = 0usize;
        let mut too_many_seen = 0usize;
        let mut fresh_count: HashMap<u64, usize> = HashMap::new();
        let mut max_cursor = 0u64;

        // Enough ticks to drain with delayed ACKs (backpressure stalls expected).
        for _ in 0..200 {
            // Apply ACKs whose due_time ≤ now.
            let mut still = Vec::new();
            for (due, id) in pending_acks.drain(..) {
                if due <= now {
                    sent.remove(&id);
                } else {
                    still.push((due, id));
                }
            }
            pending_acks = still;

            match sim_determine_broadcast_batch(
                &m,
                &mut sent,
                &cursor,
                0,
                max_broadcasts,
                ack_timeout,
                now,
                batch_size,
            ) {
                SimBatchOutcome::Fresh { id, bodies, new_cursor } => {
                    for t in &bodies {
                        *fresh_count.entry(t.sequence_number()).or_insert(0) += 1;
                    }
                    sent.insert(id, now);
                    pending_acks.push((now + d, id));
                    assert!(
                        new_cursor.id_per_bucket[0] >= cursor.id_per_bucket[0],
                        "cursor must not rewind on Fresh"
                    );
                    cursor = new_cursor;
                    max_cursor = max_cursor.max(cursor.id_per_bucket[0]);
                }
                SimBatchOutcome::Expired { .. } => {
                    expired_count += 1;
                }
                SimBatchOutcome::TooManyPendingBroadcasts { .. } => {
                    too_many_seen += 1;
                }
                SimBatchOutcome::NoTransactions => {
                    // Drain complete once ACKs catch up and cursor is past all ids.
                    if fresh_count.len() >= n {
                        break;
                    }
                }
                other => panic!("unexpected outcome under delayed-ACK model: {other:?}"),
            }
            now += tick_step;
        }

        assert_eq!(
            expired_count, 0,
            "D={d:?} < ack_timeout={ack_timeout:?}: must not see Expired storm"
        );
        assert!(
            max_cursor > 0,
            "cursor must advance overall under delayed ACK (max_cursor={max_cursor})"
        );
        // With max_broadcasts=2 and D spanning multiple ticks, backpressure is expected.
        assert!(
            too_many_seen > 0 || fresh_count.len() == n,
            "either hit TooManyPending or finished drain without stall"
        );
        assert_eq!(fresh_count.len(), n, "all seqs must appear in Fresh");
        for seq in 0..n as u64 {
            assert_eq!(
                fresh_count.get(&seq).copied().unwrap_or(0),
                1,
                "seq {seq} must appear exactly once in Fresh (no re-emit as Fresh)"
            );
        }
    }

    /// T4-B: D > ack_timeout → Expired retransmit with same MessageId window;
    /// late ACK is fine; peer cursor never rewinds; post-ACK Fresh does not
    /// re-emit already-cursor-passed ids.
    #[test]
    fn t4b_rtt_over_ack_timeout_triggers_expired_not_cursor_rewind() {
        let fee_slots = 10usize;
        let max_broadcasts = 20usize;
        let ack_timeout = Duration::from_millis(50);
        let batch_size = 2usize;
        // Enough for Fresh after Expired path is cleared.
        let txns = Arc::new(StdMutex::new((0..8u8).map(|i| mk_txn(0, i as u64, 30 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut cursor = empty_cursor(fee_slots);
        let send_at = Instant::now();
        let cursor_at_start = cursor.id_per_bucket[0];

        let (fresh_id, fresh_hashes, cursor_after_fresh) = match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            send_at,
            batch_size,
        ) {
            SimBatchOutcome::Fresh { id, bodies, new_cursor } => {
                let hashes: HashSet<u64> = bodies.iter().map(|t| t.sequence_number()).collect();
                sent.insert(id, send_at);
                cursor = new_cursor.clone();
                (id, hashes, new_cursor)
            }
            other => panic!("expected Fresh, got {other:?}"),
        };
        assert!(!fresh_hashes.is_empty());
        assert!(cursor_after_fresh.id_per_bucket[0] > cursor_at_start);

        // Advance past timeout without ACK (simulates D > ack_timeout).
        let after_timeout = send_at + ack_timeout + Duration::from_millis(1);
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            after_timeout,
            batch_size,
        ) {
            SimBatchOutcome::Expired { id, bodies } => {
                assert_eq!(id, fresh_id, "Expired retransmit must use same MessageId window");
                let expired_hashes: HashSet<u64> =
                    bodies.iter().map(|t| t.sequence_number()).collect();
                assert_eq!(expired_hashes, fresh_hashes);
                // Cursor unchanged on Expired path.
                assert_eq!(cursor.id_per_bucket[0], cursor_after_fresh.id_per_bucket[0]);
            }
            other => panic!("expected Expired when D > ack_timeout, got {other:?}"),
        }

        // Late ACK for the expired id (remove from sent) — still OK.
        sent.remove(&fresh_id);
        assert!(cursor.id_per_bucket[0] >= cursor_after_fresh.id_per_bucket[0]);

        // Fresh continues past already-scanned window; no re-emission of fresh_hashes.
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            after_timeout + Duration::from_millis(1),
            batch_size,
        ) {
            SimBatchOutcome::Fresh { bodies, new_cursor, .. } => {
                let next_hashes: HashSet<u64> =
                    bodies.iter().map(|t| t.sequence_number()).collect();
                assert!(
                    next_hashes.is_disjoint(&fresh_hashes),
                    "post-Expired Fresh must not re-emit already-cursor-passed ids: {next_hashes:?} vs {fresh_hashes:?}"
                );
                assert!(
                    new_cursor.id_per_bucket[0] >= cursor.id_per_bucket[0],
                    "cursor must not rewind after late ACK + Fresh"
                );
            }
            other => panic!("expected Fresh after late ACK, got {other:?}"),
        }
    }

    /// T4-C: WAN-like delayed ACK + small max_broadcasts → backpressure then recover.
    #[test]
    fn t4c_backpressure_under_delay_then_recover() {
        let fee_slots = 10usize;
        let max_broadcasts = 2usize;
        let ack_timeout = Duration::from_secs(60); // no expire
        let batch_size = 2usize;
        let txns =
            Arc::new(StdMutex::new((0..10u8).map(|i| mk_txn(0, i as u64, 70 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut cursor = empty_cursor(fee_slots);
        // Simulated send clock; ACKs deliberately not applied yet (large D).
        let t0 = Instant::now();
        let mut delayed_ack_ids: Vec<SimMessageId> = Vec::new();

        for tick in 0..max_broadcasts {
            match sim_determine_broadcast_batch(
                &m,
                &mut sent,
                &cursor,
                0,
                max_broadcasts,
                ack_timeout,
                t0,
                batch_size,
            ) {
                SimBatchOutcome::Fresh { id, new_cursor, .. } => {
                    sent.insert(id, t0);
                    delayed_ack_ids.push(id);
                    cursor = new_cursor;
                }
                other => panic!("tick {tick}: expected Fresh, got {other:?}"),
            }
        }

        // Without applying ACKs: next tick must backpressure.
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0 + Duration::from_millis(100), // still ≪ ack_timeout
            batch_size,
        ) {
            SimBatchOutcome::TooManyPendingBroadcasts { pending } => {
                assert!(pending >= max_broadcasts);
            }
            other => panic!("expected TooManyPendingBroadcasts, got {other:?}"),
        }

        // Apply one delayed ACK → room for Fresh again.
        let one = delayed_ack_ids.remove(0);
        sent.remove(&one);
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0 + Duration::from_millis(200),
            batch_size,
        ) {
            SimBatchOutcome::Fresh { .. } => {}
            other => panic!("after delayed ACK expected Fresh recovery, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // T2-B — immediate-ACK drain + multi-TTL Instant advance (no wall sleep)
    // -------------------------------------------------------------------------

    /// T2-B: Healthy peer (immediate ACK) drains pool; advancing Instant by
    /// ≥ 3× old Arch-A TTL (15s) without pool change stays NoTransactions —
    /// no periodic Fresh rebroadcast of already-scanned ids.
    #[test]
    fn t2b_immediate_ack_drain_then_no_fresh_rebroadcast() {
        let fee_slots = 10usize;
        let n = 12usize;
        let batch_size = 3usize;
        let max_broadcasts = 20usize;
        let ack_timeout = Duration::from_secs(60);
        let old_ttl = Duration::from_secs(5); // Arch-A MEMPOOL_BROADCAST_CACHE_TTL contrast

        let txns =
            Arc::new(StdMutex::new((0..n as u8).map(|i| mk_txn(0, i as u64, 80 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut cursor = empty_cursor(fee_slots);
        let mut now = Instant::now();
        let mut fresh_count: HashMap<u64, usize> = HashMap::new();

        // Drain with immediate ACK so pending never blocks.
        for _ in 0..n + 5 {
            match sim_determine_broadcast_batch(
                &m,
                &mut sent,
                &cursor,
                0,
                max_broadcasts,
                ack_timeout,
                now,
                batch_size,
            ) {
                SimBatchOutcome::Fresh { id, bodies, new_cursor } => {
                    for t in &bodies {
                        *fresh_count.entry(t.sequence_number()).or_insert(0) += 1;
                    }
                    // Immediate ACK: do not leave id in sent.
                    let _ = id;
                    cursor = new_cursor;
                }
                SimBatchOutcome::NoTransactions => break,
                other => panic!("during drain expected Fresh or empty, got {other:?}"),
            }
            now += Duration::from_millis(1);
        }

        assert_eq!(fresh_count.len(), n);
        for seq in 0..n as u64 {
            assert_eq!(fresh_count[&seq], 1, "seq {seq} Fresh count during drain");
        }
        let drained_cursor = cursor.clone();

        // ≥ 3 × old TTL of simulated time without wall sleep; pool unchanged.
        // Advance now by 1s per tick for 16 iterations (≥ 15s + epsilon).
        for i in 0..16 {
            now += Duration::from_secs(1);
            match sim_determine_broadcast_batch(
                &m,
                &mut sent,
                &drained_cursor,
                0,
                max_broadcasts,
                ack_timeout,
                now,
                batch_size,
            ) {
                SimBatchOutcome::NoTransactions => {}
                other => panic!(
                    "post-drain tick {i} (now +{}s) must be NoTransactions, got {other:?}",
                    i + 1
                ),
            }
        }
        // Simulated multi-TTL window: 16 × 1s ≥ 3 × old_ttl (15s).
        assert!(Duration::from_secs(16) >= old_ttl * 3);
        for seq in 0..n as u64 {
            assert_eq!(
                fresh_count[&seq], 1,
                "seq {seq} must not gain Fresh rebroadcasts across multi-TTL Instant window"
            );
        }
    }

    // -------------------------------------------------------------------------
    // T1-D / T1-E optional arms
    // -------------------------------------------------------------------------

    /// T1-D: ACK with backoff=true sets local backoff_mode; next non-scheduled
    /// tick is PeerNotScheduled; scheduled_backoff tick may Fresh again.
    #[test]
    fn t1d_backoff_ack_suppresses_fresh_until_scheduled() {
        let fee_slots = 10usize;
        let max_broadcasts = 20usize;
        let ack_timeout = Duration::from_secs(60);
        let batch_size = 2usize;
        let txns = Arc::new(StdMutex::new((0..6u8).map(|i| mk_txn(0, i as u64, 90 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut cursor = empty_cursor(fee_slots);
        let t0 = Instant::now();

        // Fresh then "ACK with backoff=true" (remove sent + set flag).
        match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
        ) {
            SimBatchOutcome::Fresh { id, new_cursor, .. } => {
                sent.insert(id, t0);
                // Immediate ACK with backoff.
                sent.remove(&id);
                cursor = new_cursor;
            }
            other => panic!("expected Fresh, got {other:?}"),
        }
        let backoff_mode = true;

        // Next tick without scheduled_backoff → PeerNotScheduled.
        match sim_determine_broadcast_batch_ex(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
            SimBatchOpts { retry: None, backoff_mode, scheduled_backoff: false },
        ) {
            SimBatchOutcome::PeerNotScheduled => {}
            other => panic!("expected PeerNotScheduled, got {other:?}"),
        }

        // Scheduled backoff tick allows Fresh again.
        match sim_determine_broadcast_batch_ex(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
            SimBatchOpts { retry: None, backoff_mode, scheduled_backoff: true },
        ) {
            SimBatchOutcome::Fresh { .. } => {}
            other => panic!("scheduled_backoff tick expected Fresh, got {other:?}"),
        }
    }

    /// T1-E: Retry set (simulating peer-full / retry=true) prefers Retry over
    /// Fresh; bodies come from timeline_range of that MessageId; cursor unchanged.
    #[test]
    fn t1e_retry_messages_use_timeline_range() {
        let fee_slots = 10usize;
        let max_broadcasts = 20usize;
        let ack_timeout = Duration::from_secs(60);
        let batch_size = 2usize;
        let txns =
            Arc::new(StdMutex::new((0..6u8).map(|i| mk_txn(0, i as u64, 110 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut sent: BTreeMap<SimMessageId, Instant> = BTreeMap::new();
        let mut retry: BTreeSet<SimMessageId> = BTreeSet::new();
        let mut cursor = empty_cursor(fee_slots);
        let t0 = Instant::now();

        let (fresh_id, fresh_hashes, cursor_after) = match sim_determine_broadcast_batch(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
        ) {
            SimBatchOutcome::Fresh { id, bodies, new_cursor } => {
                let hashes: HashSet<u64> = bodies.iter().map(|t| t.sequence_number()).collect();
                // Simulate peer-full: track for Retry, do not advance as if send failed
                // for cursor purposes — production still updates sent; here we put in retry.
                sent.insert(id, t0);
                retry.insert(id);
                // Peer cursor already advanced on successful Fresh send in gaptos;
                // keep that shape so we can prove Retry does not re-advance.
                cursor = new_cursor.clone();
                (id, hashes, new_cursor)
            }
            other => panic!("expected Fresh, got {other:?}"),
        };

        let cursor_before_retry = cursor.id_per_bucket[0];
        match sim_determine_broadcast_batch_ex(
            &m,
            &mut sent,
            &cursor,
            0,
            max_broadcasts,
            ack_timeout,
            t0,
            batch_size,
            SimBatchOpts { retry: Some(&mut retry), backoff_mode: false, scheduled_backoff: false },
        ) {
            SimBatchOutcome::Retry { id, bodies } => {
                assert_eq!(id, fresh_id);
                let retry_hashes: HashSet<u64> =
                    bodies.iter().map(|t| t.sequence_number()).collect();
                assert_eq!(
                    retry_hashes, fresh_hashes,
                    "Retry bodies must match original Fresh window via timeline_range"
                );
                assert_eq!(
                    cursor.id_per_bucket[0], cursor_before_retry,
                    "Retry path must not advance peer cursor"
                );
                assert_eq!(cursor.id_per_bucket[0], cursor_after.id_per_bucket[0]);
            }
            other => panic!("expected Retry, got {other:?}"),
        }
        assert!(retry.is_empty(), "retry id consumed after one Retry outcome");
    }

    // -------------------------------------------------------------------------
    // T3-B — larger S cover
    // -------------------------------------------------------------------------

    /// T3-B: S=60, count=5, same pass criteria as T3 (ceil(S/count)+1 cover).
    #[test]
    fn t3b_larger_s_cover() {
        let fee_slots = 10usize;
        let s = 60usize;
        let count = 5usize;
        let txns = Arc::new(StdMutex::new((0..s as u8).map(|i| mk_txn(i, 0, 150 + i)).collect()));
        let m = mempool_with(txns, Duration::ZERO, 1, fee_slots);
        m.force_reconcile_for_test();

        let mut cursor = empty_cursor(fee_slots);
        let mut inclusion: HashMap<u8, usize> = HashMap::new();
        let mut ticks = 0usize;
        let mut first_batch_len = None;

        loop {
            let (batch, new_cur) =
                m.read_timeline(0, &cursor, count, None, BroadcastPeerPriority::Primary);
            if batch.is_empty() {
                break;
            }
            if first_batch_len.is_none() {
                first_batch_len = Some(batch.len());
            }
            let mut seen_this_tick = HashSet::new();
            for (txn, _) in &batch {
                let last = txn.sender().into_bytes()[31];
                assert!(seen_this_tick.insert(last), "duplicate sender {last} in same tick");
                *inclusion.entry(last).or_insert(0) += 1;
            }
            assert!(new_cur.id_per_bucket[0] > cursor.id_per_bucket[0]);
            cursor = new_cur;
            ticks += 1;
            assert!(ticks <= s + 2);
        }

        assert_eq!(first_batch_len, Some(count));
        let max_ticks = (s + count - 1) / count + 1;
        assert!(ticks <= max_ticks, "ticks {ticks} > ceil(S/count)+1 = {max_ticks}");
        for i in 0..s as u8 {
            assert!(
                inclusion.get(&i).copied().unwrap_or(0) >= 1,
                "sender last_byte={i} never appeared"
            );
            assert_eq!(inclusion[&i], 1);
        }
    }

    // -------------------------------------------------------------------------
    // T5-B — parameterized before mid(A,B) + delay=0 edge documentation
    // -------------------------------------------------------------------------

    /// T5-B: Primary sees both; Failover with before between A and B sees only A;
    /// future before includes both; documents admit_at < before filter semantics
    /// and that delay_ms=0 (before≈now) is not a substitute for the 500ms gate.
    #[test]
    fn t5b_failover_before_admits_older_than_delay() {
        const FAILOVER_DELAY_MS: u64 = 500; // shared_mempool_failover_delay_ms default

        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)]));
        let m = mempool_with(txns.clone(), Duration::ZERO, 1, 10);
        m.force_reconcile_for_test();
        std::thread::sleep(Duration::from_millis(5));
        txns.lock().unwrap().push(mk_txn(0, 1, 2));
        m.force_reconcile_for_test();

        let a_admit = m.debug_admit_instant_for_nonce(0);
        let b_admit = m.debug_admit_instant_for_nonce(1);
        assert!(b_admit > a_admit);

        // Primary before=None → both.
        let (primary, _) =
            m.read_timeline(0, &empty_cursor(10), 16, None, BroadcastPeerPriority::Primary);
        assert_eq!(primary.len(), 2);

        // mid(A,B): production filter is admit_at < before (exclude when >=).
        let mid = a_admit + (b_admit.duration_since(a_admit) / 2);
        assert!(mid > a_admit && mid < b_admit);
        let (only_a, _) =
            m.read_timeline(0, &empty_cursor(10), 16, Some(mid), BroadcastPeerPriority::Failover);
        assert_eq!(only_a.len(), 1, "before=mid(A,B) must include only A");
        assert_eq!(only_a[0].0.sequence_number(), 0);

        // Future before: both admits are older than a far-future cutoff → both pass.
        let future = Instant::now() + Duration::from_secs(1);
        let (both, _) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(future),
            BroadcastPeerPriority::Failover,
        );
        assert_eq!(both.len(), 2, "future before includes all current admits (admit_at < before)");

        // delay_ms=0 edge: production formula before = now - 0 = now.
        // Just-admitted entries have admit_at slightly in the past, so admit_at < now
        // and they **pass** the filter — delay=0 is NOT "empty for just-admitted".
        // Contrast: the real 500ms gate (before = now - 500ms) excludes sub-delay admits.
        let before_now = Instant::now();
        let (at_now, _) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(before_now),
            BroadcastPeerPriority::Failover,
        );
        assert_eq!(
            at_now.len(),
            2,
            "delay_ms=0 (before=now) still includes just-admitted (admit_at < now); \
             only a positive failover delay creates the suppress window"
        );

        let failover_before = Instant::now() - Duration::from_millis(FAILOVER_DELAY_MS);
        assert!(a_admit > failover_before && b_admit > failover_before);
        let (suppressed, _) = m.read_timeline(
            0,
            &empty_cursor(10),
            16,
            Some(failover_before),
            BroadcastPeerPriority::Failover,
        );
        assert!(
            suppressed.is_empty(),
            "before=now-{FAILOVER_DELAY_MS}ms excludes admits younger than the delay window"
        );
    }
}
