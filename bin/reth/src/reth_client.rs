use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId};
use anyhow::Context;
use api::ExecutionApi;
use api_types::{BlockBatch, BlockHashState, GTxn};
use jsonrpsee::core::async_trait;
use reth::api::EngineTypes;
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_node_ethereum::EthereumNode;
use reth_primitives::Bytes;
use reth_provider::providers::BlockchainProvider2;
use reth_provider::{BlockReader, BlockReaderIdExt};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use revm_primitives::ruint::aliases::U256;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::info;
use tracing::log::error;

pub(crate) struct RethCli<T> {
    engine_api_client: T,
    chain_id: u64,
    block_hash_channel_sender: UnboundedSender<[u8; 32]>,
    block_hash_channel_receiver: Mutex<UnboundedReceiver<[u8; 32]>>,
    provider: Provider,
}

type Provider = BlockchainProvider2<
    reth_node_api::NodeTypesWithDBAdapter<EthereumNode, Arc<reth_db::DatabaseEnv>>,
>;

impl<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> RethCli<T> {
    pub(crate) fn new(client: T, chain_id: u64, provider: Provider) -> Self {
        let (block_hash_channel_sender, block_hash_channel_receiver) =
            tokio::sync::mpsc::unbounded_channel();
        Self {
            engine_api_client: client,
            chain_id,
            block_hash_channel_sender,
            block_hash_channel_receiver: Mutex::new(block_hash_channel_receiver),
            provider,
        }
    }

    async fn update_fork_choice(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: EthPayloadAttributes,
    ) -> anyhow::Result<ForkchoiceUpdated> {
        info!("update_fork_choice with payload attributes {:?}", payload_attributes);
        let response = <T as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v3(
            &self.engine_api_client,
            fork_choice_state,
            Some(payload_attributes),
        )
        .await
        .context("Failed to update fork choice")?;
        info!("Got response: {:?}", response);
        Ok(response)
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
                tx_envelope.chain_id().map(|x| x).unwrap_or(114),// 0 is not allowed and would trigger crash
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
        let updated_res =
            self.update_fork_choice(fork_choice_state.clone(), payload_attributes.clone()).await;
        info!("Got update res: {:?}", updated_res);
        match updated_res {
            Ok(updated) => {
                if updated.payload_id.is_none() {
                    error!("Payload ID is none");

                    return None;
                }
                Some(updated.payload_id.unwrap())
            }
            Err(e) => {
                error!("Failed to update fork choice: {}", e);
                None
            }
        }
    }

