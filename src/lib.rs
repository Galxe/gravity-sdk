
/// CPEI: Consensus Process Engine Interface
///
/// This trait defines the interface for a consensus process engine.
/// It outlines the key operations that any consensus engine should implement
/// to participate in the blockchain consensus process.
pub trait ConsensusProcessEngineInterface {
    /// Initialize the consensus engine.
    ///
    /// This function should be called when the consensus engine starts up.
    /// It may include tasks such as:
    /// - Setting up initial state
    /// - Connecting to the network
    /// - Loading configuration
    fn init();

    /// Receive and process valid transactions.
    ///
    /// This function is responsible for:
    /// - Accepting incoming transactions from the network or mempool
    /// - Validating the transactions
    /// - Adding valid transactions to the local transaction pool
    fn submit_valid_transactions();

    /// Poll for ordered blocks.
    ///
    /// This function should:
    /// - Check for new blocks that have been ordered by the consensus mechanism
    /// - Retrieve the ordered blocks
    /// - Prepare them for processing
    ///
    /// TODO(gravity_xiejian): use txn id rather than total txn in block
    /// Returns: Option<Block> - The next ordered block, if available
    fn polling_ordered_block();

    /// Submit computation results.
    ///
    /// After processing a block, this function should:
    /// - Package the results of any computations or state changes
    /// - Submit these results back to the consensus mechanism
    ///
    /// Parameters:
    /// - `result`: The computation result to be submitted
    fn submit_compute_res();

    /// Submit Block head.
    ///
    /// After processing a block, this function should:
    /// - Package the block head
    /// - Submit these results back to the consensus mechanism
    ///
    /// Parameters:
    /// - `result`: The computation result to be submitted
    fn submit_block_head();

    /// Commit finalized block IDs.
    ///
    /// This function is called when a block is finalized. It should:
    /// - Mark the specified blocks as finalized in the local state
    /// - Trigger any necessary callbacks or events related to block finalization
    ///
    /// Parameters:
    /// - `block_ids`: A vector of block IDs that have been finalized
    fn polling_commit_block_ids();
}