// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use super::proposer_election::{ProposerElection, ProposerElectionCacheKey};
use crate::liveness::cached_proposer_election::CachedProposerElection;
use aptos_consensus_types::common::{Author, Round};
use gaptos::aptos_infallible::Mutex;
use std::{cell::Cell, sync::Arc};

struct MockProposerElection {
    proposers: Vec<Author>,
    asked: Arc<Mutex<Cell<u32>>>,
}

impl MockProposerElection {
    pub fn new(proposers: Vec<Author>, asked: Arc<Mutex<Cell<u32>>>) -> Self {
        Self { proposers, asked }
    }
}

impl ProposerElection for MockProposerElection {
    fn get_valid_proposer(&self, round: Round) -> Author {
        let round_uszie = round as usize;
        let asked = self.asked.lock();
        asked.replace(asked.get() + 1);
        self.proposers[round_uszie % self.proposers.len()]
    }
}

struct VersionedMockProposerElection {
    proposers: Vec<Author>,
    asked: Arc<Mutex<Cell<u32>>>,
    version: Arc<Mutex<Cell<u8>>>,
}

impl VersionedMockProposerElection {
    pub fn new(
        proposers: Vec<Author>,
        asked: Arc<Mutex<Cell<u32>>>,
        version: Arc<Mutex<Cell<u8>>>,
    ) -> Self {
        Self { proposers, asked, version }
    }
}

impl ProposerElection for VersionedMockProposerElection {
    fn get_valid_proposer(&self, _round: Round) -> Author {
        let asked = self.asked.lock();
        asked.replace(asked.get() + 1);
        self.proposers[self.version.lock().get() as usize % self.proposers.len()]
    }

    fn cache_key(&self, round: Round) -> Option<ProposerElectionCacheKey> {
        Some(ProposerElectionCacheKey::with_version(round, vec![self.version.lock().get()]))
    }
}

#[test]
fn test_get_valid_proposer_caching() {
    let asked = Arc::new(Mutex::new(Cell::new(0)));
    let authors: Vec<Author> = (0..4).map(|_| Author::random()).collect();
    let cpe = CachedProposerElection::new(
        1,
        Box::new(MockProposerElection::new(authors.clone(), asked.clone())),
        10,
    );

    assert_eq!(asked.lock().get(), 0);

    assert_eq!(cpe.get_valid_proposer(0), authors[0]);
    assert_eq!(asked.lock().get(), 1);
    assert!(cpe.is_valid_proposer(authors[0], 0));
    assert!(!cpe.is_valid_proposer(authors[1], 0));
    assert_eq!(asked.lock().get(), 1);

    assert_eq!(cpe.get_valid_proposer(1), authors[1]);
    assert_eq!(asked.lock().get(), 2);
    assert!(cpe.is_valid_proposer(authors[1], 1));
    assert!(!cpe.is_valid_proposer(authors[0], 1));
    assert_eq!(asked.lock().get(), 2);

    assert_eq!(cpe.get_valid_proposer(0), authors[0]);
    assert_eq!(asked.lock().get(), 2);

    assert_eq!(cpe.get_valid_proposer(11), authors[3]);
    assert_eq!(asked.lock().get(), 3);
    assert!(cpe.is_valid_proposer(authors[3], 11));
    assert!(!cpe.is_valid_proposer(authors[0], 11));
    assert_eq!(asked.lock().get(), 3);

    // round=0 is outside the caching window, and round=1 is still inside
    assert_eq!(cpe.get_valid_proposer(0), authors[0]);
    assert_eq!(asked.lock().get(), 4);

    assert_eq!(cpe.get_valid_proposer(1), authors[1]);
    assert_eq!(asked.lock().get(), 4);
}

#[test]
fn test_get_valid_proposer_recomputes_when_cache_key_version_changes() {
    let asked = Arc::new(Mutex::new(Cell::new(0)));
    let version = Arc::new(Mutex::new(Cell::new(0)));
    let authors: Vec<Author> = (0..4).map(|_| Author::random()).collect();
    let cpe = CachedProposerElection::new(
        1,
        Box::new(VersionedMockProposerElection::new(
            authors.clone(),
            asked.clone(),
            version.clone(),
        )),
        10,
    );

    assert_eq!(cpe.get_valid_proposer(3), authors[0]);
    assert_eq!(cpe.get_valid_proposer(3), authors[0]);
    assert_eq!(asked.lock().get(), 1);

    version.lock().replace(1);
    assert_eq!(cpe.get_valid_proposer(3), authors[1]);
    assert_eq!(cpe.get_valid_proposer(3), authors[1]);
    assert_eq!(asked.lock().get(), 2);
}
