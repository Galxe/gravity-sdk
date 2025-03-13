use gaptos::aptos_types::transaction::TransactionToCommit;
impl DbWriter for ConsensusDB {
    /// Commit pre-committed transactions to the ledger.
    ///
    /// If a LedgerInfoWithSigs is provided, both the "synced version" and "committed version" will
    /// advance, otherwise only the synced version will advance.
    fn commit_ledger(
        &self,
        version: Version,
        ledger_info_with_sigs: Option<&LedgerInfoWithSignatures>,
        txns_to_commit: Option<&[TransactionToCommit]>,
    ) -> std::result::Result<(), AptosDbError> {
        // let old_committed_ver = self.get_and_check_commit_range(version)?;
        let ledger_batch = SchemaBatch::new();
        // Write down LedgerInfo if provided.
        if let Some(li) = ledger_info_with_sigs {
            self.put_ledger_info(version, li, &ledger_batch)?;
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