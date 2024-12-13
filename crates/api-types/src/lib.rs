pub mod account;
use std::{fmt::Display, sync::Arc};
use core::str;
use crate::account::{ExternalAccountAddress, ExternalChainId};
use async_trait::async_trait;
use ruint::aliases::U256;
use serde::{Deserialize, Serialize};
use std::future::Future;
use tokio::{runtime::Runtime, sync::Mutex};

#[derive(Clone, Copy)]
pub struct BlockHashState {
    pub safe_hash: [u8; 32],
    pub head_hash: [u8; 32],
    pub finalized_hash: [u8; 32],
}

#[async_trait]
pub trait ConsensusApi: Send + Sync {
    // TODO(gravity_byteyue: change to return () when qs is ready)
    // async fn request_payload<'a, 'b>(
    //     &'a self,
    //     closure: BoxFuture<'b, Result<(), SendError>>,
    //     state_block_hash: BlockHashState,
    // ) -> Result<BlockBatch, SendError>;

    async fn send_ordered_block(&self, parent_meta: ExternalBlockMeta, ordered_block: ExternalBlock);

    async fn recv_executed_block_hash(&self, head: ExternalBlockMeta) -> ComputeRes;

    async fn commit_block_hash(&self, head: ExternalBlockMeta);
}

pub struct BlockBatch {
    pub txns: Vec<GTxn>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ExecutionBlocks {
    pub latest_block_hash: [u8; 32],
    pub latest_block_number: u64,
    pub blocks: Vec<Vec<u8>>,
}

#[async_trait]
pub trait ExecutionApi: Send + Sync {
    // Request transactions from execution engine
    // safe block id is the last block id that has been committed in block tree
    // head block id is the last block id that received by the execution engine in block tree
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch;

    async fn send_ordered_block(&self, txns: Vec<GTxn>);

    // the block hash is the hash of the block that has been executed, which is passed by the send_ordered_block
    async fn recv_executed_block_hash(&self) -> [u8; 32];

    // this function is called by the execution layer commit the block hash
    async fn commit_block_hash(&self, block_ids: Vec<[u8; 32]>);

    fn latest_block_number(&self) -> u64;

    fn finalized_block_number(&self) -> u64;

    async fn recover_ordered_block(&self, block_batch: BlockBatch);

    async fn recover_execution_blocks(&self, blocks: ExecutionBlocks);

    fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> ExecutionBlocks;
}

#[derive(Hash, PartialEq, Eq)]
pub struct ExternalPayloadAttr {
    // s since epoch
    ts: u64,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ExternalBlockMeta {
    // Unique identifier for block: hash of block body
    pub block_id: [u8; 32],
    pub block_number: u64,
}

#[derive(Debug)]
pub struct ExternalBlock {
    pub block_meta: ExternalBlockMeta,
    pub txns: Vec<VerifiedTxn>,
}

pub struct ComputeRes([u8; 32]);

impl ComputeRes {
    pub fn new(res: [u8; 32]) -> Self {
        Self(res)
    }

    pub fn bytes(&self) -> [u8; 32] {
        self.0.clone()
    }
}

#[derive(Debug)]
pub enum ExecError {
    InternalError,
}

pub enum ExecTxn {
    RawTxn(Vec<u8>),          // from client
    VerifiedTxn(VerifiedTxn), // from peer
}

pub struct VerifiedTxnWithAccountSeqNum {
    pub txn: VerifiedTxn,
    pub account_seq_num: u64,
}

#[async_trait]
pub trait ExecutionApiV2: Send + Sync {
    async fn add_txn(&self, bytes: ExecTxn) -> Result<(), ExecError>;

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
        parent_meta: ExternalBlockMeta,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError>;

    // the block hash is the hash of the block that has been executed, which is passed by the send_ordered_block
    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError>;

    // this function is called by the execution layer commit the block hash
    async fn commit_block(&self, head: ExternalBlockMeta) -> Result<(), ExecError>;
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

    async fn recover_ordered_block(&self, block: ExternalBlock);

    async fn recover_execution_blocks(&self, blocks: ExecutionBlocks);

    async fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> Result<ExecutionBlocks, RecoveryError>;
}

pub struct DefaultRecovery {}

#[async_trait]
impl RecoveryApi for DefaultRecovery {
    async fn latest_block_number(&self) -> u64 {
        0
    }

    async fn finalized_block_number(&self) -> u64 {
        0
    }

    async fn recover_ordered_block(&self, _: ExternalBlock) {
        ()
    }

    async fn recover_execution_blocks(&self, _: ExecutionBlocks) {
        ()
    }

    async fn get_blocks_by_range(
        &self,
        _: u64,
        _: u64,
    ) -> Result<ExecutionBlocks, RecoveryError> {
        Err(RecoveryError::UnimplementError)
    }
}
<<<<<<< HEAD
#[derive(Clone)]
pub struct ExecutionLayer {
    pub execution_api: Arc<dyn ExecutionApiV2>,
    pub recovery_api: Arc<dyn RecoveryApi>,
}
=======
>>>>>>> aef67a9 (fix ts)

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerifiedTxn {
    pub bytes: Vec<u8>,
    pub sender: ExternalAccountAddress,
    pub sequence_number: u64,
    pub chain_id: ExternalChainId,
}

impl VerifiedTxn {
    pub fn new(
        bytes: Vec<u8>,
        sender: ExternalAccountAddress,
        sequence_number: u64,
        chain_id: ExternalChainId,
    ) -> Self {
        Self { bytes, sender, sequence_number, chain_id }
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
}

#[derive(Clone, Default)]
pub struct GTxn {
    pub sequence_number: u64,
    /// Maximal total gas to spend for this transaction.
    pub max_gas_amount: u64,
    /// Price to be paid per gas unit.
    pub gas_unit_price: U256,
    /// Expiration timestamp for this transaction, represented
    /// as seconds from the Unix Epoch. If the current blockchain timestamp
    /// is greater than or equal to this time, then the transaction has
    /// expired and will be discarded. This can be set to a large value far
    /// in the future to indicate that a transaction does not expire.
    pub expiration_timestamp_secs: u64,
    /// Chain ID of the Aptos network this transaction is intended for.
    pub chain_id: u64,
    /// The transaction payload, e.g., a script to execute.
    pub txn_bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum GCEIError {
    ConsensusError,
}

impl Display for GCEIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Consensus Error")
    }
}

impl GTxn {
    pub fn new(
        sequence_number: u64,
        max_gas_amount: u64,
        gas_unit_price: U256,
        expiration_timestamp_secs: u64,
        chain_id: u64,
        txn_bytes: Vec<u8>,
    ) -> Self {
        Self {
            sequence_number,
            max_gas_amount,
            gas_unit_price,
            expiration_timestamp_secs,
            chain_id,
            txn_bytes,
        }
    }

    pub fn get_bytes(&self) -> &Vec<u8> {
        &self.txn_bytes
    }
}
