// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{block::Block, quorum_cert::QuorumCert};
use anyhow::ensure;
use gaptos::{
    api_types::ExecutionBlocks,
    aptos_crypto::hash::{HashValue, GENESIS_BLOCK_ID},
    aptos_short_hex_str::AsShortHexStr,
    aptos_types::{ledger_info::LedgerInfoWithSignatures, validator_verifier::ValidatorVerifier},
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const NUM_RETRIES: usize = 5;
pub const NUM_PEERS_PER_RETRY: usize = 1;
pub const RETRY_INTERVAL_MSEC: u64 = 500;
pub const RPC_TIMEOUT_MSEC: u64 = 5000;

/// RPC to get a chain of block of the given length starting from the given block id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BlockRetrievalRequest {
    block_id: HashValue,
    num_blocks: u64,
    target_block_id: Option<HashValue>,
    epoch: Option<u64>,
}

impl BlockRetrievalRequest {
    pub fn new(block_id: HashValue, num_blocks: u64) -> Self {
        Self { block_id, num_blocks, target_block_id: None, epoch: None }
    }

    pub fn new_with_target_block_id(
        block_id: HashValue,
        num_blocks: u64,
        target_block_id: HashValue,
    ) -> Self {
        Self { block_id, num_blocks, target_block_id: Some(target_block_id), epoch: None }
    }

    pub fn new_with_epoch(
        block_id: HashValue,
        num_blocks: u64,
        target_block_id: HashValue,
        epoch: u64,
    ) -> Self {
        Self { block_id, num_blocks, target_block_id: Some(target_block_id), epoch: Some(epoch) }
    }

    pub fn block_id(&self) -> HashValue {
        self.block_id
    }

    pub fn num_blocks(&self) -> u64 {
        self.num_blocks
    }

    pub fn target_block_id(&self) -> Option<HashValue> {
        self.target_block_id
    }

    pub fn match_target_id(&self, hash_value: HashValue) -> bool {
        self.target_block_id.map_or(false, |id| id == hash_value)
    }

    pub fn epoch(&self) -> Option<u64> {
        self.epoch
    }
}

impl fmt::Display for BlockRetrievalRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[BlockRetrievalRequest starting from id {} with {} blocks]",
            self.block_id, self.num_blocks
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum BlockRetrievalStatus {
    // Successfully fill in the request.
    Succeeded,
    // Can not find the block corresponding to block_id.
    IdNotFound,
    // Can not find enough blocks but find some.
    NotEnoughBlocks,
    // Successfully found the target,
    SucceededWithTarget,
    // Can not find the quorum certificate for the block.
    QuorumCertNotFound,
}

/// Carries the returned blocks and the retrieval status.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BlockRetrievalResponse {
    status: BlockRetrievalStatus,
    blocks: Vec<(Block, Option<u64>, Option<Vec<u8>>)>, // (block, block_number, randomness)
    ledger_infos: Vec<LedgerInfoWithSignatures>,
    quorum_certs: Vec<QuorumCert>,
}

impl BlockRetrievalResponse {
    pub fn new(
        status: BlockRetrievalStatus,
        blocks: Vec<(Block, Option<u64>, Option<Vec<u8>>)>,
        quorum_certs: Vec<QuorumCert>,
        ledger_infos: Vec<LedgerInfoWithSignatures>,
    ) -> Self {
        Self { status, blocks, quorum_certs, ledger_infos }
    }

    pub fn status(&self) -> BlockRetrievalStatus {
        self.status.clone()
    }

    pub fn blocks(&self) -> &Vec<(Block, Option<u64>, Option<Vec<u8>>)> {
        &self.blocks
    }

    pub fn ledger_infos(&self) -> &Vec<LedgerInfoWithSignatures> {
        &self.ledger_infos
    }

    pub fn quorum_certs(&self) -> &Vec<QuorumCert> {
        &self.quorum_certs
    }

    pub fn verify(
        &self,
        retrieval_request: BlockRetrievalRequest,
        sig_verifier: &ValidatorVerifier,
    ) -> anyhow::Result<()> {
        ensure!(
            self.status != BlockRetrievalStatus::Succeeded ||
                self.blocks.len() as u64 == retrieval_request.num_blocks(),
            "not enough blocks returned, expect {}, get {}",
            retrieval_request.num_blocks(),
            self.blocks.len(),
        );
        // Chain linkage must be checked sequentially: each block's parent is the next in the
        // response. This is cheap (hash comparisons only).
        let mut expected_id = retrieval_request.block_id();
        for (block, _, _) in &self.blocks {
            ensure!(
                block.id() == expected_id,
                "blocks doesn't form a chain: expect {}, get {}",
                expected_id,
                block.id()
            );
            expected_id = block.parent_id();
        }
        // Signature/QC verification (BLS aggregate verify per block) is the dominant cost and is
        // independent per block, so run it in parallel across the batch. This is the hot path for
        // fast-forward sync, where one response can carry hundreds/thousands of blocks and the
        // sequential verify was the fetch bottleneck (~1.8ms/block).
        self.blocks.par_iter().try_for_each(|(block, _, _)| -> anyhow::Result<()> {
            block.validate_signature(sig_verifier)?;
            block.verify_well_formed()?;
            Ok(())
        })
    }
}

impl fmt::Display for BlockRetrievalResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.status() {
            BlockRetrievalStatus::Succeeded | BlockRetrievalStatus::SucceededWithTarget => {
                write!(
                    f,
                    "[BlockRetrievalResponse: status: {:?}, num_blocks: {}, block_ids: ",
                    self.status(),
                    self.blocks().len(),
                )?;

                f.debug_list()
                    .entries(self.blocks.iter().map(|(b, _, _)| b.id().short_str()))
                    .finish()?;

                write!(f, "]")
            }
            _ => write!(f, "[BlockRetrievalResponse: status: {:?}]", self.status()),
        }
    }
}
