// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    consensusdb::{CommittedBlockAnchor, ConsensusDB},
    liveness::proposer_election::{choose_index, ProposerElection},
};
use anyhow::{anyhow, ensure, Context, Result};
use aptos_consensus_types::common::{Author, Round};
use gaptos::{
    api_types::{
        config_storage::{BlockNumber, OnChainConfig, GLOBAL_CONFIG_STORAGE},
        on_chain_config::{
            consensus_hardfork::{
                is_consensus_fork_active, is_consensus_fork_active_at_epoch, ConsensusHardfork,
            },
            validator_performances::ValidatorPerformances,
        },
    },
    aptos_bitvec::BitVec,
    aptos_consensus::counters::{
        CHAIN_HEALTH_PARTICIPATING_NUM_VALIDATORS, CHAIN_HEALTH_PARTICIPATING_VOTING_POWER,
        CHAIN_HEALTH_REPUTATION_PARTICIPATING_VOTING_POWER_FRACTION,
        CHAIN_HEALTH_TOTAL_NUM_VALIDATORS, CHAIN_HEALTH_TOTAL_VOTING_POWER,
        CHAIN_HEALTH_WINDOW_SIZES, COMMITTED_PROPOSALS_IN_WINDOW, COMMITTED_VOTES_IN_WINDOW,
        CONSENSUS_PARTICIPATION_STATUS, FAILED_PROPOSALS_IN_WINDOW,
        LEADER_REPUTATION_ROUND_HISTORY_SIZE,
    },
    aptos_crypto::HashValue,
    aptos_infallible::{Mutex, MutexGuard},
    aptos_logger::prelude::*,
    aptos_storage_interface::DbReader,
    aptos_types::{
        account_config::NewBlockEvent, epoch_change::EpochChangeProof, epoch_state::EpochState,
    },
};

use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    convert::TryFrom,
    sync::Arc,
};

pub type VotingPowerRatio = f64;

/// Interface to query committed NewBlockEvent.
pub trait MetadataBackend: Send + Sync {
    /// Return a contiguous NewBlockEvent window in which last one is at target_round or
    /// latest committed, return all previous one if not enough.
    fn get_block_metadata(
        &self,
        target_epoch: u64,
        target_round: Round,
    ) -> (Vec<NewBlockEvent>, HashValue);
}

pub trait ReputationAnchorBackend: Send + Sync {
    fn get_anchor(
        &self,
        target_epoch: u64,
        target_round: Round,
    ) -> Result<Option<CommittedBlockAnchor>>;
}

pub struct ConsensusDBReputationAnchorBackend {
    consensus_db: Arc<ConsensusDB>,
}

impl ConsensusDBReputationAnchorBackend {
    pub fn new(consensus_db: Arc<ConsensusDB>) -> Self {
        Self { consensus_db }
    }
}

impl ReputationAnchorBackend for ConsensusDBReputationAnchorBackend {
    fn get_anchor(
        &self,
        target_epoch: u64,
        target_round: Round,
    ) -> Result<Option<CommittedBlockAnchor>> {
        self.consensus_db.get_reputation_anchor(target_epoch, target_round)
    }
}

#[derive(Debug, Clone)]
pub struct VersionedNewBlockEvent {
    /// event
    pub event: NewBlockEvent,
    /// version
    pub version: u64,
}

pub struct AptosDBBackend {
    window_size: usize,
    seek_len: usize,
    aptos_db: Arc<dyn DbReader>,
    db_result: Mutex<Option<(Vec<VersionedNewBlockEvent>, u64, bool)>>,
}

impl AptosDBBackend {
    pub fn new(window_size: usize, seek_len: usize, aptos_db: Arc<dyn DbReader>) -> Self {
        Self { window_size, seek_len, aptos_db, db_result: Mutex::new(None) }
    }

    fn refresh_db_result(
        &self,
        locked: &mut MutexGuard<'_, Option<(Vec<VersionedNewBlockEvent>, u64, bool)>>,
        latest_db_version: u64,
    ) -> Result<(Vec<VersionedNewBlockEvent>, u64, bool)> {
        // assumes target round is not too far from latest commit
        let limit = self.window_size + self.seek_len;

        let events = self.aptos_db.get_latest_block_events(limit)?;

        let max_returned_version = events.first().map_or(0, |first| first.transaction_version);

        let new_block_events = events
            .into_iter()
            .map(|event| {
                Ok(VersionedNewBlockEvent {
                    event: bcs::from_bytes::<NewBlockEvent>(event.event.event_data())?,
                    version: event.transaction_version,
                })
            })
            .collect::<Result<Vec<VersionedNewBlockEvent>, bcs::Error>>()?;

        let hit_end = new_block_events.len() < limit;

        let result =
            (new_block_events, std::cmp::max(latest_db_version, max_returned_version), hit_end);
        **locked = Some(result.clone());
        Ok(result)
    }

