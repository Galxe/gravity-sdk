use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use aptos_config::config::{RocksdbConfig, RocksdbConfigs};
use aptos_logger::info;
use aptos_schemadb::{ColumnFamilyDescriptor, ColumnFamilyName, SchemaBatch, DB};
use aptos_storage_interface::Result;
use aptos_types::transaction::Version;
use ledger_metadata_db::LedgerMetadataDb;

use crate::storage::utils::gen_rocksdb_options;

use super::{
    db_options::{gen_ledger_cfds, ledger_db_column_families},
    schema::db_metadata::{DbMetadataKey, DbMetadataSchema},
};

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
    pub(crate) fn new<P: AsRef<Path>>(
        db_root_path: P,
        rocksdb_configs: RocksdbConfigs,
        readonly: bool,
    ) -> Result<Self> {
        let ledger_metadata_db_path = Self::metadata_db_path(db_root_path.as_ref());
        let ledger_metadata_db = Arc::new(Self::open_rocksdb(
            ledger_metadata_db_path.clone(),
            LEDGER_DB_NAME,
            &rocksdb_configs.ledger_db_config,
            readonly,
        )?);

        info!(
            ledger_metadata_db_path = ledger_metadata_db_path,
            "Opened ledger metadata db!"
        );

        return Ok(Self {
            ledger_metadata_db: LedgerMetadataDb::new(Arc::clone(&ledger_metadata_db)),
        });
    }

    pub(crate) fn get_in_progress_state_kv_snapshot_version(&self) -> Result<Option<Version>> {
        let mut iter = self.ledger_metadata_db.db().iter::<DbMetadataSchema>()?;
        iter.seek_to_first();
        while let Some((k, _v)) = iter.next().transpose()? {
            if let DbMetadataKey::StateSnapshotKvRestoreProgress(version) = k {
                return Ok(Some(version));
            }
        }
        Ok(None)
    }

    pub(crate) fn create_checkpoint(
        db_root_path: impl AsRef<Path>,
        cp_root_path: impl AsRef<Path>,
        sharding: bool,
    ) -> Result<()> {
        let rocksdb_configs = RocksdbConfigs {
            enable_storage_sharding: sharding,
            ..Default::default()
        };
        let ledger_db = Self::new(db_root_path, rocksdb_configs, /*readonly=*/ false)?;

        ledger_db
            .metadata_db()
            .create_checkpoint(Self::metadata_db_path(cp_root_path.as_ref()))?;

        Ok(())
    }

    // Only expect to be used by fast sync when it is finished.
    pub(crate) fn write_pruner_progress(&self, version: Version) -> Result<()> {
        info!("Fast sync is done, writing pruner progress {version} for all ledger sub pruners.");
        self.ledger_metadata_db.write_pruner_progress(version)?;

        Ok(())
    }

    pub(crate) fn metadata_db(&self) -> &LedgerMetadataDb {
        &self.ledger_metadata_db
    }

    // TODO(grao): Remove this after sharding migration.
    pub(crate) fn metadata_db_arc(&self) -> Arc<DB> {
        self.ledger_metadata_db.db_arc()
    }

    fn open_rocksdb(
        path: PathBuf,
        name: &str,
        db_config: &RocksdbConfig,
        readonly: bool,
    ) -> Result<DB> {
        let db = if readonly {
            DB::open_cf_readonly(
                &gen_rocksdb_options(db_config, true),
                path.clone(),
                name,
                Self::get_column_families_by_name(name),
            )?
        } else {
            DB::open_cf(
                &gen_rocksdb_options(db_config, false),
                path.clone(),
                name,
                Self::gen_cfds_by_name(db_config, name),
            )?
        };

        info!("Opened {name} at {path:?}!");

        Ok(db)
    }

    fn get_column_families_by_name(name: &str) -> Vec<ColumnFamilyName> {
        match name {
            LEDGER_DB_NAME => ledger_db_column_families(),
            _ => unreachable!(),
        }
    }

    fn gen_cfds_by_name(db_config: &RocksdbConfig, name: &str) -> Vec<ColumnFamilyDescriptor> {
        match name {
            LEDGER_DB_NAME => gen_ledger_cfds(db_config),
            _ => unreachable!(),
        }
    }

    fn metadata_db_path<P: AsRef<Path>>(db_root_path: P) -> PathBuf {
        db_root_path.as_ref().join(LEDGER_DB_NAME)
    }

    pub fn write_schemas(&self, schemas: LedgerDbSchemaBatches) -> Result<()> {
        self.ledger_metadata_db
            .write_schemas(schemas.ledger_metadata_db_batches)
    }
}
