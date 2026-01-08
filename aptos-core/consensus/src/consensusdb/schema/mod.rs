// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod block;
pub(crate) mod dag;
pub mod epoch_by_block_number;
pub mod ledger_info;
pub(crate) mod quorum_certificate;
pub(crate) mod randomness;
pub(crate) mod single_entry;

use anyhow::{ensure, Result};

pub const LEDGER_INFO_CF_NAME: ColumnFamilyName = "ledger_info";
pub const EPOCH_BY_BLOCK_NUMBER_CF_NAME: ColumnFamilyName = "epoch_by_block_number";
pub const RANDOMNESS_CF_NAME: ColumnFamilyName = "randomness";

pub(crate) fn ensure_slice_len_eq(data: &[u8], len: usize) -> Result<()> {
    ensure!(data.len() == len, "Unexpected data len {}, expected {}.", data.len(), len,);
    Ok(())
}

/// Copied from aptos-schemdadb to define pub struct instead of pub(crate)
#[macro_export]
macro_rules! define_schema {
    ($schema_type:ident, $key_type:ty, $value_type:ty, $cf_name:expr) => {
        #[derive(Debug)]
        pub struct $schema_type;

        impl gaptos::aptos_schemadb::schema::Schema for $schema_type {
            type Key = $key_type;
            type Value = $value_type;

            const COLUMN_FAMILY_NAME: ColumnFamilyName = $cf_name;
        }
    };
}

pub use block::BLOCK_CF_NAME;
pub use dag::{CERTIFIED_NODE_CF_NAME, DAG_VOTE_CF_NAME, NODE_CF_NAME};
use gaptos::aptos_schemadb::ColumnFamilyName;
pub use quorum_certificate::QC_CF_NAME;
pub use single_entry::SINGLE_ENTRY_CF_NAME;
