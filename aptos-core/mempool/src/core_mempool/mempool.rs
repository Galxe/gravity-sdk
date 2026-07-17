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

/// Per-entry age cache for `read_timeline` deduplication (mempool-broadcast
/// impl-d §3). Replaces the previous "global wipe" `HashSet`: each entry now
/// remembers when it was last dispatched and to which target slot, so TTL is
/// scoped per-tx and TTL-triggered re-emits prefer a different slot (§6.4).
pub struct TxnCache {
    entries: HashMap<TxnHash, CacheEntry>,
    size: usize,
    ttl: Duration,
}

#[derive(Clone, Copy)]
struct CacheEntry {
    /// For dispatched entries: when the tx was last handed to a peer.
    /// For placeholders (`dispatched == false`): the Failover first-sighting
    /// time — i.e. when the TTL grace clock started ticking.
    last_dispatched_at: Instant,
    last_target: TargetSlot,
    /// `false` ⇒ placeholder seeded by a Failover first-sighting awaiting
    /// Primary claim within `cache.ttl`. `true` ⇒ tx has been dispatched
    /// at least once (the normal in-TTL-suppress / TTL-re-emit regime).
    dispatched: bool,
}

/// `(bucket, priority_discriminant)` — a zero-cost proxy for the destination
/// peer at this moment in time. `priority.rs` keeps `(bucket, priority)`
/// 1:1-mapped to a peer per prioritization window, so this pair fully
/// identifies the slot we last handed the tx to without copying a
/// `PeerNetworkId`.
type TargetSlot = (MempoolSenderBucket, u8);

fn priority_discriminant(p: &BroadcastPeerPriority) -> u8 {
    match p {
        BroadcastPeerPriority::Primary => 0,
        BroadcastPeerPriority::Failover => 1,
    }
}

fn sender_to_bucket(
    sender: &ExternalAccountAddress,
    num_sender_buckets: u8,
) -> MempoolSenderBucket {
    let bytes = sender.bytes();
    let n = num_sender_buckets.max(1);
    bytes[31] % n
}

impl TxnCache {
    fn new(size: usize, ttl: Duration) -> Self {
        Self { entries: HashMap::new(), size, ttl }
    }
}

/// A per-round snapshot of `pool.pending_transactions()` sliced by sender
/// bucket. Amortises N peer × M bucket × 2 priority `pool.pending_*` calls
/// down to ≈ one per `max_age` window. See impl-d §5.
struct Snapshot {
    shards: HashMap<MempoolSenderBucket, Vec<SnapshotEntry>>,
    taken_at: Instant,
    max_age: Duration,
    /// False until the first refresh runs, so `read_timeline` can tell
    /// "never snapshotted yet" apart from "snapshot is empty because reth
    /// pool is empty".
    initialized: bool,
}

#[derive(Clone)]
struct SnapshotEntry {
    hash: TxnHash,
    txn: SignedTransaction,
}

/// Mempool-local self-observation of which `(bucket, priority)` slots have
/// been queried recently. impl-d §3.1 places the topology view inside gaptos;
/// to keep this change zero-invasion on gaptos we instead infer the slot
/// count from `read_timeline`'s own call pattern — every read_timeline call
/// proves its `(bucket, priority)` slot is active right now. A slot is
/// "active" as long as it was observed within `ttl`. This preserves the
/// §6.4 single-peer auto-degrade semantics (count=1 ⇒ permit same-slot
/// resend) without touching the gaptos crate.
struct ObservedTopology {
    last_seen: HashMap<TargetSlot, Instant>,
    ttl: Duration,
}

impl ObservedTopology {
    fn new(ttl: Duration) -> Self {
        Self { last_seen: HashMap::new(), ttl }
    }

    fn observe(&mut self, slot: TargetSlot) {
        self.last_seen.insert(slot, Instant::now());
    }

    fn priority_count_for_bucket(&self, bucket: MempoolSenderBucket) -> u8 {
        let now = Instant::now();
        let mut count = 0u8;
        for prio_disc in 0u8..=1u8 {
            if let Some(t) = self.last_seen.get(&(bucket, prio_disc)) {
                if now.duration_since(*t) <= self.ttl {
                    count += 1;
                }
            }
        }
        count
    }
}

