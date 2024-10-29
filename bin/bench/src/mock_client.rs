use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId};
use anyhow::Context;
use api_types::{BlockBatch, BlockHashState, ExecutionApi, GTxn};
use jsonrpsee::core::async_trait;
use rand::Rng;
use reth::api::EngineTypes;
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_primitives::Bytes;
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use revm::primitives::{alloy_primitives::U160, Env, SpecId, TransactTo, TxEnv, U256};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::info;
use tracing::log::error;

pub(crate) struct MockCli {
    chain_id: u64,
    block_hash_channel_sender: UnboundedSender<[u8; 32]>,
    block_hash_channel_receiver: Mutex<UnboundedReceiver<[u8; 32]>>,
}

const TRANSFER_GAS_LIMIT: u64 = 21_000;
// skip precompile address
const MINER_ADDRESS: usize = 999;
const START_ADDRESS: usize = 1000;

fn get_account_idx(num_eoa: usize, hot_start_idx: usize, hot_ratio: f64) -> usize {
    if hot_ratio <= 0.0 {
        // Uniform workload
        rand::random::<usize>() % num_eoa
    } else if rand::thread_rng().gen_range(0.0..1.0) < hot_ratio {
        // Access hot
        hot_start_idx + rand::random::<usize>() % (num_eoa - hot_start_idx)
    } else {
        rand::random::<usize>() % (num_eoa - hot_start_idx)
    }
}

impl MockCli {
    pub(crate) fn new(chain_id: u64) -> Self {
        let (block_hash_channel_sender, block_hash_channel_receiver) =
            tokio::sync::mpsc::unbounded_channel();
        Self {
            chain_id,
            block_hash_channel_sender,
            block_hash_channel_receiver: Mutex::new(block_hash_channel_receiver),
        }
    }

    fn construct_reth_txn(&self) -> Vec<TxEnv> {
        let num_eoa = std::env::var("NUM_EOA").map(|s| s.parse().unwrap()).unwrap_or(0);
        let hot_ratio = std::env::var("HOT_RATIO").map(|s| s.parse().unwrap()).unwrap_or(0.0);
        let hot_start_idx = START_ADDRESS + (num_eoa as f64 * 0.9) as usize;
        let txn_nums = std::env::var("BLOCK_TXN_NUMS").map(|s| s.parse().unwrap()).unwrap_or(1000);
        (0..txn_nums)
            .map(|_| {
                let from = Address::from(U160::from(
                    START_ADDRESS + get_account_idx(num_eoa, hot_start_idx, hot_ratio),
                ));
                let to = Address::from(U160::from(
                    START_ADDRESS + get_account_idx(num_eoa, hot_start_idx, hot_ratio),
                ));
                TxEnv {
                    caller: from,
                    transact_to: TransactTo::Call(to),
                    value: U256::from(1),
                    gas_limit: TRANSFER_GAS_LIMIT,
                    gas_price: U256::from(1),
                    ..TxEnv::default()
                }
            })
            .collect::<Vec<_>>()
    }

    fn produce_gtxns(&self) -> Vec<GTxn> {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24;

        self.construct_reth_txn()
            .iter_mut()
            .map(|txn| GTxn {
                sequence_number: txn.nonce.map(|x| x as u64).unwrap_or(0),
                max_gas_amount: txn.gas_limit,
                gas_unit_price: txn.gas_price,
                expiration_timestamp_secs: secs,
                chain_id: txn.chain_id.unwrap(),
                txn_bytes: bincode::serialize(txn).expect("failed to serialize"),
            })
            .collect::<Vec<_>>()
    }

    fn transform_to_reth_txn(gtxn: GTxn) -> TxEnv {
        bincode::deserialize::<TxEnv>(&gtxn.txn_bytes).expect("failed to deserialize")
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

    fn payload_to_txns(
        &self,
        payload_id: PayloadId,
        payload: <EthEngineTypes as EngineTypes>::ExecutionPayloadV3,
    ) -> Vec<GTxn> {
        let bytes = self.construct_bytes(&payload);
        let eth_txns = payload.execution_payload.payload_inner.payload_inner.transactions;
        let mut gtxns = Vec::new();
        bytes.into_iter().enumerate().for_each(|(idx, bytes)| {
            let secs =
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60 * 60 * 24;
            if eth_txns.is_empty() {
                // when eth txns is empty, we need mock a txn
                let gtxn = GTxn::new(0, 0, U256::from(0), secs, self.chain_id, bytes);
                gtxns.push(gtxn);
                return;
            }
            let txn_bytes = eth_txns[idx].clone();
            let tx_envelope = self.deserialization_txn(txn_bytes.to_vec());
            tx_envelope.access_list();
            let gtxn = GTxn::new(
                tx_envelope.nonce(),
                tx_envelope.gas_limit() as u64,
                U256::from(tx_envelope.gas_price().map(|x| x as u64).unwrap_or(0)),
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
        BlockBatch { txns: self.produce_gtxns(), block_hash: [0; 32] }
    }

    async fn send_ordered_block(&self, txns: Vec<GTxn>) {
        let _reth_txns = txns
            .iter()
            .map(|gtxn| MockCli::transform_to_reth_txn(gtxn.clone()))
            .collect::<Vec<_>>();
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        [0; 32]
    }

    async fn commit_block_hash(&self, _block_ids: Vec<[u8; 32]>) {
        // do nothing for reth
    }
}
