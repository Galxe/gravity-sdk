// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Block-number anchored, forward epoch synchronization.
//!
//! This module owns the ephemeral server index, versioned RPC handling, authenticated client
//! verification, and batch persistence/replay. The legacy reverse-sync implementation remains in
//! the parent module as the rolling-upgrade fallback.

use super::{BlockReader, BlockRetriever, BlockStore};
use crate::{
    consensusdb::{
        schema::{
            block::BlockNumberSchema, epoch_by_block_number::EpochByBlockNumberSchema,
            ledger_info::LedgerInfoSchema,
        },
        ConsensusDB,
    },
    network::IncomingForwardEpochSyncRequest,
    network_interface::ConsensusMsg,
};
use anyhow::{anyhow, bail, ensure};
use aptos_consensus_types::{
    block_retrieval::{NUM_RETRIES, RETRY_INTERVAL_MSEC, RPC_TIMEOUT_MSEC},
    common::Round,
    forward_epoch_sync::{
        ForwardEpochSyncBatch, ForwardEpochSyncError, ForwardEpochSyncFetchRequest,
        ForwardEpochSyncManifest, ForwardEpochSyncPrepareRequest, ForwardEpochSyncRecord,
        ForwardEpochSyncRequest, ForwardEpochSyncRequestV1, ForwardEpochSyncResponse,
        ForwardEpochSyncResponseV1,
    },
};
use futures::{future::Shared, FutureExt};
use gaptos::{
    aptos_config::network_id::PeerNetworkId,
    aptos_consensus::counters::BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC,
    aptos_crypto::{hash::CryptoHash, HashValue},
    aptos_infallible::Mutex,
    aptos_logger::prelude::*,
    aptos_network::{
        application::error::Error as NetworkError, constants::INBOUND_RPC_TIMEOUT_MS,
        protocols::network::RpcError,
    },
    aptos_schemadb::batch::SchemaBatch,
    aptos_types::{account_address::AccountAddress, ledger_info::LedgerInfoWithSignatures},
};
use lru::LruCache;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{oneshot, OwnedSemaphorePermit, Semaphore, SemaphorePermit},
    time,
};

#[derive(Clone)]
struct ForwardEpochSyncIndexEntry {
    block_number: Option<u64>,
    /// Highest execution block number reached at this consensus block. Unnumbered certifying
    /// suffix blocks retain the preceding value so they can still be used as resumable cursors.
    anchor_block_number: u64,
    block_id: HashValue,
    parent_id: HashValue,
}

#[derive(Clone)]
struct ForwardEpochSyncBoundary {
    certifying_position: usize,
    target_block_number: u64,
    ledger_info: LedgerInfoWithSignatures,
}

/// Immutable metadata snapshot for one epoch. Blocks, payloads, QCs, and randomness stay in the
/// existing databases and are loaded only for the requested batch.
struct ForwardEpochSyncIndex {
    manifest: ForwardEpochSyncManifest,
    entries: Vec<ForwardEpochSyncIndexEntry>,
    positions: HashMap<HashValue, usize>,
    boundaries: Vec<ForwardEpochSyncBoundary>,
}

fn select_forward_batch_end(start: usize, requested: usize, total: usize) -> Option<usize> {
    let end = start.saturating_add(requested).min(total);
    (end > start).then_some(end)
}

fn certifying_position_in_batch(position: usize, start: usize, end: usize) -> bool {
    position >= start && position < end
}

/// A validated fetch cursor at the end of the server index has no page to return. Keep that
/// condition distinct from protocol and verification failures so the caller can wait for the
/// already-fetched epoch target to commit instead of failing the whole forward-sync attempt.
fn decode_forward_epoch_sync_fetch_response(
    response: ForwardEpochSyncResponseV1,
) -> anyhow::Result<Option<ForwardEpochSyncBatch>> {
    match response {
        ForwardEpochSyncResponseV1::Batch(batch) => Ok(Some(batch)),
        ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound) => Ok(None),
        ForwardEpochSyncResponseV1::Error(error) => {
            bail!("Forward epoch sync fetch rejected: {error:?}")
        }
        ForwardEpochSyncResponseV1::Prepared(_) => {
            bail!("Forward epoch sync fetch returned a manifest")
        }
    }
}

const FORWARD_EPOCH_SYNC_BUSY_BACKOFF_BASE_MSEC: u64 = 500;
const FORWARD_EPOCH_SYNC_BUSY_BACKOFF_MAX_MSEC: u64 = 8_000;
/// How many times one Prepare attempt sleeps and re-asks the peers that answered `Busy` before
/// it hands the retry to the next epoch-change trigger (which arrives within about a second).
const FORWARD_EPOCH_SYNC_MAX_BUSY_RETRIES: u32 = 3;
/// Wall-clock cap on one Prepare attempt (nine probes at the 10 s timeout). Old binaries never
/// answer, so without the cap a mixed fleet would cost two probes per old peer before legacy
/// sync, and a peer answering `Busy` slowly could stretch every pass.
const FORWARD_EPOCH_SYNC_PREPARE_ATTEMPT_MSEC: u64 = 90_000;
/// Client-side timeout of one Prepare probe. The network layer drops an inbound RPC after
/// `INBOUND_RPC_TIMEOUT_MS` without writing a frame, so waiting any longer only idles.
const FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC: u64 = INBOUND_RPC_TIMEOUT_MS;
/// Pause before re-probing a peer whose Prepare timed out. A serving peer without early-`Busy`
/// (an older binary) finishes the cold build it started even though the network layer already
/// discarded its reply, so the second probe is usually a cache hit.
const FORWARD_EPOCH_SYNC_REPROBE_DELAY_MSEC: u64 = 5_000;
/// Serving side: a Prepare whose cold build is still running this long after the request was
/// received answers `Busy` while the RPC is still open, leaving 2 s for the reply to get out
/// before the network layer tears the RPC down. The build carries on in the background.
const FORWARD_EPOCH_SYNC_SLOW_BUILD_REPLY_MSEC: u64 = INBOUND_RPC_TIMEOUT_MS - 2_000;
/// How many epochs' metadata indexes a serving node keeps (5-8 MiB each for a 30k-block epoch).
const FORWARD_EPOCH_SYNC_INDEX_CACHE_EPOCHS: usize = 4;
/// Fetch progress watchdog: a serving peer that delivers fewer than
/// `FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW` blocks in one window is abandoned so the next
/// trigger can pick another peer. 10 blocks/s is about half the throughput measured for an honest
/// sync (19 blocks/s, execution-bound); a peer slower than that needs over 45 minutes per epoch.
const FORWARD_EPOCH_SYNC_PROGRESS_WINDOW_MSEC: u64 = 60_000;
const FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW: u64 = 600;

/// Exponential backoff between `Busy` replies: 0.5 s doubling up to 8 s.
///
/// `Busy` is transient by construction: a serving quota is saturated or a cold index build is
/// still running, and both clear on their own. The client therefore waits it out instead of
/// degrading to legacy sync.
struct BusyBackoff {
    attempt: u32,
}

impl BusyBackoff {
    fn new() -> Self {
        Self { attempt: 0 }
    }

    fn next_delay(&mut self) -> Duration {
        let scale = 2u64.saturating_pow(self.attempt);
        self.attempt += 1;
        Duration::from_millis(
            FORWARD_EPOCH_SYNC_BUSY_BACKOFF_BASE_MSEC
                .saturating_mul(scale)
                .min(FORWARD_EPOCH_SYNC_BUSY_BACKOFF_MAX_MSEC),
        )
    }
}

/// Watches that Fetch keeps delivering blocks, judged at page boundaries so a slow page is never
/// interrupted mid-replay. A peer that dribbles small pages passes every per-request timeout yet
/// would take days for an epoch; the window catches it without a wall-clock cap on the sync as a
/// whole, which would chop an honest 27-minute sync into pieces for nothing.
struct ProgressWatchdog {
    window_end: time::Instant,
    blocks_in_window: u64,
}

impl ProgressWatchdog {
    fn start(now: time::Instant) -> Self {
        Self {
            window_end: now + Duration::from_millis(FORWARD_EPOCH_SYNC_PROGRESS_WINDOW_MSEC),
            blocks_in_window: 0,
        }
    }

    fn window_end(&self) -> time::Instant {
        self.window_end
    }

    fn record(&mut self, blocks: u64) {
        self.blocks_in_window = self.blocks_in_window.saturating_add(blocks);
    }

    /// `false` once a window has elapsed with too few blocks. A window still running is not
    /// judged; a window that passes starts a fresh one from `now`.
    fn check(&mut self, now: time::Instant) -> bool {
        if now < self.window_end {
            return true;
        }
        let enough = self.blocks_in_window >= FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW;
        *self = Self::start(now);
        enough
    }
}

/// One Prepare reply, classified by what the client does with that peer next.
enum PrepareStep {
    Prepared(Box<ForwardEpochSyncManifest>),
    /// Forward sync exists on this peer but it is saturated right now: ask again next pass.
    Busy,
    /// Explicit rejection (`Disabled`, missing data, internal error): this peer cannot serve
    /// forward sync for this epoch.
    Rejected(ForwardEpochSyncError),
    /// No reply within the probe timeout. An old binary drops the unknown message without
    /// answering. A serving peer without early-`Busy` still finishes and caches the cold build
    /// whose reply the network layer discarded, so one more probe after a pause is worth it.
    TimedOut,
    /// Failed without waiting (not connected, send error): one more probe, without a pause.
    Unreachable(anyhow::Error),
}

/// The application layer stringifies `RpcError`, so a timeout is recognisable only by its message.
fn is_rpc_timeout(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<NetworkError>(),
        Some(NetworkError::RpcError(message)) if *message == RpcError::TimedOut.to_string()
    )
}

fn classify_prepare_reply(
    result: anyhow::Result<ForwardEpochSyncResponse>,
) -> anyhow::Result<PrepareStep> {
    let ForwardEpochSyncResponse::V1(response) = match result {
        Ok(response) => response,
        Err(error) if is_rpc_timeout(&error) => return Ok(PrepareStep::TimedOut),
        Err(error) => return Ok(PrepareStep::Unreachable(error)),
    };
    Ok(match response {
        ForwardEpochSyncResponseV1::Prepared(manifest) => PrepareStep::Prepared(manifest),
        ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Busy) => PrepareStep::Busy,
        ForwardEpochSyncResponseV1::Error(error) => PrepareStep::Rejected(error),
        ForwardEpochSyncResponseV1::Batch(_) => {
            bail!("Forward epoch sync prepare returned a batch")
        }
    })
}

/// How one Prepare attempt over all candidates ended.
enum PrepareOutcome {
    Prepared(Box<ForwardEpochSyncManifest>, AccountAddress),
    /// Every candidate rejected or never answered: nobody serves forward sync for this epoch.
    NoServingPeer,
    /// Some candidate answered `Busy` and the attempt ran out of retries or time; the next
    /// epoch-change trigger asks again.
    StillBusy,
}

