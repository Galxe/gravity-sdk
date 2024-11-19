use leveldb::batch::Writebatch;
use leveldb::database::Database;
use db_key::Key;
use leveldb::options::WriteOptions;
use std::collections::HashMap;
use leveldb::batch::Batch;
use std::sync::Mutex;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct StringKey(Vec<u8>);

impl StringKey {
    fn new(s: &str) -> Self {
        StringKey(s.as_bytes().to_vec())
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

pub struct BatchManager<'a> {
    db: &'a Mutex<Database<StringKey>>,
    batches: HashMap<u64, Writebatch<StringKey>>,
    current_id: u64,
}

impl<'a> BatchManager<'a> {
    pub fn new(db: &'a Mutex<Database<StringKey>>) -> Self {
        BatchManager {
            db,
            batches: HashMap::new(),
            current_id: 0,
        }
    }

    pub fn create_batch(&mut self) -> u64 {
        self.current_id += 1;
        self.batches.insert(self.current_id, Writebatch::new());
        self.current_id
    }

    pub fn add_to_batch(&mut self, batch_id: u64, key: StringKey, value: &[u8]) {
        if let Some(batch) = self.batches.get_mut(&batch_id) {
            batch.put(key, value);
        } else {
            eprintln!("Batch ID {} not found!", batch_id);
        }
    }

    pub fn delete_from_batch(&mut self, batch_id: u64, key: StringKey) {
        if let Some(batch) = self.batches.get_mut(&batch_id) {
            batch.delete(key);
        } else {
            eprintln!("Batch ID {} not found!", batch_id);
        }
    }

    pub fn commit_batch(&mut self, batch_id: u64) {
        if let Some(batch) = self.batches.remove(&batch_id) {
            let write_opts = WriteOptions::new();
            let db_guard = self.db.lock().expect("Failed to lock database");
            match db_guard.write(write_opts, &batch) {
                Ok(_) => println!("Batch {} committed successfully!", batch_id),
                Err(e) => eprintln!("Failed to commit batch {}: {:?}", batch_id, e),
            }
        } else {
            eprintln!("Batch ID {} not found!", batch_id);
        }
    }

    pub fn rollback_batch(&mut self, batch_id: u64) {
        if self.batches.remove(&batch_id).is_some() {
            println!("Batch {} rolled back successfully!", batch_id);
        } else {
            eprintln!("Batch ID {} not found!", batch_id);
        }
    }
}