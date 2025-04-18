use gaptos::aptos_storage_interface::chunk_to_commit::ChunkToCommit;

impl DbWriter for ConsensusDB {
    fn save_transactions(
        &self,
        chunk: Option<ChunkToCommit>,
        ledger_info_with_sigs: Option<&LedgerInfoWithSignatures>,
        sync_commit: bool,
    ) -> std::result::Result<(), AptosDbError> {
        // let old_committed_ver = self.get_and_check_commit_range(version)?;
        let mut ledger_batch = SchemaBatch::new();
        // Write down LedgerInfo if provided.
        if let Some(li) = ledger_info_with_sigs {
            self.put_ledger_info(0, li, &mut ledger_batch)?;
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
        ledger_batch: &mut SchemaBatch,
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