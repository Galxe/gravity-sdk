use api_types::u256_define::BlockId;
use tokio::sync::{mpsc, Mutex};

pub struct Queue {
    exec_sender: mpsc::Sender<BlockId>,
    exec_receiver: Mutex<mpsc::Receiver<BlockId>>,
    commit_sender: mpsc::Sender<BlockId>,
    commit_receiver: Mutex<mpsc::Receiver<BlockId>>,
}

impl Queue {
    pub fn new(size: usize) -> Self {
        let (exec_sender, exec_receiver) = mpsc::channel(size);
        let (commit_sender, commit_receiver) = mpsc::channel(size);
        Queue {
            exec_sender,
            exec_receiver: Mutex::new(exec_receiver),
            commit_sender,
            commit_receiver: Mutex::new(commit_receiver),
        }
    }

    pub async fn send_exec(&self, block_id: BlockId) {
        self.exec_sender.clone().send(block_id).await.unwrap();
    }

    pub async fn recv_exec(&self) {
        // only commit can clean the queue
    }

    pub async fn send_commit(&self, block_id: BlockId) {
        self.commit_sender.clone().send(block_id).await.unwrap();
    }

    pub async fn recv_commit(&self) -> BlockId {
        let mut recv = self.exec_receiver.lock().await;
        recv.recv().await.unwrap();
        let mut recv = self.commit_receiver.lock().await;
        recv.recv().await.unwrap()
    }
}