    fn get_from_db_result(
        &self,
        target_epoch: u64,
        target_round: Round,
        events: &Vec<VersionedNewBlockEvent>,
        hit_end: bool,
    ) -> (Vec<NewBlockEvent>, HashValue) {
        // Do not warn when round==0, because check will always be unsure of whether we have
        // all events from the previous epoch. If there is an actual issue, next round will log it.
        if target_round != 0 {
            let has_larger = events.first().map_or(false, |e| {
                (e.event.epoch(), e.event.round()) >= (target_epoch, target_round)
            });
            if !has_larger {
                // error, and not a fatal, in an unlikely scenario that we have many failed
                // consecutive rounds, and nobody has any newer successful blocks.
                warn!(
                    "Local history is too old, asking for {} epoch and {} round, and latest from db is {} epoch and {} round! Elected proposers are unlikely to match!!",
                    target_epoch, target_round, events.first().map_or(0, |e| e.event.epoch()), events.first().map_or(0, |e| e.event.round()))
            }
        }

        let mut max_version = 0;
        let mut result = vec![];
        for event in events {
            if (event.event.epoch(), event.event.round()) <= (target_epoch, target_round) &&
                result.len() < self.window_size
            {
                max_version = std::cmp::max(max_version, event.version);
                result.push(event.event.clone());
            }
        }

        if result.len() < self.window_size && !hit_end {
            error!(
                "We are not fetching far enough in history, we filtered from {} to {}, but asked for {}. Target ({}, {}), received from {:?} to {:?}.",
                events.len(),
                result.len(),
                self.window_size,
                target_epoch,
                target_round,
                events.last().map_or((0, 0), |e| (e.event.epoch(), e.event.round())),
                events.first().map_or((0, 0), |e| (e.event.epoch(), e.event.round())),
            );
        }

        if result.is_empty() {
            error!("[leader reputation] No events in the requested window could be found, leader election may degrade");
            (result, HashValue::zero())
        } else {
            let root_hash = self
                .aptos_db
                .get_accumulator_root_hash(max_version)
                .unwrap_or_else(|_| {
                    error!(
                        "We couldn't fetch accumulator hash for the {} version, for {} epoch, {} round",
                        max_version, target_epoch, target_round,
                    );
                    HashValue::zero()
                });
            (result, root_hash)
        }
    }
}

impl MetadataBackend for AptosDBBackend {
    // assume the target_round only increases
    fn get_block_metadata(
        &self,
        target_epoch: u64,
        target_round: Round,
    ) -> (Vec<NewBlockEvent>, HashValue) {
        let mut locked = self.db_result.lock();
        let latest_db_version = self.aptos_db.get_latest_ledger_info_version().unwrap_or(0);
        // lazy init db_result
        if locked.is_none() {
            if let Err(e) = self.refresh_db_result(&mut locked, latest_db_version) {
                error!(
                    error = ?e, "[leader reputation] Fail to initialize db result, leader election may degrade",
                );
                return (vec![], HashValue::zero());
            }
        }
        let (events, version, hit_end) = {
            // locked is somenthing
            #[allow(clippy::unwrap_used)]
            let result = locked.as_ref().unwrap();
            (&result.0, result.1, result.2)
        };

        let has_larger = events
            .first()
            .map_or(false, |e| (e.event.epoch(), e.event.round()) >= (target_epoch, target_round));
        // check if fresher data has potential to give us different result
        if !has_larger && version < latest_db_version {
            let fresh_db_result = self.refresh_db_result(&mut locked, latest_db_version);
            match fresh_db_result {
                Ok((events, _version, hit_end)) => {
                    self.get_from_db_result(target_epoch, target_round, &events, hit_end)
                }
                Err(e) => {
                    // fails if requested events were pruned / or we never backfil them.
                    error!(
                        error = ?e, "[leader reputation] Fail to refresh window, leader election may degrade",
                    );
                    (vec![], HashValue::zero())
                }
            }
        } else {
            self.get_from_db_result(target_epoch, target_round, events, hit_end)
        }
    }
}