/// Asks `candidates` in order, pass after pass, until one returns a manifest or `deadline`
/// passes.
///
/// A pass always runs to the end before anyone is re-asked, so a healthy candidate is reached
/// without waiting on a busy one. Peers that answer `Busy` stay for the next pass, peers that
/// reject leave, peers that do not answer get exactly one more probe. Between passes the client
/// pauses once, for as long as the longest wait any re-asked peer needs: the `Busy` backoff (at
/// most `FORWARD_EPOCH_SYNC_MAX_BUSY_RETRIES` times) or the re-probe delay of a timed-out peer.
/// Each probe gets `rpc_timeout` or whatever is left before the deadline, whichever is shorter.
async fn prepare_from_candidates<F, Fut>(
    mut request: F,
    candidates: Vec<AccountAddress>,
    rpc_timeout: Duration,
    deadline: time::Instant,
) -> anyhow::Result<PrepareOutcome>
where
    F: FnMut(AccountAddress, Duration) -> Fut,
    Fut: Future<Output = anyhow::Result<ForwardEpochSyncResponse>>,
{
    let mut queue: Vec<(AccountAddress, bool)> =
        candidates.into_iter().map(|peer| (peer, true)).collect();
    let mut backoff = BusyBackoff::new();
    let mut busy_retries = 0;
    let mut busy_seen = false;
    let out_of_time = |busy_seen: bool| {
        if busy_seen {
            PrepareOutcome::StillBusy
        } else {
            PrepareOutcome::NoServingPeer
        }
    };
    loop {
        let mut next_pass = Vec::with_capacity(queue.len());
        let mut busy_peers = 0usize;
        let mut timed_out_peers = 0usize;
        for (peer, probe_again) in queue {
            let remaining = deadline.saturating_duration_since(time::Instant::now());
            if remaining.is_zero() {
                return Ok(out_of_time(busy_seen));
            }
            match classify_prepare_reply(request(peer, rpc_timeout.min(remaining)).await)? {
                PrepareStep::Prepared(manifest) => {
                    return Ok(PrepareOutcome::Prepared(manifest, peer));
                }
                PrepareStep::Busy => {
                    busy_seen = true;
                    busy_peers += 1;
                    next_pass.push((peer, probe_again));
                }
                PrepareStep::Rejected(error) => {
                    info!(remote_peer = peer, error = ?error, "Forward epoch sync prepare rejected");
                }
                PrepareStep::TimedOut => {
                    warn!(
                        remote_peer = peer,
                        probe_again = probe_again,
                        "Forward epoch sync prepare timed out"
                    );
                    if probe_again {
                        timed_out_peers += 1;
                        next_pass.push((peer, false));
                    }
                }
                PrepareStep::Unreachable(error) => {
                    warn!(
                        remote_peer = peer,
                        error = ?error,
                        probe_again = probe_again,
                        "Forward epoch sync prepare got no reply"
                    );
                    if probe_again {
                        next_pass.push((peer, false));
                    }
                }
            }
        }
        if next_pass.is_empty() {
            return Ok(PrepareOutcome::NoServingPeer);
        }
        let mut delay = Duration::ZERO;
        if busy_peers > 0 {
            if busy_retries >= FORWARD_EPOCH_SYNC_MAX_BUSY_RETRIES {
                return Ok(PrepareOutcome::StillBusy);
            }
            busy_retries += 1;
            delay = backoff.next_delay();
        }
        if timed_out_peers > 0 {
            delay = delay.max(Duration::from_millis(FORWARD_EPOCH_SYNC_REPROBE_DELAY_MSEC));
        }
        let delay = delay.min(deadline.saturating_duration_since(time::Instant::now()));
        if !delay.is_zero() {
            info!(
                delay_ms = delay.as_millis() as u64,
                busy_peers = busy_peers,
                timed_out_peers = timed_out_peers,
                "Forward epoch sync pausing before the next Prepare pass"
            );
            time::sleep(delay).await;
        }
        queue = next_pass;
    }
}

/// How `BlockStore::fast_forward_sync_by_epoch` returned.
#[derive(Debug)]
pub enum EpochSyncOutcome {
    /// The epoch-ending ledger info is committed locally and the epoch change was announced.
    Completed,
    /// This attempt could not go further: every peer was busy, or the serving peer stalled.
    /// Everything fetched so far is persisted and replayed, so the next epoch-change trigger
    /// resumes from the ordered root, usually through another peer.
    Resume,
}

/// How one forward attempt ended, before the legacy fallback decision.
enum ForwardAttempt {
    Completed,
    Resume,
    UseLegacy,
}

/// One page of the Fetch loop.
enum PageFetch {
    Batch(ForwardEpochSyncBatch),
    EndOfData,
    /// The peer kept answering `Busy` for the rest of the progress window.
    WindowElapsed,
}

/// One Fetch exchange with the serving peer.
enum FetchStep {
    Reply(ForwardEpochSyncResponse),
    /// The peer kept answering `Busy` up to the end of the progress window; the watchdog
    /// decides whether the pages before that were enough to keep waiting.
    WindowElapsed,
}

/// Repeats one Fetch request to the serving `peer`: `Busy` backs off (never sleeping past
/// `window_end`), RPC failures retry up to `attempts` times at `RETRY_INTERVAL_MSEC`.
async fn fetch_from_peer<F, Fut>(
    peer: AccountAddress,
    mut request: F,
    attempts: usize,
    window_end: time::Instant,
) -> anyhow::Result<FetchStep>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<ForwardEpochSyncResponse>>,
{
    let attempts = attempts.max(1);
    let mut backoff = BusyBackoff::new();
    let mut rpc_failures = 0;
    loop {
        match request().await {
            Ok(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Error(
                ForwardEpochSyncError::Busy,
            ))) => {
                let now = time::Instant::now();
                if now >= window_end {
                    return Ok(FetchStep::WindowElapsed);
                }
                let delay = backoff.next_delay().min(window_end - now);
                info!(
                    remote_peer = peer,
                    delay_ms = delay.as_millis() as u64,
                    "Forward epoch sync peer is busy; backing off"
                );
                time::sleep(delay).await;
            }
            Ok(response) => return Ok(FetchStep::Reply(response)),
            Err(error) => {
                rpc_failures += 1;
                warn!(
                    remote_peer = peer,
                    error = ?error,
                    attempt = rpc_failures,
                    "Forward epoch sync RPC failed"
                );
                if rpc_failures >= attempts {
                    return Err(error);
                }
                time::sleep(Duration::from_millis(RETRY_INTERVAL_MSEC)).await;
            }
        }
    }
}

type IndexBuildResult = Result<Arc<ForwardEpochSyncIndex>, ForwardEpochSyncError>;
type PendingIndexBuild = Shared<oneshot::Receiver<IndexBuildResult>>;

/// Serving-side state of forward epoch sync, shared by every `BlockStore` of the node: the index
/// cache, the cold builds in flight, and the two admission quotas.
///
/// Indexes describe finished epochs, which never change, so the cache outlives the epoch the
/// node itself is in.
pub struct ForwardEpochSyncService {
    indexes: Mutex<ForwardEpochSyncIndexes>,
    cold_build_quota: Arc<Semaphore>,
    fetch_quota: Semaphore,
}

struct ForwardEpochSyncIndexes {
    ready: LruCache<u64, Arc<ForwardEpochSyncIndex>>,
    in_flight: HashMap<u64, PendingIndexBuild>,
    /// Peers with a cold build running. Cache hits and joiners of a running build are not
    /// counted, so an honest client (sequential, one epoch at a time) never needs a second slot.
    cold_build_peers: HashSet<AccountAddress>,
}

enum IndexLookup {
    Ready(Arc<ForwardEpochSyncIndex>),
    Building(PendingIndexBuild),
}

impl ForwardEpochSyncService {
    pub fn from_env() -> Self {
        Self::new(
            crate::forward_epoch_sync_cold_build_quota(),
            crate::forward_epoch_sync_fetch_quota(),
        )
    }

    fn new(cold_build_quota: usize, fetch_quota: usize) -> Self {
        Self {
            indexes: Mutex::new(ForwardEpochSyncIndexes {
                ready: LruCache::new(FORWARD_EPOCH_SYNC_INDEX_CACHE_EPOCHS),
                in_flight: HashMap::new(),
                cold_build_peers: HashSet::new(),
            }),
            cold_build_quota: Arc::new(Semaphore::new(cold_build_quota)),
            fetch_quota: Semaphore::new(fetch_quota),
        }
    }

    /// A Fetch handler runs only while holding one of these.
    fn try_acquire_fetch(&self) -> Option<SemaphorePermit<'_>> {
        self.fetch_quota.try_acquire().ok()
    }

    /// The index for `epoch`: cached, or built by `build` on a cold miss.
    ///
    /// A cache hit or a build already running for the epoch costs `peer` nothing. Starting a
    /// cold build takes the peer's single slot and a `cold_build_quota` permit, both held until
    /// the build finishes; if either is unavailable the answer is `Busy`. A running build is
    /// awaited until `reply_by` (not at all when `None`), after which the answer is `Busy` too;
    /// the build keeps running and the next request for the epoch joins it or finds it cached.
    async fn index(
        self: &Arc<Self>,
        epoch: u64,
        peer: AccountAddress,
        reply_by: Option<time::Instant>,
        build: impl Future<Output = Result<ForwardEpochSyncIndex, ForwardEpochSyncError>>
            + Send
            + 'static,
    ) -> IndexBuildResult {
        let pending = match self.lookup_or_start_build(epoch, peer, build)? {
            IndexLookup::Ready(index) => return Ok(index),
            IndexLookup::Building(pending) => pending,
        };
        let waited = time::Instant::now();
        let reported = match reply_by {
            Some(reply_by) => time::timeout_at(reply_by, pending).await.ok(),
            None => pending.now_or_never(),
        };
        match reported {
            Some(Ok(result)) => result,
            // The build task was dropped before it could report: the runtime is shutting down.
            Some(Err(_)) => Err(ForwardEpochSyncError::Internal),
            None => {
                info!(
                    epoch = epoch,
                    remote_peer = peer,
                    waited_ms = waited.elapsed().as_millis() as u64,
                    reason = "build_running",
                    "Forward epoch sync index is still building; answering Busy"
                );
                Err(ForwardEpochSyncError::Busy)
            }
        }
    }

    fn lookup_or_start_build(
        self: &Arc<Self>,
        epoch: u64,
        peer: AccountAddress,
        build: impl Future<Output = Result<ForwardEpochSyncIndex, ForwardEpochSyncError>>
            + Send
            + 'static,
    ) -> Result<IndexLookup, ForwardEpochSyncError> {
        let mut indexes = self.indexes.lock();
        if let Some(index) = indexes.ready.get(&epoch) {
            return Ok(IndexLookup::Ready(index.clone()));
        }
        if let Some(pending) = indexes.in_flight.get(&epoch) {
            return Ok(IndexLookup::Building(pending.clone()));
        }
        if indexes.cold_build_peers.contains(&peer) {
            warn!(
                epoch = epoch,
                remote_peer = peer,
                reason = "cold_build_per_peer",
                "Reject forward epoch sync cold build"
            );
            return Err(ForwardEpochSyncError::Busy);
        }
        let Ok(permit) = self.cold_build_quota.clone().try_acquire_owned() else {
            warn!(
                epoch = epoch,
                remote_peer = peer,
                reason = "cold_build_quota",
                "Reject forward epoch sync cold build"
            );
            return Err(ForwardEpochSyncError::Busy);
        };
        indexes.cold_build_peers.insert(peer);
        let (report, pending) = oneshot::channel();
        let pending = pending.shared();
        indexes.in_flight.insert(epoch, pending.clone());
        // The build outlives the request that started it: a requester that answered `Busy`
        // meanwhile comes back to the cache or joins here.
        let service = self.clone();
        tokio::spawn(async move {
            let build_start = Instant::now();
            let result = build.await.map(Arc::new);
            if let Ok(index) = &result {
                info!(
                    epoch = epoch,
                    entries = index.entries.len(),
                    boundaries = index.boundaries.len(),
                    build_elapsed_ms = build_start.elapsed().as_millis() as u64,
                    "Built forward epoch sync index"
                );
            }
            service.finish_cold_build(epoch, peer, permit, &result);
            // Nobody may be listening: the initiator answered `Busy` and no request joined since.
            let _ = report.send(result);
        });
        Ok(IndexLookup::Building(pending))
    }

    /// Publishes the outcome and frees the slot and the permit in one critical section, so a
    /// peer never finds its slot free while its permit is still held.
    fn finish_cold_build(
        &self,
        epoch: u64,
        peer: AccountAddress,
        permit: OwnedSemaphorePermit,
        result: &IndexBuildResult,
    ) {
        let mut indexes = self.indexes.lock();
        indexes.in_flight.remove(&epoch);
        indexes.cold_build_peers.remove(&peer);
        if let Ok(index) = result {
            indexes.ready.put(epoch, index.clone());
        }
        drop(permit);
    }
}

