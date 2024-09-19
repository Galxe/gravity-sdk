use std::sync::Arc;

use crate::mock_db::MockStorage;

use super::ledger_db::LedgerDb;

/// This holds a handle to the underlying DB responsible for physical storage and provides APIs for
/// access to the core Aptos data structures.
pub struct GravityDB {
    pub(crate) ledger_db: Arc<LedgerDb>,
    pub(crate) mock_db: Arc<MockStorage>,
    pre_commit_lock: std::sync::Mutex<()>,
    commit_lock: std::sync::Mutex<()>,
}

impl GravityDB {
    pub fn open(
        db_paths: &StorageDirPaths,
        rocksdb_configs: RocksdbConfigs,
        network_address: String,
        path: &Path,
    ) -> Result<Self> {
        let ledger_db = Self::open_dbs(db_paths, rocksdb_configs)?;
        let mock_db = MockStorage::new(network_address, path);
        let myself = Self::new_with_dbs(ledger_db, mock_db);

        Ok(myself)
    }
}

include!("include/gravitydb_internal.rs");
include!("include/gravitydb_writer.rs");
include!("include/gravitydb_reader.rs");