/// Interface to calculate weights for proposers based on history.
pub trait ReputationHeuristic: Send + Sync {
    /// Return the weights of all candidates based on the history.
    fn get_weights(
        &self,
        epoch: u64,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
    ) -> Vec<u64>;

    /// Legacy Gravity behavior used before ConsensusAlpha.
    fn get_legacy_weights(
        &self,
        epoch: u64,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
    ) -> Vec<u64> {
        self.get_weights(epoch, epoch_to_candidates, history)
    }

    /// Compute weights from the EVM state at a consensus-agreed block height.
    fn get_weights_at_block(
        &self,
        _epoch: u64,
        _epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        _block_number: u64,
    ) -> Result<Vec<u64>> {
        Err(anyhow!("anchored validator performance is not supported"))
    }
}

pub struct NewBlockEventAggregation {
    // Window sizes are in number of succesfull blocks, not number of rounds.
    // i.e. we can be looking at different number of rounds for the same window,
    // dependig on how many failures we have.
    voter_window_size: usize,
    proposer_window_size: usize,
    reputation_window_from_stale_end: bool,
}

impl NewBlockEventAggregation {
    pub fn new(
        voter_window_size: usize,
        proposer_window_size: usize,
        reputation_window_from_stale_end: bool,
    ) -> Self {
        Self { voter_window_size, proposer_window_size, reputation_window_from_stale_end }
    }

    pub fn bitvec_to_voters<'a>(
        validators: &'a [Author],
        bitvec: &BitVec,
    ) -> Result<Vec<&'a Author>, String> {
        if BitVec::required_buckets(validators.len() as u16) != bitvec.num_buckets() {
            return Err(format!(
                "bitvec bucket {} does not match validators len {}",
                bitvec.num_buckets(),
                validators.len()
            ));
        }

        Ok(validators
            .iter()
            .enumerate()
            .filter_map(
                |(index, validator)| {
                    if bitvec.is_set(index as u16) {
                        Some(validator)
                    } else {
                        None
                    }
                },
            )
            .collect())
    }

    pub fn indices_to_validators<'a>(
        validators: &'a [Author],
        indices: &[u64],
    ) -> Result<Vec<&'a Author>, String> {
        indices
            .iter()
            .map(|index| {
                usize::try_from(*index)
                    .map_err(|_err| format!("index {} out of bounds", index))
                    .and_then(|index| {
                        validators.get(index).ok_or(format!(
                            "index {} is larger than number of validators {}",
                            index,
                            validators.len()
                        ))
                    })
            })
            .collect()
    }

    fn history_iter<'a>(
        history: &'a [NewBlockEvent],
        epoch_to_candidates: &'a HashMap<u64, Vec<Author>>,
        window_size: usize,
        from_stale_end: bool,
    ) -> impl Iterator<Item = &'a NewBlockEvent> {
        let sub_history = if from_stale_end {
            let start = if history.len() > window_size { history.len() - window_size } else { 0 };

            &history[start..]
        } else {
            if let (Some(first), Some(last)) = (history.first(), history.last()) {
                assert!((first.epoch(), first.round()) >= (last.epoch(), last.round()));
            }
            let end = if history.len() > window_size { window_size } else { history.len() };

            &history[..end]
        };
        sub_history.iter().filter(move |&meta| epoch_to_candidates.contains_key(&meta.epoch()))
    }

    pub fn get_aggregated_metrics(
        &self,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
        author: &Author,
    ) -> (HashMap<Author, u32>, HashMap<Author, u32>, HashMap<Author, u32>) {
        let votes = self.count_votes(epoch_to_candidates, history);
        let proposals = self.count_proposals(epoch_to_candidates, history);
        let failed_proposals = self.count_failed_proposals(epoch_to_candidates, history);

        COMMITTED_PROPOSALS_IN_WINDOW.set(*proposals.get(author).unwrap_or(&0) as i64);
        FAILED_PROPOSALS_IN_WINDOW.set(*failed_proposals.get(author).unwrap_or(&0) as i64);
        COMMITTED_VOTES_IN_WINDOW.set(*votes.get(author).unwrap_or(&0) as i64);

        LEADER_REPUTATION_ROUND_HISTORY_SIZE.set(
            proposals.values().sum::<u32>() as i64 + failed_proposals.values().sum::<u32>() as i64,
        );

        (votes, proposals, failed_proposals)
    }

    pub fn count_votes(
        &self,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
    ) -> HashMap<Author, u32> {
        Self::count_votes_custom(
            epoch_to_candidates,
            history,
            self.voter_window_size,
            self.reputation_window_from_stale_end,
        )
    }

    pub fn count_votes_custom(
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
        window_size: usize,
        from_stale_end: bool,
    ) -> HashMap<Author, u32> {
        Self::history_iter(history, epoch_to_candidates, window_size, from_stale_end).fold(
            HashMap::new(),
            |mut map, meta| {
                match Self::bitvec_to_voters(
                    &epoch_to_candidates[&meta.epoch()],
                    &meta.previous_block_votes_bitvec().clone().into(),
                ) {
                    Ok(voters) => {
                        for &voter in voters {
                            let count = map.entry(voter).or_insert(0u32);
                            *count = count.saturating_add(1);
                        }
                    }
                    Err(msg) => {
                        error!(
                            "Voter conversion from bitmap failed at epoch {}, round {}: {}",
                            meta.epoch(),
                            meta.round(),
                            msg
                        )
                    }
                }
                map
            },
        )
    }

    pub fn count_proposals(
        &self,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
    ) -> HashMap<Author, u32> {
        Self::count_proposals_custom(
            epoch_to_candidates,
            history,
            self.proposer_window_size,
            self.reputation_window_from_stale_end,
        )
    }

    pub fn count_proposals_custom(
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
        window_size: usize,
        from_stale_end: bool,
    ) -> HashMap<Author, u32> {
        Self::history_iter(history, epoch_to_candidates, window_size, from_stale_end).fold(
            HashMap::new(),
            |mut map, meta| {
                let count = map.entry(meta.proposer()).or_insert(0u32);
                *count = count.saturating_add(1);
                map
            },
        )
    }

    pub fn count_failed_proposals(
        &self,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
    ) -> HashMap<Author, u32> {
        Self::history_iter(
            history,
            epoch_to_candidates,
            self.proposer_window_size,
            self.reputation_window_from_stale_end,
        )
        .fold(HashMap::new(), |mut map, meta| {
            match Self::indices_to_validators(
                &epoch_to_candidates[&meta.epoch()],
                meta.failed_proposer_indices(),
            ) {
                Ok(failed_proposers) => {
                    for &failed_proposer in failed_proposers {
                        let count = map.entry(failed_proposer).or_insert(0u32);
                        *count = count.saturating_add(1);
                    }
                }
                Err(msg) => {
                    error!(
                        "Failed proposer conversion from indices failed at epoch {}, round {}: {}",
                        meta.epoch(),
                        meta.round(),
                        msg
                    )
                }
            }
            map
        })
    }
}

