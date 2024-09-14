use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes, EthPayloadBuilderAttributes};
use reth_node_core::rpc::types::engine::{ExecutionPayloadInputV2, ForkchoiceState, PayloadId, PayloadStatus, PayloadStatusEnum};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use eyre::{Context, Result};
use reth_node_api::{PayloadAttributes, PayloadTypes};
use reth_node_builder::EngineTypes;
use reth_node_core::primitives::{Address, B256};
use reth_node_core::rpc::types::ExecutionPayloadV2;

pub struct MockEthConsensusLayer<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> {
    engine_api_client: T,
}

impl<T: EngineEthApiClient<EthEngineTypes> + Send + Sync> MockEthConsensusLayer<T> {
    pub fn new(client: T) -> Self {  // <1>
        Self {
            engine_api_client: client
        }
    }

    pub async fn run_round(&self, mut fork_choice_state: ForkchoiceState) -> Result<()> {
        // 创建 PayloadAttributes
        let payload_attributes = Self::create_payload_attributes();
        loop {
            // 更新 ForkchoiceState 并获取 payload_id
            let payload_id = self
                .update_fork_choice(fork_choice_state, payload_attributes.clone())
                .await
                .context("Failed to update fork choice")?;

            // 获取 ExecutionPayloadv2
            let payload = <T as EngineApiClient<EthEngineTypes>>::get_payload_v2(&self.engine_api_client, payload_id)
                .await
                .context("Failed to get payload")?;
            let payload = ExecutionPayloadInputV2 {
                withdrawals: None,
                execution_payload: payload.into_v1_payload(),
            };
            println!("Got payload: {:?}", payload);
            // 提交新的 payload 到 engine
            let payload_status = <T as EngineApiClient<EthEngineTypes>>::new_payload_v2(&self.engine_api_client, payload)
                .await
                .context("Failed to submit payload")?;

            // 根据 payload 的状态更新 ForkchoiceState
            fork_choice_state = self.handle_payload_status(fork_choice_state, payload_status)?;
        }
    }

    /// 创建 PayloadAttributes 的辅助函数
    fn create_payload_attributes() -> EthPayloadAttributes {
        EthPayloadAttributes {
            timestamp: 0,
            prev_randao: B256::ZERO,
            suggested_fee_recipient: Address::ZERO,
            withdrawals: Some(vec![]),
            parent_beacon_block_root: Some(B256::ZERO),
        }
    }

    /// 更新 ForkchoiceState 并获取 payload_id
    async fn update_fork_choice(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: EthPayloadAttributes,
    ) -> Result<PayloadId> {
        let response = <T as EngineApiClient<EthEngineTypes>>::
        fork_choice_updated_v2(&self.engine_api_client, fork_choice_state, Some(payload_attributes))
            .await
            .context("Failed to update fork choice")?;

        match response.payload_id {
            Some(pid) => Ok(pid),
            None => Err(eyre::anyhow!("No payload_id received from fork_choice_updated_v2")),
        }
    }

    /// 根据 PayloadStatus 更新 ForkchoiceState
    fn handle_payload_status(
        &self,
        mut fork_choice_state: ForkchoiceState,
        payload_status: PayloadStatus,
    ) -> Result<ForkchoiceState> {
        match payload_status.status {
            PayloadStatusEnum::Valid => {
                if let Some(latest_valid_hash) = payload_status.latest_valid_hash {
                    fork_choice_state.head_block_hash = latest_valid_hash;
                    fork_choice_state.safe_block_hash = latest_valid_hash;
                    fork_choice_state.finalized_block_hash = latest_valid_hash;
                }
                Ok(fork_choice_state)
            }
            PayloadStatusEnum::Accepted => {
                if let Some(latest_valid_hash) = payload_status.latest_valid_hash {
                    fork_choice_state.head_block_hash = latest_valid_hash;
                    fork_choice_state.safe_block_hash = latest_valid_hash;
                    fork_choice_state.finalized_block_hash = latest_valid_hash;
                }
                Ok(fork_choice_state)
            }
            PayloadStatusEnum::Invalid { validation_error } => {
                // 记录错误并根据需要采取进一步措施
                eprintln!("Invalid payload: {}", validation_error);
                // 在这里可以选择返回错误或者继续
                Err(eyre::anyhow!("Invalid payload: {}", validation_error))
            }
            PayloadStatusEnum::Syncing => {
                eprintln!("Syncing, awaiting data...");
                // 根据需要选择是否返回错误或等待
                Err(eyre::anyhow!("Syncing, awaiting data"))
            }
        }
    }
}