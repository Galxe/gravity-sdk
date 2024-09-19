use aptos_config::config::RocksdbConfig;
use aptos_schemadb::ColumnFamilyName;
use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, Options,
    DEFAULT_COLUMN_FAMILY_NAME,
};

use crate::storage::schema::{
    BLOCK_BY_VERSION_CF_NAME, BLOCK_INFO_CF_NAME, DB_METADATA_CF_NAME, EPOCH_BY_VERSION_CF_NAME,
    LEDGER_INFO_CF_NAME, VERSION_DATA_CF_NAME,
};

pub(super) fn ledger_db_column_families() -> Vec<ColumnFamilyName> {
    vec![
        /* empty cf */ DEFAULT_COLUMN_FAMILY_NAME,
        BLOCK_BY_VERSION_CF_NAME,
        BLOCK_INFO_CF_NAME,
        DB_METADATA_CF_NAME,
        EPOCH_BY_VERSION_CF_NAME,
        LEDGER_INFO_CF_NAME,
        VERSION_DATA_CF_NAME,
    ]
}

pub(super) fn gen_ledger_cfds(rocksdb_config: &RocksdbConfig) -> Vec<ColumnFamilyDescriptor> {
    let cfs = ledger_db_column_families();
    gen_cfds(rocksdb_config, cfs, |_, _| {})
}

fn gen_cfds<F>(
    rocksdb_config: &RocksdbConfig,
    cfs: Vec<ColumnFamilyName>,
    cf_opts_post_processor: F,
) -> Vec<ColumnFamilyDescriptor>
where
    F: Fn(ColumnFamilyName, &mut Options),
{
    let mut table_options = BlockBasedOptions::default();
    table_options.set_cache_index_and_filter_blocks(rocksdb_config.cache_index_and_filter_blocks);
    table_options.set_block_size(rocksdb_config.block_size as usize);
    let cache = Cache::new_lru_cache(rocksdb_config.block_cache_size as usize);
    table_options.set_block_cache(&cache);
    let mut cfds = Vec::with_capacity(cfs.len());
    for cf_name in cfs {
        let mut cf_opts = Options::default();
        cf_opts.set_compression_type(DBCompressionType::Lz4);
        cf_opts.set_block_based_table_factory(&table_options);
        cf_opts_post_processor(cf_name, &mut cf_opts);
        cfds.push(ColumnFamilyDescriptor::new((*cf_name).to_string(), cf_opts));
    }
    cfds
}