/// Heuristic that looks at successful and failed proposals, as well as voting history,
/// to define node reputation, used for leader selection.
///
/// We want to optimize leader selection to primarily maximize network's throughput,
/// but we also, in combinatoin with staking rewards logic, need to be reasonably fair.
///
/// Logic is:
///  * if proposer round failure rate within the proposer window is strictly above threshold, use
///    failed_weight (default 1).
///  * otherwise, if node had no proposal rounds and no successful votes, use inactive_weight
///    (default 10).
///  * otherwise, use the default active_weight (default 100).
///
/// We primarily want to avoid failed rounds, as they have a largest negative effect on the network.
/// So if we see a node having failures to propose, when it was the leader, we want to avoid that
/// node. We add a threshold (instead of penalizing on a single failure), so that transient issues
/// in the network, or malicious behaviour of the next leader is avoided. In general, we expect
/// there to be proposer_window_size/num_validators opportunities for a node to be a leader, so a
/// single failure, or a subset of following leaders being malicious will not be enough to exclude a
/// node. On the other hand, single failure, without any successes before will exclude the note.
/// Threshold probably makes the most sense to be between:
///  * 10% (aggressive exclusion with 1 failure in 10 proposals being enough for exclusion)
///  * and 33% (much less aggressive exclusion, with 1 failure for every 2 successes, should still
///    reduce failed rounds by at least 66%, and is enough to avoid byzantine attacks as well as the
///    rest of the protocol)
/// How often the pre-Alpha path re-fetches ValidatorPerformances from local latest EVM state.
const LEGACY_PERFORMANCE_CACHE_REFRESH_CALLS: u64 = 100;

pub struct ProposerAndVoterHeuristic {
    author: Author,
    active_weight: u64,
    inactive_weight: u64,
    failed_weight: u64,
    failure_threshold_percent: u32,
    aggregation: NewBlockEventAggregation,
    /// Pre-Alpha call-count cache retained for compatibility with the running network.
    legacy_cached_weights: Mutex<(u64, Option<Vec<u64>>)>,
    /// Post-Alpha cache keyed by the consensus-agreed execution block number.
    anchored_cached_weights: Mutex<Option<(u64, Vec<u64>)>>,
}