impl BlockStore {
    fn build_forward_epoch_sync_index(
        db: &ConsensusDB,
        epoch: u64,
    ) -> Result<ForwardEpochSyncIndex, ForwardEpochSyncError> {
        let epoch_end_block_number = db
            .get_all::<EpochByBlockNumberSchema>()
            .map_err(|error| {
                error!(epoch = epoch, error = ?error, "Failed to scan epoch boundaries for forward sync");
                ForwardEpochSyncError::Internal
            })?
            .into_iter()
            .filter_map(|(block_number, stored_epoch)| {
                (stored_epoch == epoch).then_some(block_number)
            })
            .max()
            .ok_or(ForwardEpochSyncError::EpochNotFound)?;
        let target_ledger_info = db
            .get::<LedgerInfoSchema>(&epoch_end_block_number)
            .map_err(|error| {
                error!(epoch = epoch, error = ?error, "Failed to read epoch-ending ledger info");
                ForwardEpochSyncError::Internal
            })?
            .ok_or(ForwardEpochSyncError::EpochNotFound)?;

        let start_key = (epoch, HashValue::zero());
        let end_key = (epoch, HashValue::new([u8::MAX; HashValue::LENGTH]));
        let quorum_certs = db.get_qc_range(&start_key, &end_key).map_err(|error| {
            error!(epoch = epoch, error = ?error, "Failed to read QCs for forward sync");
            ForwardEpochSyncError::Internal
        })?;
        let qcs_by_certified_id = quorum_certs
            .into_iter()
            .map(|qc| (qc.certified_block().id(), qc))
            .collect::<HashMap<_, _>>();
        let terminal_qc = qcs_by_certified_id
            .values()
            .filter(|qc| {
                qc.commit_info().id() == target_ledger_info.ledger_info().consensus_block_id()
            })
            .max_by_key(|qc| qc.certified_block().round())
            .ok_or_else(|| {
                error!(
                    epoch = epoch,
                    target = %target_ledger_info.ledger_info().consensus_block_id(),
                    "Epoch-ending commit has no certifying QC"
                );
                ForwardEpochSyncError::Internal
            })?;

        let mut reverse_entries = Vec::new();
        let mut visited = HashSet::new();
        let mut cursor = terminal_qc.certified_block().id();
        loop {
            if !visited.insert(cursor) {
                error!(epoch = epoch, block_id = %cursor, "Cycle in persisted consensus block chain");
                return Err(ForwardEpochSyncError::Internal);
            }
            let block = db.get_block(epoch, cursor).map_err(|error| {
                error!(epoch = epoch, block_id = %cursor, error = ?error, "Failed to read block");
                ForwardEpochSyncError::Internal
            })?;
            let Some(block) = block else { break };
            let block_number = match block.block_number() {
                Some(block_number) => Some(block_number),
                None => db.get::<BlockNumberSchema>(&(epoch, block.id())).map_err(|error| {
                    error!(
                        epoch = epoch,
                        block_id = %block.id(),
                        error = ?error,
                        "Failed to read forward-sync block number"
                    );
                    ForwardEpochSyncError::Internal
                })?,
            };
            if !qcs_by_certified_id.contains_key(&block.id()) {
                error!(epoch = epoch, block_id = %block.id(), "Forward-sync block has no QC");
                return Err(ForwardEpochSyncError::Internal);
            }
            reverse_entries.push(ForwardEpochSyncIndexEntry {
                block_number,
                anchor_block_number: 0,
                block_id: block.id(),
                parent_id: block.parent_id(),
            });
            cursor = block.parent_id();
        }
        reverse_entries.reverse();
        if reverse_entries.is_empty() {
            return Err(ForwardEpochSyncError::EpochNotFound);
        }
        for pair in reverse_entries.windows(2) {
            if pair[1].parent_id != pair[0].block_id {
                error!(
                    epoch = epoch,
                    parent_id = %pair[0].block_id,
                    child_id = %pair[1].block_id,
                    "Persisted epoch path is not contiguous"
                );
                return Err(ForwardEpochSyncError::Internal);
            }
        }
        let first_block_number =
            reverse_entries.iter().find_map(|entry| entry.block_number).ok_or_else(|| {
                error!(epoch = epoch, "Forward-sync epoch path has no numbered blocks");
                ForwardEpochSyncError::Internal
            })?;
        let mut anchor_block_number = first_block_number.checked_sub(1).ok_or_else(|| {
            error!(epoch = epoch, "Forward-sync epoch path starts at block number zero");
            ForwardEpochSyncError::Internal
        })?;
        for entry in &mut reverse_entries {
            if let Some(block_number) = entry.block_number {
                if block_number != anchor_block_number.saturating_add(1) {
                    error!(
                        epoch = epoch,
                        block_id = %entry.block_id,
                        previous_number = anchor_block_number,
                        block_number = block_number,
                        "Persisted numbered epoch path is not contiguous"
                    );
                    return Err(ForwardEpochSyncError::Internal);
                }
                anchor_block_number = block_number;
            }
            entry.anchor_block_number = anchor_block_number;
        }

        let positions = reverse_entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.block_id, position))
            .collect::<HashMap<_, _>>();
        let target_epoch_info = target_ledger_info.ledger_info().commit_info().epoch_block_info();
        let target_block_id = target_epoch_info
            .map(|info| info.block_id)
            .unwrap_or_else(|| target_ledger_info.ledger_info().consensus_block_id());
        let target_block_number = target_epoch_info
            .map(|info| info.block_number)
            .or_else(|| {
                positions
                    .get(&target_block_id)
                    .and_then(|pos| reverse_entries[*pos].block_number)
            })
            .ok_or_else(|| {
                error!(epoch = epoch, target = %target_block_id, "Epoch target is not on canonical path");
                ForwardEpochSyncError::Internal
            })?;

        // Ledger infos are keyed by block number and the consensus DB is never pruned, so a
        // whole-table scan grows with chain height (tens of millions of rows on a mature node)
        // even though this epoch only spans [first_block_number, target_block_number].
        let persisted_ledger_infos = db
            .ledger_db
            .metadata_db()
            .get_ledger_infos_by_range((first_block_number, target_block_number + 1))
            .map_err(|error| {
                error!(epoch = epoch, error = ?error, "Failed to read ledger infos for forward sync");
                ForwardEpochSyncError::Internal
            })?;
        // A ledger info is attached at the earliest canonical position whose QC commits it.
        // Resolve that once per commit id instead of rescanning every QC per ledger info.
        let mut certifying_position_by_commit_id: HashMap<HashValue, (Round, usize)> =
            HashMap::new();
        for qc in qcs_by_certified_id.values() {
            let Some(&position) = positions.get(&qc.certified_block().id()) else { continue };
            let candidate = (qc.certified_block().round(), position);
            certifying_position_by_commit_id
                .entry(qc.commit_info().id())
                .and_modify(|best| {
                    if candidate.0 < best.0 {
                        *best = candidate;
                    }
                })
                .or_insert(candidate);
        }
        let mut boundaries = Vec::new();
        for ledger_info in persisted_ledger_infos {
            if ledger_info.ledger_info().epoch() != epoch {
                continue;
            }
            let Some(&(_, certifying_position)) = certifying_position_by_commit_id
                .get(&ledger_info.ledger_info().consensus_block_id())
            else {
                continue;
            };
            let epoch_info = ledger_info.ledger_info().commit_info().epoch_block_info();
            let boundary_id = epoch_info
                .map(|info| info.block_id)
                .unwrap_or_else(|| ledger_info.ledger_info().consensus_block_id());
            let Some(target_position) = positions.get(&boundary_id).copied() else {
                continue;
            };
            if target_position > certifying_position {
                continue;
            }
            let boundary_number = ledger_info.ledger_info().block_number();
            boundaries.push(ForwardEpochSyncBoundary {
                certifying_position,
                target_block_number: boundary_number,
                ledger_info,
            });
        }
        boundaries.sort_unstable_by_key(|boundary| {
            (boundary.certifying_position, boundary.target_block_number)
        });

        let terminal = reverse_entries.last().expect("non-empty checked above");
        let manifest_bytes = bcs::to_bytes(&(
            epoch,
            first_block_number,
            terminal.anchor_block_number,
            terminal.block_id,
            target_block_number,
            target_block_id,
            &target_ledger_info,
        ))
        .map_err(|error| {
            error!(epoch = epoch, error = ?error, "Failed to hash forward-sync manifest");
            ForwardEpochSyncError::Internal
        })?;
        let manifest = ForwardEpochSyncManifest {
            epoch,
            manifest_id: HashValue::sha3_256_of(&manifest_bytes),
            first_block_number,
            target_block_number,
            target_block_id,
            target_ledger_info,
        };
        Ok(ForwardEpochSyncIndex { manifest, entries: reverse_entries, positions, boundaries })
    }

    async fn forward_epoch_sync_index(
        &self,
        peer: AccountAddress,
        epoch: u64,
        reply_by: Option<time::Instant>,
    ) -> IndexBuildResult {
        let db = self.storage.consensus_db();
        self.forward_epoch_sync_service
            .index(epoch, peer, reply_by, async move {
                // The build scans RocksDB; keep it off the async workers.
                match tokio::task::spawn_blocking(move || {
                    Self::build_forward_epoch_sync_index(&db, epoch)
                })
                .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        error!(epoch = epoch, error = ?error, "Forward epoch sync index build task failed");
                        Err(ForwardEpochSyncError::Internal)
                    }
                }
            })
            .await
    }

    fn validate_forward_anchor(
        index: &ForwardEpochSyncIndex,
        block_number: u64,
        block_id: HashValue,
    ) -> Result<usize, ForwardEpochSyncError> {
        if let Some(position) = index.positions.get(&block_id).copied() {
            return (index.entries[position].anchor_block_number == block_number)
                .then_some(position.saturating_add(1))
                .ok_or(ForwardEpochSyncError::AnchorMismatch);
        }
        let first = index.entries.first().ok_or(ForwardEpochSyncError::EpochNotFound)?;
        let first_follows_anchor = match first.block_number {
            Some(first_number) => first_number == block_number.saturating_add(1),
            None => first.anchor_block_number == block_number,
        };
        if first.parent_id == block_id && first_follows_anchor {
            Ok(0)
        } else {
            Err(ForwardEpochSyncError::AnchorMismatch)
        }
    }

    async fn prepare_forward_epoch_sync(
        &self,
        peer: AccountAddress,
        request: ForwardEpochSyncPrepareRequest,
        reply_by: time::Instant,
    ) -> ForwardEpochSyncResponseV1 {
        let index = match self.forward_epoch_sync_index(peer, request.epoch, Some(reply_by)).await {
            Ok(index) => index,
            Err(error) => return ForwardEpochSyncResponseV1::Error(error),
        };
        match Self::validate_forward_anchor(
            &index,
            request.anchor_block_number,
            request.anchor_block_id,
        ) {
            Ok(_) => ForwardEpochSyncResponseV1::Prepared(Box::new(index.manifest.clone())),
            Err(error) => ForwardEpochSyncResponseV1::Error(error),
        }
    }

    async fn fetch_forward_epoch_sync(
        &self,
        peer: AccountAddress,
        request: ForwardEpochSyncFetchRequest,
        max_blocks_allowed: u64,
    ) -> ForwardEpochSyncResponseV1 {
        if request.batch_size_blocks == 0 || request.batch_size_blocks > max_blocks_allowed {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::InvalidBatchSize);
        }
        let Some(_permit) = self.forward_epoch_sync_service.try_acquire_fetch() else {
            warn!(
                remote_peer = peer,
                epoch = request.epoch,
                reason = "fetch_quota",
                "Reject forward epoch sync fetch because the service is busy"
            );
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Busy);
        };
        // A Fetch never waits for a cold build (a miss here means the serving node restarted or
        // evicted the epoch): its client times out after `RPC_TIMEOUT_MSEC`, well inside the
        // inbound cap, and its `Busy` backoff polls until the index is ready.
        let index = match self.forward_epoch_sync_index(peer, request.epoch, None).await {
            Ok(index) => index,
            Err(error) => return ForwardEpochSyncResponseV1::Error(error),
        };
        if request.manifest_id != index.manifest.manifest_id {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::ManifestMismatch);
        }
        let start = match Self::validate_forward_anchor(
            &index,
            request.anchor_block_number,
            request.anchor_block_id,
        ) {
            Ok(start) => start,
            Err(error) => return ForwardEpochSyncResponseV1::Error(error),
        };
        if start >= index.entries.len() {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound);
        }
        let requested = usize::try_from(request.batch_size_blocks).unwrap_or(usize::MAX);
        let Some(end) = select_forward_batch_end(start, requested, index.entries.len()) else {
            return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound);
        };
        let ledger_infos = index
            .boundaries
            .iter()
            .filter(|boundary| {
                certifying_position_in_batch(boundary.certifying_position, start, end)
            })
            .map(|boundary| boundary.ledger_info.clone())
            .collect::<Vec<_>>();

        let db = self.storage.consensus_db();
        let mut records = Vec::with_capacity(end - start);
        for entry in &index.entries[start..end] {
            let block = match db.get_block(request.epoch, entry.block_id) {
                Ok(Some(block)) => block,
                Ok(None) => {
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal)
                }
                Err(error) => {
                    error!(epoch = request.epoch, block_id = %entry.block_id, error = ?error, "Failed to read forward-sync block");
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal);
                }
            };
            let quorum_cert = match db.get_qc(request.epoch, entry.block_id) {
                Ok(Some(qc)) => qc,
                Ok(None) => {
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal)
                }
                Err(error) => {
                    error!(epoch = request.epoch, block_id = %entry.block_id, error = ?error, "Failed to read forward-sync QC");
                    return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal);
                }
            };
            let randomness = match entry.block_number {
                Some(block_number) => match db.get_randomness(block_number) {
                    Ok(randomness) => randomness,
                    Err(error) => {
                        error!(epoch = request.epoch, block_number = block_number, error = ?error, "Failed to read forward-sync randomness");
                        return ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::Internal);
                    }
                },
                None => None,
            };
            records.push(ForwardEpochSyncRecord {
                block,
                block_number: entry.block_number,
                randomness,
                quorum_cert,
            });
        }
        let tail = index.entries.get(end - 1).expect("non-empty batch");
        ForwardEpochSyncResponseV1::Batch(ForwardEpochSyncBatch {
            epoch: request.epoch,
            manifest_id: request.manifest_id,
            anchor_block_number: request.anchor_block_number,
            anchor_block_id: request.anchor_block_id,
            records,
            ledger_infos,
            next_anchor_block_number: tail.anchor_block_number,
            next_anchor_block_id: tail.block_id,
        })
    }

    pub async fn process_forward_epoch_sync(
        &self,
        request: IncomingForwardEpochSyncRequest,
        max_blocks_allowed: u64,
    ) -> anyhow::Result<()> {
        let remote_peer = request.sender;
        let (kind, epoch) = match &request.req {
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Prepare(prepare)) => {
                ("Prepare", prepare.epoch)
            }
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Fetch(fetch)) => {
                ("Fetch", fetch.epoch)
            }
        };
        info!(
            remote_peer = remote_peer,
            epoch = epoch,
            kind = kind,
            queued_ms = request.received_at.elapsed().as_millis() as u64,
            "Received forward epoch sync request"
        );
        let started = Instant::now();
        let response = match request.req {
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Prepare(prepare)) => {
                let reply_by = request.received_at +
                    Duration::from_millis(FORWARD_EPOCH_SYNC_SLOW_BUILD_REPLY_MSEC);
                self.prepare_forward_epoch_sync(remote_peer, prepare, reply_by).await
            }
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Fetch(fetch)) => {
                self.fetch_forward_epoch_sync(remote_peer, fetch, max_blocks_allowed).await
            }
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let (result, detail) = match &response {
            ForwardEpochSyncResponseV1::Prepared(manifest) => (
                "Prepared",
                format!(
                    "manifest_id={} first_bn={} target_bn={}",
                    manifest.manifest_id, manifest.first_block_number, manifest.target_block_number
                ),
            ),
            ForwardEpochSyncResponseV1::Batch(batch) => (
                "Batch",
                format!(
                    "records={} ledger_infos={} next_anchor_bn={}",
                    batch.records.len(),
                    batch.ledger_infos.len(),
                    batch.next_anchor_block_number
                ),
            ),
            ForwardEpochSyncResponseV1::Error(error) => ("Error", format!("{error:?}")),
        };
        info!(
            remote_peer = remote_peer,
            epoch = epoch,
            kind = kind,
            result = result,
            detail = %detail,
            elapsed_ms = elapsed_ms,
            "Responded forward epoch sync request"
        );
        let response = ConsensusMsg::ForwardEpochSyncResponse(Box::new(
            ForwardEpochSyncResponse::V1(response),
        ));
        let response_bytes = request.protocol.to_bytes(&response)?;
        request
            .response_sender
            .send(Ok(response_bytes.into()))
            .map_err(|_| anyhow::anyhow!("Failed to send forward epoch sync response"))
    }
}

