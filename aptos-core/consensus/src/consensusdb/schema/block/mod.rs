// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! This module defines physical storage schema for consensus block.
//!
//! Serialized block bytes identified by block_hash.
//! ```text
//! |<---key---->|<---value--->|
//! | block_hash |    block    |
//! ```

use crate::define_schema;
use anyhow::Result;
use aptos_consensus_types::block::Block;
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_schemadb::{
    schema::{KeyCodec, ValueCodec},
    ColumnFamilyName,
};
use byteorder::{BigEndian, ReadBytesExt};

use super::ensure_slice_len_eq;

pub const BLOCK_CF_NAME: ColumnFamilyName = "block";
pub const BLOCK_NUMBER_CF_NAME: ColumnFamilyName = "block_number";

define_schema!(BlockSchema, HashValue, Block, BLOCK_CF_NAME);

impl KeyCodec<BlockSchema> for HashValue {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        Ok(HashValue::from_slice(data)?)
    }
}

impl ValueCodec<BlockSchema> for Block {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(&self)?)
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}

define_schema!(BlockNumberSchema, HashValue, u64, BLOCK_NUMBER_CF_NAME);

impl KeyCodec<BlockNumberSchema> for HashValue {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        Ok(HashValue::from_slice(data)?)
    }
}

impl ValueCodec<BlockNumberSchema> for u64 {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }
    fn decode_value(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, std::mem::size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}

#[cfg(test)]
mod test;
