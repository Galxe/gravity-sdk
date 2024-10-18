use std::sync::Arc;

use aptos_config::config::NodeConfig;
use tokio::sync::Mutex;

use crate::{GCEIError, GTxn, GravityConsensusEngineInterface};

pub struct SimpleConsensusEngine {
    txns: Mutex<Vec<GTxn>>,
    block_id: Mutex<[u8; 32]>,
}

#[async_trait::async_trait]
impl GravityConsensusEngineInterface for SimpleConsensusEngine {
    fn init(args: NodeConfig) -> Arc<SimpleConsensusEngine> {
        Arc::new(SimpleConsensusEngine { txns: Mutex::new(Vec::new()), block_id: Mutex::new([0; 32]) })
    }

    async fn send_valid_block_transactions(
        &self,
        new_block_id: [u8; 32],
        new_txns: Vec<GTxn>,
    ) -> Result<(), GCEIError> {
        let mut txns = self.txns.lock().await;
        let mut block_id = self.block_id.lock().await;
        *txns = new_txns;
        *block_id = new_block_id;
        Ok(())
    }

    async fn receive_ordered_block(&self) -> Result<([u8; 32], Vec<GTxn>), GCEIError> {
        let mut txns = self.txns.lock().await;
        let block_id = self.block_id.lock().await;
        let txns = std::mem::replace(&mut *txns, Vec::new());
        Ok((*block_id, txns))
    }

    async fn send_compute_res(&self, block_id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {
        Ok(())
    }

    async fn send_block_head(&self, block_id: [u8; 32], header: [u8; 32]) -> Result<(), GCEIError> {
        Ok(())
    }

    async fn receive_commit_block_ids(&self) -> Result<Vec<[u8; 32]>, GCEIError> {
        let mut res = Vec::new();
        res.push(*self.block_id.lock().await);
        Ok(res)
    }

    async fn send_persistent_block_id(&self, id: [u8; 32]) -> Result<(), GCEIError> {
        Ok(())
    }

    fn is_leader(&self) -> bool {
        true
    }
}
