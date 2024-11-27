// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{core_mempool::TXN_INDEX_ESTIMATED_BYTES, counters, network::BroadcastPeerPriority};
use aptos_crypto::HashValue;
use aptos_types::{account_address::AccountAddress, transaction::SignedTransaction};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use std::{
    mem::size_of,
    sync::{atomic::AtomicUsize, Arc},
    time::{Duration, SystemTime},
};

/// Estimated per-txn size minus the raw transaction
pub const TXN_FIXED_ESTIMATED_BYTES: usize = size_of::<MempoolTransaction>();



#[derive(Clone, Debug)]
pub struct MempoolTransaction {
    pub txn_bytes: Vec<u8>,
    pub account: AccountAddress,
    pub timeline_state: TimelineState,
    pub seq_number: u64,
    insertion_info: InsertionInfo,
    priority_of_sender: Option<BroadcastPeerPriority>
}

impl MempoolTransaction {
    pub(crate) fn new(
        txn_bytes: Vec<u8>,
        account: AccountAddress,
        seq_number: u64,
        timeline_state: TimelineState,
        insertion_info: InsertionInfo,
        priority_of_sender: Option<BroadcastPeerPriority>,
    ) -> Self {
        Self {
            txn_bytes,
            account,
            timeline_state,
            seq_number,
            insertion_info,
            priority_of_sender,
        }
    }

    pub(crate) fn txn(&self) -> &[u8] {
        &self.txn_bytes
    }

    pub(crate) fn priority_of_sender(&self) -> &Option<BroadcastPeerPriority> {
        &self.priority_of_sender
    }

    pub(crate) fn get_hash(&self) -> HashValue {
        HashValue::sha3_256_of(&self.txn_bytes)
    }

    pub(crate) fn get_sender(&self) -> AccountAddress {
        self.account
    }

    pub(crate) fn get_estimated_bytes(&self) -> usize {
        TXN_FIXED_ESTIMATED_BYTES + self.txn_bytes.len()
    }

    pub(crate) fn get_sequence_number(&self) -> u64 {
        self.seq_number
    }

    pub(crate) fn ranking_score(&self) -> u64 {
        // diff from sequence number of the account to current txn sequence number
        todo!()
    }

    pub(crate) fn insertion_info(&self) -> &InsertionInfo {
        &self.insertion_info
    }

    pub(crate) fn get_mut_insertion_info(&mut self) -> &mut InsertionInfo {
        &mut self.insertion_info
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Hash, Serialize)]
pub enum TimelineState {
    // The transaction is ready for broadcast.
    // Associated integer represents it's position in the log of such transactions.
    Ready(u64),
    // Transaction is not yet ready for broadcast, but it might change in a future.
    NotReady,
    // Transaction will never be qualified for broadcasting.
    // Currently we don't broadcast transactions originated on other peers.
    NonQualified,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SequenceInfo {
    pub transaction_sequence_number: u64,
    pub account_sequence_number: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum SubmittedBy {
    /// The transaction was received from a client REST API submission, rather than a mempool
    /// broadcast. This can be used as the time a transaction first entered the network,
    /// to measure end-to-end latency within the entire network. However, if a transaction is
    /// submitted to multiple nodes (by the client) then the end-to-end latency measured will not
    /// be accurate (the measured value will be lower than the correct value).
    Client,
    /// The transaction was received from a downstream peer, i.e., not a client or a peer validator.
    /// At a validator, a transaction from downstream can be used as the time a transaction first
    /// entered the validator network, to measure end-to-end latency within the validator network.
    /// However, if a transaction enters via multiple validators (due to duplication outside of the
    /// validator network) then the validator end-to-end latency measured will not be accurate
    /// (the measured value will be lower than the correct value).
    Downstream,
    /// The transaction was received at a validator from another validator, rather than from the
    /// downstream VFN. This transaction should not be used to measure end-to-end latency within the
    /// validator network (see Downstream).
    /// Note, with Quorum Store enabled, no transactions will be classified as PeerValidator.
    PeerValidator,
}

#[derive(Debug, Clone)]
pub struct InsertionInfo {
    pub insertion_time: SystemTime,
    pub ready_time: SystemTime,
    pub park_time: Option<SystemTime>,
    pub submitted_by: SubmittedBy,
    pub consensus_pulled_counter: Arc<AtomicUsize>,
}

impl InsertionInfo {
    pub fn new(
        insertion_time: SystemTime,
        client_submitted: bool,
        timeline_state: TimelineState,
    ) -> Self {
        let submitted_by = if client_submitted {
            SubmittedBy::Client
        } else if timeline_state == TimelineState::NonQualified {
            SubmittedBy::PeerValidator
        } else {
            SubmittedBy::Downstream
        };
        Self {
            insertion_time,
            ready_time: insertion_time,
            park_time: None,
            submitted_by,
            consensus_pulled_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn submitted_by_label(&self) -> &'static str {
        match self.submitted_by {
            SubmittedBy::Client => counters::SUBMITTED_BY_CLIENT_LABEL,
            SubmittedBy::Downstream => counters::SUBMITTED_BY_DOWNSTREAM_LABEL,
            SubmittedBy::PeerValidator => counters::SUBMITTED_BY_PEER_VALIDATOR_LABEL,
        }
    }
}