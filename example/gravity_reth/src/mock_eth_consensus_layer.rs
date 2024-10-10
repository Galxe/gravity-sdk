use crate::gcei_sender::GCEISender;
use eyre::{Context, Result};
use gravity_sdk::{consensus_engine::GravityConsensusEngine, NodeConfig};
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_node_core::primitives::{Address, B256};
use reth_node_core::rpc::types::engine::{
    ForkchoiceState, PayloadId, PayloadStatus, PayloadStatusEnum,
};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use reth_rpc_types::engine::{ForkchoiceUpdated, PayloadAttributes};
use tracing::{error, info};

pub struct MockEthConsensusLayer<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> {
    engine_api_client: T,
    gcei: GCEISender<GravityConsensusEngine>,
}

impl<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> MockEthConsensusLayer<T> {
    pub(crate) fn new(client: T, chain_id: u64, gcei_config: NodeConfig) -> Self {
        Self { engine_api_client: client, gcei: GCEISender::new(chain_id, gcei_config) }
    }

    pub(crate) async fn start_round(&mut self, genesis_hash: B256) -> Result<()> {
        let fork_choice_state = ForkchoiceState {
            head_block_hash: genesis_hash,
            safe_block_hash: genesis_hash,
            finalized_block_hash: genesis_hash,
        };

        // construct PayloadAttributes
        self.run_round(fork_choice_state).await
    }

    pub(crate) async fn get_new_payload_id(
        &self,
        fork_choice_state: &mut ForkchoiceState,
        payload_attributes: &PayloadAttributes,
    ) -> Option<PayloadId> {
        let updated_res =
            self.update_fork_choice(fork_choice_state.clone(), payload_attributes.clone()).await;
        info!("Got update res: {:?}", updated_res);
        match updated_res {
            Ok(updated) => {
                if updated.payload_id.is_none() {
                    error!("Payload ID is none");
                    if let Some(latest_valid_hash) = updated.payload_status.latest_valid_hash {
                        fork_choice_state.safe_block_hash = latest_valid_hash;
                        fork_choice_state.head_block_hash = latest_valid_hash;
                    }
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

    pub fn is_leader(&self) -> bool {
        self.gcei.is_leader()
    }

    pub async fn construct_payload(
        &mut self,
        fork_choice_state: &mut ForkchoiceState,
    ) -> anyhow::Result<bool> {
        let parent_beacon_block_root = fork_choice_state.head_block_hash;
        let payload_attributes = Self::create_payload_attributes(parent_beacon_block_root);
        // update ForkchoiceState and get payload_id
        let payload_id = match self.get_new_payload_id(fork_choice_state, &payload_attributes).await
        {
            Some(payload_id) => payload_id,
            None => return Ok(false),
        };
        // try to get payload
        let payload = <T as EngineApiClient<EthEngineTypes>>::get_payload_v3(
            &self.engine_api_client,
            payload_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        // 1. submit valid transactions
        self.gcei.submit_valid_transactions_v3(&payload_id, &payload).await;
        info!("Got payload: {:?}", payload);
        Ok(true)
    }

    pub(crate) async fn run_round(&mut self, mut fork_choice_state: ForkchoiceState) -> Result<()> {
        let mut round = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            round += 1;
            info!("Round {:?}, Cli ForkchoiceState: {:?}", round, fork_choice_state);
            // submit valid transactions for leader
            if self.is_leader() {
                let res = self.construct_payload(&mut fork_choice_state).await;
                if res.is_err() || !res.unwrap() {
                    continue;
                }
            }

            // 2. polling order blocks
            let payload = self.gcei.polling_order_blocks_v3().await;
            if let Ok(payload) = payload {
                let parent_hash = payload.execution_payload.payload_inner.payload_inner.parent_hash;
                let payload_status = <T as EngineApiClient<EthEngineTypes>>::new_payload_v3(
                    &self.engine_api_client,
                    payload.execution_payload,
                    Vec::new(),
                    parent_hash,
                )
                .await
                .context("Failed to submit payload")?;
                // 3. submit compute res
                if payload_status.latest_valid_hash.is_none() {
                    info!("payload status latest valid hash is none");
                    continue;
                }
                self.gcei
                    .submit_compute_res(payload_status.latest_valid_hash.unwrap())
                    .await
                    .expect("TODO: panic message");
                // 4. polling submit blocks
                if self.gcei.polling_submit_blocks().await.is_ok() {
                    fork_choice_state =
                        self.handle_payload_status(fork_choice_state, payload_status)?;
                    // 5. submit max persistence block id
                    self.gcei.submit_max_persistence_block_id().await;
                } else {
                    info!("failed to polling_submit_blocks");
                }
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

    async fn update_fork_choice(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: EthPayloadAttributes,
    ) -> Result<ForkchoiceUpdated> {
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

    /// 根据 PayloadStatus 更新 ForkchoiceState
    fn handle_payload_status(
        &self,
        mut fork_choice_state: ForkchoiceState,
        payload_status: PayloadStatus,
    ) -> Result<ForkchoiceState> {
        match payload_status.status {
            PayloadStatusEnum::Valid => {
                info!("Payload is valid");
                if let Some(latest_valid_hash) = payload_status.latest_valid_hash {
                    fork_choice_state.head_block_hash = latest_valid_hash;
                    fork_choice_state.safe_block_hash = latest_valid_hash;
                    fork_choice_state.finalized_block_hash = latest_valid_hash;
                }
                Ok(fork_choice_state)
            }
            PayloadStatusEnum::Accepted => {
                info!("Payload is accepted");
                if let Some(latest_valid_hash) = payload_status.latest_valid_hash {
                    fork_choice_state.head_block_hash = latest_valid_hash;
                    fork_choice_state.safe_block_hash = latest_valid_hash;
                    fork_choice_state.finalized_block_hash = latest_valid_hash;
                }
                Ok(fork_choice_state)
            }
            PayloadStatusEnum::Invalid { validation_error } => {
                error!("Invalid payload: {}", validation_error);
                Err(eyre::anyhow!("Invalid payload: {}", validation_error))
            }
            PayloadStatusEnum::Syncing => {
                error!("Syncing, awaiting data...");
                Err(eyre::anyhow!("Syncing, awaiting data"))
            }
        }
    }
}
