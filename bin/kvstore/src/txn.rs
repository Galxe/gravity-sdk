use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId},
    u256_define::TxnHash,
};
use gaptos::api_types::{simple_hash::hash_to_fixed_array, VerifiedTxn};
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
        let hash = hash_to_fixed_array(&bytes);
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

#[cfg(test)]
mod test {
    use gaptos::api_types::account::ExternalAccountAddress;

    use crate::txn::RawTxn;

    #[test]
    fn print_txn_bytes() {
        let txn = RawTxn {
            account: ExternalAccountAddress::random(),
            sequence_number: 1,
            key: "aa".to_string(),
            val: "bb".to_string(),
        };
        println!("curl -X POST -H \"Content-Type:application/json\" -d '{{\"tx\": {:?}}}' https://127.0.0.1:1998/tx/submit_tx", txn.to_bytes());
    }
}
