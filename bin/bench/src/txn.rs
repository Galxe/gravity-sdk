use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId},
    simple_hash,
    u256_define::TxnHash,
    VerifiedTxn,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct RawTxn {
    pub(crate) account: ExternalAccountAddress,
    pub(crate) sequence_number: u64,
    pub(crate) key: String,
    pub(crate) val: String,
}

impl From<VerifiedTxn> for RawTxn {
    fn from(value: VerifiedTxn) -> Self {
        let txn: RawTxn = serde_json::from_slice(&value.bytes()).unwrap();
        txn
    }
}

impl RawTxn {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let txn: RawTxn = serde_json::from_slice(&bytes).unwrap();
        txn
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    pub fn key(&self) -> &String {
        &self.key
    }

    pub fn val(&self) -> &String {
        &self.val
    }

    pub fn into_verified(self) -> VerifiedTxn {
        let bytes = self.to_bytes();
        let hash = simple_hash::hash_to_fixed_array(&bytes);
        VerifiedTxn::new(
            bytes,
            self.account,
            self.sequence_number,
            ExternalChainId::new(0),
            TxnHash::new(hash),
        )
    }

    pub fn account(&self) -> ExternalAccountAddress {
        self.account.clone()
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_number
    }
}
