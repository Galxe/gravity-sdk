use gaptos::aptos_types::{transaction::TransactionToCommit, state_store::ShardedStateUpdates};
use gaptos::aptos_storage_interface::{state_delta::StateDelta, cached_state_view::ShardedStateCache};
impl DbWriter for ConsensusDB {
    fn save_transactions(
        &self,
        txns_to_commit: &[TransactionToCommit],
        first_version: Version,
        base_state_version: Option<Version>,
        ledger_info_with_sigs: Option<&LedgerInfoWithSignatures>,
        sync_commit: bool,
        latest_in_memory_state: StateDelta,
        state_updates_until_last_checkpoint: Option<ShardedStateUpdates>,
        sharded_state_cache: Option<&ShardedStateCache>,
    ) -> std::result::Result<(), AptosDbError> {
        // let old_committed_ver = self.get_and_check_commit_range(version)?;
        let ledger_batch = SchemaBatch::new();
        // Write down LedgerInfo if provided.
        if let Some(li) = ledger_info_with_sigs {
            self.put_ledger_info(first_version, li, &ledger_batch)?;
        }
        self.ledger_db.metadata_db().write_schemas(ledger_batch)?;
        // Notify the pruners, invoke the indexer, and update in-memory ledger info.
        self.post_commit(ledger_info_with_sigs)
    }
}
impl ConsensusDB {
    fn put_ledger_info(
        &self,
        version: Version,
        ledger_info_with_sig: &LedgerInfoWithSignatures,
        ledger_batch: &SchemaBatch,
    ) -> Result<()> {
        self.ledger_db
            .metadata_db()
            .put_ledger_info(ledger_info_with_sig, ledger_batch)?;
        Ok(())
    }
    fn post_commit(&self, ledger_info_with_sigs: Option<&LedgerInfoWithSignatures>) -> std::result::Result<(), AptosDbError> {
        // Once everything is successfully persisted, update the latest in-memory ledger info.
        if let Some(x) = ledger_info_with_sigs {
            self.ledger_db
                .metadata_db()
                .set_latest_ledger_info(x.clone());
        }
        Ok(())
    }
}