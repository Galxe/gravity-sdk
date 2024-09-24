mod bootstrap;
pub mod consensus_engine;
mod mock_db;
mod network;
mod storage;

use aptos_config::config::NodeConfig;
use bootstrap::{check_bootstrap_config, start};
use clap::Parser;
use std::path::PathBuf;
use std::fmt::Display;

pub struct GTxn {
    sequence_number: u64,
    /// Maximal total gas to spend for this transaction.
    max_gas_amount: u64,
    /// Price to be paid per gas unit.
    gas_unit_price: u64,
    /// Expiration timestamp for this transaction, represented
    /// as seconds from the Unix Epoch. If the current blockchain timestamp
    /// is greater than or equal to this time, then the transaction has
    /// expired and will be discarded. This can be set to a large value far
    /// in the future to indicate that a transaction does not expire.
    expiration_timestamp_secs: u64,
    /// Chain ID of the Aptos network this transaction is intended for.
    chain_id: u64,
    /// The transaction payload, e.g., a script to execute.
    txn_bytes: Vec<u8>,
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
        gas_unit_price: u64,
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

/// GCEI: Gravity Consensus Engine Interface
///
/// This trait defines the interface for a consensus process engine.
/// It outlines the key operations that any consensus engine should implement
/// to participate in the blockchain consensus process.
#[async_trait::async_trait]
pub trait GravityConsensusEngineInterface: Send + Sync {
    /// Initialize the consensus engine.
    ///
    /// This function should be called when the consensus engine starts up.
    /// It may include tasks such as:
    /// - Setting up initial state
    /// - Connecting to the network
    /// - Loading configuration
    fn init() -> Self;

    /// Receive and process valid transactions.
    ///
    /// This function is responsible for:
    /// - Accepting incoming transactions from the network or mempool
    /// - Validating the transactions
    /// - Adding valid transactions to the local transaction pool
    async fn send_valid_block_transactions(
        &self,
        block_id: [u8; 32],
        txns: Vec<GTxn>,
    ) -> Result<(), GCEIError>;

    /// Poll for ordered blocks.
    ///
    /// This function should:
    /// - Check for new blocks that have been ordered by the consensus mechanism
    /// - Retrieve the ordered blocks
    /// - Prepare them for processing
    ///
    /// TODO(gravity_xiejian): use txn id rather than total txn in block
    /// Returns: Option<Block> - The next ordered block, if available
    async fn receive_ordered_block(&mut self) -> Result<([u8; 32], Vec<GTxn>), GCEIError>;

    /// Submit computation results.
    ///
    /// After processing a block, this function should:
    /// - Package the results of any computations or state changes
    /// - Submit these results back to the consensus mechanism
    ///
    /// Parameters:
    /// - `result`: The computation result to be submitted
    async fn send_compute_res(&self, block_id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError>;

    /// Submit Block head.
    ///
    /// After processing a block, this function should:
    /// - Package the block head
    /// - Submit these results back to the consensus mechanism
    ///
    /// Parameters:
    /// - `result`: The computation result to be submitted
    async fn send_block_head(&self, block_id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError>;

    /// Commit batch finalized block IDs.
    ///
    /// This function is called when a block is finalized. It should:
    /// - Mark the specified blocks as finalized in the local state
    /// - Trigger any necessary callbacks or events related to block finalization
    ///
    /// Parameters:
    /// - `block_ids`: A vector of block IDs that have been finalized
    async fn receive_commit_block_ids(&mut self) -> Result<Vec<[u8; 32]>, GCEIError>;

    /// Return the commit ids, the consensus can delete these transactions after submitting.
    async fn send_persistent_block_id(&self, block_id: [u8; 32]) -> Result<(), GCEIError>;
}

/// Runs an Gravity validator or fullnode
#[derive(Clone, Debug, Parser)]
#[clap(name = "Gravity Node", author, version)]
pub struct GravityNodeArgs {
    #[clap(short = 'f', long)]
    /// Path to node configuration file (or template for local test mode).
    node_config_path: Option<PathBuf>,
    #[clap(long)]
    mockdb_config_path: Option<PathBuf>,
}

impl GravityNodeArgs {
    pub fn run(mut self) {
        // Start the node
        start(
            check_bootstrap_config(self.node_config_path),
            self.mockdb_config_path,
        )
        .expect("Node should start correctly");
    }
}
