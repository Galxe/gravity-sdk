use super::{ensure_slice_len_eq, EPOCH_BY_BLOCK_NUMBER_CF_NAME};
use anyhow::Result;
use byteorder::{BigEndian, ReadBytesExt};
use gaptos::aptos_schemadb::{
    define_pub_schema,
    schema::{KeyCodec, ValueCodec},
};
use std::mem::size_of;

define_pub_schema!(
    EpochByBlockNumberSchema,
    u64, // block num
    u64, // epoch num
    EPOCH_BY_BLOCK_NUMBER_CF_NAME
);

impl KeyCodec<EpochByBlockNumberSchema> for u64 {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }
    fn decode_key(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, std::mem::size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}

impl ValueCodec<EpochByBlockNumberSchema> for u64 {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }

    fn decode_value(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}
