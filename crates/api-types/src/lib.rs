pub mod account;
use std::{fmt::Display, sync::Arc};
use core::str;
use crate::account::{ExternalAccountAddress, ExternalChainId};
use aptos_crypto::HashValue;
use async_trait::async_trait;
use ruint::aliases::U256;
use serde::{Deserialize, Serialize};

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


#[derive(Hash, PartialEq, Eq, Clone)]
pub struct ExternalPayloadAttr {
    // s since epoch
    pub ts: u64,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct ExternalBlockMeta {
    // Unique identifier for block: hash of block body
    pub block_id: BlockId,
    pub block_number: u64,
    pub ts: u64,
}

#[derive(Debug, Clone)]
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


#[derive(Debug, Clone)]
pub struct VerifiedTxnWithAccountSeqNum {
    pub txn: VerifiedTxn,
    pub account_seq_num: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash, PartialEq, Eq, Copy)]
pub struct BlockId(pub [u8; 32]);

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

    async fn recover_ordered_block(&self, block: ExternalBlock);

    async fn recover_execution_blocks(&self, blocks: ExecutionBlocks);

    async fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> Result<ExecutionBlocks, RecoveryError>;
}

#[derive(Default)]
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
#[derive(Clone)]
pub struct ExecutionLayer {
    pub execution_api: Arc<dyn ExecutionApiV2>,
    pub recovery_api: Arc<dyn RecoveryApi>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash)]
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

#[derive(Debug)]
pub enum GCEIError {
    ConsensusError,
}

impl Display for GCEIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Consensus Error")
    }
}


pub struct MockExecutionApi {}

#[async_trait]
impl ExecutionApiV2 for MockExecutionApi {
    async fn add_txn(&self, bytes: ExecTxn) -> Result<(), ExecError> {
        Ok(())
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        Ok(vec![])
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
        Ok(true)
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        Ok(vec![])
    }

    async fn send_ordered_block(
        &self,
        parent_id: BlockId,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        Ok(())
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        Ok(ComputeRes::new(*HashValue::random()))
    }

    async fn commit_block(&self, head: BlockId) -> Result<(), ExecError> {
        Ok(())
    }
}

pub fn mock_execution_layer() -> ExecutionLayer {
    ExecutionLayer {
        execution_api: Arc::new(MockExecutionApi {}),
        recovery_api: Arc::new(DefaultRecovery {}),
    }
}