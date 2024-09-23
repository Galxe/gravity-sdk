use alloy::consensus::{Transaction, TxEnvelope};
use alloy::eips::eip2718::Decodable2718;
use alloy::primitives::B256;
use anyhow::Ok;
use gravity_sdk::{GTxn, GravityConsensusEngineInterface, NodeConfig};
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_node_api::EngineTypes;
use reth_payload_builder::PayloadId;
use reth_primitives::Bytes;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct GCEISender<T: GravityConsensusEngineInterface> {
    curret_block_id: Option<PayloadId>,
    gcei_sender: T,
    chain_id: u64,
}

impl<T: GravityConsensusEngineInterface> GCEISender<T> {
    pub fn new(chain_id: u64) -> Self {
        let node_config = NodeConfig::load_from_path("/Users/jingyue/projects/gravity-sdk/node1/genesis/validator.yaml").unwrap();
        Self {
            curret_block_id: None,
            gcei_sender: T::init(node_config),
            chain_id: chain_id,
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
        if payload
            .execution_payload
            .payload_inner
            .payload_inner
            .transactions
            .len()
            > 1
        {
            payload
                .execution_payload
                .payload_inner
                .payload_inner
                .transactions
                .drain(1..)
                .for_each(|txn_bytes| {
                    bytes.push(txn_bytes.to_vec());
                });
        }
        bytes.insert(0, serde_json::to_vec(&payload).unwrap());
        bytes
    }

    pub async fn submit_valid_transactions_v3(
        &mut self,
        payload_id: &PayloadId,
        payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3,
    ) {
        let payload = payload.clone();
        let bytes = self.construct_bytes(&payload);
        let eth_txns = payload
            .execution_payload
            .payload_inner
            .payload_inner
            .transactions;
        let mut gtxns = Vec::new();
        bytes.into_iter().enumerate().for_each(|(idx, bytes)| {
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24;
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
            println!("expiration time second is {:?}", secs);
            gtxns.push(gtxn);
        });
        println!("Submit valid transactions: {:?}, block id {:?}, payload is {:?}", gtxns.len(), self.payload_id_to_slice(&payload_id), payload_id);
        self.curret_block_id = Some(payload_id.clone());
        self.gcei_sender
            .send_valid_block_transactions(self.payload_id_to_slice(&payload_id), gtxns)
            .await
            .expect("TODO: panic message");
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

    pub async fn polling_order_blocks_v3(
        &mut self,
    ) -> anyhow::Result<<EthEngineTypes as EngineTypes>::ExecutionPayloadV3> {
        let mut res = self
            .gcei_sender
            .receive_ordered_block()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let payload_id = self.slice_to_payload_id(&res.0);
        println!("verify the polling_order_block payload_id {:?}, curret_block_id is {:?}, is same {:?}", payload_id, self.curret_block_id, Some(payload_id) == self.curret_block_id);
        if self.curret_block_id == Some(payload_id) {
            let mut payload: <EthEngineTypes as EngineTypes>::ExecutionPayloadV3 =
                serde_json::from_slice(res.1[0].get_bytes()).map_err(|e| anyhow::anyhow!(e))?;
            if res.1.len() > 1 {
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
            }
            return Ok(payload);
        }
        Err(anyhow::anyhow!("Block id is not equal"))
    }

    pub async fn submit_compute_res(&self, compute_res: B256) -> anyhow::Result<()> {
        if self.curret_block_id.is_none() {
            return Err(anyhow::anyhow!("Block id is none"));
        }
        println!("current block id is not none");
        let block_id = self.payload_id_to_slice(&self.curret_block_id.unwrap());
        println!("submit compute res block id is {:?}", block_id);

        let mut compute_res_bytes = [0u8; 32];
        compute_res_bytes.copy_from_slice(&compute_res.as_slice());
        self.gcei_sender
            .send_compute_res(block_id, compute_res_bytes)
            .await.map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn polling_submit_blocks(&mut self) -> anyhow::Result<()> {
        let payload_id_bytes = self
            .gcei_sender
            .receive_commit_block_ids()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let payload: HashSet<PayloadId> = payload_id_bytes.clone()
            .into_iter()
            .map(|x| self.slice_to_payload_id(&x))
            .collect();
        println!("the polling submit blocks payload is {:?}, current is {:?}, block ids {:?}", payload, self.curret_block_id, payload_id_bytes);
        if self.curret_block_id.is_some() && payload.contains(&self.curret_block_id.unwrap()) {
            return Ok(());
        }
        Err(anyhow::anyhow!("Block id is not equal"))
    }

    pub async fn submit_max_persistence_block_id(&self) {
        let block_id = self.payload_id_to_slice(&self.curret_block_id.unwrap());
        self.gcei_sender.send_persistent_block_id(block_id).await.expect("send max persistence block id failed");
    }
}