impl ProposerAndVoterHeuristic {
    pub fn new(
        author: Author,
        active_weight: u64,
        inactive_weight: u64,
        failed_weight: u64,
        failure_threshold_percent: u32,
        voter_window_size: usize,
        proposer_window_size: usize,
        reputation_window_from_stale_end: bool,
    ) -> Self {
        Self {
            author,
            active_weight,
            inactive_weight,
            failed_weight,
            failure_threshold_percent,
            aggregation: NewBlockEventAggregation::new(
                voter_window_size,
                proposer_window_size,
                reputation_window_from_stale_end,
            ),
            legacy_cached_weights: Mutex::new((0, None)),
            anchored_cached_weights: Mutex::new(None),
        }
    }

    /// Fetch ValidatorPerformances from EVM config storage and compute weights.
    fn fetch_and_compute_weights(
        &self,
        candidates: &[Author],
        block_number: BlockNumber,
    ) -> Result<Vec<u64>> {
        let storage = GLOBAL_CONFIG_STORAGE
            .get()
            .ok_or_else(|| anyhow!("global config storage is not initialized"))?;
        let bytes_res = storage
            .fetch_config_bytes(OnChainConfig::ValidatorPerformances, block_number)
            .ok_or_else(|| {
                anyhow!("ValidatorPerformances unavailable at block {:?}", block_number)
            })?;

        let raw_bytes: bytes::Bytes = bytes_res.try_into().unwrap_or_default();
        let perf = bcs::from_bytes::<ValidatorPerformances>(&raw_bytes)
            .context("ValidatorPerformances BCS decode failed")?;

        ensure!(
            perf.validators.len() == candidates.len(),
            "ValidatorPerformances length mismatch: expected {}, got {}",
            candidates.len(),
            perf.validators.len()
        );

        Ok(perf
            .validators
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let total = p.successful_proposals + p.failed_proposals;
                if total > 0 &&
                    (p.failed_proposals as u64) * 100 >
                        (total as u64) * (self.failure_threshold_percent as u64)
                {
                    debug!(
                        "Validator {} has {} failed and {} successful. Assigned failed_weight {}",
                        i, p.failed_proposals, p.successful_proposals, self.failed_weight
                    );
                    self.failed_weight
                } else if total > 0 {
                    debug!(
                        "Validator {} has {} failed and {} successful. Assigned active_weight {}",
                        i, p.failed_proposals, p.successful_proposals, self.active_weight
                    );
                    self.active_weight
                } else {
                    debug!(
                        "Validator {} has 0 proposals. Assigned inactive_weight {}",
                        i, self.inactive_weight
                    );
                    self.inactive_weight
                }
            })
            .collect())
    }
}

impl ReputationHeuristic for ProposerAndVoterHeuristic {
    fn get_weights(
        &self,
        epoch: u64,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        history: &[NewBlockEvent],
    ) -> Vec<u64> {
        assert!(epoch_to_candidates.contains_key(&epoch));

        let (votes, proposals, failed_proposals) =
            self.aggregation.get_aggregated_metrics(epoch_to_candidates, history, &self.author);

        epoch_to_candidates[&epoch]
            .iter()
            .map(|author| {
                let cur_votes = *votes.get(author).unwrap_or(&0);
                let cur_proposals = *proposals.get(author).unwrap_or(&0);
                let cur_failed_proposals = *failed_proposals.get(author).unwrap_or(&0);

                if cur_failed_proposals * 100 >
                    (cur_proposals + cur_failed_proposals) * self.failure_threshold_percent
                {
                    self.failed_weight
                } else if cur_proposals > 0 || cur_votes > 0 {
                    self.active_weight
                } else {
                    self.inactive_weight
                }
            })
            .collect()
    }

    fn get_legacy_weights(
        &self,
        epoch: u64,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        _history: &[NewBlockEvent],
    ) -> Vec<u64> {
        assert!(epoch_to_candidates.contains_key(&epoch));
        let candidates = &epoch_to_candidates[&epoch];

        let mut cache = self.legacy_cached_weights.lock();
        let counter = cache.0;
        let need_refresh = match cache.1 {
            Some(ref w) => {
                w.len() != candidates.len() || counter % LEGACY_PERFORMANCE_CACHE_REFRESH_CALLS == 0
            }
            None => true,
        };
        if !need_refresh {
            cache.0 = counter + 1;
            return cache.1.clone().unwrap();
        }

        let weights = self
            .fetch_and_compute_weights(candidates, BlockNumber::Latest)
            .unwrap_or_else(|error| {
                debug!(
                    error = ?error,
                    "ValidatorPerformances unavailable, fallback to active_weight"
                );
                candidates.iter().map(|_| self.active_weight).collect()
            });

        *cache = (counter + 1, Some(weights.clone()));
        weights
    }