impl BlockStore {
    /// Fast-forwards the local consensus state by synchronizing blocks and ledger infos for a given
    /// epoch.
    ///
    /// This function retrieves all blocks, quorum certificates, and ledger infos for the specified
    /// epoch from a remote retriever. It then prefetches payload data for each block, saves the
    /// blocks and certificates to local storage, and updates the ledger info in the database.
    /// After updating storage, it attempts to recover the consensus state from the latest
    /// ledger info and rebuilds the in-memory state. If the epoch ends, it sends an epoch
    /// change proof to the network.
    ///
    /// # Arguments
    /// * `retriever` - The block retriever used to fetch blocks and related data.
    /// * `epoch` - The epoch to fast-forward to.
    ///
    /// # Returns
    /// * `Ok(EpochSyncOutcome::Completed)` if the synchronization and state rebuild succeed.
    /// * `Ok(EpochSyncOutcome::Resume)` if the serving peer stalled; the next trigger continues.
    /// * `Err` if any step fails.
    pub async fn fast_forward_sync_by_epoch(
        &self,
        mut retriever: BlockRetriever,
        epoch: u64,
        batch_size_blocks: u64,
    ) -> anyhow::Result<EpochSyncOutcome> {
        if !crate::forward_epoch_sync_enabled() {
            info!(
                epoch = epoch,
                "Forward epoch sync is not enabled; using legacy reverse epoch sync"
            );
            self.fast_forward_sync_by_epoch_legacy(retriever, epoch).await?;
            return Ok(EpochSyncOutcome::Completed);
        }
        ensure!(batch_size_blocks > 0, "Forward epoch sync batch size must be positive");
        match self
            .fast_forward_sync_by_epoch_forward(&mut retriever, epoch, batch_size_blocks)
            .await?
        {
            ForwardAttempt::Completed => Ok(EpochSyncOutcome::Completed),
            ForwardAttempt::Resume => Ok(EpochSyncOutcome::Resume),
            ForwardAttempt::UseLegacy => {
                info!(epoch = epoch, "Falling back to legacy reverse epoch sync");
                self.fast_forward_sync_by_epoch_legacy(retriever, epoch).await?;
                Ok(EpochSyncOutcome::Completed)
            }
        }
    }

    async fn fast_forward_sync_by_epoch_forward(
        &self,
        retriever: &mut BlockRetriever,
        epoch: u64,
        batch_size_blocks: u64,
    ) -> anyhow::Result<ForwardAttempt> {
        let fetch_root = self.ordered_root();
        let mut fetch_anchor_block_number = fetch_root
            .block()
            .block_number()
            .ok_or_else(|| anyhow!("Ordered root has no block number"))?;
        let mut fetch_anchor_block_id = fetch_root.id();

        let (manifest, serving_peer) = match retriever
            .try_prepare_forward_epoch_sync(epoch, fetch_anchor_block_number, fetch_anchor_block_id)
            .await?
        {
            PrepareOutcome::Prepared(manifest, serving_peer) => (*manifest, serving_peer),
            PrepareOutcome::NoServingPeer => return Ok(ForwardAttempt::UseLegacy),
            PrepareOutcome::StillBusy => return Ok(ForwardAttempt::Resume),
        };
        info!(
            epoch = epoch,
            manifest_id = manifest.manifest_id,
            first_block_number = manifest.first_block_number,
            target_block_number = manifest.target_block_number,
            batch_size_blocks = batch_size_blocks,
            "Prepared forward epoch sync"
        );

        if self.forward_epoch_sync_target_committed(&manifest)? {
            // The blocks and epoch-ending LI are durable, but the self-directed epoch-change
            // message is not. Re-send it after restart before reporting the sync as complete.
            self.send_committed_epoch_change(retriever, &manifest.target_ledger_info).await?;
            return Ok(ForwardAttempt::Completed);
        }

        // The live ordered pipeline can already contain the block targeted by the epoch-ending
        // commit decision when sync starts. In that case Prepare supplied the missing signed
        // decision, so commit the existing local path instead of fetching past the snapshot tail.
        if self.forward_epoch_sync_commit_proof_target_is_ordered(&manifest) {
            self.persist_forward_epoch_sync_ledger_infos(std::slice::from_ref(
                &manifest.target_ledger_info,
            ))?;
            retriever.network.send_commit_proof(manifest.target_ledger_info.clone()).await;
            if self.wait_for_forward_epoch_sync_target(&manifest).await? {
                self.send_committed_epoch_change(retriever, &manifest.target_ledger_info).await?;
                return Ok(ForwardAttempt::Completed);
            }
            info!(
                epoch = epoch,
                target_block_number = manifest.target_block_number,
                "Local ordered epoch target did not commit in time; use legacy fallback"
            );
            return Ok(ForwardAttempt::UseLegacy);
        }

        let mut watchdog = ProgressWatchdog::start(time::Instant::now());
        loop {
            let request = ForwardEpochSyncFetchRequest {
                epoch,
                manifest_id: manifest.manifest_id,
                anchor_block_number: fetch_anchor_block_number,
                anchor_block_id: fetch_anchor_block_id,
                batch_size_blocks,
            };
            let batch = match retriever
                .fetch_forward_epoch_sync_batch(
                    request.clone(),
                    serving_peer,
                    watchdog.window_end(),
                )
                .await?
            {
                PageFetch::Batch(batch) => batch,
                PageFetch::WindowElapsed => {
                    if !watchdog.check(time::Instant::now()) {
                        info!(
                            epoch = epoch,
                            remote_peer = serving_peer,
                            fetch_anchor_block_number = fetch_anchor_block_number,
                            "Forward epoch sync peer stayed busy for a whole window; resume on the next trigger"
                        );
                        return Ok(ForwardAttempt::Resume);
                    }
                    continue;
                }
                PageFetch::EndOfData => {
                    if self.forward_epoch_sync_target_committed(&manifest)? {
                        self.send_committed_epoch_change(retriever, &manifest.target_ledger_info)
                            .await?;
                        return Ok(ForwardAttempt::Completed);
                    }
                    self.ensure_forward_epoch_sync_target_fetched(&manifest)?;
                    // All server pages have been consumed and the authenticated epoch target is
                    // local. Re-submit its decision to unblock an ordered-but-not-committed
                    // pipeline, then give execution a bounded window to advance the commit root.
                    self.persist_forward_epoch_sync_ledger_infos(std::slice::from_ref(
                        &manifest.target_ledger_info,
                    ))?;
                    retriever.network.send_commit_proof(manifest.target_ledger_info.clone()).await;
                    if self.wait_for_forward_epoch_sync_target(&manifest).await? {
                        self.send_committed_epoch_change(retriever, &manifest.target_ledger_info)
                            .await?;
                        return Ok(ForwardAttempt::Completed);
                    }
                    info!(
                        epoch = epoch,
                        target_block_number = manifest.target_block_number,
                        fetch_anchor_block_number = fetch_anchor_block_number,
                        "Forward epoch sync reached end of data before target committed; use legacy fallback"
                    );
                    return Ok(ForwardAttempt::UseLegacy);
                }
            };

            BLOCKS_FETCHED_FROM_NETWORK_WHILE_FAST_FORWARD_SYNC.inc_by(batch.records.len() as u64);
            self.persist_and_process_forward_epoch_sync_batch(&batch).await?;
            watchdog.record(batch.records.len() as u64);

            let committed = self.commit_root();
            let committed_number = committed
                .block()
                .block_number()
                .ok_or_else(|| anyhow!("Commit root has no block number after forward replay"))?;

            fetch_anchor_block_number = batch.next_anchor_block_number;
            fetch_anchor_block_id = batch.next_anchor_block_id;

            info!(
                epoch = epoch,
                fetch_anchor_block_number = fetch_anchor_block_number,
                committed_block_number = committed_number,
                ledger_info_count = batch.ledger_infos.len(),
                "Forward epoch sync batch persisted and processed"
            );
            if self.forward_epoch_sync_target_committed(&manifest)? {
                self.send_committed_epoch_change(retriever, &manifest.target_ledger_info).await?;
                return Ok(ForwardAttempt::Completed);
            }
            if !watchdog.check(time::Instant::now()) {
                info!(
                    epoch = epoch,
                    remote_peer = serving_peer,
                    fetch_anchor_block_number = fetch_anchor_block_number,
                    "Forward epoch sync peer delivered too few blocks in a window; resume on the next trigger"
                );
                return Ok(ForwardAttempt::Resume);
            }
        }
    }

