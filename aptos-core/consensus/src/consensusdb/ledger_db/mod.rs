use std::sync::Arc;
use aptos_schemadb::{SchemaBatch, DB};
use aptos_storage_interface::Result;
use ledger_metadata_db::LedgerMetadataDb;

mod ledger_metadata_db;
pub(crate) const MAX_NUM_EPOCH_ENDING_LEDGER_INFO: usize = 100;
pub const LEDGER_DB_NAME: &str = "ledger_db";
pub const LEDGER_METADATA_DB_NAME: &str = "ledger_metadata_db";
#[derive(Debug)]
pub struct LedgerDbSchemaBatches {
    pub ledger_metadata_db_batches: SchemaBatch,
}
impl Default for LedgerDbSchemaBatches {
    fn default() -> Self {
        Self {
            ledger_metadata_db_batches: SchemaBatch::new(),
        }
    }
}
impl LedgerDbSchemaBatches {
    pub fn new() -> Self {
        Self::default()
    }
}
#[derive(Debug)]
pub struct LedgerDb {
    ledger_metadata_db: LedgerMetadataDb,
}
impl LedgerDb {
    pub(crate) fn new(
        db: Arc<DB>,
    ) -> Self {
        Self {
            ledger_metadata_db: LedgerMetadataDb::new(db),
        }
    }
    pub(crate) fn metadata_db(&self) -> &LedgerMetadataDb {
        &self.ledger_metadata_db
    }
    // TODO(grao): Remove this after sharding migration.
    pub(crate) fn metadata_db_arc(&self) -> Arc<DB> {
        self.ledger_metadata_db.db_arc()
    }
    pub fn write_schemas(&self, schemas: LedgerDbSchemaBatches) -> Result<()> {
        self.ledger_metadata_db
            .write_schemas(schemas.ledger_metadata_db_batches)
    }
}