use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::{Address, B256};
use alloy_rlp::Encodable;
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV3, ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId, PayloadStatusEnum
};
use alloy_signer::k256::sha2;
use anyhow::Context;
use api::ExecutionApi;
use api_types::{BlockBatch, BlockHashState, GTxn};
use jsonrpsee::core::async_trait;
use reth::api::EngineTypes;
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_node_ethereum::EthereumNode;
use reth_payload_builder::EthBuiltPayload;
use reth_primitives::{Block, Bytes};
use reth_provider::providers::BlockchainProvider2;
use reth_provider::{BlockNumReader, BlockReaderIdExt, DatabaseProviderFactory};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use revm_primitives::ruint::aliases::U256;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::info;
use tracing::log::error;

pub(crate) struct RethCli<T> {
    engine_api_client: T,
    chain_id: u64,
    head_block: Mutex<Option<B256>>,
    block_hash_channel_sender: UnboundedSender<[u8; 32]>,
    block_hash_channel_receiver: Mutex<UnboundedReceiver<[u8; 32]>>,
    provider: Provider,
}

type Provider = BlockchainProvider2<
    reth_node_api::NodeTypesWithDBAdapter<EthereumNode, Arc<reth_db::DatabaseEnv>>,
>;

/// Generates the payload id for the configured payload from the [`PayloadAttributes`].
///
/// Returns an 8-byte identifier by hashing the payload components with sha256 hash.
fn payload_id(parent: &B256, attributes: &PayloadAttributes) -> PayloadId {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(parent.as_slice());
    hasher.update(&attributes.timestamp.to_be_bytes()[..]);
    hasher.update(attributes.prev_randao.as_slice());
    hasher.update(attributes.suggested_fee_recipient.as_slice());
    if let Some(withdrawals) = &attributes.withdrawals {
        let mut buf = Vec::new();
        withdrawals.encode(&mut buf);
        hasher.update(buf);
    }

    if let Some(parent_beacon_block) = attributes.parent_beacon_block_root {
        hasher.update(parent_beacon_block);
    }

    let out = hasher.finalize();
    PayloadId::new(out.as_slice()[..8].try_into().expect("sufficient length"))
}

impl<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> RethCli<T> {
    pub(crate) fn new(client: T, chain_id: u64, provider: Provider) -> Self {
        let (block_hash_channel_sender, block_hash_channel_receiver) =
            tokio::sync::mpsc::unbounded_channel();
        Self {
            engine_api_client: client,
            chain_id,
            head_block: Mutex::new(None),
            block_hash_channel_sender,
            block_hash_channel_receiver: Mutex::new(block_hash_channel_receiver),
            provider,
        }
    }

