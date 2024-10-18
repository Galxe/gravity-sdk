use anyhow::{ensure, Result};
use aptos_schemadb::ColumnFamilyName;
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use serde::{Deserialize, Serialize};

pub(crate) mod block_by_version;
pub(crate) mod block_info;
pub(crate) mod db_metadata;
pub(crate) mod epoch_by_version;
pub(crate) mod ledger_info;
pub(crate) mod version_data;

pub const LEDGER_INFO_CF_NAME: ColumnFamilyName = "ledger_info";
pub const DB_METADATA_CF_NAME: ColumnFamilyName = "db_metadata";
pub const EPOCH_BY_VERSION_CF_NAME: ColumnFamilyName = "epoch_by_version";
pub const BLOCK_INFO_CF_NAME: ColumnFamilyName = "block_info";
pub const BLOCK_BY_VERSION_CF_NAME: ColumnFamilyName = "block_by_version";
pub const VERSION_DATA_CF_NAME: ColumnFamilyName = "version_data";

fn ensure_slice_len_eq(data: &[u8], len: usize) -> Result<()> {
    ensure!(
        data.len() == len,
        "Unexpected data len {}, expected {}.",
        data.len(),
        len,
    );
    Ok(())
}

/// A simple struct for recording the progress of a state snapshot sync
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize, Clone)]
pub struct StateSnapshotProgress {
    pub target_ledger_info: LedgerInfoWithSignatures,
    pub last_persisted_state_value_index: u64,
    pub snapshot_sync_completed: bool,
}
