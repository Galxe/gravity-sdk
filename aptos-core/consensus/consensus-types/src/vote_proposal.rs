// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{block::Block, vote_data::VoteData};
use aptos_crypto::hash::{TransactionAccumulatorHasher, ACCUMULATOR_PLACEHOLDER_HASH};
use aptos_crypto_derive::{BCSCryptoHash, CryptoHasher};
use aptos_types::{
    epoch_state::EpochState,
    proof::{accumulator::InMemoryTransactionAccumulator, AccumulatorExtensionProof},
};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// This structure contains all the information needed by safety rules to
/// evaluate a proposal / block for correctness / safety and to produce a Vote.
#[derive(Clone, Debug, CryptoHasher, Deserialize, BCSCryptoHash, Serialize)]
pub struct VoteProposal {
    /// The block / proposal to evaluate
    #[serde(bound(deserialize = "Block: Deserialize<'de>"))]
    block: Block,
    /// An optional field containing the next epoch info.
    next_epoch_state: Option<EpochState>,
    /// Represents whether the executed state id is dummy or not.
    decoupled_execution: bool,
}

impl VoteProposal {
    pub fn new(
        block: Block,
        next_epoch_state: Option<EpochState>,
        decoupled_execution: bool,
    ) -> Self {
        Self {
            block,
            next_epoch_state,
            decoupled_execution,
        }
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn next_epoch_state(&self) -> Option<&EpochState> {
        self.next_epoch_state.as_ref()
    }

    /// This function returns the vote data with a dummy executed_state_id and version
    fn vote_data_ordering_only(&self) -> VoteData {
        VoteData::new(
            self.block().gen_block_info(
                *ACCUMULATOR_PLACEHOLDER_HASH,
                0,
                self.next_epoch_state().cloned(),
            ),
            self.block().quorum_cert().certified_block().clone(),
        )
    }

    /// This function returns the vote data with a extension proof.
    /// Attention: this function itself does not verify the proof.
    fn vote_data_with_extension_proof(
        &self,
        new_tree: &InMemoryTransactionAccumulator,
    ) -> VoteData {
        VoteData::new(
            self.block().gen_block_info(
                new_tree.root_hash(),
                new_tree.version(),
                self.next_epoch_state().cloned(),
            ),
            self.block().quorum_cert().certified_block().clone(),
        )
    }

    /// Generate vote data depends on the config.
    pub fn gen_vote_data(&self) -> anyhow::Result<VoteData> {
        if self.decoupled_execution {
            Ok(self.vote_data_ordering_only())
        } else {
            panic!("decoupled_execution must be true")
        }
    }
}

impl Display for VoteProposal {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "VoteProposal[block: {}]", self.block,)
    }
}
