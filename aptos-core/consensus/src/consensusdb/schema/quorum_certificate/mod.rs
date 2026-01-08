// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! This module defines physical storage schema for consensus quorum certificate (of a block).
//!
//! Serialized quorum certificate bytes identified by block_hash.
//! ```text
//! |<---key---->|<----value--->|
//! | block_hash |  QuorumCert  |
//! ```

use crate::define_schema;
use anyhow::Result;
use aptos_consensus_types::quorum_cert::QuorumCert;
use gaptos::{
    aptos_crypto::HashValue,
    aptos_schemadb::{
        schema::{KeyCodec, ValueCodec},
        ColumnFamilyName,
    },
};

pub const QC_CF_NAME: ColumnFamilyName = "quorum_certificate";

define_schema!(QCSchema, (u64, HashValue), QuorumCert, QC_CF_NAME);

impl KeyCodec<QCSchema> for (u64, HashValue) {
    fn encode_key(&self) -> Result<Vec<u8>> {
        let (seq_num, hash_value) = self;
        let mut key_bytes = Vec::with_capacity(8 + hash_value.to_vec().len());
        key_bytes.extend_from_slice(&seq_num.to_be_bytes());
        key_bytes.extend_from_slice(&hash_value.to_vec());
        Ok(key_bytes)
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        let seq_num_bytes: [u8; 8] = data[0..8].try_into()?;
        let seq_num = u64::from_be_bytes(seq_num_bytes);
        let hash_value_data = &data[8..];
        let hash_value = HashValue::from_slice(hash_value_data)?;
        Ok((seq_num, hash_value))
    }
}

impl ValueCodec<QCSchema> for QuorumCert {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(self)?)
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}

#[cfg(test)]
mod test;
