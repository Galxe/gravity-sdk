use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use api_types::{BlockBatch, GTxn};
use futures_channel::oneshot;
use leveldb::{database::Database, kv::KV, options::ReadOptions};
use ruint::aliases::U256;
use tokio::sync::Mutex;

use crate::{
    batch_manager::{BatchManager, HashValue, StringKey},
    request::{GetRequest, SetRequest},
};

pub struct KvStore {
    db: Arc<Mutex<Database<StringKey>>>,
    batch_manager: Arc<Mutex<BatchManager>>,
    pending_set_opertions: Mutex<Vec<(SetRequest, oneshot::Sender<()>)>>,
    chain_id: u64,
}

impl KvStore {
    pub fn new(db: Arc<Mutex<Database<StringKey>>>, chain_id: u64) -> Self {
        let batch_manager = Arc::new(Mutex::new(BatchManager::new(db.clone())));
        KvStore { db, batch_manager, pending_set_opertions: Mutex::new(vec![]), chain_id }
    }

    pub async fn generate_proposal(&self) -> BlockBatch {
        let mut external_vec = vec![];
        {
            let mut guard = self.pending_set_opertions.lock().await;
            std::mem::swap(&mut *guard, &mut external_vec);
        }
        let (requests, senders): (Vec<_>, Vec<_>) = external_vec.into_iter().unzip();
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24;
        let mut gtxns: Vec<_> = requests
            .iter()
            .map(|req| GTxn {
                sequence_number: 0,
                max_gas_amount: 0,
                gas_unit_price: U256::from(0),
                expiration_timestamp_secs: secs,
                chain_id: self.chain_id,
                txn_bytes: serde_json::to_vec(&req).expect("Failed"),
            })
            .collect();
        // generate one block hash, it would serve as the block id return by future commit blocks
        let mut batch_manager = self.batch_manager.lock().await;
        let batch_id = batch_manager.add_set_callbacks(senders);
        gtxns.insert(
            0,
            GTxn {
                sequence_number: 0,
                max_gas_amount: 0,
                gas_unit_price: U256::from(0),
                expiration_timestamp_secs: secs,
                chain_id: self.chain_id,
                txn_bytes: batch_id.as_bytes().to_vec(),
            },
        );
        BlockBatch { txns: gtxns, block_hash: *batch_id.as_bytes() }
    }

    pub async fn process_block(&self, gtxns: Vec<GTxn>) -> HashValue {
        let batch_id = HashValue::new_from_vec(gtxns[0].get_bytes().to_vec()).expect("Fail");
        let set_requests : Vec<_> = gtxns.iter().skip(1).map(|gtxn| {
            let set_request : SetRequest = serde_json::from_slice(gtxn.get_bytes()).expect("Fail");
            set_request
        }).collect();
        let mut batch_manager = self.batch_manager.lock().await;
        batch_manager.append_write_sets(set_requests, batch_id)
    }

    pub async fn commit_block(&self, block_id: HashValue) {
        let mut batch_manager = self.batch_manager.lock().await;
        batch_manager.commit_batch(block_id);
    }

    pub async fn append_set(&self, req: SetRequest) {
        let (callback, callback_rcv) = oneshot::channel();
        {
            let mut pendings = self.pending_set_opertions.lock().await;
            pendings.push((req, callback));
        }
        let _ = callback_rcv.await;
    }

    pub async fn get_value(&self, req: GetRequest) {
        let db = self.db.lock().await;
        let option = ReadOptions::new();
        match (*db).get(option, StringKey::new_from_vec(req.key)) {
            Ok(value) => todo!(),
            Err(err) => todo!(),
        }
    }
}
