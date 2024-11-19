use api::ExecutionApi;
use api_types::{BlockBatch, BlockHashState, GTxn};
use async_trait::async_trait;


struct MockClient;

impl MockClient {
    pub fn new() -> Self {
        Self {
        }
    }
}

#[async_trait]
impl ExecutionApi for MockClient {
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch {
        // generate_proposal()
        todo!()
    }

    async fn send_ordered_block(&self, txns: Vec<GTxn>) {
        todo!()
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        [0; 32]
    }

    async fn commit_block_hash(&self, block_ids: Vec<[u8; 32]>) {
        todo!()
    }

    fn latest_block_number(&self) -> u64 {
        0
    }

    async fn recover_ordered_block(&self, block: Vec<GTxn>, res: [u8; 32]) {
        unimplemented!("No need for bench mode")
    }
}
