// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use super::proposer_election::{ProposerElection, ProposerElectionCacheKey};
use aptos_consensus_types::common::{Author, Round};
use gaptos::{
    aptos_consensus::counters::PROPOSER_ELECTION_DURATION, aptos_infallible::Mutex,
    aptos_logger::prelude::info,
};
use std::collections::BTreeMap;

// Wrapper around ProposerElection.
//
// Function get_valid_proposer can be expensive, and we want to make sure
// it is computed only once for a given round.
pub struct CachedProposerElection {
    epoch: u64,
    proposer_election: Box<dyn ProposerElection + Send + Sync>,
    // We use BTreeMap since we want a fixed window of cached elements
    // to look back (and caller knows how big of a window it needs).
    // LRU cache wouldn't work as well, as access order of the elements
    // would define eviction, and could lead to evicting still needed elements.
    recent_elections: Mutex<BTreeMap<ProposerElectionCacheKey, (Author, f64)>>,
    window: usize,
}

impl CachedProposerElection {
    pub fn new(
        epoch: u64,
        proposer_election: Box<dyn ProposerElection + Send + Sync>,
        window: usize,
    ) -> Self {
        Self { epoch, proposer_election, recent_elections: Mutex::new(BTreeMap::new()), window }
    }

    pub fn get_or_compute_entry(&self, round: Round) -> (Author, f64) {
        let compute = || {
            let _timer = PROPOSER_ELECTION_DURATION.start_timer();
            let result = self
                .proposer_election
                .get_valid_proposer_and_voting_power_participation_ratio(round);
            info!("ProposerElection for epoch {} and round {}: {:?}", self.epoch, round, result);
            result
        };
        let Some(cache_key) = self.proposer_election.cache_key(round) else {
            return compute();
        };
        let mut recent_elections = self.recent_elections.lock();

        if round > self.window as u64 {
            *recent_elections = recent_elections
                .split_off(&ProposerElectionCacheKey::new(round - self.window as u64));
        }

        *recent_elections.entry(cache_key).or_insert_with(compute)
    }
}

impl ProposerElection for CachedProposerElection {
    fn get_valid_proposer(&self, round: Round) -> Author {
        self.get_or_compute_entry(round).0
    }

    fn get_voting_power_participation_ratio(&self, round: Round) -> f64 {
        self.get_or_compute_entry(round).1
    }
}
