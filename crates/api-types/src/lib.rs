pub mod account;
pub mod default_recover;
pub mod mock_execution_layer;
pub mod simple_hash;
pub mod u256_define;
use crate::account::{ExternalAccountAddress, ExternalChainId};
use async_trait::async_trait;
use core::str;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::{fmt::Debug, hash::Hasher, sync::Arc};
use u256_define::{BlockId, ComputeRes, Random, TxnHash};

#[async_trait]
pub trait ConsensusApi: Send + Sync {
    async fn send_ordered_block(&self, parent_id: [u8; 32], ordered_block: ExternalBlock);

    async fn recv_executed_block_hash(&self, head: ExternalBlockMeta) -> ComputeRes;

    async fn commit_block_hash(&self, head: [u8; 32]);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ExecutionBlocks {
    pub latest_block_hash: [u8; 32],
    pub latest_block_number: u64,
    pub latest_ts: u64,
    pub blocks: Vec<Vec<u8>>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct ExternalPayloadAttr {
    // s since epoch
    pub ts: u64,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ExternalBlockMeta {
    // Unique identifier for block: hash of block body
    pub block_id: BlockId,
    pub block_number: u64,
    pub usecs: u64,
    pub randomness: Option<Random>,
}

#[derive(Debug, Clone)]
pub struct ExternalBlock {
    pub block_meta: ExternalBlockMeta,
    pub txns: Vec<VerifiedTxn>,
}

#[derive(Debug)]
pub enum ExecError {
    InternalError,
}

pub enum ExecTxn {
    RawTxn(Vec<u8>),          // from client
    VerifiedTxn(VerifiedTxn), // from peer
}

#[derive(Debug, Clone)]
pub struct VerifiedTxnWithAccountSeqNum {
    pub txn: VerifiedTxn,
    pub account_seq_num: u64,
}

#[async_trait]
pub trait ExecutionApiV2: Send + Sync {
    ///
    /// # Returns
    /// A `Vec` containing tuples, where each tuple consists of:
    /// - `TxnHash`: The committed hash for the newly added txn(不论是raw还是verifiedtxn都应该重新计算一次,广播过来的verified txn也应该计算一次——aptos是这样).
    /// - `sender_latest_committed_sequence_number`: The latest committed sequence number associated with the sender on the execution layer.
    ///
    async fn add_txn(&self, bytes: ExecTxn) -> Result<TxnHash, ExecError>;

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError>;

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError>;

    ///
    /// # Returns
    /// A `Vec` containing tuples, where each tuple consists of:
    /// - `VerifiedTxn`: The transaction object.
    /// - `sender_latest_committed_sequence_number`: The latest committed sequence number associated with the sender on the execution layer.
    ///
    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError>;

    // parent的: reth的hash -> 这个得等yuxuan重构reth
    // 当前block的: txns, 自己的block_number(aptos和reth一样)
    // async fn send_ordered_block(&self, ordered_block: Vec<Txns>, block_number: BlockNumber, parent_mata_data: ExternalBlockMeta) -> Result<(), ExecError>;
    async fn send_ordered_block(
        &self,
        parent_id: BlockId,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError>;

    // the block hash is the hash of the block that has been executed, which is passed by the send_ordered_block
    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError>;

    // this function is called by the execution layer commit the block hash
    async fn commit_block(&self, block_id: BlockId) -> Result<(), ExecError>;
}

#[derive(Debug)]
pub enum RecoveryError {
    InternalError(String),
    UnimplementError,
}

#[async_trait]
pub trait RecoveryApi: Send + Sync {
    async fn latest_block_number(&self) -> u64;

    async fn finalized_block_number(&self) -> u64;

    async fn recover_ordered_block(&self, parent_id: BlockId, block: ExternalBlock) -> Result<(), ExecError>;

    async fn recover_execution_blocks(&self, blocks: ExecutionBlocks);

    async fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> Result<ExecutionBlocks, RecoveryError>;
}

#[derive(Clone)]
pub struct ExecutionLayer {
    pub execution_api: Arc<dyn ExecutionApiV2>,
    pub recovery_api: Arc<dyn RecoveryApi>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerifiedTxn {
    pub bytes: Vec<u8>,
    pub sender: ExternalAccountAddress,
    pub sequence_number: u64,
    pub chain_id: ExternalChainId,
    #[serde(skip)]
    pub committed_hash: OnceCell<TxnHash>,
}

impl Hash for VerifiedTxn {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bytes.hash(state);
        self.sender.hash(state);
        self.sequence_number.hash(state);
        self.chain_id.hash(state);
    }
}

impl VerifiedTxn {
    pub fn new(
        bytes: Vec<u8>,
        sender: ExternalAccountAddress,
        sequence_number: u64,
        chain_id: ExternalChainId,
        committed_hash: TxnHash,
    ) -> Self {
        Self { bytes, sender, sequence_number, chain_id, committed_hash: committed_hash.into() }
    }

    pub fn bytes(&self) -> &Vec<u8> {
        &self.bytes
    }

    pub fn sender(&self) -> &ExternalAccountAddress {
        &self.sender
    }

    pub fn seq_number(&self) -> u64 {
        self.sequence_number
    }

    pub fn committed_hash(&self) -> [u8; 32] {
        self.committed_hash
            .get_or_init(|| {
                u256_define::TxnHash::new(simple_hash::hash_to_fixed_array(self.bytes()))
            })
            .bytes()
    }
}