    pub async fn construct_payload(
        &self,
        fork_choice_state: ForkchoiceState,
    ) -> anyhow::Result<Vec<GTxn>> {
        let parent_beacon_block_root = fork_choice_state.head_block_hash;
        let payload_attributes = Self::create_payload_attributes(parent_beacon_block_root);
        // update ForkchoiceState and get payload_id
        let mut payload_id = PayloadId::new([0; 8]);
        match self.get_new_payload_id(fork_choice_state, &payload_attributes).await {
            Some(pid) => {
                payload_id = pid;
            }
            None => {
                panic!("get None payload id");
            }
        };
        // try to get payload
        let payload = <T as EngineApiClient<EthEngineTypes>>::get_payload_v3(
            &self.engine_api_client,
            payload_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        info!("Got payload: {:?}", payload);
        Ok(self.payload_to_txns(payload_id, payload))
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
impl<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> ExecutionApi for RethCli<T> {
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch {
        let fork_choice_state = ForkchoiceState {
            head_block_hash: B256::new(state_block_hash.head_hash),
            safe_block_hash: B256::new(state_block_hash.safe_hash),
            finalized_block_hash: B256::new(state_block_hash.finalized_hash),
        };
        let mut payload_id = PayloadId::new([0; 8]);
        let payload_attr = Self::create_payload_attributes(fork_choice_state.head_block_hash);
        match self.get_new_payload_id(fork_choice_state, &payload_attr).await {
            Some(pid) => {
                payload_id = pid;
            }
            None => {
                panic!("payload id is none");
            }
        };
        // try to get payload
        let payload = <T as EngineApiClient<EthEngineTypes>>::get_payload_v3(
            &self.engine_api_client,
            payload_id,
        )
        .await
        .expect("Failed to get payload");
        info!("Got payload: {:?}", payload);
        let _ = tokio::time::sleep(Duration::from_secs(2)).await;
        let block_bytes = payload.execution_payload.payload_inner.payload_inner.block_hash;
        info!(
            "send block batch with hash {:?}, block number {}",
            block_bytes, payload.execution_payload.payload_inner.payload_inner.block_number
        );
        let mut block_hash = [0u8; 32];
        block_hash.copy_from_slice(block_bytes.as_slice());
        let txns = self.payload_to_txns(payload_id, payload);
        BlockBatch { txns, block_hash }
    }

    async fn send_ordered_block(&self, txns: Vec<GTxn>) {
        let mut payload: <EthEngineTypes as EngineTypes>::ExecutionPayloadV3 =
            serde_json::from_slice(txns[0].get_bytes()).expect("Failed to deserialize payload");
        if txns.len() > 1 {
            txns.iter().skip(1).for_each(|gtxn| {
                let txn_bytes = gtxn.get_bytes();
                let bytes: Bytes = Bytes::from(txn_bytes.clone());
                payload.execution_payload.payload_inner.payload_inner.transactions.push(bytes);
            });
        }
        let parent_hash = payload.execution_payload.payload_inner.payload_inner.parent_hash;
        let block_number = payload.execution_payload.payload_inner.payload_inner.block_number;
        let payload_status = <T as EngineApiClient<EthEngineTypes>>::new_payload_v3(
            &self.engine_api_client,
            payload.execution_payload,
            Vec::new(),
            parent_hash,
        )
        .await
        .expect("Failed to submit payload");
        // 3. submit compute res
        if payload_status.latest_valid_hash.is_none() {
            panic!("payload status latest valid hash is none");
        }
        info!(
            "get block hash in payload statue {:?}, block number {}",
            payload_status, block_number
        );
        let mut hash = [0u8; 32];
        hash.copy_from_slice(payload_status.latest_valid_hash.unwrap().as_slice());
        self.block_hash_channel_sender.send(hash).expect("send block hash failed");
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        let mut receiver = self.block_hash_channel_receiver.lock().await;
        let block_hash = receiver.recv().await.expect("recv block hash failed");
        block_hash
    }

    async fn commit_block_hash(&self, _block_ids: Vec<[u8; 32]>) {
        // do nothing for reth
    }

    fn latest_block_number(&self) -> u64 {
        match self.provider.block_by_number_or_tag(BlockNumberOrTag::Latest).unwrap() {
            Some(block) => block.number, // The genesis block has a number of zero;
            None => 0,
        }
    }

    async fn recover_ordered_block(&self, txns: Vec<GTxn>, res: [u8; 32]) {
        let mut payload: <EthEngineTypes as EngineTypes>::ExecutionPayloadV3 =
            serde_json::from_slice(txns[0].get_bytes()).expect("Failed to deserialize payload");
        if txns.len() > 1 {
            txns.iter().skip(1).for_each(|gtxn| {
                let txn_bytes = gtxn.get_bytes();
                let bytes: Bytes = Bytes::from(txn_bytes.clone());
                payload.execution_payload.payload_inner.payload_inner.transactions.push(bytes);
            });
        }
        let parent_hash = payload.execution_payload.payload_inner.payload_inner.parent_hash;
        let payload_status = <T as EngineApiClient<EthEngineTypes>>::new_payload_v3(
            &self.engine_api_client,
            payload.execution_payload,
            Vec::new(),
            parent_hash,
        )
        .await;
        match payload_status {
            Ok(payload_status) => {
                // 3. submit compute res
                if payload_status.latest_valid_hash.is_none() {
                    panic!("payload status latest valid hash is none");
                }
                info!("get block hash in payload statue{:?}", payload_status);
                let mut hash = [0u8; 32];
                hash.copy_from_slice(payload_status.latest_valid_hash.unwrap().as_slice());
                assert!(res == hash);
            }
            Err(e) => panic!("new payload error {}", e),
        }
    }
}
