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
        println!("Unwinding consensus DB to block {}...", self.target);
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
        let quorum_store_db = aptos_consensus::quorum_store::quorum_store_db::QuorumStoreDB::new(
            &self.consensus_db_path,
        );

        // Perform the unwind
        let (batches_to_delete, cancelled_epochs) = consensus_db
            .unwind_to_block(self.target)
            .map_err(|e| anyhow::anyhow!("Failed to unwind consensus DB: {:?}", e))?;

        println!("Successfully unwound consensus DB to block {}.", self.target);

        let max_retained_epoch = consensus_db.get_max_epoch();
        let mut all_cancelled_epochs = cancelled_epochs;

        use aptos_consensus::quorum_store::quorum_store_db::QuorumStoreStorage;
        if let Ok(qs_epochs) = quorum_store_db.get_all_batch_id_epochs() {
            for ep in qs_epochs {
                if ep > max_retained_epoch && !all_cancelled_epochs.contains(&ep) {
                    all_cancelled_epochs.push(ep);
                }
            }
        }

        if !batches_to_delete.is_empty() || !all_cancelled_epochs.is_empty() {
            println!(
                "Deleting {} unused QuorumStore batches and {} cancelled epochs...",
                batches_to_delete.len(),
                all_cancelled_epochs.len()
            );

            if !batches_to_delete.is_empty() {
                quorum_store_db.delete_batches(batches_to_delete).map_err(|e| {
                    anyhow::anyhow!("Failed to clean up QuorumStore batches: {:?}", e)
                })?;
            }

            for epoch in all_cancelled_epochs {
                quorum_store_db.delete_batch_id(epoch).map_err(|e| {
                    anyhow::anyhow!("Failed to clean up QuorumStore epoch {}: {:?}", epoch, e)
                })?;
            }

            println!("Successfully cleaned up QuorumStore DB.");
        }

        let data_dir = if self.consensus_db_path.ends_with("consensus_db") {
            self.consensus_db_path.parent().unwrap_or(&self.consensus_db_path).to_path_buf()
        } else {
            self.consensus_db_path.clone()
        };

        let secure_json_path = data_dir.join("secure.json");
        if secure_json_path.exists() {
            println!("Deleting secure.json at {:?}", secure_json_path);
            let _ = std::fs::remove_file(secure_json_path);
        }

        let rand_db_path = data_dir.join("rand_db");
        if rand_db_path.exists() {
            println!("Deleting rand_db at {:?}", rand_db_path);
            let _ = std::fs::remove_dir_all(rand_db_path);
        }

        Ok(())
    }
}
