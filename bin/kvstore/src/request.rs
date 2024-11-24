use leveldb::batch::Writebatch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::batch_manager::StringKey;

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Get(GetRequest),
    Set(SetRequest),
}

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct GetRequest {
    pub key: Vec<u8>,
}

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct SetRequest {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SerializableWriteBatch {
    operations: Option<Vec<SetRequest>>,
}

impl SerializableWriteBatch {
    pub fn new_from_batch(operations: Vec<SetRequest>) -> Self {
        Self {
            operations: Some(operations),
        }
    }

    pub fn calculate_hash(&self) -> [u8; 32] {
        let serialized = bincode::serialize(&self).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    pub fn into_write_batch(&mut self) -> Writebatch<StringKey> {
        let mut write_batch = Writebatch::new();
        self.operations.take().expect("No operations").into_iter().for_each(|req| {
            write_batch.put(StringKey::new_from_vec(req.key), &req.value);
        });
        write_batch
    }
}