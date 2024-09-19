use aptos_schemadb::SchemaBatch;
use aptos_storage_interface::{db_ensure as ensure, DbWriter};
use aptos_types::{
    ledger_info::LedgerInfoWithSignatures,
    transaction::{TransactionToCommit, Version},
};

use super::schema::db_metadata::{DbMetadataKey, DbMetadataSchema, DbMetadataValue};

impl DbWriter for GravityDB {
    fn pre_commit_ledger(
        &self,
        txns_to_commit: &[TransactionToCommit],
        first_version: Version,
    ) -> Result<()> {
        // Pre-committing and committing in concurrency is allowed but not pre-committing at the
        // same time from multiple threads, the same for committing.
        // Consensus and state sync must hand over to each other after all pending execution and
        // committing complete.
        let _lock = self
            .pre_commit_lock
            .try_lock()
            .expect("Concurrent committing detected.");

        let last_version = first_version + txns_to_commit.len() as u64 - 1;

        self.commit_ledger_metadata(txns_to_commit, first_version)?;

        self.ledger_db
            .metadata_db()
            .set_pre_committed_version(last_version);
        Ok(())
    }

    /// Commit pre-committed transactions to the ledger.
    ///
    /// If a LedgerInfoWithSigs is provided, both the "synced version" and "committed version" will
    /// advance, otherwise only the synced version will advance.
    fn commit_ledger(
        &self,
        version: Version,
        ledger_info_with_sigs: Option<&LedgerInfoWithSignatures>,
        txns_to_commit: Option<&[TransactionToCommit]>,
    ) -> Result<()> {
        let _lock = self
            .commit_lock
            .try_lock()
            .expect("Concurrent committing detected.");
        // let old_committed_ver = self.get_and_check_commit_range(version)?;

        let ledger_batch = SchemaBatch::new();
        // Write down LedgerInfo if provided.
        if let Some(li) = ledger_info_with_sigs {
            self.put_ledger_info(version, li, &ledger_batch)?;
        }
        // TODO(Gravity_lightman)
        // Write down commit progress
        // ledger_batch.put::<DbMetadataSchema>(
        //     &DbMetadataKey::OverallCommitProgress,
        //     &DbMetadataValue::Version(version),
        // )?;
        self.ledger_db.metadata_db().write_schemas(ledger_batch)?;

        // Notify the pruners, invoke the indexer, and update in-memory ledger info.
        self.post_commit(ledger_info_with_sigs)
    }
}

impl GravityDB {
    fn commit_ledger_metadata(
        &self,
        txns_to_commit: &[TransactionToCommit],
        first_version: Version,
    ) -> Result<()> {
        if txns_to_commit.is_empty() {
            return Ok(());
        }

        let ledger_metadata_batch = SchemaBatch::new();
        let last_version = first_version + txns_to_commit.len() as u64 - 1;
        ledger_metadata_batch
            .put::<DbMetadataSchema>(
                &DbMetadataKey::LedgerCommitProgress,
                &DbMetadataValue::Version(last_version),
            )
            .unwrap();
        self.ledger_db
            .metadata_db()
            .write_schemas(ledger_metadata_batch)
            .unwrap();
        Ok(())
    }

    fn get_and_check_commit_range(&self, version_to_commit: Version) -> Result<Option<Version>> {
        let old_committed_ver = self.ledger_db.metadata_db().get_synced_version()?;
        let pre_committed_ver = self.ledger_db.metadata_db().get_pre_committed_version();
        ensure!(
            old_committed_ver.is_none() || version_to_commit >= old_committed_ver.unwrap(),
            "Version too old to commit. Committed: {:?}; Trying to commit with LI: {}",
            old_committed_ver,
            version_to_commit,
        );
        ensure!(
            pre_committed_ver.is_some() && version_to_commit <= pre_committed_ver.unwrap(),
            "Version too new to commit. Pre-committed: {:?}, Trying to commit with LI: {}",
            pre_committed_ver,
            version_to_commit,
        );
        Ok(old_committed_ver)
    }

    fn put_ledger_info(
        &self,
        version: Version,
        ledger_info_with_sig: &LedgerInfoWithSignatures,
        ledger_batch: &SchemaBatch,
    ) -> Result<()> {
        let ledger_info = ledger_info_with_sig.ledger_info();

        // Verify epoch continuity.
        // let current_epoch = self
        //     .ledger_db
        //     .metadata_db()
        //     .get_latest_ledger_info_option()
        //     .map_or(0, |li| li.ledger_info().next_block_epoch());
        // ensure!(
        //     ledger_info_with_sig.ledger_info().epoch() == current_epoch,
        //     "Gap in epoch history. Trying to put in LedgerInfo in epoch: {}, current epoch: {}",
        //     ledger_info_with_sig.ledger_info().epoch(),
        //     current_epoch,
        // );

        // Put write to batch.
        self.ledger_db
            .metadata_db()
            .put_ledger_info(ledger_info_with_sig, ledger_batch)?;
        Ok(())
    }

    fn post_commit(&self, ledger_info_with_sigs: Option<&LedgerInfoWithSignatures>) -> Result<()> {
        // Once everything is successfully persisted, update the latest in-memory ledger info.
        if let Some(x) = ledger_info_with_sigs {
            self.ledger_db
                .metadata_db()
                .set_latest_ledger_info(x.clone());
        }

        Ok(())
    }
}