    fn get_weights_at_block(
        &self,
        epoch: u64,
        epoch_to_candidates: &HashMap<u64, Vec<Author>>,
        block_number: u64,
    ) -> Result<Vec<u64>> {
        let candidates = epoch_to_candidates
            .get(&epoch)
            .ok_or_else(|| anyhow!("missing proposer candidates for epoch {}", epoch))?;

        let mut cache = self.anchored_cached_weights.lock();
        if let Some((cached_block_number, cached_weights)) = cache.as_ref() {
            if *cached_block_number == block_number && cached_weights.len() == candidates.len() {
                return Ok(cached_weights.clone());
            }
        }

        let weights =
            self.fetch_and_compute_weights(candidates, BlockNumber::Number(block_number))?;
        *cache = Some((block_number, weights.clone()));
        Ok(weights)
    }
}

/// Committed history based proposer election implementation that could help bias towards
/// successful leaders to help improve performance.
pub struct LeaderReputation {
    epoch: u64,
    epoch_to_proposers: HashMap<u64, Vec<Author>>,
    voting_powers: Vec<u64>,
    backend: Arc<dyn MetadataBackend>,
    reputation_anchor_backend: Option<Arc<dyn ReputationAnchorBackend>>,
    heuristic: Box<dyn ReputationHeuristic>,
    exclude_round: u64,
    use_root_hash: bool,
    window_for_chain_health: usize,
    #[cfg(test)]
    consensus_alpha_override: Option<bool>,
}

impl LeaderReputation {
    pub fn new(
        epoch: u64,
        epoch_to_proposers: HashMap<u64, Vec<Author>>,
        voting_powers: Vec<u64>,
        backend: Arc<dyn MetadataBackend>,
        heuristic: Box<dyn ReputationHeuristic>,
        exclude_round: u64,
        use_root_hash: bool,
        window_for_chain_health: usize,
    ) -> Self {
        assert!(epoch_to_proposers.contains_key(&epoch));
        assert_eq!(epoch_to_proposers[&epoch].len(), voting_powers.len());

        Self {
            epoch,
            epoch_to_proposers,
            voting_powers,
            backend,
            reputation_anchor_backend: None,
            heuristic,
            exclude_round,
            use_root_hash,
            window_for_chain_health,
            #[cfg(test)]
            consensus_alpha_override: None,
        }
    }

    pub fn with_reputation_anchor_backend(
        mut self,
        backend: Arc<dyn ReputationAnchorBackend>,
    ) -> Self {
        self.reputation_anchor_backend = Some(backend);
        self
    }

    #[cfg(test)]
    pub fn with_consensus_alpha_for_test(mut self, active: bool) -> Self {
        self.consensus_alpha_override = Some(active);
        self
    }

    fn is_consensus_alpha_active(&self, anchor: Option<CommittedBlockAnchor>) -> bool {
        let active =
            is_consensus_fork_active_at_epoch(ConsensusHardfork::ConsensusAlpha, self.epoch) ||
                anchor.is_some_and(|anchor| {
                    is_consensus_fork_active(
                        ConsensusHardfork::ConsensusAlpha,
                        anchor.timestamp_usecs,
                    )
                });

        #[cfg(test)]
        {
            self.consensus_alpha_override.unwrap_or(active)
        }
        #[cfg(not(test))]
        {
            active
        }
    }

