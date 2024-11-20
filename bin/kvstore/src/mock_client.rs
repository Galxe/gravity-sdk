use api::ExecutionApi;
use api_types::{BlockBatch, BlockHashState, GTxn};
use async_trait::async_trait;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

use crate::{batch_manager::HashValue, kvstore::KvStore};

struct MockClient {
    kv_store: KvStore,
    block_hash_channel_sender: UnboundedSender<HashValue>,
    block_hash_channel_receiver: Mutex<UnboundedReceiver<HashValue>>,
}

impl MockClient {
    pub fn new(kv_store: KvStore) -> Self {
        let (block_hash_channel_sender, block_hash_channel_receiver) =
            tokio::sync::mpsc::unbounded_channel();
        Self {
            kv_store,
            block_hash_channel_sender,
            block_hash_channel_receiver: Mutex::new(block_hash_channel_receiver),
        }
    }
}

#[async_trait]
impl ExecutionApi for MockClient {
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch {
        self.kv_store.generate_proposal().await
    }

    async fn send_ordered_block(&self, txns: Vec<GTxn>) {
        self.block_hash_channel_sender
            .send(self.kv_store.process_block(txns).await)
            .expect("Fail");
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        let mut receiver = self.block_hash_channel_receiver.lock().await;
        let block_hash = receiver.recv().await.expect("recv block hash failed");
        *block_hash.as_bytes()
    }

    async fn commit_block_hash(&self, block_ids: Vec<[u8; 32]>) {
        for block_id in block_ids.into_iter() {
            self.kv_store.commit_block(HashValue::new(block_id)).await
        }
    }

    fn latest_block_number(&self) -> u64 {
        0
    }

    async fn recover_ordered_block(&self, block: Vec<GTxn>, res: [u8; 32]) {
        unimplemented!("No need for bench mode")
    }
}