    fn ensure_forward_epoch_sync_target_fetched(
        &self,
        manifest: &ForwardEpochSyncManifest,
    ) -> anyhow::Result<()> {
        let target = self.get_block(manifest.target_block_id).ok_or_else(|| {
            anyhow!(
                "Forward epoch sync source exhausted before target block {} was fetched",
                manifest.target_block_id
            )
        })?;
        ensure!(
            target.block().block_number() == Some(manifest.target_block_number),
            "Forward epoch sync target block number does not match manifest"
        );
        let commit_proof_target = manifest.target_ledger_info.ledger_info().commit_info().id();
        ensure!(
            self.block_exists(commit_proof_target),
            "Forward epoch sync source exhausted before commit-proof target {} was fetched",
            commit_proof_target
        );
        Ok(())
    }

    fn forward_epoch_sync_target_committed(
        &self,
        manifest: &ForwardEpochSyncManifest,
    ) -> anyhow::Result<bool> {
        let committed = self.commit_root();
        let committed_number = committed
            .block()
            .block_number()
            .ok_or_else(|| anyhow!("Commit root has no block number during forward epoch sync"))?;
        if committed_number == manifest.target_block_number {
            ensure!(
                committed.id() == manifest.target_block_id,
                "Forward epoch sync target conflicts with local commit root"
            );
        }
        Ok(committed_number >= manifest.target_block_number)
    }

    fn forward_epoch_sync_commit_proof_target_is_ordered(
        &self,
        manifest: &ForwardEpochSyncManifest,
    ) -> bool {
        // For a non-blocking epoch change the signed commit decision can point at a suffix block,
        // while `target_block_id` is the earlier epoch-change boundary. The buffer manager indexes
        // commit decisions by `commit_info().id()`, so only take this recovery path after that
        // exact block is already present on the local ordered path. Otherwise ordinary
        // paging must keep fetching the suffix until the proof can be applied.
        let commit_proof_target = manifest.target_ledger_info.ledger_info().commit_info().id();
        self.path_from_commit_root(self.ordered_root().id())
            .is_some_and(|path| path.iter().any(|block| block.id() == commit_proof_target))
    }

