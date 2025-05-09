use log::warn;
use tokio::sync::mpsc::error::TryRecvError;
use crate::txn::RawTxn;
use gaptos::api_types::{account::ExternalAccountAddress, VerifiedTxnWithAccountSeqNum};
use gaptos::api_types::VerifiedTxn;
use tokio::sync::mpsc::Sender;
use std::collections::{BTreeMap, HashMap};
use tokio::sync::Mutex;

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
    broadcast_send: Sender<VerifiedTxn>,
    broadcast_recv: Mutex<tokio::sync::mpsc::Receiver<VerifiedTxn>>,
}

impl Mempool {
    pub fn new() -> Self {
        let (send, recv) = tokio::sync::mpsc::channel::<VerifiedTxn>(1024);
        let (broadcast_send, broadcast_recv) = tokio::sync::mpsc::channel::<VerifiedTxn>(1024);
        Mempool {
            water_mark: tokio::sync::Mutex::new(HashMap::new()),
            mempool: tokio::sync::Mutex::new(HashMap::new()),
            pending_recv: Mutex::new(recv),
            pending_send: send,
            broadcast_send,
            broadcast_recv: Mutex::new(broadcast_recv),
        }
    }

    pub async fn remove_txn(&self, verified_txn: &VerifiedTxn) {
        let sender = verified_txn.sender();
        let seq = verified_txn.seq_number();
        let mut pool = self.mempool.lock().await;
        match pool.get_mut(sender) {
            Some(sender_txns) => {
                sender_txns.remove(&seq);
            },
            None => {
                warn!("might be follower");
            },
        }
    }

    pub async fn add_verified_txn(&self, txn: VerifiedTxn) {
        let account = txn.sender().clone();
        let sequence_number = txn.seq_number();
        let status = TxnStatus::Waiting;
        let mempool_txn = MempoolTxn {
            raw_txn: txn.into(),
            status: status,
        };
        self.mempool
            .lock()
            .await
            .entry(account.clone())
            .or_insert(BTreeMap::new())
            .insert(sequence_number, mempool_txn);
        self.process_txn(account).await;
    }

    pub async fn add_raw_txn(&self, bytes: Vec<u8>) {
        let raw_txn = RawTxn::from_bytes(bytes);
        let _ = self.broadcast_send.send(raw_txn.clone().into_verified()).await;
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

    pub async fn recv_unbroadcasted_txn(&self) -> Vec<VerifiedTxn> {
        let mut txns = Vec::new();
        
        while let Some(result) = {
            let mut receiver = self.broadcast_recv.lock().await;
            Some(receiver.try_recv())
        } {
            match result {
                Ok(txn) => {
                    txns.push(txn)
                },
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        }
        txns
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

    pub async fn pending_txns(&self) -> Vec<VerifiedTxnWithAccountSeqNum> {
        let mut txns = Vec::new();
        
        while let Some(result) = {
            let mut receiver = self.pending_recv.lock().await;
            Some(receiver.try_recv())
        } {
            match result {
                Ok(txn) => {
                    txns.push(VerifiedTxnWithAccountSeqNum {
                        txn,
                        account_seq_num: 1,
                    });
                },
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        }
        txns
    }
}