pub struct Mempool {
    pool: Box<dyn TxPool>,
    txn_cache: Arc<Mutex<TxnCache>>,
    snapshot: Arc<Mutex<Snapshot>>,
    topology: Arc<Mutex<ObservedTopology>>,
    num_sender_buckets: u8,
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
        sender_bucket: MempoolSenderBucket,
        _timeline_id: &MultiBucketTimelineIndexIds,
        count: usize,
        _before: Option<Instant>,
        priority_of_receiver: BroadcastPeerPriority,
    ) -> (Vec<(SignedTransaction, u64)>, MultiBucketTimelineIndexIds) {
        // Self-observe topology: this call IS proof that
        // (sender_bucket, priority_of_receiver) is currently an active slot.
        let target_slot: TargetSlot = (sender_bucket, priority_discriminant(&priority_of_receiver));
        let priority_count = {
            let mut topo = self.topology.lock().unwrap();
            topo.observe(target_slot);
            topo.priority_count_for_bucket(sender_bucket)
        };

        let shard: Vec<SnapshotEntry> = {
            let mut snap = self.snapshot.lock().unwrap();
            if !snap.initialized || snap.taken_at.elapsed() >= snap.max_age {
                self.refresh_snapshot_locked(&mut snap);
            }
            snap.shards.get(&sender_bucket).cloned().unwrap_or_default()
        };

        let now = Instant::now();
        let mut out: Vec<(SignedTransaction, u64)> = Vec::with_capacity(count.min(shard.len()));
        let mut cache = self.txn_cache.lock().unwrap();

        for entry in shard {
            if out.len() >= count {
                break;
            }
            // PR #722 review point 3: the TTL cache is now self-sufficient
            // for failover semantics. Primary first-sighting dispatches
            // immediately. Failover first-sighting seeds a placeholder so
            // the TTL clock starts here. Within the `cache.ttl` grace,
            // Primary can still claim the placeholder (preserves the
            // Primary-first invariant). After the grace elapses, Failover
            // takes over — no dependency on `priority.rs` promotion.
            let dispatch = match cache.entries.get(&entry.hash) {
                None => matches!(priority_of_receiver, BroadcastPeerPriority::Primary),
                Some(e) if !e.dispatched => match priority_of_receiver {
                    BroadcastPeerPriority::Primary => true,
                    BroadcastPeerPriority::Failover => {
                        now.duration_since(e.last_dispatched_at) >= cache.ttl
                    }
                },
                Some(e) if now.duration_since(e.last_dispatched_at) < cache.ttl => false,
                Some(e) if e.last_target == target_slot && priority_count >= 2 => false,
                Some(_) => true,
            };
            if !dispatch {
                // Failover first-sighting seeds a placeholder so the TTL
                // clock starts. `or_insert` (not `insert`) preserves the
                // original first_seen_at across repeated Failover ticks
                // during the grace window.
                if matches!(priority_of_receiver, BroadcastPeerPriority::Failover) {
                    cache.entries.entry(entry.hash).or_insert(CacheEntry {
                        last_dispatched_at: now,
                        last_target: target_slot,
                        dispatched: false,
                    });
                }
                continue;
            }
            out.push((entry.txn, 0));
            cache.entries.insert(
                entry.hash,
                CacheEntry { last_dispatched_at: now, last_target: target_slot, dispatched: true },
            );
        }
        let len = out.len();
        (out, MultiBucketTimelineIndexIds { id_per_bucket: vec![0; len] })
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
        let ttl_secs = std::env::var("MEMPOOL_BROADCAST_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(5);
        let snapshot_max_age_ms = std::env::var("MEMPOOL_SNAPSHOT_MAX_AGE_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(20);
        let topology_ttl_ms = std::env::var("MEMPOOL_TOPOLOGY_OBSERVATION_TTL_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1500);
        let num_sender_buckets = config.mempool.num_sender_buckets.max(1);

        Self {
            pool,
            txn_cache: Arc::new(Mutex::new(TxnCache::new(100_000, Duration::from_secs(ttl_secs)))),
            snapshot: Arc::new(Mutex::new(Snapshot {
                shards: HashMap::new(),
                taken_at: Instant::now(),
                max_age: Duration::from_millis(snapshot_max_age_ms),
                initialized: false,
            })),
            topology: Arc::new(Mutex::new(ObservedTopology::new(Duration::from_millis(
                topology_ttl_ms,
            )))),
            num_sender_buckets,
        }
    }

    fn refresh_snapshot_locked(&self, snap: &mut Snapshot) {
        let mut shards: HashMap<MempoolSenderBucket, Vec<SnapshotEntry>> = HashMap::new();
        let mut alive: HashSet<TxnHash> = HashSet::new();
        for txn in self.pool.get_broadcast_txns(None) {
            let bucket = sender_to_bucket(txn.sender(), self.num_sender_buckets);
            let hash = TxnHash::from_bytes(txn.committed_hash().as_slice());
            alive.insert(hash);
            let signed: SignedTransaction = VerifiedTxn::from(txn).into();
            shards.entry(bucket).or_default().push(SnapshotEntry { hash, txn: signed });
        }
        snap.shards = shards;
        snap.taken_at = Instant::now();
        snap.initialized = true;

        // Lazy GC: drop cache entries whose hash is no longer alive in the
        // reth pool (committed / replaced / evicted). Then cap by size.
        let mut cache = self.txn_cache.lock().unwrap();
        cache.entries.retain(|h, _| alive.contains(h));
        if cache.entries.len() > cache.size {
            let mut by_age: Vec<(TxnHash, Instant)> =
                cache.entries.iter().map(|(h, e)| (*h, e.last_dispatched_at)).collect();
            by_age.sort_by_key(|&(_, t)| t);
            let to_drop = cache.entries.len() - cache.size;
            for (h, _) in by_age.into_iter().take(to_drop) {
                cache.entries.remove(&h);
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use gaptos::api_types::{
        account::ExternalChainId, VerifiedTxn as ApiVerifiedTxn, GLOBAL_CRYPTO_TXN_HASHER,
    };
    use std::sync::Mutex as StdMutex;

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

    fn mempool_with(
        txns: Arc<StdMutex<Vec<ApiVerifiedTxn>>>,
        ttl: Duration,
        snapshot_max_age: Duration,
        num_buckets: u8,
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
            txn_cache: Arc::new(Mutex::new(TxnCache::new(100_000, ttl))),
            snapshot: Arc::new(Mutex::new(Snapshot {
                shards: HashMap::new(),
                taken_at: Instant::now(),
                max_age: snapshot_max_age,
                initialized: false,
            })),
            topology: Arc::new(Mutex::new(ObservedTopology::new(Duration::from_secs(10)))),
            num_sender_buckets: num_buckets,
        }
    }

    fn read(
        m: &Mempool,
        bucket: MempoolSenderBucket,
        prio: BroadcastPeerPriority,
        count: usize,
    ) -> Vec<(SignedTransaction, u64)> {
        m.read_timeline(
            bucket,
            &MultiBucketTimelineIndexIds { id_per_bucket: vec![] },
            count,
            None,
            prio,
        )
        .0
    }

    #[test]
    fn first_dispatch_then_in_ttl_suppress() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 1)]));
        let m = mempool_with(txns, Duration::from_secs(60), Duration::from_millis(20), 1);
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
        assert!(
            read(&m, 0, BroadcastPeerPriority::Primary, 16).is_empty(),
            "within TTL must suppress"
        );
    }

    #[test]
    fn failover_cannot_steal_first_dispatch() {
        // A Failover tick that lands before any Primary tick must NOT take the
        // tx — first sighting is reserved for Primary. PR #722 review point 3:
        // a placeholder is seeded so the TTL clock starts; Primary's subsequent
        // tick claims the placeholder and dispatches.
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 6)]));
        let m = mempool_with(txns, Duration::from_secs(60), Duration::from_millis(20), 1);
        assert!(
            read(&m, 0, BroadcastPeerPriority::Failover, 16).is_empty(),
            "Failover must not steal first-dispatch from Primary"
        );
        // A placeholder entry must exist (dispatched == false).
        {
            let cache = m.txn_cache.lock().unwrap();
            assert_eq!(cache.entries.len(), 1);
            let e = cache.entries.values().next().unwrap();
            assert!(!e.dispatched, "Failover first-sighting must seed a placeholder");
        }
        // Primary then queries — claims the placeholder and dispatches.
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
    }

    #[test]
    fn ttl_expired_single_priority_dispatches() {
        // priority_count = 1 (only Primary ever observed) ⇒ TTL-expired
        // same-slot resend is allowed (otherwise X is blackholed forever).
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 2)]));
        let m = mempool_with(txns, Duration::from_millis(10), Duration::from_millis(0), 1);
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            read(&m, 0, BroadcastPeerPriority::Primary, 16).len(),
            1,
            "single-peer must re-dispatch after TTL"
        );
    }

    #[test]
    fn ttl_expired_same_slot_suppressed_when_two_priorities() {
        // After both priorities have been observed, TTL-expired same-slot
        // resend yields the slot so the alt priority can pick it up.
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 3)]));
        let m = mempool_with(txns, Duration::from_millis(10), Duration::from_millis(0), 1);
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
        // Failover queries to register the observation (in-TTL ⇒ no dispatch).
        assert!(read(&m, 0, BroadcastPeerPriority::Failover, 16).is_empty());
        std::thread::sleep(Duration::from_millis(20));
        assert!(
            read(&m, 0, BroadcastPeerPriority::Primary, 16).is_empty(),
            "multi-peer same-slot resend must suppress"
        );
    }

    #[test]
    fn ttl_expired_alt_slot_dispatches() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 4)]));
        let m = mempool_with(txns, Duration::from_millis(10), Duration::from_millis(0), 1);
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            read(&m, 0, BroadcastPeerPriority::Failover, 16).len(),
            1,
            "alt slot must dispatch after TTL"
        );
    }

    #[test]
    fn bucket_shard_isolation() {
        let txns = Arc::new(StdMutex::new(vec![
            mk_txn(0, 0, 10),
            mk_txn(1, 0, 11),
            mk_txn(2, 0, 12),
            mk_txn(3, 0, 13),
        ]));
        let m = mempool_with(txns, Duration::from_secs(60), Duration::from_millis(20), 4);
        for k in 0u8..4u8 {
            assert_eq!(
                read(&m, k, BroadcastPeerPriority::Primary, 16).len(),
                1,
                "bucket {k} should see exactly its own txn"
            );
        }
    }

    #[test]
    fn failover_first_sighting_creates_placeholder_no_dispatch() {
        // PR #722 review point 3: Failover first-sighting seeds a placeholder.
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 30)]));
        let m = mempool_with(txns, Duration::from_secs(60), Duration::from_millis(20), 1);
        assert!(read(&m, 0, BroadcastPeerPriority::Failover, 16).is_empty());
        let cache = m.txn_cache.lock().unwrap();
        assert_eq!(cache.entries.len(), 1);
        let e = cache.entries.values().next().unwrap();
        assert!(!e.dispatched, "placeholder must have dispatched == false");
    }

    #[test]
    fn primary_claims_placeholder_within_grace() {
        // PR #722 review point 3: within TTL grace, Primary claims the
        // placeholder Failover left behind.
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 31)]));
        let m = mempool_with(txns, Duration::from_secs(60), Duration::from_millis(20), 1);
        assert!(read(&m, 0, BroadcastPeerPriority::Failover, 16).is_empty());
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
        let cache = m.txn_cache.lock().unwrap();
        let e = cache.entries.values().next().unwrap();
        assert!(e.dispatched, "placeholder must flip to dispatched after Primary claim");
        assert_eq!(
            e.last_target,
            (0, priority_discriminant(&BroadcastPeerPriority::Primary)),
            "last_target must reflect Primary's slot"
        );
    }

    #[test]
    fn failover_takes_over_after_grace() {
        // PR #722 review point 3: after TTL grace elapses, Failover takes
        // over without depending on priority.rs promotion.
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 32)]));
        let m = mempool_with(txns, Duration::from_millis(10), Duration::from_millis(0), 1);
        assert!(read(&m, 0, BroadcastPeerPriority::Failover, 16).is_empty());
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Failover, 16).len(), 1);
        let cache = m.txn_cache.lock().unwrap();
        let e = cache.entries.values().next().unwrap();
        assert!(e.dispatched, "entry must flip to dispatched after Failover takeover");
        assert_eq!(
            e.last_target,
            (0, priority_discriminant(&BroadcastPeerPriority::Failover)),
            "last_target must reflect Failover's slot"
        );
    }

    #[test]
    fn lazy_gc_drops_committed_hashes() {
        let txns = Arc::new(StdMutex::new(vec![mk_txn(0, 0, 20)]));
        let m = mempool_with(
            txns.clone(),
            Duration::from_secs(60),
            Duration::from_millis(0), // every read refreshes
            1,
        );
        assert_eq!(read(&m, 0, BroadcastPeerPriority::Primary, 16).len(), 1);
        assert_eq!(m.txn_cache.lock().unwrap().entries.len(), 1);

        // Simulate commit: tx leaves the reth pool.
        txns.lock().unwrap().clear();
        std::thread::sleep(Duration::from_millis(1));
        let _ = read(&m, 0, BroadcastPeerPriority::Primary, 16);
        assert_eq!(
            m.txn_cache.lock().unwrap().entries.len(),
            0,
            "lazy GC should drop entries whose hashes are no longer in the pool"
        );
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
            txn_cache: Arc::new(Mutex::new(TxnCache::new(100_000, Duration::from_secs(60)))),
            snapshot: Arc::new(Mutex::new(Snapshot {
                shards: HashMap::new(),
                taken_at: Instant::now(),
                max_age: Duration::from_millis(20),
                initialized: false,
            })),
            topology: Arc::new(Mutex::new(ObservedTopology::new(Duration::from_secs(10)))),
            num_sender_buckets: 1,
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
