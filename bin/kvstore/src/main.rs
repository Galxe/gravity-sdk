use std::sync::Arc;

use kvstore::KvStore;
use tokio::sync::Mutex;

mod mock_client;
mod block;
mod batch_manager;
mod kvstore;
mod request;

fn main() {
    let db_path = "example_leveldb";
    let path = std::path::Path::new(db_path);
    let mut options = leveldb::options::Options::new();
    options.create_if_missing = true;

    let db = Arc::new(Mutex::new(leveldb::database::Database::open(path, options).unwrap()));

    // Create the KvStore which includes a BatchManager
    let kv_store = KvStore::new(db, 1337);
    todo!()
}
