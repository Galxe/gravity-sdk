use anyhow::Result;
use aptos_consensus::consensusdb::ConsensusDB;
use clap::Parser;
use std::path::PathBuf;

/// Unwind consensus state to a specific block number.
/// This deletes all consensus data (blocks, QCs, ledger info, randomness, etc.)
/// for blocks with block_number > target.
#[derive(Debug, Parser)]
pub struct UnwindCommand {
    /// Path to the consensus DB data directory.
    /// This is typically `<deploy-path>/data/consensus_db`.
    #[arg(long)]
    consensus_db_path: PathBuf,

    /// Target block number to unwind to.
    /// All data for blocks with block_number > target will be deleted.
    /// The target block itself will be kept.
    #[arg(long)]
    target: u64,
}

impl super::command::Executable for UnwindCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        println!(
            "Unwinding consensus DB to block {}...",
            self.target
        );
        println!("Consensus DB path: {:?}", self.consensus_db_path);

        if !self.consensus_db_path.exists() {
            return Err(anyhow::anyhow!(
                "Consensus DB path does not exist: {:?}",
                self.consensus_db_path
            ));
        }

        // Open ConsensusDB. The second argument is the node config path,
        // which is not needed for unwind operations.
        let consensus_db = ConsensusDB::new(&self.consensus_db_path, &PathBuf::new());

        // Perform the unwind
        consensus_db
            .unwind_to_block(self.target)
            .map_err(|e| anyhow::anyhow!("Failed to unwind consensus DB: {:?}", e))?;

        println!(
            "Successfully unwound consensus DB to block {}.",
            self.target
        );

        Ok(())
    }
}
