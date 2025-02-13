use serde::{Deserialize, Serialize};
use rand::Rng;
use std::fmt;
use hex;

#[derive(Clone, Deserialize, Serialize, Hash, PartialEq, Eq, Copy)]
pub struct ComputeRes {
    pub data: [u8; 32],
    // todo(gravity_byteyue): Refactor to TxnInfo when refactoring
    pub txn_num: u64,
}

impl fmt::Display for ComputeRes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ComputeRes({}, txn_num: {})", hex::encode(self.data), self.txn_num)
    }
}

impl fmt::Debug for ComputeRes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ComputeRes({}, txn_num: {})", hex::encode(self.data), self.txn_num)
    }
}

impl ComputeRes {
    pub fn from_bytes(bytes: &[u8], txn_num: u64) -> Self {
        let mut data = [0u8; 32];
        data.copy_from_slice(bytes);
        Self { data, txn_num }
    }

    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let random_bytes: [u8; 32] = rng.gen();
        let txn_num = rng.gen(); // 生成随机的 txn_num
        Self { data: random_bytes, txn_num }
    }

    pub fn new(data: [u8; 32], txn_num: u64) -> Self {
        Self { data, txn_num }
    }

    pub fn bytes(&self) -> [u8; 32] {
        self.data
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn txn_num(&self) -> u64 {
        self.txn_num
    }
}
