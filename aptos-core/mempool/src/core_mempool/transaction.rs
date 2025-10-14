// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::network::BroadcastPeerPriority;
use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId},
    u256_define::TxnHash,
};
use gaptos::aptos_crypto::{HashValue, Uniform};
use gaptos::aptos_types::{
    account_address::AccountAddress,
    chain_id::{self, ChainId},
    transaction::{RawTransaction, SignedTransaction, TransactionPayload},
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    hash::Hash,
    mem::size_of,
    sync::{atomic::AtomicUsize, Arc},
    time::SystemTime,
};
use gaptos::aptos_mempool::counters as counters;

/// Estimated per-txn size minus the raw transaction
pub const TXN_FIXED_ESTIMATED_BYTES: usize = size_of::<MempoolTransaction>();

// TODO(gravity_byteyue&gravity_jan): consider elegant way for verifiedTxn and signedTransaction
static GLOBAL_PUBLIC_KEY: Lazy<gaptos::aptos_crypto::ed25519::Ed25519PublicKey> = Lazy::new(|| {
    gaptos::aptos_crypto::PrivateKey::public_key(
        &gaptos::aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing(),
    )
});

static GLOBAL_SIGNATURE: Lazy<gaptos::aptos_crypto::ed25519::Ed25519Signature> =
    Lazy::new(|| gaptos::aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap());

/// TODO(gravity_byteyue): is this function useful? it seems not right
impl From<&SignedTransaction> for VerifiedTxn {
    fn from(signed_txn: &SignedTransaction) -> Self {
        let raw_txn = signed_txn.payload();
        let bytes = match raw_txn {
            TransactionPayload::GTxnBytes(bytes) => bytes.clone(),
            _ => panic!("Unexpected TransactionPayload type"),
        };
        Self {
            bytes,
            sender: signed_txn.sender(),
            sequence_number: signed_txn.sequence_number(),
            chain_id: signed_txn.chain_id(),
            committed_hash: signed_txn.committed_hash(),
        }
    }
}

impl Into<SignedTransaction> for VerifiedTxn {
    fn into(self) -> SignedTransaction {
        let raw_txn = RawTransaction::new(
            self.sender,
            self.sequence_number,
            TransactionPayload::GTxnBytes(self.bytes.clone()),
            u64::MAX,
            0,
            u64::MAX,
            self.chain_id,
        );
        SignedTransaction::new_with_committed_hash(
            raw_txn,
            GLOBAL_PUBLIC_KEY.clone(),
            GLOBAL_SIGNATURE.clone(),
            self.committed_hash,
        )
    }
}

impl VerifiedTxn {
    pub fn new(
        bytes: Vec<u8>,
        sender: AccountAddress,
        sequence_number: u64,
        chain_id: ChainId,
        committed_hash: HashValue,
    ) -> Self {
        Self { bytes, sender, sequence_number, chain_id, committed_hash }
    }

    pub fn bytes(&self) -> &Vec<u8> {
        &self.bytes
    }

    pub fn sender(&self) -> AccountAddress {
        self.sender
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_number
    }

    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    pub fn get_hash(&self) -> HashValue {
        self.committed_hash
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SequenceInfo {
    pub transaction_sequence_number: u64,
    pub account_sequence_number: u64,
}

#[derive(Clone)]
pub struct VerifiedTxn {
    pub(crate) bytes: Vec<u8>,
    pub(crate) sender: AccountAddress,
    pub(crate) sequence_number: u64,
    pub(crate) chain_id: chain_id::ChainId,
    pub(crate) committed_hash: HashValue,
}

impl std::fmt::Debug for VerifiedTxn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VerifiedTxn {{ sender: {:?}, sequence_number: {:?}, committed_hash: {:?} }}", self.sender, self.sequence_number, self.committed_hash)
    }
}

impl From<gaptos::api_types::VerifiedTxn> for VerifiedTxn {
    fn from(value: gaptos::api_types::VerifiedTxn) -> Self {
        let committed_hash = HashValue::new(value.committed_hash());
        VerifiedTxn {
            bytes: value.bytes,
            sender: AccountAddress::new(value.sender.bytes()),
            sequence_number: value.sequence_number,
            chain_id: value.chain_id.into_u64().into(),
            committed_hash,
        }
    }
}

impl From<VerifiedTxn> for gaptos::api_types::VerifiedTxn {
    fn from(value: VerifiedTxn) -> Self {
        gaptos::api_types::VerifiedTxn::new(
            value.bytes,
            ExternalAccountAddress::new(value.sender.into_bytes()),
            value.sequence_number,
            ExternalChainId::new(value.chain_id.into()),
            TxnHash::new(*value.committed_hash),
        )
    }
}

impl From<SignedTransaction> for VerifiedTxn {
    fn from(value: SignedTransaction) -> Self {
        let committed_hash = value.committed_hash();
        let bytes = match value.payload() {
            TransactionPayload::GTxnBytes(bytes) => bytes.clone(),
            _ => panic!("Unexpected TransactionPayload type"),
        };
        VerifiedTxn {
            bytes,
            sender: AccountAddress::new(value.sender().into_bytes()),
            sequence_number: value.sequence_number(),
            chain_id: value.chain_id(),
            committed_hash,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MempoolTransaction {
    verified_txn: SignedTransaction,
    pub timeline_state: TimelineState,
    insertion_info: InsertionInfo,
    ranking_score: u64,
    priority_of_sender: Option<BroadcastPeerPriority>,
    sequence_info: SequenceInfo,
}

impl MempoolTransaction {
    pub(crate) fn new(
        verified_txn: VerifiedTxn,
        timeline_state: TimelineState,
        insertion_info: InsertionInfo,
        priority_of_sender: Option<BroadcastPeerPriority>,
        ranking_score: u64,
        account_sequence_number: u64,
    ) -> Self {
        let txn_sequence_number = verified_txn.sequence_number;
        Self {
            verified_txn: (verified_txn).into(),
            timeline_state,
            insertion_info,
            priority_of_sender,
            ranking_score,
            sequence_info: SequenceInfo {
                transaction_sequence_number: txn_sequence_number,
                account_sequence_number,
            },
        }
    }

    pub fn sender(&self) -> AccountAddress {
        self.verified_txn.sender()
    }

    pub fn account_sequence_number(&self) -> u64 {
        self.sequence_info.account_sequence_number
    }

    pub fn set_account_sequence_number(&mut self, account_sequence_number: u64) {
        self.sequence_info.account_sequence_number = account_sequence_number;
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_info.transaction_sequence_number
    }

    pub(crate) fn priority_of_sender(&self) -> &Option<BroadcastPeerPriority> {
        &self.priority_of_sender
    }

    pub(crate) fn get_hash(&self) -> HashValue {
        // HashValue::sha3_256_of(&self.verified_txn.bytes)
        // TODO(gravity_byteyue&lk)
        self.verified_txn.committed_hash()
    }

    pub(crate) fn get_estimated_bytes(&self) -> usize {
        // TXN_FIXED_ESTIMATED_BYTES + self.verified_txn.bytes.len()
        TXN_FIXED_ESTIMATED_BYTES + {
            match self.verified_txn.payload() {
                TransactionPayload::GTxnBytes(b) => b.len(),
                _ => panic!(),
            }
        }
    }

    pub(crate) fn ranking_score(&self) -> u64 {
        self.ranking_score
    }

    pub(crate) fn verified_txn(&self) -> &SignedTransaction {
        &self.verified_txn
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