    // NOTE(gravity): In Gravity, history is always empty because get_block_metadata
    // (Aptos-native) is bypassed. This function always returns 1.0 and metrics report zeros.
    // Kept for Aptos code compatibility; chain health is not tracked via this path.
    //
    // Compute chain health metrics, and
    // - return participating voting power percentage for the window_for_chain_health
    // - update metric counters for different windows
    fn compute_chain_health_and_add_metrics(
        &self,
        history: &[NewBlockEvent],
        round: Round,
    ) -> VotingPowerRatio {
        let candidates =
            self.epoch_to_proposers.get(&self.epoch).expect("Epoch should always map to proposers");
        // use f64 counter, as total voting power is u128
        let total_voting_power = self.voting_powers.iter().map(|v| *v as f64).sum();
        CHAIN_HEALTH_TOTAL_VOTING_POWER.set(total_voting_power);
        CHAIN_HEALTH_TOTAL_NUM_VALIDATORS.set(candidates.len() as i64);

        let mut result = None;

        for (counter_index, participants_window_size) in
            CHAIN_HEALTH_WINDOW_SIZES.iter().enumerate()
        {
            let chosen = self.window_for_chain_health == *participants_window_size;
            let sample_fraction = participants_window_size / 10;
            // Sample longer durations
            if chosen || sample_fraction <= 1 || (round % sample_fraction as u64) == 1 {
                let participants: HashSet<_> = NewBlockEventAggregation::count_votes_custom(
                    &self.epoch_to_proposers,
                    history,
                    *participants_window_size,
                    false,
                )
                .into_keys()
                .chain(
                    NewBlockEventAggregation::count_proposals_custom(
                        &self.epoch_to_proposers,
                        history,
                        *participants_window_size,
                        false,
                    )
                    .into_keys(),
                )
                .collect();

                let participating_voting_power = candidates
                    .iter()
                    .zip(self.voting_powers.iter())
                    .filter(|(c, _vp)| participants.contains(c))
                    .map(|(_c, vp)| *vp as f64)
                    .sum();

                if counter_index == max(CHAIN_HEALTH_WINDOW_SIZES.len() - 2, 0) {
                    // Only emit this for one window value. Currently defaults to 100
                    candidates.iter().for_each(|author| {
                        if participants.contains(author) {
                            CONSENSUS_PARTICIPATION_STATUS
                                .with_label_values(&[&author.to_hex()])
                                .set(1_i64)
                        } else {
                            CONSENSUS_PARTICIPATION_STATUS
                                .with_label_values(&[&author.to_hex()])
                                .set(0_i64)
                        }
                    });
                }

                CHAIN_HEALTH_PARTICIPATING_VOTING_POWER[counter_index]
                    .set(participating_voting_power);
                CHAIN_HEALTH_PARTICIPATING_NUM_VALIDATORS[counter_index]
                    .set(participants.len() as i64);

                if chosen {
                    // In Gravity, get_block_metadata (Aptos-native) is not used, so history
                    // is always empty. Return 1.0 to avoid treating the chain as unhealthy.
                    let voting_power_participation_ratio: VotingPowerRatio = if history.is_empty() ||
                        (history.len() < *participants_window_size && self.epoch <= 2)
                    {
                        1.0
                    } else if total_voting_power >= 1.0 {
                        participating_voting_power / total_voting_power
                    } else {
                        error!("Total voting power is {}, should never happen", total_voting_power);
                        1.0
                    };
                    CHAIN_HEALTH_REPUTATION_PARTICIPATING_VOTING_POWER_FRACTION
                        .set(voting_power_participation_ratio);
                    result = Some(voting_power_participation_ratio);
                }
            }
        }

        result.unwrap_or_else(|| {
            panic!(
                "asked window size {} not found in predefined window sizes: {:?}",
                self.window_for_chain_health, CHAIN_HEALTH_WINDOW_SIZES
            )
        })
    }
}

