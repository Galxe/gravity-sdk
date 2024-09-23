use alloy::consensus::{Transaction, TxEnvelope};
use alloy::eips::eip2718::Decodable2718;
use alloy::primitives::B256;
use anyhow::Ok;
use gravity_sdk::consensus_execution_adapter::GravityConsensusEngine;
use gravity_sdk::{GTxn, GravityConsensusEngineInterface};
use jsonrpsee::core::Serialize;
use reth::payload;
use reth_ethereum_engine_primitives::{EthEngineTypes, ExecutionPayloadEnvelopeV3};
use reth_node_api::EngineTypes;
use reth_payload_builder::PayloadId;
use reth_primitives::Bytes;
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use serde_json::Serializer;
use std::collections::HashSet;
use std::marker::PhantomData;
use tracing::instrument::WithSubscriber;

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

    fn deserialization_txn(&self, bytes: Vec<u8>) -> TxEnvelope {
        let txn = TxEnvelope::decode_2718(&mut bytes.as_ref()).unwrap();
        txn
    }

    pub fn construct_bytes(
        &self,
        payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3,
    ) -> Vec<Vec<u8>> {
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        let mut payload = payload.clone();
        payload
            .execution_payload
            .payload_inner
            .payload_inner
            .transactions
            .drain(1..)
            .for_each(|txn_bytes| {
                bytes.push(txn_bytes.to_vec());
            });
        bytes.insert(0, bincode::serialize(&payload).unwrap());
        bytes
    }

    pub async fn submit_valid_transactions_v3(
        &mut self,
        payload_id: &PayloadId,
        payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3,
    ) {
        let payload = payload.clone();
        let bytes = self.construct_bytes(&payload);

        let txns: Vec<GTxn> = payload
            .execution_payload
            .payload_inner
            .payload_inner
            .transactions
            .iter()
            .zip(bytes.into_iter())
            .map(|(txn_bytes, bytes)| {
                let tx_envelope = self.deserialization_txn(txn_bytes.to_vec());
                tx_envelope.access_list();
                let x = tx_envelope.signature_hash().as_slice();
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
            })
            .collect();

        let mut block_id = [0u8; 32];
        block_id.copy_from_slice(&payload_id.0.as_slice());
        self.curret_block_id = Some(payload_id.clone());
        self.gcei_sender
            .send_valid_block_transactions(block_id, txns)
            .await;
    }

    pub async fn polling_order_blocks_v3(
        &mut self,
    ) -> anyhow::Result<<EthEngineTypes as EngineTypes>::ExecutionPayloadV3> {
        let mut res = self
            .gcei_sender
            .receive_ordered_block()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&res.0);
        let payload_id = PayloadId::new(bytes);
        if self.curret_block_id == Some(payload_id) {
            let mut payload: <EthEngineTypes as EngineTypes>::ExecutionPayloadV3 =
                bincode::deserialize(res.1[0].get_bytes()).map_err(|e| anyhow::anyhow!(e))?;
            res.1.drain(1..).for_each(|gtxn| {
                let txn_bytes = gtxn.get_bytes();
                let bytes: Bytes = Bytes::from(txn_bytes.clone());
                payload
                    .execution_payload
                    .payload_inner
                    .payload_inner
                    .transactions
                    .push(bytes);
            });
            return Ok(payload);
        }
        Err(anyhow::anyhow!("Block id is not equal"))
    }

    pub async fn submit_compute_res(&self, compute_res: B256) -> anyhow::Result<()> {
        if self.curret_block_id.is_none() {
            return Err(anyhow::anyhow!("Block id is none"));
        }
        let mut block_id = [0u8; 32];
        block_id.copy_from_slice(&self.curret_block_id.unwrap().0.as_slice());

        let mut compute_res_bytes = [0u8; 32];
        compute_res_bytes.copy_from_slice(&compute_res.as_slice());
        self.gcei_sender
            .send_compute_res(block_id, compute_res_bytes)
            .await;
        Ok(())
    }

    pub async fn polling_submit_blocks(&mut self) -> anyhow::Result<()> {
        let payload_id_bytes = self.gcei_sender.receive_commit_block_ids().await.map_err(|e| anyhow::anyhow!(e))?;
        let payload: HashSet<PayloadId> = payload_id_bytes.into_iter().map(|x| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&x);
            PayloadId::new(bytes)
        }).collect();
        if self.curret_block_id.is_some() && payload.contains(&self.curret_block_id.unwrap()) {
            return Ok(());
        }
        Err(anyhow::anyhow!("Block id is not equal"))
    }

    pub fn submit_max_persistence_block_id(&self) {
        let mut block_id = [0u8; 32];
        block_id.copy_from_slice(&self.curret_block_id.unwrap().0.as_slice());
        self.gcei_sender.send_persistent_block_id(block_id);
    }
}