    async fn update_fork_choice(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<EthPayloadAttributes>,
    ) -> anyhow::Result<ForkchoiceUpdated> {
        info!("update_fork_choice with payload attributes {:?}", payload_attributes);
        let response = <T as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v3(
            &self.engine_api_client,
            fork_choice_state,
            payload_attributes,
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

    fn construct_bytes(
        &self,
        payload: &<EthEngineTypes as EngineTypes>::ExecutionPayloadV3,
    ) -> Vec<Vec<u8>> {
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        let mut payload = payload.clone();
        info!(
            "Transaction num returned by reth is {:?}",
            payload.execution_payload.payload_inner.payload_inner.transactions.len()
        );
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
                secs,                                             // hardcode 1day
                tx_envelope.chain_id().map(|x| x).unwrap_or(114), // 0 is not allowed and would trigger crash
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
        let updated_res = self
            .update_fork_choice(fork_choice_state.clone(), Some(payload_attributes.clone()))
            .await;
        info!("Got update res: {:?}", updated_res);
        match updated_res {
            Ok(updated) => {
                if updated.payload_id.is_none() {
                    error!("Payload ID is none, fork_choice_state {:?}", fork_choice_state);
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
                info!("get new payload id {}", payload_id);
            }
            None => {
                panic!("payload id is none");
            }
        };
        let get_payload_sleep_seconds =
            std::env::var("GET_PAYLOAD_SLEEP_SECOND").map(|s| s.parse().unwrap()).unwrap_or(1);
        let _ = tokio::time::sleep(Duration::from_secs(get_payload_sleep_seconds)).await;
        // try to get payload
        let payload = <T as EngineApiClient<EthEngineTypes>>::get_payload_v3(
            &self.engine_api_client,
            payload_id,
        )
        .await
        .expect("Failed to get payload");
        info!("Got payload: {:?}", payload);
        let mut mut_ref = self.head_block.lock().await;
        *mut_ref = Some(payload.execution_payload.payload_inner.payload_inner.block_hash);

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

    async fn commit_block_hash(&self, block_ids: Vec<[u8; 32]>) {
        let head_id = self.head_block.lock().await.take();
        if head_id.is_some() {
            // Don't commit for leader
            return;
        }
        for block_id in block_ids {
            let commit_id = B256::new(block_id);
            let fork_choice_state = ForkchoiceState {
                head_block_hash: commit_id,
                safe_block_hash: commit_id,
                finalized_block_hash: commit_id,
            };
            let res = self.update_fork_choice(fork_choice_state, None).await;
            info!("commit block hash {:?} with res {:?}", block_id, res);
            match res.as_ref().unwrap().payload_status.status {
                PayloadStatusEnum::Valid => {
                    info!("commit success {:?}", res);
                }
                PayloadStatusEnum::Invalid { .. } => {
                    info!("commit invalid {:?}", res);
                }
                PayloadStatusEnum::Syncing => {
                    info!("commit syncing {:?}", res);
                }
                PayloadStatusEnum::Accepted => {
                    info!("commit accepted {:?}", res);
                }
            }
        }
    }

    fn latest_block_number(&self) -> u64 {
        match self.provider.block_by_number_or_tag(BlockNumberOrTag::Latest).unwrap() {
            Some(block) => block.number, // The genesis block has a number of zero;
            None => 0,
        }
    }

    fn finalized_block_number(&self) -> u64 {
        match self.provider.database_provider_ro().unwrap().last_block_number() {
            Ok(block_number) => {
                return block_number;
            },
            Err(e) => {
                error!("finalized_block_number error {}", e);
                return 0;
            } 
        }
    }

    async fn recover_ordered_block(&self, block_batch: BlockBatch) {
        let txns = &block_batch.txns;
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
                if payload_status.status != PayloadStatusEnum::Valid {
                    panic!("payload status status is {}", payload_status.status);
                }
                // 3. submit compute res
                if payload_status.latest_valid_hash.is_none() {
                    panic!("payload status latest valid hash is none");
                }
                info!("recover success {:?}", payload_status);
                let mut hash = [0u8; 32];
                hash.copy_from_slice(payload_status.latest_valid_hash.unwrap().as_slice());
                assert!(block_batch.block_hash == hash);
            }
            Err(e) => panic!("new payload error {}", e),
        }
    }

    async fn recover_execution_blocks(&self, blocks: Vec<Block>) {
        for block in blocks {   
            let withdrawals = match block.withdrawals.clone() {
                Some(withdrawals) => Some(withdrawals.into_inner()),
                None => None,
            };
            let payload_attr = PayloadAttributes {
                timestamp: block.header.timestamp,
                prev_randao: block.header.mix_hash,
                suggested_fee_recipient: block.header.beneficiary,
                withdrawals,
                parent_beacon_block_root: block.header.parent_beacon_block_root.clone(),
            };
            let payload_id = payload_id(&block.header.parent_hash, &payload_attr);
            info!("recover execution block payload id {}", payload_id);
            assert!(block.header.base_fee_per_gas.is_some());
            let base_fee_per_gas = block.header.base_fee_per_gas.unwrap();
            let total_fees = U256::from(base_fee_per_gas) * U256::from(block.header.gas_used);
            let payload_builder = EthBuiltPayload::new(payload_id, block.seal_slow(), total_fees, None);
            let payload: ExecutionPayloadEnvelopeV3 = payload_builder.into();
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
                    if payload_status.status != PayloadStatusEnum::Valid {
                        panic!("payload status status is {}", payload_status.status);
                    }
                    // 3. submit compute res
                    if payload_status.latest_valid_hash.is_none() {
                        panic!("payload status latest valid hash is none");
                    }
                    info!("recover reth block success {:?}", payload_status);
                }
                Err(e) => panic!("new payload error {}", e),
            }
        }
    }

    fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> Vec<Block> {
        let mut blocks = vec![];
        for block_number in start_block_number..end_block_number {
            match self.provider.block_by_number_or_tag(BlockNumberOrTag::Number(block_number)) {
                Ok(block) => {
                    assert!(block.is_some());
                    let block = block.unwrap();
                    blocks.push(block);
                }
                Err(e) => panic!("get_blocks_by_range error {}", e),
            }
        }
        blocks
    }
}
