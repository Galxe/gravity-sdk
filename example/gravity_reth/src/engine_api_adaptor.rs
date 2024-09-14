use std::marker::PhantomData;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::HttpClient;
use reth_ethereum_engine_primitives::ExecutionPayloadV1;
use reth_node_api::EngineTypes;
use reth_node_core::primitives::{BlockHash, B256, U64};
use reth_node_core::rpc::types::engine::{ClientVersionV1, ExecutionPayloadBodiesV1, ExecutionPayloadBodiesV2, ExecutionPayloadInputV2, ExecutionPayloadV4, ForkchoiceState, ForkchoiceUpdated, PayloadId, PayloadStatus, TransitionConfiguration};
use reth_node_core::rpc::types::{BlobAndProofV1, ExecutionPayloadV3};
use reth_rpc::EthApi;
use reth_rpc_api::{EngineApiClient, EngineEthApiClient, NetApiClient, Web3ApiClient};

pub struct EngineApiServer<Engine: EngineTypes> {
    http_client: HttpClient,
    _phantom: PhantomData<Engine>,
}

impl<Engine: EngineTypes> EngineApiServer<Engine> {

    pub fn new(addr: String) -> Self {
        let http_client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(format!("{}", addr))
            .unwrap();
        Self {
            http_client,
            _phantom: PhantomData,
        }
    }

    pub async fn new_payload_v1(&self, payload: ExecutionPayloadV1) -> PayloadStatus {
        EngineApiClient::<Engine>::new_payload_v1(&self.http_client, payload).await.unwrap()
    }

    pub async fn new_payload_v2(&self, payload: ExecutionPayloadInputV2) -> PayloadStatus {
        EngineApiClient::<Engine>::new_payload_v2(&self.http_client, payload).await.unwrap()
    }

    pub async fn new_payload_v3(&self, payload: ExecutionPayloadV3, versioned_hashes: Vec<B256>, parent_beacon_block_root: B256) -> PayloadStatus {
        EngineApiClient::<Engine>::new_payload_v3(&self.http_client, payload, versioned_hashes, parent_beacon_block_root).await.unwrap()
    }

    pub async fn new_payload_v4(&self, payload: ExecutionPayloadV4, versioned_hashes: Vec<B256>, parent_beacon_block_root: B256) -> PayloadStatus {
        EngineApiClient::<Engine>::new_payload_v4(&self.http_client, payload, versioned_hashes, parent_beacon_block_root).await.unwrap()
    }

    pub async fn fork_choice_updated_v1(&self, fork_choice_state: ForkchoiceState, payload_attributes: Option<Engine::PayloadAttributes>) -> ForkchoiceUpdated {
        EngineApiClient::<Engine>::fork_choice_updated_v1(&self.http_client, fork_choice_state, payload_attributes).await.unwrap()
    }

    pub async fn fork_choice_updated_v2(&self, fork_choice_state: ForkchoiceState, payload_attributes: Option<Engine::PayloadAttributes>) -> ForkchoiceUpdated {
        EngineApiClient::<Engine>::fork_choice_updated_v2(&self.http_client, fork_choice_state, payload_attributes).await.unwrap()
    }

    pub async fn fork_choice_updated_v3(&self, fork_choice_state: ForkchoiceState, payload_attributes: Option<Engine::PayloadAttributes>) -> ForkchoiceUpdated {
        EngineApiClient::<Engine>::fork_choice_updated_v3(&self.http_client, fork_choice_state, payload_attributes).await.unwrap()
    }

    pub async fn get_payload_v1(&self, payload_id: PayloadId) -> Engine::ExecutionPayloadV1 {
        EngineApiClient::<Engine>::get_payload_v1(&self.http_client, payload_id).await.unwrap()
    }

    pub async fn get_payload_v2(&self, payload_id: PayloadId) -> Engine::ExecutionPayloadV2 {
        EngineApiClient::<Engine>::get_payload_v2(&self.http_client, payload_id).await.unwrap()
    }

    pub async fn get_payload_v3(&self, payload_id: PayloadId) -> Engine::ExecutionPayloadV3 {
        EngineApiClient::<Engine>::get_payload_v3(&self.http_client, payload_id).await.unwrap()
    }

    pub async fn get_payload_v4(&self, payload_id: PayloadId) -> Engine::ExecutionPayloadV4 {
        EngineApiClient::<Engine>::get_payload_v4(&self.http_client, payload_id).await.unwrap()
    }

    pub async fn get_payload_bodies_by_hash_v1(&self, block_hashes: Vec<BlockHash>) -> ExecutionPayloadBodiesV1 {
        EngineApiClient::<Engine>::get_payload_bodies_by_hash_v1(&self.http_client, block_hashes).await.unwrap()
    }

    pub async fn get_payload_bodies_by_hash_v2(&self, block_hashes: Vec<BlockHash>) -> ExecutionPayloadBodiesV2 {
        EngineApiClient::<Engine>::get_payload_bodies_by_hash_v2(&self.http_client, block_hashes).await.unwrap()
    }

    pub async fn get_payload_bodies_by_range_v1(&self, start: U64, count: U64) -> ExecutionPayloadBodiesV1 {
        EngineApiClient::<Engine>::get_payload_bodies_by_range_v1(&self.http_client, start, count).await.unwrap()
    }

    pub async fn get_payload_bodies_by_range_v2(&self, start: U64, count: U64) -> ExecutionPayloadBodiesV2 {
        EngineApiClient::<Engine>::get_payload_bodies_by_range_v2(&self.http_client, start, count).await.unwrap()
    }

    pub async fn exchange_transition_configuration(&self, transition_configuration: TransitionConfiguration) -> TransitionConfiguration {
        EngineApiClient::<Engine>::exchange_transition_configuration(&self.http_client, transition_configuration).await.unwrap()
    }

    pub async fn get_client_version_v1(&self, client_version: ClientVersionV1) -> Vec<ClientVersionV1> {
        EngineApiClient::<Engine>::get_client_version_v1(&self.http_client, client_version).await.unwrap()
    }

    pub async fn exchange_capabilities(&self, capabilities: Vec<String>) -> Vec<String> {
        EngineApiClient::<Engine>::exchange_capabilities(&self.http_client, capabilities).await.unwrap()
    }

    pub async fn get_blobs_v1(&self, transaction_ids: Vec<B256>) -> Vec<Option<BlobAndProofV1>> {
        EngineApiClient::<Engine>::get_blobs_v1(&self.http_client, transaction_ids).await.unwrap()
    }
}