impl ProposerElection for LeaderReputation {
    fn get_valid_proposer_and_voting_power_participation_ratio(
        &self,
        round: Round,
    ) -> (Author, VotingPowerRatio) {
        let target_round = round.saturating_sub(self.exclude_round);
        let anchor = self.reputation_anchor_backend.as_ref().and_then(|backend| {
            backend.get_anchor(self.epoch, target_round).unwrap_or_else(|error| {
                panic!(
                    "failed to resolve reputation anchor for epoch {}, round {}: {:?}",
                    self.epoch, target_round, error
                )
            })
        });
        let alpha_active = self.is_consensus_alpha_active(anchor);

        let proposers = &self.epoch_to_proposers[&self.epoch];
        let (sliding_window, root_hash, mut weights) = if alpha_active {
            if let Some(anchor) = anchor {
                let weights = self
                    .heuristic
                    .get_weights_at_block(self.epoch, &self.epoch_to_proposers, anchor.block_number)
                    .unwrap_or_else(|error| {
                        panic!(
                            "failed to read ValidatorPerformances at committed block {}: {:?}",
                            anchor.block_number, error
                        )
                    });
                debug!(
                    "Using consensus-anchored validator performance: epoch={}, round={}, target_round={}, block_number={}",
                    self.epoch, round, target_round, anchor.block_number
                );
                (vec![], anchor.block_hash, weights)
            } else if self.reputation_anchor_backend.is_some() {
                warn!(
                    "No committed reputation anchor; using empty committed history: epoch={}, round={}, target_round={}",
                    self.epoch, round, target_round
                );
                let history = vec![];
                let weights =
                    self.heuristic.get_weights(self.epoch, &self.epoch_to_proposers, &history);
                (history, HashValue::zero(), weights)
            } else {
                let (history, root_hash) =
                    self.backend.get_block_metadata(self.epoch, target_round);
                let weights =
                    self.heuristic.get_weights(self.epoch, &self.epoch_to_proposers, &history);
                (history, root_hash, weights)
            }
        } else {
            let history = vec![];
            let weights =
                self.heuristic.get_legacy_weights(self.epoch, &self.epoch_to_proposers, &history);
            (history, HashValue::zero(), weights)
        };

        let voting_power_participation_ratio =
            self.compute_chain_health_and_add_metrics(&sliding_window, round);
        assert_eq!(weights.len(), proposers.len());

        // Multiply weights by voting power:
        let stake_weights: Vec<u128> = weights
            .iter_mut()
            .enumerate()
            .map(|(i, w)| *w as u128 * self.voting_powers[i] as u128)
            .collect();

        let state = if self.use_root_hash {
            [root_hash.to_vec(), self.epoch.to_le_bytes().to_vec(), round.to_le_bytes().to_vec()]
                .concat()
        } else {
            [self.epoch.to_le_bytes().to_vec(), round.to_le_bytes().to_vec()].concat()
        };

        let chosen_index = choose_index(stake_weights, state);
        (proposers[chosen_index], voting_power_participation_ratio)
    }

    fn get_valid_proposer(&self, round: Round) -> Author {
        self.get_valid_proposer_and_voting_power_participation_ratio(round).0
    }

    fn get_voting_power_participation_ratio(&self, round: Round) -> VotingPowerRatio {
        self.get_valid_proposer_and_voting_power_participation_ratio(round).1
    }
}

pub(crate) fn extract_epoch_to_proposers_impl(
    next_epoch_states_and_cur_epoch_rounds: &[(&EpochState, u64)],
    epoch: u64,
    proposers: &[Author],
    needed_rounds: u64,
) -> Result<HashMap<u64, Vec<Author>>> {
    let last_index = next_epoch_states_and_cur_epoch_rounds.len() - 1;
    let mut num_rounds = 0;
    let mut result = HashMap::new();
    for (index, (next_epoch_state, cur_epoch_rounds)) in
        next_epoch_states_and_cur_epoch_rounds.iter().enumerate().rev()
    {
        let next_epoch_proposers =
            next_epoch_state.verifier.get_ordered_account_addresses_iter().collect::<Vec<_>>();
        if index == last_index {
            ensure!(
                epoch == next_epoch_state.epoch,
                "fetched epoch_ending ledger_infos are for a wrong epoch {} vs {}",
                epoch,
                next_epoch_state.epoch
            );
            ensure!(
                proposers == next_epoch_proposers,
                "proposers from state and fetched epoch_ending ledger_infos are missaligned"
            );
        }
        result.insert(next_epoch_state.epoch, next_epoch_proposers);

        if num_rounds > needed_rounds {
            break;
        }
        // LI contains validator set for next epoch (i.e. in next_epoch_state)
        // so after adding the number of rounds in the epoch, we need to process the
        // corresponding ValidatorSet on the previous ledger info,
        // before checking if we are done.
        num_rounds += cur_epoch_rounds;
    }

    ensure!(
        result.contains_key(&epoch),
        "Current epoch ({}) not in fetched map ({:?})",
        epoch,
        result.keys().collect::<Vec<_>>()
    );
    Ok(result)
}

pub fn extract_epoch_to_proposers(
    proof: EpochChangeProof,
    epoch: u64,
    proposers: &[Author],
    needed_rounds: u64,
) -> Result<HashMap<u64, Vec<Author>>> {
    extract_epoch_to_proposers_impl(
        &proof
            .ledger_info_with_sigs
            .iter()
            .map::<Result<(&EpochState, u64)>, _>(|ledger_info| {
                let cur_epoch_rounds = ledger_info.ledger_info().round();
                let next_epoch_state = ledger_info
                    .ledger_info()
                    .next_epoch_state()
                    .ok_or_else(|| anyhow::anyhow!("no cur_epoch_state"))?;
                Ok((next_epoch_state, cur_epoch_rounds))
            })
            .collect::<Result<Vec<_>, _>>()?,
        epoch,
        proposers,
        needed_rounds,
    )
}
