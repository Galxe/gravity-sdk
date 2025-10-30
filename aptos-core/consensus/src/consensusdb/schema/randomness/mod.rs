use super::{ensure_slice_len_eq, RANDOMNESS_CF_NAME};
use anyhow::Result;
use gaptos::aptos_schemadb::{
    define_schema,
    schema::{KeyCodec, ValueCodec},
};
use byteorder::{BigEndian, ReadBytesExt};
use std::mem::size_of;

define_schema!(
    RandomnessSchema,
    u64, // block num
    Vec<u8>, // randomness
    RANDOMNESS_CF_NAME
);

impl KeyCodec<RandomnessSchema> for u64 {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }
    fn decode_key(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, std::mem::size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}

impl ValueCodec<RandomnessSchema> for Vec<u8> {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(self.clone())
    }

    fn decode_value(mut data: &[u8]) -> Result<Self> {
        Ok(data.to_vec())
    }
}
