// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    pending_votes::{PendingVotes, VoteReceptionResult},
    util::time_service::TimeService,
};
use gaptos::aptos_config::config::QcAggregatorType;
use aptos_consensus_types::{delayed_qc_msg::DelayedQcMsg, vote::Vote};
use gaptos::aptos_logger::{error, info};
use gaptos::aptos_types::{
    ledger_info::LedgerInfoWithVerifiedSignatures, validator_verifier::ValidatorVerifier,
};
use futures::SinkExt;
use futures_channel::mpsc::UnboundedSender;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

pub trait QcAggregator: Send + Sync {
    fn handle_aggregated_qc(
        &mut self,
        validator_verifier: &ValidatorVerifier,
        aggregated_voting_power: u128,
        vote: &Vote,
        li_with_sig: &LedgerInfoWithVerifiedSignatures,
    ) -> VoteReceptionResult;
}

struct NoDelayQcAggregator {}

pub fn create_qc_aggregator(
    qc_aggregator_type: QcAggregatorType,
    time_service: Arc<dyn TimeService>,
    delayed_qc_tx: UnboundedSender<DelayedQcMsg>,
) -> Box<dyn QcAggregator> {
    match qc_aggregator_type {
        QcAggregatorType::NoDelay => Box::new(NoDelayQcAggregator {}),
    }
}

impl QcAggregator for NoDelayQcAggregator {
    fn handle_aggregated_qc(
        &mut self,
        validator_verifier: &ValidatorVerifier,
        aggregated_voting_power: u128,
        vote: &Vote,
        li_with_sig: &LedgerInfoWithVerifiedSignatures,
    ) -> VoteReceptionResult {
        assert!(
            aggregated_voting_power >= validator_verifier.quorum_voting_power(),
            "QC aggregation should not be triggered if we don't have enough votes to form a QC"
        );
        PendingVotes::aggregate_qc_now(validator_verifier, li_with_sig, vote.vote_data())
    }
}