    async fn wait_for_forward_epoch_sync_target(
        &self,
        manifest: &ForwardEpochSyncManifest,
    ) -> anyhow::Result<bool> {
        match time::timeout(Duration::from_millis(RPC_TIMEOUT_MSEC), async {
            loop {
                if self.forward_epoch_sync_target_committed(manifest)? {
                    return Ok::<(), anyhow::Error>(());
                }
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        {
            Ok(result) => {
                result?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    fn persist_forward_epoch_sync_ledger_infos(
        &self,
        ledger_infos: &[LedgerInfoWithSignatures],
    ) -> anyhow::Result<()> {
        if ledger_infos.is_empty() {
            return Ok(());
        }
        let consensus_db = self.storage.consensus_db();
        let metadata_db = consensus_db.ledger_db.metadata_db();
        let mut ledger_info_batch = SchemaBatch::new();
        for ledger_info in ledger_infos {
            metadata_db.put_ledger_info(ledger_info, &mut ledger_info_batch)?;
        }
        metadata_db.write_schemas(ledger_info_batch)?;
        metadata_db.update_latest_ledger_info()?;
        Ok(())
    }

    async fn persist_and_process_forward_epoch_sync_batch(
        &self,
        batch: &ForwardEpochSyncBatch,
    ) -> anyhow::Result<()> {
        for record in &batch.records {
            if let Some(payload) = record.block.payload() {
                self.payload_manager.prefetch_payload_data(payload, record.block.timestamp_usecs());
            }
        }
        let blocks = batch.records.iter().map(|record| record.block.clone()).collect::<Vec<_>>();
        let quorum_certs =
            batch.records.iter().map(|record| record.quorum_cert.clone()).collect::<Vec<_>>();
        let block_numbers = batch
            .records
            .iter()
            .filter_map(|record| {
                record
                    .block_number
                    .map(|block_number| (batch.epoch, block_number, record.block.id()))
            })
            .collect::<Vec<_>>();
        self.storage.save_tree(blocks, quorum_certs.clone(), block_numbers)?;
        self.storage.consensus_db().put_randomness(
            &batch
                .records
                .iter()
                .filter_map(|record| record.block_number.zip(record.randomness.clone()))
                .collect::<Vec<_>>(),
        )?;

        self.persist_forward_epoch_sync_ledger_infos(&batch.ledger_infos)?;

        let sync_blocks = batch
            .records
            .iter()
            .map(|record| (record.block.clone(), record.block_number, record.randomness.clone()))
            .collect();
        self.append_blocks_for_sync_checked(sync_blocks, quorum_certs).await
    }
}

impl BlockRetriever {
    /// Asks peers (preferred first, then random order) until one returns a manifest, within
    /// `FORWARD_EPOCH_SYNC_PREPARE_ATTEMPT_MSEC`.
    async fn prepare_forward_epoch_sync_from_any_peer(
        &mut self,
        request: ForwardEpochSyncRequest,
        rpc_timeout: Duration,
    ) -> anyhow::Result<PrepareOutcome> {
        ensure!(!self.available_peers.is_empty(), "No peers available for forward epoch sync");
        let mut pool = self.available_peers.clone();
        let mut candidates = vec![self.pick_peer(true, &mut pool)];
        while !pool.is_empty() {
            candidates.push(self.pick_peer(false, &mut pool));
        }

        let network = self.network.clone();
        let network_id = self.network_id;
        prepare_from_candidates(
            move |peer, timeout| {
                let network = network.clone();
                let request = request.clone();
                async move {
                    network
                        .request_forward_epoch_sync(
                            request,
                            PeerNetworkId::new(network_id, peer),
                            timeout,
                        )
                        .await
                }
            },
            candidates,
            rpc_timeout,
            time::Instant::now() + Duration::from_millis(FORWARD_EPOCH_SYNC_PREPARE_ATTEMPT_MSEC),
        )
        .await
    }

    /// `NoServingPeer` means legacy sync is the right fallback; `StillBusy` means a peer does
    /// support forward sync but is saturated, so the next epoch-change trigger should ask again
    /// rather than degrade.
    async fn try_prepare_forward_epoch_sync(
        &mut self,
        epoch: u64,
        anchor_block_number: u64,
        anchor_block_id: HashValue,
    ) -> anyhow::Result<PrepareOutcome> {
        let request = ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Prepare(
            ForwardEpochSyncPrepareRequest { epoch, anchor_block_number, anchor_block_id },
        ));
        // A peer that cannot decode the appended message variant drops it silently, so an old
        // binary also shows up as a timeout.
        info!(
            epoch = epoch,
            prepare_timeout_msec = FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC,
            "Trying forward epoch sync Prepare"
        );
        let (manifest, serving_peer) = match self
            .prepare_forward_epoch_sync_from_any_peer(
                request,
                Duration::from_millis(FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC),
            )
            .await?
        {
            PrepareOutcome::Prepared(manifest, serving_peer) => (manifest, serving_peer),
            PrepareOutcome::NoServingPeer => {
                info!(epoch = epoch, "No peer can serve forward epoch sync; use legacy fallback");
                return Ok(PrepareOutcome::NoServingPeer);
            }
            PrepareOutcome::StillBusy => {
                info!(
                    epoch = epoch,
                    "Forward epoch sync peers stayed busy; retry on the next trigger"
                );
                return Ok(PrepareOutcome::StillBusy);
            }
        };

        ensure!(manifest.epoch == epoch, "Forward manifest epoch mismatch");
        ensure!(
            manifest.target_ledger_info.ledger_info().epoch() == epoch,
            "Forward manifest target LI epoch mismatch"
        );
        ensure!(
            manifest.target_ledger_info.ledger_info().ends_epoch(),
            "Forward manifest target does not end epoch"
        );
        manifest.target_ledger_info.verify_signatures(self.network.validators())?;
        let epoch_info = manifest.target_ledger_info.ledger_info().commit_info().epoch_block_info();
        let expected_target_id = epoch_info
            .map(|info| info.block_id)
            .unwrap_or_else(|| manifest.target_ledger_info.ledger_info().consensus_block_id());
        let expected_target_number = epoch_info
            .map(|info| info.block_number)
            .unwrap_or_else(|| manifest.target_ledger_info.ledger_info().block_number());
        ensure!(
            manifest.target_block_id == expected_target_id &&
                manifest.target_block_number == expected_target_number,
            "Forward manifest target mismatch"
        );
        ensure!(
            manifest.first_block_number <= manifest.target_block_number,
            "Forward manifest block range is invalid"
        );
        Ok(PrepareOutcome::Prepared(manifest, serving_peer))
    }

    /// `EndOfData` only when the server accepted the cursor and reported that no page remains.
    /// All other server and verification failures remain hard errors.
    async fn fetch_forward_epoch_sync_batch(
        &self,
        request: ForwardEpochSyncFetchRequest,
        serving_peer: AccountAddress,
        window_end: time::Instant,
    ) -> anyhow::Result<PageFetch> {
        let network = self.network.clone();
        let peer = PeerNetworkId::new(self.network_id, serving_peer);
        let rpc_request =
            ForwardEpochSyncRequest::V1(ForwardEpochSyncRequestV1::Fetch(request.clone()));
        let step = fetch_from_peer(
            serving_peer,
            move || {
                let network = network.clone();
                let rpc_request = rpc_request.clone();
                async move {
                    network
                        .request_forward_epoch_sync(
                            rpc_request,
                            peer,
                            Duration::from_millis(RPC_TIMEOUT_MSEC),
                        )
                        .await
                }
            },
            NUM_RETRIES,
            window_end,
        )
        .await?;
        let ForwardEpochSyncResponse::V1(response) = match step {
            FetchStep::Reply(response) => response,
            FetchStep::WindowElapsed => return Ok(PageFetch::WindowElapsed),
        };
        let Some(batch) = decode_forward_epoch_sync_fetch_response(response)? else {
            return Ok(PageFetch::EndOfData);
        };
        self.verify_forward_epoch_sync_batch(&request, &batch)?;
        Ok(PageFetch::Batch(batch))
    }

    fn verify_forward_epoch_sync_batch(
        &self,
        request: &ForwardEpochSyncFetchRequest,
        batch: &ForwardEpochSyncBatch,
    ) -> anyhow::Result<()> {
        ensure!(batch.epoch == request.epoch, "Forward batch epoch mismatch");
        ensure!(batch.manifest_id == request.manifest_id, "Forward batch manifest mismatch");
        ensure!(
            batch.anchor_block_number == request.anchor_block_number &&
                batch.anchor_block_id == request.anchor_block_id,
            "Forward batch anchor echo mismatch"
        );
        ensure!(!batch.records.is_empty(), "Forward batch is empty");
        ensure!(
            batch.records.len() as u64 <= request.batch_size_blocks,
            "Forward batch exceeds requested size"
        );

        let mut expected_parent = request.anchor_block_id;
        let mut anchor_block_number = request.anchor_block_number;
        for record in &batch.records {
            ensure!(record.block.epoch() == request.epoch, "Forward block epoch mismatch");
            ensure!(
                record.block.id() == record.block.block_data().hash(),
                "Forward block ID does not match its contents"
            );
            ensure!(record.block.parent_id() == expected_parent, "Forward blocks are not chained");
            if let Some(block_number) = record.block_number {
                ensure!(
                    block_number == anchor_block_number.saturating_add(1),
                    "Forward block number gap"
                );
                anchor_block_number = block_number;
            } else {
                ensure!(record.randomness.is_none(), "Unnumbered forward block carries randomness");
            }
            if let Some(embedded_number) = record.block.block_number() {
                ensure!(
                    Some(embedded_number) == record.block_number,
                    "Forward block carries conflicting block number"
                );
            }
            record.block.validate_signature(self.network.validators())?;
            record.block.verify_well_formed()?;
            ensure!(
                record.quorum_cert.certified_block().id() == record.block.id(),
                "Forward QC certifies a different block"
            );
            record.quorum_cert.verify(self.network.validators())?;
            expected_parent = record.block.id();
        }
        let tail = batch.records.last().expect("non-empty checked above");
        ensure!(
            batch.next_anchor_block_id == tail.block.id() &&
                batch.next_anchor_block_number == anchor_block_number,
            "Forward batch next anchor is not its tail"
        );
        for ledger_info in &batch.ledger_infos {
            ensure!(
                ledger_info.ledger_info().epoch() == request.epoch,
                "Forward ledger info belongs to a different epoch"
            );
            ledger_info.verify_signatures(self.network.validators())?;
            ensure!(
                batch.records.iter().any(|record| {
                    record.quorum_cert.commit_info().id() ==
                        ledger_info.ledger_info().consensus_block_id()
                }),
                "Forward ledger info has no certifying QC in its batch"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod forward_epoch_sync_tests {
    use super::{
        certifying_position_in_batch, decode_forward_epoch_sync_fetch_response, fetch_from_peer,
        prepare_from_candidates, select_forward_batch_end, BlockStore, BusyBackoff, FetchStep,
        ForwardEpochSyncIndex, ForwardEpochSyncService, PrepareOutcome, ProgressWatchdog,
        FORWARD_EPOCH_SYNC_INDEX_CACHE_EPOCHS, FORWARD_EPOCH_SYNC_MAX_BUSY_RETRIES,
        FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW, FORWARD_EPOCH_SYNC_PREPARE_ATTEMPT_MSEC,
        FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC, FORWARD_EPOCH_SYNC_PROGRESS_WINDOW_MSEC,
        FORWARD_EPOCH_SYNC_REPROBE_DELAY_MSEC, FORWARD_EPOCH_SYNC_SLOW_BUILD_REPLY_MSEC,
    };
    use crate::consensusdb::{
        schema::{epoch_by_block_number::EpochByBlockNumberSchema, ledger_info::LedgerInfoSchema},
        ConsensusDB,
    };
    use aptos_consensus_types::{
        block::{block_test_utils::certificate_for_genesis, Block},
        block_retrieval::{NUM_RETRIES, RETRY_INTERVAL_MSEC},
        common::Payload,
        forward_epoch_sync::{
            ForwardEpochSyncError, ForwardEpochSyncManifest, ForwardEpochSyncResponse,
            ForwardEpochSyncResponseV1,
        },
        quorum_cert::QuorumCert,
        vote_data::VoteData,
    };
    use gaptos::{
        aptos_crypto::HashValue,
        aptos_network::{application::error::Error as NetworkError, protocols::network::RpcError},
        aptos_temppath::TempPath,
        aptos_types::{
            account_address::AccountAddress,
            aggregate_signature::AggregateSignature,
            block_info::BlockInfo,
            ledger_info::{LedgerInfo, LedgerInfoWithSignatures},
            validator_signer::ValidatorSigner,
        },
    };
    use std::{
        cell::RefCell,
        collections::HashMap,
        future::{self, Future},
        path::PathBuf,
        rc::Rc,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };
    use tokio::time;

    #[test]
    fn forward_batches_are_regular_pages() {
        assert_eq!(select_forward_batch_end(0, 3, 10), Some(3));
        assert_eq!(select_forward_batch_end(3, 4, 10), Some(7));
        assert_eq!(select_forward_batch_end(7, 4, 10), Some(10));
    }

    #[test]
    fn forward_batch_has_no_page_after_end() {
        assert_eq!(select_forward_batch_end(10, 4, 10), None);
    }

    #[test]
    fn batch_boundary_not_found_is_end_of_data() {
        let response =
            ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::BatchBoundaryNotFound);
        assert!(decode_forward_epoch_sync_fetch_response(response).unwrap().is_none());
    }

    #[test]
    fn other_fetch_errors_are_not_end_of_data() {
        let response = ForwardEpochSyncResponseV1::Error(ForwardEpochSyncError::AnchorMismatch);
        assert!(decode_forward_epoch_sync_fetch_response(response).is_err());
    }

    #[test]
    fn proof_is_attached_by_certifying_position_only() {
        assert!(!certifying_position_in_batch(4, 5, 8));
        assert!(certifying_position_in_batch(5, 5, 8));
        assert!(certifying_position_in_batch(7, 5, 8));
        assert!(!certifying_position_in_batch(8, 5, 8));
    }

    fn ledger_info_committing(
        commit_info: BlockInfo,
        block_number: u64,
    ) -> LedgerInfoWithSignatures {
        LedgerInfoWithSignatures::new(
            LedgerInfo::new_with_block_info(
                commit_info,
                HashValue::zero(),
                HashValue::zero(),
                block_number,
            ),
            AggregateSignature::empty(),
        )
    }

    #[test]
    fn index_build_spans_only_the_requested_epoch() {
        let tmp_dir = TempPath::new();
        let db = ConsensusDB::new(&tmp_dir, &PathBuf::new());
        let signer = ValidatorSigner::random(None);
        let block_info = |block: &Block| block.gen_block_info(HashValue::zero(), 0, None);

        // Canonical chain G <- B1 <- ... <- B5 numbered 1..=5. QC_i certifies B_i and commits its
        // parent, so ledger infos exist for B1..=B4 and the epoch ends at B4 (block number 4).
        let genesis = Block::make_genesis_block();
        let epoch = genesis.epoch();
        let mut parent = genesis;
        let mut parent_qc = certificate_for_genesis();
        let mut blocks = Vec::new();
        let mut qcs = Vec::new();
        for number in 1..=5u64 {
            let block = Block::new_proposal(
                Payload::empty(false, true),
                number,
                number,
                parent_qc.clone(),
                &signer,
                Vec::new(),
            )
            .unwrap();
            block.set_block_number(number);
            let commit = ledger_info_committing(block_info(&parent), number - 1);
            let qc = QuorumCert::new(
                VoteData::new(block_info(&block), block_info(&parent)),
                commit.clone(),
            );
            if number > 1 {
                db.put::<LedgerInfoSchema>(&(number - 1), &commit).unwrap();
            }
            db.save_block_numbers(vec![(epoch, number, block.id())]).unwrap();
            blocks.push(block.clone());
            qcs.push(qc.clone());
            parent = block;
            parent_qc = qc;
        }
        db.save_blocks_and_quorum_certificates(blocks.clone(), qcs).unwrap();
        db.put::<EpochByBlockNumberSchema>(&4, &epoch).unwrap();

        // Neighbouring epochs' ledger infos sit right outside this epoch's block-number span.
        let foreign = |epoch: u64, number: u64| {
            ledger_info_committing(
                BlockInfo::new(epoch, number, HashValue::random(), HashValue::zero(), 0, 0, None),
                number,
            )
        };
        db.put::<LedgerInfoSchema>(&0, &foreign(epoch - 1, 0)).unwrap();
        for number in 5..=8u64 {
            db.put::<LedgerInfoSchema>(&number, &foreign(epoch + 1, number)).unwrap();
        }

        let index = BlockStore::build_forward_epoch_sync_index(&db, epoch).unwrap();

        assert_eq!(index.manifest.first_block_number, 1);
        assert_eq!(index.manifest.target_block_number, 4);
        assert_eq!(index.manifest.target_block_id, blocks[3].id());
        assert_eq!(
            index.entries.iter().map(|entry| entry.block_id).collect::<Vec<_>>(),
            blocks.iter().map(|block| block.id()).collect::<Vec<_>>()
        );
        // Ledger info k is committed by QC_{k+1}, whose certified block sits at position k.
        assert_eq!(
            index
                .boundaries
                .iter()
                .map(|boundary| {
                    (
                        boundary.certifying_position,
                        boundary.target_block_number,
                        boundary.ledger_info.ledger_info().consensus_block_id(),
                    )
                })
                .collect::<Vec<_>>(),
            (1..=4usize).map(|k| (k, k as u64, blocks[k - 1].id())).collect::<Vec<_>>()
        );
    }

    fn busy_reply() -> anyhow::Result<ForwardEpochSyncResponse> {
        Ok(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Error(
            ForwardEpochSyncError::Busy,
        )))
    }

    fn rejected_reply(error: ForwardEpochSyncError) -> anyhow::Result<ForwardEpochSyncResponse> {
        Ok(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Error(error)))
    }

    fn prepared_reply(
        manifest: &ForwardEpochSyncManifest,
    ) -> anyhow::Result<ForwardEpochSyncResponse> {
        Ok(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Prepared(Box::new(
            manifest.clone(),
        ))))
    }

    fn sample_manifest() -> ForwardEpochSyncManifest {
        ForwardEpochSyncManifest {
            epoch: 3,
            manifest_id: HashValue::random(),
            first_block_number: 10,
            target_block_number: 20,
            target_block_id: HashValue::random(),
            target_ledger_info: ledger_info_committing(BlockInfo::empty(), 20),
        }
    }

    const PREPARE_TIMEOUT: Duration =
        Duration::from_millis(FORWARD_EPOCH_SYNC_PREPARE_TIMEOUT_MSEC);
    const REPROBE_DELAY: Duration = Duration::from_millis(FORWARD_EPOCH_SYNC_REPROBE_DELAY_MSEC);

    fn attempt_deadline() -> time::Instant {
        time::Instant::now() + Duration::from_millis(FORWARD_EPOCH_SYNC_PREPARE_ATTEMPT_MSEC)
    }

    /// The error the network layer hands the client when a probe times out.
    fn rpc_timed_out() -> anyhow::Error {
        NetworkError::RpcError(RpcError::TimedOut.to_string()).into()
    }

    /// A peer that answers `reply` after `delay`, or times out like the network layer would if
    /// `delay` exceeds the probe's timeout.
    async fn slow_reply(
        delay: Duration,
        timeout: Duration,
        reply: anyhow::Result<ForwardEpochSyncResponse>,
    ) -> anyhow::Result<ForwardEpochSyncResponse> {
        if delay > timeout {
            time::sleep(timeout).await;
            return Err(rpc_timed_out());
        }
        time::sleep(delay).await;
        reply
    }

    #[test]
    fn busy_backoff_doubles_and_caps_at_eight_seconds() {
        let mut backoff = BusyBackoff::new();
        let delays: Vec<_> = (0..6).map(|_| backoff.next_delay()).collect();
        assert_eq!(delays, [500, 1_000, 2_000, 4_000, 8_000, 8_000].map(Duration::from_millis));
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_reaches_healthy_alternate_while_preferred_is_busy() {
        let preferred = AccountAddress::random();
        let alternate = AccountAddress::random();
        let manifest = sample_manifest();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |peer, _timeout| {
                calls.borrow_mut().push(peer);
                let reply =
                    if peer == preferred { busy_reply() } else { prepared_reply(&manifest) };
                async move { reply }
            },
            vec![preferred, alternate],
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(
            matches!(outcome, PrepareOutcome::Prepared(got, peer) if peer == alternate && *got == manifest)
        );
        assert_eq!(*calls.borrow(), vec![preferred, alternate]);
        assert_eq!(time::Instant::now(), started, "no backoff before the alternate is asked");
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_probes_each_unreachable_peer_twice_without_pausing() {
        let peers: Vec<_> = (0..3).map(|_| AccountAddress::random()).collect();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |peer, _timeout| {
                calls.borrow_mut().push(peer);
                async { Err(anyhow::anyhow!("not connected")) }
            },
            peers.clone(),
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::NoServingPeer));
        let mut expected = peers.clone();
        expected.extend(peers.iter().copied());
        assert_eq!(*calls.borrow(), expected, "one full pass, then one retry pass");
        assert_eq!(
            time::Instant::now(),
            started,
            "a failure that did not wait is re-probed without a pause"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_pauses_before_re_probing_a_timed_out_peer() {
        // The peer's cold build overran the inbound cap on the first probe (no reply) and is
        // cached by the second.
        let peer = AccountAddress::random();
        let manifest = sample_manifest();
        let calls = Rc::new(RefCell::new(0usize));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |_, timeout| {
                *calls.borrow_mut() += 1;
                let first = *calls.borrow() == 1;
                let reply = prepared_reply(&manifest);
                async move {
                    if first {
                        slow_reply(Duration::MAX, timeout, reply).await
                    } else {
                        reply
                    }
                }
            },
            vec![peer],
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::Prepared(_, got) if got == peer));
        assert_eq!(*calls.borrow(), 2);
        assert_eq!(time::Instant::now() - started, PREPARE_TIMEOUT + REPROBE_DELAY);
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_gives_up_after_the_allowed_busy_retries() {
        let peers: Vec<_> = (0..2).map(|_| AccountAddress::random()).collect();
        let calls = Rc::new(RefCell::new(0usize));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |_, _| {
                *calls.borrow_mut() += 1;
                async { busy_reply() }
            },
            peers,
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::StillBusy));
        let passes = FORWARD_EPOCH_SYNC_MAX_BUSY_RETRIES as usize + 1;
        assert_eq!(*calls.borrow(), 2 * passes);
        // 0.5 + 1 + 2 s between the four passes.
        assert_eq!(time::Instant::now() - started, Duration::from_millis(3_500));
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_drops_rejecting_peer_and_re_asks_busy_one_after_backoff() {
        let preferred = AccountAddress::random();
        let alternate = AccountAddress::random();
        let manifest = sample_manifest();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |peer, _timeout| {
                calls.borrow_mut().push(peer);
                let preferred_calls = calls.borrow().iter().filter(|p| **p == preferred).count();
                let reply = if peer == alternate {
                    rejected_reply(ForwardEpochSyncError::Disabled)
                } else if preferred_calls == 1 {
                    busy_reply()
                } else {
                    prepared_reply(&manifest)
                };
                async move { reply }
            },
            vec![preferred, alternate],
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::Prepared(_, peer) if peer == preferred));
        assert_eq!(*calls.borrow(), vec![preferred, alternate, preferred]);
        assert_eq!(time::Instant::now() - started, Duration::from_millis(500));
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_attempt_ends_at_its_deadline_with_old_binaries() {
        // Five old binaries that never answer: the first pass costs 50 s, the re-probe pause 5 s,
        // and the fourth probe of the second pass is cut short at the 90 s deadline.
        let peers: Vec<_> = (0..5).map(|_| AccountAddress::random()).collect();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |peer, timeout| {
                calls.borrow_mut().push(peer);
                slow_reply(Duration::MAX, timeout, busy_reply())
            },
            peers.clone(),
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::NoServingPeer));
        let mut expected = peers.clone();
        expected.extend_from_slice(&peers[..4]);
        assert_eq!(*calls.borrow(), expected);
        assert_eq!(time::Instant::now() - started, Duration::from_secs(90));
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_attempt_with_slow_busy_peers_is_bounded_by_the_deadline() {
        // Peers that answer `Busy` just under the probe timeout cannot stretch the attempt beyond
        // the deadline: the ninth probe is cut short and the attempt reports StillBusy.
        let peers: Vec<_> = (0..3).map(|_| AccountAddress::random()).collect();
        let calls = Rc::new(RefCell::new(0usize));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |_, timeout| {
                *calls.borrow_mut() += 1;
                slow_reply(Duration::from_millis(9_900), timeout, busy_reply())
            },
            peers,
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::StillBusy));
        // 3 × 9.9 + 0.5 + 3 × 9.9 + 1 = 60.9 s, two more probes reach 80.7 s, and the last one is
        // limited to the 9.3 s left.
        assert_eq!(*calls.borrow(), 9);
        assert_eq!(time::Instant::now() - started, Duration::from_secs(90));
    }

    #[tokio::test(start_paused = true)]
    async fn prepare_pauses_once_per_pass_for_busy_and_timed_out_peers() {
        // Preferred is saturated and the alternate timed out (instantly here, to isolate the
        // pause): the pass switch waits the longer of the two, the 5 s re-probe delay.
        let preferred = AccountAddress::random();
        let alternate = AccountAddress::random();
        let manifest = sample_manifest();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let started = time::Instant::now();

        let outcome = prepare_from_candidates(
            |peer, _timeout| {
                calls.borrow_mut().push(peer);
                let alternate_calls = calls.borrow().iter().filter(|p| **p == alternate).count();
                let reply = if peer == preferred {
                    busy_reply()
                } else if alternate_calls == 1 {
                    Err(rpc_timed_out())
                } else {
                    prepared_reply(&manifest)
                };
                async move { reply }
            },
            vec![preferred, alternate],
            PREPARE_TIMEOUT,
            attempt_deadline(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, PrepareOutcome::Prepared(_, peer) if peer == alternate));
        assert_eq!(*calls.borrow(), vec![preferred, alternate, preferred, alternate]);
        assert_eq!(time::Instant::now() - started, REPROBE_DELAY);
    }

    #[test]
    fn progress_watchdog_judges_only_elapsed_windows() {
        let window = Duration::from_millis(FORWARD_EPOCH_SYNC_PROGRESS_WINDOW_MSEC);
        let start = time::Instant::now();
        let mut watchdog = ProgressWatchdog::start(start);
        assert_eq!(watchdog.window_end(), start + window);

        assert!(watchdog.check(start + window / 2), "an unfinished window is not judged");
        watchdog.record(FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW);
        assert!(watchdog.check(start + window), "enough blocks: window passes");
        assert_eq!(watchdog.window_end(), start + 2 * window, "a fresh window starts");

        watchdog.record(FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW);
        assert!(watchdog.check(start + 5 * window / 2), "a late check judges the window");
        assert_eq!(
            watchdog.window_end(),
            start + 7 * window / 2,
            "the next window starts at the check, not at the old window end"
        );

        watchdog.record(FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW - 1);
        assert!(!watchdog.check(start + 7 * window / 2), "one block short: stalled");
    }

    #[test]
    fn progress_watchdog_does_not_carry_blocks_across_windows() {
        let window = Duration::from_millis(FORWARD_EPOCH_SYNC_PROGRESS_WINDOW_MSEC);
        let start = time::Instant::now();
        let mut watchdog = ProgressWatchdog::start(start);
        watchdog.record(10 * FORWARD_EPOCH_SYNC_MIN_BLOCKS_PER_WINDOW);
        assert!(watchdog.check(start + window));
        assert!(!watchdog.check(start + 2 * window), "the surplus does not cover an idle window");
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_busy_backoff_stops_at_window_end() {
        let started = time::Instant::now();
        let window_end = started + Duration::from_secs(60);
        let calls = Rc::new(RefCell::new(0usize));

        let step = fetch_from_peer(
            AccountAddress::random(),
            || {
                *calls.borrow_mut() += 1;
                async { busy_reply() }
            },
            NUM_RETRIES,
            window_end,
        )
        .await
        .unwrap();

        assert!(matches!(step, FetchStep::WindowElapsed));
        assert_eq!(time::Instant::now(), window_end, "the last backoff is cut at the window end");
        // 0.5 + 1 + 2 + 4 + 8 + 8 + 8 + 8 + 8 + 8 = 55.5 s, then a 4.5 s remainder, then one
        // more probe past the window end.
        assert_eq!(*calls.borrow(), 12);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_delayed_busy_replies_still_end_at_the_window() {
        let started = time::Instant::now();
        let window_end = started + Duration::from_secs(60);
        let reply_delay = Duration::from_millis(4_900);
        let calls = Rc::new(RefCell::new(0usize));

        let step = fetch_from_peer(
            AccountAddress::random(),
            || {
                *calls.borrow_mut() += 1;
                async move {
                    time::sleep(reply_delay).await;
                    busy_reply()
                }
            },
            NUM_RETRIES,
            window_end,
        )
        .await
        .unwrap();

        assert!(matches!(step, FetchStep::WindowElapsed));
        // Replies land at 4.9, 10.3, 16.2, 23.1, 32.0, 44.9, 57.8 s (backoff 0.5/1/2/4/8/8), the
        // next sleep is cut at 60 s, and the eighth reply at 64.9 s ends the call.
        assert_eq!(*calls.borrow(), 8);
        assert_eq!(time::Instant::now() - started, Duration::from_millis(64_900));
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_rpc_failures_retry_then_fail() {
        let started = time::Instant::now();
        let calls = Rc::new(RefCell::new(0usize));

        let Err(error) = fetch_from_peer(
            AccountAddress::random(),
            || {
                *calls.borrow_mut() += 1;
                async { Err(anyhow::anyhow!("timed out")) }
            },
            NUM_RETRIES,
            started + Duration::from_secs(60),
        )
        .await
        else {
            panic!("the last RPC failure must propagate");
        };

        assert_eq!(error.to_string(), "timed out");
        assert_eq!(*calls.borrow(), NUM_RETRIES);
        assert_eq!(
            time::Instant::now() - started,
            Duration::from_millis(RETRY_INTERVAL_MSEC * (NUM_RETRIES as u64 - 1))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_passes_non_busy_errors_through_without_retrying() {
        let started = time::Instant::now();
        let calls = Rc::new(RefCell::new(0usize));

        let step = fetch_from_peer(
            AccountAddress::random(),
            || {
                *calls.borrow_mut() += 1;
                async { rejected_reply(ForwardEpochSyncError::AnchorMismatch) }
            },
            NUM_RETRIES,
            started + Duration::from_secs(60),
        )
        .await
        .unwrap();

        assert!(matches!(
            step,
            FetchStep::Reply(ForwardEpochSyncResponse::V1(ForwardEpochSyncResponseV1::Error(
                ForwardEpochSyncError::AnchorMismatch
            )))
        ));
        assert_eq!(*calls.borrow(), 1, "only Busy is retried here; other errors go to the caller");
        assert_eq!(time::Instant::now(), started);
    }

    // Serving side: index cache, cold-build admission, early `Busy`.

    fn service(cold_build_quota: usize, fetch_quota: usize) -> Arc<ForwardEpochSyncService> {
        Arc::new(ForwardEpochSyncService::new(cold_build_quota, fetch_quota))
    }

    fn built_index(epoch: u64) -> ForwardEpochSyncIndex {
        ForwardEpochSyncIndex {
            manifest: ForwardEpochSyncManifest { epoch, ..sample_manifest() },
            entries: Vec::new(),
            positions: HashMap::new(),
            boundaries: Vec::new(),
        }
    }

    /// A build that completes after `delay` and counts how often it actually ran.
    fn timed_build(
        epoch: u64,
        delay: Duration,
        builds: &Arc<AtomicUsize>,
    ) -> impl Future<Output = Result<ForwardEpochSyncIndex, ForwardEpochSyncError>> + Send + 'static
    {
        let builds = builds.clone();
        async move {
            builds.fetch_add(1, Ordering::SeqCst);
            time::sleep(delay).await;
            Ok(built_index(epoch))
        }
    }

    /// The deadline a Prepare received right now gets.
    fn reply_by() -> Option<time::Instant> {
        Some(time::Instant::now() + Duration::from_millis(FORWARD_EPOCH_SYNC_SLOW_BUILD_REPLY_MSEC))
    }

    #[tokio::test(start_paused = true)]
    async fn index_cache_hit_needs_no_cold_build_quota() {
        let service = service(1, 1);
        let (a, b) = (AccountAddress::random(), AccountAddress::random());
        let builds = Arc::new(AtomicUsize::new(0));

        // Epoch 2 is cached first; then peer A's build of epoch 1 takes the only permit for good.
        service.index(2, a, reply_by(), timed_build(2, Duration::ZERO, &builds)).await.unwrap();
        let refused = service.index(1, a, reply_by(), future::pending()).await;
        assert_eq!(refused.err(), Some(ForwardEpochSyncError::Busy));

        let started = time::Instant::now();
        let hit = service.index(2, b, reply_by(), timed_build(2, Duration::ZERO, &builds)).await;
        assert!(hit.is_ok());
        assert_eq!(time::Instant::now(), started, "a cache hit neither waits nor builds");
        assert_eq!(builds.load(Ordering::SeqCst), 1);

        let cold = service.index(3, b, reply_by(), timed_build(3, Duration::ZERO, &builds)).await;
        assert_eq!(cold.err(), Some(ForwardEpochSyncError::Busy), "a cold build needs a permit");
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_prepares_for_one_epoch_share_one_build() {
        let service = service(1, 1);
        let (a, b) = (AccountAddress::random(), AccountAddress::random());
        let builds = Arc::new(AtomicUsize::new(0));
        let started = time::Instant::now();

        let (first, second) = tokio::join!(
            service.index(7, a, reply_by(), timed_build(7, Duration::from_secs(1), &builds)),
            service.index(7, b, reply_by(), timed_build(7, Duration::from_secs(1), &builds)),
        );

        let (first, second) = (first.unwrap(), second.unwrap());
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(builds.load(Ordering::SeqCst), 1, "the second request joined the build");
        assert_eq!(time::Instant::now() - started, Duration::from_secs(1));
        // The joiner took neither the permit nor a peer slot, and the build gave its own back.
        let next = service.index(8, b, reply_by(), timed_build(8, Duration::ZERO, &builds)).await;
        assert!(next.is_ok());
    }

    // Also the lock-scope check the design asks for: peer A's build stays parked for the rest of
    // the test, so a guard held across the build would deadlock the next `index()` call.
    #[tokio::test(start_paused = true)]
    async fn one_peer_gets_one_cold_build_at_a_time() {
        let service = service(4, 1);
        let (a, b) = (AccountAddress::random(), AccountAddress::random());
        let builds = Arc::new(AtomicUsize::new(0));

        // Peer A's build of epoch 1 never finishes, so A keeps its slot.
        let slow = service.index(1, a, reply_by(), future::pending()).await;
        assert_eq!(slow.err(), Some(ForwardEpochSyncError::Busy));

        let started = time::Instant::now();
        let second = service.index(2, a, reply_by(), timed_build(2, Duration::ZERO, &builds)).await;
        assert_eq!(second.err(), Some(ForwardEpochSyncError::Busy));
        assert_eq!(time::Instant::now(), started, "refused at once, not after waiting");
        assert_eq!(builds.load(Ordering::SeqCst), 0);

        let other = service.index(2, b, reply_by(), timed_build(2, Duration::ZERO, &builds)).await;
        assert!(other.is_ok(), "another peer may build epoch 2 meanwhile");
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_quota_is_separate_from_cold_build_quota() {
        let service = service(1, 1);
        let a = AccountAddress::random();

        let slow = service.index(1, a, reply_by(), future::pending()).await;
        assert_eq!(slow.err(), Some(ForwardEpochSyncError::Busy), "the cold-build permit is taken");

        let permit = service.try_acquire_fetch();
        assert!(permit.is_some(), "Fetch has its own pool");
        assert!(service.try_acquire_fetch().is_none());
        drop(permit);
        assert!(service.try_acquire_fetch().is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn index_cache_keeps_the_most_recent_epochs() {
        let service = service(4, 1);
        let a = AccountAddress::random();
        let builds = Arc::new(AtomicUsize::new(0));

        let cap = FORWARD_EPOCH_SYNC_INDEX_CACHE_EPOCHS as u64;
        for epoch in 1..=cap {
            service
                .index(epoch, a, reply_by(), timed_build(epoch, Duration::ZERO, &builds))
                .await
                .unwrap();
        }
        // A hit on the oldest epoch makes it recent again, so the next miss evicts epoch 2.
        service.index(1, a, reply_by(), timed_build(1, Duration::ZERO, &builds)).await.unwrap();
        service
            .index(cap + 1, a, reply_by(), timed_build(cap + 1, Duration::ZERO, &builds))
            .await
            .unwrap();

        assert_eq!(builds.load(Ordering::SeqCst), cap as usize + 1, "the hit did not rebuild");
        let indexes = service.indexes.lock();
        assert!(!indexes.ready.contains(&2), "the least recently used epoch is evicted");
        assert!(indexes.ready.contains(&1));
        assert!((3..=cap + 1).all(|epoch| indexes.ready.contains(&epoch)));
        assert!(indexes.in_flight.is_empty());
        assert!(indexes.cold_build_peers.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn slow_build_answers_busy_then_serves_from_cache() {
        let service = service(1, 1);
        let a = AccountAddress::random();
        let builds = Arc::new(AtomicUsize::new(0));
        let started = time::Instant::now();

        let first =
            service.index(1, a, reply_by(), timed_build(1, Duration::from_secs(20), &builds)).await;
        assert_eq!(first.err(), Some(ForwardEpochSyncError::Busy));
        assert_eq!(
            time::Instant::now() - started,
            Duration::from_millis(FORWARD_EPOCH_SYNC_SLOW_BUILD_REPLY_MSEC),
            "Busy leaves the RPC with time to spare before the inbound cap"
        );

        // The build carries on without a requester and lands in the cache.
        time::sleep(Duration::from_secs(12) + Duration::from_millis(1)).await;
        {
            let indexes = service.indexes.lock();
            assert!(indexes.ready.contains(&1));
            assert!(indexes.in_flight.is_empty());
            assert!(indexes.cold_build_peers.is_empty());
        }

        let started = time::Instant::now();
        let retry = service.index(1, a, reply_by(), timed_build(1, Duration::ZERO, &builds)).await;
        assert!(retry.is_ok());
        assert_eq!(time::Instant::now(), started);
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn reply_deadline_counts_from_receipt_not_from_handler_start() {
        let service = service(1, 1);
        let a = AccountAddress::random();
        let builds = Arc::new(AtomicUsize::new(0));

        // The request sat in a queue for 5 s before its handler ran: 3 s of the budget are left.
        let received_at = time::Instant::now();
        let reply_by =
            Some(received_at + Duration::from_millis(FORWARD_EPOCH_SYNC_SLOW_BUILD_REPLY_MSEC));
        time::sleep(Duration::from_secs(5)).await;
        let started = time::Instant::now();
        let late =
            service.index(1, a, reply_by, timed_build(1, Duration::from_secs(20), &builds)).await;
        assert_eq!(late.err(), Some(ForwardEpochSyncError::Busy));
        assert_eq!(time::Instant::now() - started, Duration::from_secs(3));

        // A deadline that has already passed still joins (or starts) the build, but answers at
        // once.
        let expired = Some(received_at);
        let started = time::Instant::now();
        let joined = service.index(1, a, expired, timed_build(1, Duration::ZERO, &builds)).await;
        assert_eq!(joined.err(), Some(ForwardEpochSyncError::Busy));
        assert_eq!(time::Instant::now(), started);
        assert_eq!(builds.load(Ordering::SeqCst), 1);

        // Once cached, an expired deadline is no obstacle.
        time::sleep(Duration::from_secs(20)).await;
        let hit = service.index(1, a, expired, timed_build(1, Duration::ZERO, &builds)).await;
        assert!(hit.is_ok());
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn re_ask_after_early_busy_joins_the_running_build() {
        let service = service(1, 1);
        let (a, b) = (AccountAddress::random(), AccountAddress::random());
        let builds = Arc::new(AtomicUsize::new(0));

        let first =
            service.index(1, a, reply_by(), timed_build(1, Duration::from_secs(12), &builds)).await;
        assert_eq!(first.err(), Some(ForwardEpochSyncError::Busy));
        // The client's Busy backoff, then its retry, alongside another peer's first ask.
        time::sleep(Duration::from_millis(500)).await;
        let started = time::Instant::now();

        let (again, other) = tokio::join!(
            service.index(1, a, reply_by(), timed_build(1, Duration::ZERO, &builds)),
            service.index(1, b, reply_by(), timed_build(1, Duration::ZERO, &builds)),
        );

        let (again, other) = (again.unwrap(), other.unwrap());
        assert!(Arc::ptr_eq(&again, &other));
        assert_eq!(builds.load(Ordering::SeqCst), 1, "no second build for the same epoch");
        assert_eq!(
            time::Instant::now() - started,
            Duration::from_millis(3_500),
            "both are served the moment the running build finishes"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_lookup_never_waits_for_a_build() {
        let service = service(1, 1);
        let a = AccountAddress::random();
        let builds = Arc::new(AtomicUsize::new(0));
        let started = time::Instant::now();

        let miss = service.index(1, a, None, timed_build(1, Duration::from_secs(1), &builds)).await;
        assert_eq!(miss.err(), Some(ForwardEpochSyncError::Busy));
        assert_eq!(time::Instant::now(), started, "answered at once");

        time::sleep(Duration::from_secs(1) + Duration::from_millis(1)).await;
        let hit = service.index(1, a, None, timed_build(1, Duration::ZERO, &builds)).await;
        assert!(hit.is_ok());
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn failed_build_is_reported_to_every_waiter_and_frees_the_slot() {
        let service = service(1, 1);
        let (a, b) = (AccountAddress::random(), AccountAddress::random());
        let builds = Arc::new(AtomicUsize::new(0));

        let (first, second) = tokio::join!(
            service.index(1, a, reply_by(), async { Err(ForwardEpochSyncError::EpochNotFound) }),
            service.index(1, b, reply_by(), timed_build(1, Duration::ZERO, &builds)),
        );

        assert_eq!(first.err(), Some(ForwardEpochSyncError::EpochNotFound));
        assert_eq!(second.err(), Some(ForwardEpochSyncError::EpochNotFound), "the joiner too");
        assert_eq!(builds.load(Ordering::SeqCst), 0);
        {
            let indexes = service.indexes.lock();
            assert!(!indexes.ready.contains(&1), "failures are not cached");
            assert!(indexes.in_flight.is_empty());
            assert!(indexes.cold_build_peers.is_empty());
        }
        // Slot and permit are back: the same peer may start another build at once.
        let next = service.index(2, a, reply_by(), timed_build(2, Duration::ZERO, &builds)).await;
        assert!(next.is_ok());
    }
}
