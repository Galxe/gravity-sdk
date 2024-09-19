use std::path::Path;

use aptos_config::config::{RocksdbConfigs, StorageDirPaths};
use aptos_storage_interface::Result;

impl GravityDB {
    fn new_with_dbs(ledger_db: LedgerDb, mock_db: MockStorage) -> Self {
        let ledger_db = Arc::new(ledger_db);
        let mock_db = Arc::new(mock_db);

        GravityDB {
            ledger_db: Arc::clone(&ledger_db),
            mock_db: Arc::clone(&mock_db),
            pre_commit_lock: std::sync::Mutex::new(()),
            commit_lock: std::sync::Mutex::new(()),
        }
    }

    pub fn open_dbs(
        db_paths: &StorageDirPaths,
        rocksdb_configs: RocksdbConfigs,
    ) -> Result<LedgerDb> {
        let ledger_db = LedgerDb::new(db_paths.ledger_db_root_path(), rocksdb_configs, false)?;
        Ok(ledger_db)
    }
}
