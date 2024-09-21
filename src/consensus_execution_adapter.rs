use std::collections::HashMap;
use std::hash::Hash;
use crate::{GCEIError, GTxn, GravityConsensusEngineInterface};
use anyhow::Result;
use aptos_consensus::gravity_state_computer::ConsensusAdapterArgs;
use aptos_crypto::hash::HashValue;
use aptos_crypto::{PrivateKey, Uniform};
use aptos_mempool::{MempoolClientRequest, SubmissionStatus};
use aptos_types::account_address::AccountAddress;
use aptos_types::chain_id::ChainId;
use aptos_types::mempool_status::MempoolStatusCode;
use aptos_types::transaction::{GravityExtension, RawTransaction, SignedTransaction, TransactionPayload};
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use tokio::sync::RwLock;
use aptos_types::transaction::authenticator::TransactionAuthenticator;

pub struct ConsensusExecutionAdapter {
    mempool_sender: mpsc::Sender<MempoolClientRequest>,
    pipeline_block_receiver:
        Option<mpsc::UnboundedReceiver<(HashValue, HashValue, Vec<SignedTransaction>, oneshot::Sender<HashValue>)>>,

    execute_result_receivers: RwLock<HashMap<HashValue, oneshot::Sender<HashValue>>>,
}

impl ConsensusExecutionAdapter {
    pub fn new(args: &mut ConsensusAdapterArgs) -> Self {
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

#[async_trait::async_trait]
impl GravityConsensusEngineInterface for ConsensusExecutionAdapter {
    fn init(&mut self) {
        todo!()
    }

    async fn send_valid_transactions(
        &self,
        block_id: [u8; 32],
        txns: Vec<GTxn>,
    ) -> Result<(), GCEIError> {
        let len = txns.len();
        for (i, txn) in txns.into_iter().enumerate() {
            let raw_txn = RawTransaction::new(
                AccountAddress::random(),
                txn.sequence_number,
                TransactionPayload::GTxnBytes(txn.txn_bytes),
                txn.max_gas_amount,
                txn.gas_unit_price,
                txn.expiration_timestamp_secs,
                ChainId::new(txn.chain_id),
            );
            let sign_txn = SignedTransaction::new_with_gtxn(
                raw_txn,
                aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing().public_key(),
                aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap(),
                GravityExtension::new(HashValue::new(block_id), i as u32, len as u32),
            );
            let (mempool_status, _vm_status_opt) =
                self.submit_transaction(sign_txn).await.unwrap();
            match mempool_status.code {
                MempoolStatusCode::Accepted => {}
                _ => {
                    return Err(GCEIError::ConsensusError);
                }
            }
        }
        Ok(())
    }

    async fn receive_ordered_block(&mut self) -> anyhow::Result<([u8; 32], Vec<GTxn>), GCEIError> {
        let (parent_id, block_id, txns, callback) = self
            .pipeline_block_receiver
            .as_mut()
            .unwrap()
            .try_next()
            .unwrap()
            .unwrap();
        let gtxns = txns.iter().map(|txn| {
            let txn_bytes = match txn.payload() {
                TransactionPayload::GTxnBytes(bytes) => { bytes }
                _ => { todo!() }
            };
            let (pkey, sig) = match txn.authenticator() {
                TransactionAuthenticator::Ed25519 { public_key, signature } => {(public_key, signature)}
                _ => { todo!() }
            };
            GTxn {
                sequence_number : txn.sequence_number(),
                max_gas_amount: txn.max_gas_amount(),
                gas_unit_price: txn.gas_unit_price(),
                expiration_timestamp_secs: txn.expiration_timestamp_secs(),
                chain_id: txn.chain_id().to_u8(),
                txn_bytes: (*txn_bytes.clone()).to_owned(),
                public_key: pkey.to_bytes(),
                signature: sig.to_bytes(),
            }
        }).collect();
        self.execute_result_receivers
            .write()
            .await
            .insert(parent_id, callback);

        Ok((*parent_id, gtxns))
    }

    async fn send_compute_res(&self, id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {

        match self.execute_result_receivers.write().await.remove(&HashValue::new(id)) {
            Some(callback) => Ok(callback.send(HashValue::new(res)).unwrap()),
            None => todo!(),
        }
    }

    async fn send_block_head(&self, block_id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {
        todo!()
    }

    async fn receive_commit_block_ids(&self) -> Result<Vec<[u8; 32]>, GCEIError> {
        todo!()
    }

    async fn send_persistent_block_id(&self, id: [u8; 32]) -> Result<(), GCEIError> {
        todo!()
    }
}
