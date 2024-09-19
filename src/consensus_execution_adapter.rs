use std::collections::HashMap;

use aptos_crypto::hash::HashValue;
use aptos_mempool::{MempoolClientRequest, SubmissionStatus};
use aptos_types::transaction::SignedTransaction;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use anyhow::{Result};
use tokio::sync::RwLock;
use aptos_consensus::gravity_state_computer::ConsensusAdapterArgs;
use aptos_consensus_types::block::Block;
use aptos_types::mempool_status::MempoolStatusCode;
use crate::{GCEIError};

pub struct ConsensusExecutionAdapter {
    mempool_sender: mpsc::Sender<MempoolClientRequest>,
    pipeline_block_receiver:
        Option<mpsc::UnboundedReceiver<(HashValue, Block, oneshot::Sender<HashValue>)>>,

    execute_result_receivers: RwLock<HashMap<HashValue, oneshot::Sender<HashValue>>>,
}

impl ConsensusExecutionAdapter {
    pub fn new(
        args: &mut ConsensusAdapterArgs
    ) -> Self {
        Self {
            mempool_sender: args.mempool_sender.clone(),
            pipeline_block_receiver: args.pipeline_block_receiver.take(),
            execute_result_receivers: RwLock::new(HashMap::new()),
        }
    }

    async fn submit_transaction(&self, txn: SignedTransaction) -> Result<SubmissionStatus> {
        let (req_sender, callback) = oneshot::channel();
        self.mempool_sender
            .clone()
            .send(MempoolClientRequest::SubmitTransaction(txn, req_sender))
            .await?;

        callback.await?
    }
}

impl ConsensusExecutionAdapter {
    pub async fn send_valid_transactions(&self, mut txns: Vec<SignedTransaction>) -> Result<(), GCEIError> {
        for txn in txns.into_iter() {
            let (mempool_status, _vm_status_opt) = self
                .submit_transaction(txn)
                .await.unwrap();
            match mempool_status.code {
                MempoolStatusCode::Accepted => {}
                _ => {
                    return Err(GCEIError::ConsensusError);
                }
            }
        }
        Ok(())
    }

    pub async fn receive_ordered_block(&mut self) -> Result<(HashValue, Block), GCEIError> {
        let (id, block, callback) = self.pipeline_block_receiver.as_mut().unwrap().try_next().unwrap().unwrap();
        self.execute_result_receivers
            .write()
            .await
            .insert(id, callback);
        Ok((id, block))
    }

    pub async fn send_compute_res(&self, id: HashValue, res: HashValue) -> Result<(), GCEIError> {
        match self.execute_result_receivers.write().await.remove(&id) {
            Some(callback) => Ok(callback.send(res).unwrap()),
            None => todo!(),
        }
    }

    pub async fn receive_commit_block_ids(&self) -> Vec<HashValue> {
        todo!()
    }

    pub async fn send_persistent_block_id(&self, id: HashValue) {
        todo!()
    }
}
