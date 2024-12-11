use tokio::sync::mpsc::error::TryRecvError;
use crate::txn::RawTxn;
use api_types::VerifiedTxn;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;


pub struct Mempool {
    pending_recv: Mutex<tokio::sync::mpsc::Receiver<VerifiedTxn>>,
    pending_send: tokio::sync::mpsc::Sender<VerifiedTxn>,
    broadcast_send: Sender<VerifiedTxn>,
    broadcast_recv: Mutex<tokio::sync::mpsc::Receiver<VerifiedTxn>>,
}

impl Mempool {
    pub fn new() -> Self {
        let (send, recv) = tokio::sync::mpsc::channel::<VerifiedTxn>(1024 * 1024);
        let (broadcast_send, broadcast_recv) = tokio::sync::mpsc::channel::<VerifiedTxn>(1024 * 1024);
        Mempool {
            pending_recv: Mutex::new(recv),
            pending_send: send,
            broadcast_send,
            broadcast_recv: Mutex::new(broadcast_recv),
        }
    }

    pub async fn add_verified_txn(&self, txn: VerifiedTxn) {
        self.process_txn(txn.into()).await;
    }

    pub async fn add_raw_txn(&self, bytes: Vec<u8>) {
        let raw_txn = RawTxn::from_bytes(bytes);
        let _ = self.broadcast_send.send(raw_txn.clone().into_verified()).await;
        self.process_txn(raw_txn).await;
    }

    pub async fn recv_unbroadcasted_txn(&self) -> Vec<VerifiedTxn> {
        let mut recv = self.broadcast_recv.lock().await;
        let mut buffer = Vec::new();
        recv.recv_many(&mut buffer, 1024).await;
        buffer
    }

    pub async fn process_txn(&self, raw_txn: RawTxn) {
        self.pending_send.send(raw_txn.clone().into_verified()).await.unwrap();
    }

    pub async fn pending_txns(&self) -> Vec<(VerifiedTxn, u64)> {
        let mut txns = Vec::new();
        
        while let Some(result) = {
            let mut receiver = self.pending_recv.lock().await;
            Some(receiver.try_recv())
        } {
            match result {
                Ok(txn) => {
                    txns.push((txn, 1))
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
