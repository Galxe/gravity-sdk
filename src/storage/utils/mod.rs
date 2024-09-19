use aptos_config::config::RocksdbConfig;
use aptos_schemadb::DB;
use aptos_storage_interface::Result;
use aptos_types::transaction::Version;
use rocksdb::Options;

use super::schema::db_metadata::{DbMetadataKey, DbMetadataSchema};

pub mod iterators;

pub(crate) fn get_progress(db: &DB, progress_key: &DbMetadataKey) -> Result<Option<Version>> {
    Ok(db
        .get::<DbMetadataSchema>(progress_key)?
        .map(|v| v.expect_version()))
}

pub fn gen_rocksdb_options(config: &RocksdbConfig, readonly: bool) -> Options {
    let mut db_opts = Options::default();
    db_opts.set_max_open_files(config.max_open_files);
    db_opts.set_max_total_wal_size(config.max_total_wal_size);
    db_opts.set_max_background_jobs(config.max_background_jobs);
    if !readonly {
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
    }

    db_opts
}
