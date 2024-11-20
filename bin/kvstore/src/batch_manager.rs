use futures_channel::oneshot;
use leveldb::database::Database;
use db_key::Key;
use leveldb::options::WriteOptions;
use tokio::sync::Mutex;
use std::collections::HashMap;
use leveldb::batch::Batch;
use std::fmt;
use std::sync::Arc;

use crate::request::{SerializableWriteBatch, SetRequest};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct StringKey(Vec<u8>);

impl StringKey {
    pub fn new(s: &str) -> Self {
        StringKey(s.as_bytes().to_vec())
    }

    pub fn new_from_vec(vec: Vec<u8>) -> Self {
        StringKey(vec)
    }
}

impl Key for StringKey {
    fn from_u8(key: &[u8]) -> Self {
        StringKey(key.to_vec())
    }

    fn as_slice<T, F: FnOnce(&[u8]) -> T>(&self, f: F) -> T {
        f(&self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HashValue([u8; 32]);

impl HashValue {
    pub fn new(array: [u8; 32]) -> Self {
        Self(array)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn new_from_vec(vec: Vec<u8>) -> Result<Self, &'static str> {
        if vec.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(&vec);
            Ok(HashValue(array))
        } else {
            Err("Vec must have exactly 32 elements")
        }
    }
}

impl From<u64> for HashValue {
    fn from(value: u64) -> Self {
        let mut array = [0u8; 32];
        array[24..].copy_from_slice(&value.to_be_bytes());
        Self(array)
    }
}

impl Into<u64> for HashValue {
    fn into(self) -> u64 {
        let bytes: [u8; 8] = self.0[24..].try_into().expect("Slice should have exactly 8 bytes");
        u64::from_be_bytes(bytes)
    }
}

impl fmt::Debug for HashValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HashValue({:?})", self.0)
    }
}

struct IdGenerator {
    current_id: u64,
}

impl IdGenerator {
    pub fn new() -> Self {
        Self { current_id: 0 }
    }

    pub fn next_id(&mut self) -> HashValue {
        let mut id_bytes = [0u8; 32];
        id_bytes[24..].copy_from_slice(&self.current_id.to_be_bytes());

        self.current_id += 1;

        HashValue(id_bytes)
    }
}


pub struct BatchManager {
    db: Arc<Mutex<Database<StringKey>>>,
    batches: HashMap<HashValue, SerializableWriteBatch>,
    current_id: IdGenerator,
    pending_callbacks: HashMap<HashValue, Vec<oneshot::Sender<()>>>,
}

impl BatchManager {
    pub fn new(db: Arc<Mutex<Database<StringKey>>>) -> Self {
        BatchManager {
            db,
            batches: HashMap::new(),
            current_id: IdGenerator::new(),
            pending_callbacks: HashMap::new(),
        }
    }

    // Only the proposal would call this function
    pub fn add_set_callbacks(&mut self, callbacks: Vec<oneshot::Sender<()>>) -> HashValue {
        let next_id = self.current_id.next_id();
        self.pending_callbacks.insert(next_id, callbacks);
        next_id
    }

    pub fn append_write_sets(&mut self, set_requests: Vec<SetRequest>, id: HashValue) -> HashValue {
        let batch = SerializableWriteBatch::new_from_batch(set_requests);
        let hash = HashValue::new(batch.calculate_hash());
        self.batches.insert(id, batch);
        hash
    }

    pub async fn commit_batch(&mut self, batch_id: HashValue) {
        if let Some(mut batch) = self.batches.remove(&batch_id) {
            let write_opts = WriteOptions::new();
            let db_guard = self.db.lock().await;
            match db_guard.write(write_opts, &batch.into_write_batch()) {
                Ok(_) => println!("Batch {:?} committed successfully!", batch_id),
                Err(e) => eprintln!("Failed to commit batch {:?}: {:?}", batch_id, e),
            }
            // Only leader would trigger this branch
            if let Some(callbacks) = self.pending_callbacks.remove(&batch_id) {
                callbacks.into_iter().for_each(|cb| {
                    cb.send(()).expect("Failed");
                })
            }
        } else {
            eprintln!("Batch ID {:?} not found!", batch_id);
        }
    }
}