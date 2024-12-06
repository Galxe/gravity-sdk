use std::collections::{BTreeMap, HashMap};
use tokio::sync::Mutex;
use api_types::account::ExternalAccountAddress;
use api_types::VerifiedTxn;
use crate::txn::RawTxn;

#[derive(Clone, Debug, PartialEq)]
pub enum TxnStatus {
    Pending,
    Waiting,
}


pub struct MempoolTxn {
    raw_txn: RawTxn,
    status: TxnStatus,
}

pub struct Mempool {
    water_mark: tokio::sync::Mutex<HashMap<ExternalAccountAddress, u64>>,
    mempool: tokio::sync::Mutex<HashMap<ExternalAccountAddress, BTreeMap<u64, MempoolTxn>>>,
    pending_recv: Mutex<tokio::sync::mpsc::Receiver<VerifiedTxn>>,
    pending_send: tokio::sync::mpsc::Sender<VerifiedTxn>,
}

impl Mempool {
    pub fn new() -> Self {
        let (send, recv) = tokio::sync::mpsc::channel::<VerifiedTxn>(1024);
        Mempool {
            water_mark: tokio::sync::Mutex::new(HashMap::new()),
            mempool: tokio::sync::Mutex::new(HashMap::new()),
            pending_recv: Mutex::new(recv),
            pending_send: send,
        }
    }

    pub async fn remove_txn(&self, verified_txn: &VerifiedTxn) {
        let sender = verified_txn.sender();
        let seq = verified_txn.seq_number();
        let mut pool = self.mempool.lock().await;
        pool.get_mut(sender).unwrap().remove(&seq);
    }

    pub async fn add_txn(&self, bytes: Vec<u8>) {
        let raw_txn = RawTxn::from_bytes(bytes);
        let sequence_number = raw_txn.sequence_number();
        let status = TxnStatus::Waiting;
        let account = raw_txn.account();
        let txn = MempoolTxn {
            raw_txn,
            status,
        };
        {
            self.mempool.lock().await.entry(account.clone()).or_insert(BTreeMap::new()).insert(sequence_number, txn);
        }
        self.process_txn(account).await;
    }

    pub async fn process_txn(&self, account: ExternalAccountAddress) {
        let mut mempool = self.mempool.lock().await;
        let mut water_mark = self.water_mark.lock().await;
        let account_mempool = mempool.get_mut(&account).unwrap();
        let sequence_number = water_mark.entry(account).or_insert(0);
        for txn in account_mempool.values_mut() {
            if txn.raw_txn.sequence_number() == *sequence_number + 1 {
                *sequence_number += 1;
                txn.status = TxnStatus::Pending;
                self.pending_send.send(txn.raw_txn.clone().into_verified()).await.unwrap();
            }
        }
    }

    pub async fn pending_txns(&self) -> Vec<VerifiedTxn> {
        let mut txns = Vec::new();
        while let Some(txn) = self.pending_recv.lock().await.recv().await {
            txns.push(txn);
        }
        txns
    }
}