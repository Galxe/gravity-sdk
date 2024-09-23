use std::marker::PhantomData;
use alloy::consensus::{Transaction, TxEnvelope};
use alloy::eips::eip2718::Decodable2718;
use alloy::primitives::B256;
use alloy::rlp::Bytes;
use anyhow::Ok;
use jsonrpsee::core::Serialize;
use reth::payload;
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_node_api::EngineTypes;
use reth_payload_builder::PayloadId;
use serde_json::Serializer;
use tracing::instrument::WithSubscriber;
use gravity_sdk::{GTxn, GravityConsensusEngine, GravityConsensusEngineInterface};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};

pub struct GCEISender {
    curret_block_id: Option<PayloadId>,
    gcei_sender: GravityConsensusEngine,
}

impl GCEISender {
    pub fn new() -> Self {
        Self {
            curret_block_id: None,
            gcei_sender: GravityConsensusEngine::init(),
        }
    }


    fn deserialization_txn(&self, bytes: &Bytes) -> TxEnvelope {
        let bytes: Vec<u8> = bytes.to_vec();
        let txn = TxEnvelope::decode_2718(&mut bytes.as_ref()).unwrap();
        txn
    }

    pub fn construct_bytes(&self, payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3) -> Vec<Vec<u8>> {
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        let mut payload = payload.clone();
        payload.execution_payload.payload_inner.payload_inner.transactions.drain(1..).for_each(|txn_bytes| {
            bytes.push(txn_bytes.to_vec());
        });
        bytes.insert(0, bincode::serialize(&payload).unwrap());
        bytes
    }

    pub fn submit_valid_transactions_v3(&mut self, payload_id: &PayloadId, payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3) {
        let payload = payload.clone();
        let bytes = self.construct_bytes(&payload);
        
        let txns: Vec<GTxn> = payload.execution_payload.payload_inner.payload_inner.transactions.iter().zip(bytes.into_iter()).map(
            |(txn_bytes, bytes)| {
                let tx_envelope = self.deserialization_txn(txn_bytes);
                tx_envelope.access_list();
                let x =
                    tx_envelope.signature_hash().as_slice();
                let mut signature = [0u8; 64];
        
                signature[0..64].copy_from_slice(tx_envelope.signature_hash().as_slice());
                GTxn::new(
                    tx_envelope.nonce(),
                    tx_envelope.gas_limit() as u64,
                    tx_envelope.gas_price().map(|x| x as u64).unwrap_or(0),
                    60 * 60 * 24, // hardcode 1day
                    tx_envelope.chain_id().map(|x| x).unwrap_or(0) as u8,
                    bytes,
                )
            }
        ).collect();

        let mut block_id = [0u8; 32];
        block_id.copy_from_slice(&payload_id.0.as_slice());
        self.curret_block_id = Some(payload_id.clone());
        self.gcei_sender.send_valid_transactions(block_id, txns);
    }

    pub fn polling_order_blocks(&self) -> eyre::Result<<EthEngineTypes as EngineTypes>::ExecutionPayloadV3> {
        let res = self.gcei_sender.receive_ordered_block()?;
        let payload_id = PayloadId::from_slice(&res.block_id);
        if self.curret_block_id == Some(payload_id) {
            Ok(())
        }
        Err(())
    }

    pub fn submit_compute_res(&self, compute_res: B256) -> eyre::Result<()> {
        if self.curret_block_id.is_none() {
            return Err(());
        }
        let mut block_id = [0u8; 32];
        block_id.copy_from_slice(&self.curret_block_id.unwrap().0.as_slice());
        self.gcei_sender.send_compute_res(block_id, res);
        Ok(())
    }

    pub fn polling_submit_blocks(&self) -> eyre::Result<()> {
        let payload_id = self.gcei_sender.receive_commit_block_ids();
        if self.curret_block_id == Some(payload_id) {
            Ok(())
        }
        Err(())
    }

    pub fn submit_max_persistence_block_id(&self) {
        let mut block_id = [0u8; 32];
        block_id.copy_from_slice(&self.curret_block_id.unwrap().0.as_slice());
        self.gcei_sender.send_persistent_block_id(block_id);
    }

}