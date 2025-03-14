use anyhow::Result;
use gaptos::aptos_schemadb::{
    define_schema,
    schema::{KeyCodec, ValueCodec},
};
use gaptos::aptos_types::ledger_info::LedgerInfoWithSignatures;
use super::ensure_slice_len_eq;
use super::LEDGER_INFO_CF_NAME;
use byteorder::{BigEndian, ReadBytesExt};
define_schema!(
    LedgerInfoSchema,
    u64, /* block num */
    LedgerInfoWithSignatures,
    LEDGER_INFO_CF_NAME
);
impl KeyCodec<LedgerInfoSchema> for u64 {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }
    fn decode_key(mut data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, std::mem::size_of::<Self>())?;
        Ok(data.read_u64::<BigEndian>()?)
    }
}
impl ValueCodec<LedgerInfoSchema> for LedgerInfoWithSignatures {
    fn encode_value(&self) -> Result<Vec<u8>> {
        bcs::to_bytes(self).map_err(Into::into)
    }
    fn decode_value(data: &[u8]) -> Result<Self> {
        bcs::from_bytes(data).map_err(Into::into)
    }
}
