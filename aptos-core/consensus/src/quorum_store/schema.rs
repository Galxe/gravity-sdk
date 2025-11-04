// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::quorum_store::types::{BatchKey, PersistedValue};
use anyhow::Result;
use aptos_consensus_types::proof_of_store::BatchId;
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_schemadb::{
    schema::{KeyCodec, Schema, ValueCodec},
    ColumnFamilyName,
};

pub(crate) const BATCH_CF_NAME: ColumnFamilyName = "batch";
pub(crate) const BATCH_ID_CF_NAME: ColumnFamilyName = "batch_ID";


#[derive(Debug)]
pub(crate) struct BatchSchema;

impl Schema for BatchSchema {
    type Key = BatchKey;
    type Value = PersistedValue;

    const COLUMN_FAMILY_NAME: gaptos::aptos_schemadb::ColumnFamilyName = BATCH_CF_NAME;
}

impl KeyCodec<BatchSchema> for BatchKey {
    fn encode_key(&self) -> Result<Vec<u8>> {
        let mut key_bytes = Vec::with_capacity(8 + self.digest.to_vec().len());
        key_bytes.extend_from_slice(&self.epoch.to_be_bytes());
        key_bytes.extend_from_slice(&self.digest.to_vec());
        Ok(key_bytes)
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        let epoch_bytes: [u8; 8] = data[0..8].try_into()?;
        let epoch = u64::from_be_bytes(epoch_bytes);
        let digest_data = &data[8..];
        let digest = HashValue::from_slice(digest_data)?;
        Ok(BatchKey { epoch, digest })
    }
}

impl ValueCodec<BatchSchema> for PersistedValue {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self)?)
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}

#[derive(Debug)]
pub(crate) struct BatchIdSchema;

impl Schema for BatchIdSchema {
    type Key = u64;
    type Value = BatchId;

    const COLUMN_FAMILY_NAME: gaptos::aptos_schemadb::ColumnFamilyName = BATCH_ID_CF_NAME;
}

impl KeyCodec<BatchIdSchema> for u64 {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self)?)
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}

impl ValueCodec<BatchIdSchema> for BatchId {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self)?)
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}
