// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! This module defines the storage schema for block events used by leader reputation.
//!
//! Block events are stored with a monotonically increasing sequence number as key,
//! and BCS-encoded NewBlockEvent as value. This provides an ordered history of
//! committed blocks that LeaderReputation can use for off-chain proposer election.
//!
//! ```text
//! |<---key--->|<------value------>|
//! | seq_num   | NewBlockEvent(bcs)|
//! ```

use crate::define_schema;
use anyhow::Result;
use gaptos::{
    aptos_schemadb::{
        schema::{KeyCodec, ValueCodec},
        ColumnFamilyName,
    },
    aptos_types::account_config::NewBlockEvent,
};

use super::ensure_slice_len_eq;

pub const BLOCK_EVENT_CF_NAME: ColumnFamilyName = "block_event";

define_schema!(BlockEventSchema, u64, NewBlockEvent, BLOCK_EVENT_CF_NAME);

impl KeyCodec<BlockEventSchema> for u64 {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, std::mem::size_of::<Self>())?;
        let bytes: [u8; 8] = data.try_into()?;
        Ok(u64::from_be_bytes(bytes))
    }
}

impl ValueCodec<BlockEventSchema> for NewBlockEvent {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(self)?)
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}
