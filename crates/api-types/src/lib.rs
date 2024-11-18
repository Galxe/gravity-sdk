use std::{fmt::Display, sync::Arc};

use async_trait::async_trait;
use futures::channel::mpsc::SendError;
use futures::future::BoxFuture;
use reth_primitives::Block;
use ruint::aliases::U256;
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
    async fn request_payload<'a, 'b>(
        &'a self,
        closure: BoxFuture<'b, Result<(), SendError>>,
        state_block_hash: BlockHashState,
    ) -> Result<BlockBatch, SendError>;

    async fn send_order_block(&self, txns: Vec<GTxn>);

    async fn recv_executed_block_hash(&self) -> [u8; 32];

    async fn commit_block_hash(&self, block_ids: Vec<[u8; 32]>);
}

pub struct BlockBatch {
    pub txns: Vec<GTxn>,
    pub block_hash: [u8; 32],
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

    async fn recover_execution_blocks(&self, block: Vec<Block>);

    fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> Vec<Block>;
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

pub struct BatchClient {
    pending_payloads: Mutex<Vec<Vec<GTxn>>>,
}

impl BatchClient {
    pub fn new() -> Self {
        Self { pending_payloads: Mutex::new(vec![]) }
    }

    pub fn submit(&self, txns: Vec<GTxn>) {
        self.pending_payloads.blocking_lock().push(txns);
    }

    pub fn pull(&self) -> Vec<Vec<GTxn>> {
        let mut payloads = self.pending_payloads.blocking_lock();
        std::mem::take(&mut *payloads)
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
