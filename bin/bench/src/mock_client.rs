use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId};
use anyhow::Context;
use jsonrpsee::core::async_trait;
use reth::api::EngineTypes;
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use tracing::info;
use api_types::{BlockBatch, BlockHashState, ExecutionApi, GTxn};
use reth_primitives::Bytes;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::log::error;

pub(crate) struct MockCli {
    chain_id: u64,
    block_hash_channel_sender: UnboundedSender<[u8; 32]>,
    block_hash_channel_receiver: Mutex<UnboundedReceiver<[u8; 32]>>,
}


impl MockCli {
    pub(crate) fn new(chain_id: u64) -> Self {
        let (block_hash_channel_sender, block_hash_channel_receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            chain_id,
            block_hash_channel_sender,
            block_hash_channel_receiver: Mutex::new(block_hash_channel_receiver),
        }
    }

    fn deserialization_txn(&self, bytes: Vec<u8>) -> TxEnvelope {
        let txn = TxEnvelope::decode_2718(&mut bytes.as_ref()).unwrap();
        txn
    }

    fn payload_id_to_slice(&self, payload_id: &PayloadId) -> [u8; 32] {
        let mut block_id = [0u8; 32];
        for (id, byte) in payload_id.0.iter().enumerate() {
            block_id[id] = *byte;
        }
        block_id
    }

    fn slice_to_payload_id(&self, block_id: &[u8; 32]) -> PayloadId {
        let mut bytes = [0u8; 8];
        for i in 0..8 {
            bytes[i] = block_id[i];
        }
        PayloadId::new(bytes)
    }

    fn construct_bytes(
        &self,
        payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3,
    ) -> Vec<Vec<u8>> {
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        let mut payload = payload.clone();
        if payload.execution_payload.payload_inner.payload_inner.transactions.len() > 1 {
            payload.execution_payload.payload_inner.payload_inner.transactions.drain(1..).for_each(
                |txn_bytes| {
                    bytes.push(txn_bytes.to_vec());
                },
            );
        }
        bytes.insert(0, serde_json::to_vec(&payload).unwrap());
        bytes
    }

    fn payload_to_txns(&self, payload_id: PayloadId, payload: <EthEngineTypes as EngineTypes>::ExecutionPayloadV3) -> Vec<GTxn> {
        let bytes = self.construct_bytes(&payload);
        let eth_txns = payload.execution_payload.payload_inner.payload_inner.transactions;
        let mut gtxns = Vec::new();
        bytes.into_iter().enumerate().for_each(|(idx, bytes)| {
            let secs =
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24;
            if eth_txns.is_empty() {
                // when eth txns is empty, we need mock a txn
                let gtxn = GTxn::new(0, 0, 0, secs, self.chain_id, bytes);
                gtxns.push(gtxn);
                return;
            }
            let txn_bytes = eth_txns[idx].clone();
            let tx_envelope = self.deserialization_txn(txn_bytes.to_vec());
            tx_envelope.access_list();
            let x = tx_envelope.signature_hash().as_slice();
            let mut signature = [0u8; 64];

            signature[0..64].copy_from_slice(tx_envelope.signature_hash().as_slice());
            let gtxn = GTxn::new(
                tx_envelope.nonce(),
                tx_envelope.gas_limit() as u64,
                tx_envelope.gas_price().map(|x| x as u64).unwrap_or(0),
                secs, // hardcode 1day
                tx_envelope.chain_id().map(|x| x).unwrap_or(0),
                bytes,
            );
            info!("expiration time second is {:?}", secs);
            gtxns.push(gtxn);
        });
        info!(
            "Submit valid transactions: {:?}, block id {:?}, payload is {:?}",
            gtxns.len(),
            self.payload_id_to_slice(&payload_id),
            payload_id
        );
        gtxns
    }

    async fn get_new_payload_id(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: &PayloadAttributes,
    ) -> Option<PayloadId> {
        todo!()
    }

    pub async fn construct_payload(
        &self,
        fork_choice_state: ForkchoiceState,
    ) -> anyhow::Result<Vec<GTxn>> {
        todo!()
    }

    fn create_payload_attributes(parent_beacon_block_root: B256) -> EthPayloadAttributes {
        EthPayloadAttributes {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            prev_randao: B256::ZERO,
            suggested_fee_recipient: Address::ZERO,
            withdrawals: Some(Vec::new()),
            parent_beacon_block_root: Some(parent_beacon_block_root),
        }
    }
}

#[async_trait]
impl ExecutionApi for MockCli {
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch {
        todo!()
    }

    async fn send_ordered_block(&self, txns: Vec<GTxn>) {
        todo!()
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        todo!()
    }

    async fn commit_block_hash(&self, _block_ids: Vec<[u8; 32]>) {
        // do nothing for reth
    }
}