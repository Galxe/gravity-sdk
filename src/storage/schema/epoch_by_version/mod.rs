use anyhow::Result;
use aptos_schemadb::{
    define_schema,
    schema::{KeyCodec, ValueCodec},
};
use aptos_types::transaction::Version;
use byteorder::{BigEndian, ReadBytesExt};
use std::mem::size_of;

use super::{ensure_slice_len_eq, EPOCH_BY_VERSION_CF_NAME};

define_schema!(
    EpochByVersionSchema,
    Version,
    u64, // epoch_num
    EPOCH_BY_VERSION_CF_NAME
);

impl KeyCodec<EpochByVersionSchema> for Version {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }

    fn decode_key(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}

impl ValueCodec<EpochByVersionSchema> for u64 {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }

    fn decode_value(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}
