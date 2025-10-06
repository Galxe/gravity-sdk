use std::collections::HashMap;

use async_trait::async_trait;
use block_buffer_manager::get_block_buffer_manager;
use bytes::Bytes;
use gaptos::api_types::{
    config_storage::{OnChainConfig, GLOBAL_CONFIG_STORAGE},
    on_chain_config::jwks::OIDCProvider,
    relayer::{PollResult, Relayer},
    ExecError,
};
use greth::reth_pipe_exec_layer_ext_v2::RelayerManager;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
struct ProviderState {
    /// 上次 fetch 的 block number
    fetched_block_number: u64,
    /// 上次 poll 是否返回了数据
    last_had_update: bool,
}

struct ProviderProgressTracker {
    states: Mutex<HashMap<String, ProviderState>>,
}

impl ProviderProgressTracker {
    fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
        }
    }

    async fn get_state(&self, name: &str) -> ProviderState {
        let guard = self.states.lock().await;
        guard.get(name).cloned().unwrap_or_default()
    }

    async fn update_state(&self, name: &str, block_number: u64, had_update: bool) {
        let mut guard = self.states.lock().await;
        guard.insert(
            name.to_string(),
            ProviderState {
                fetched_block_number: block_number,
                last_had_update: had_update,
            },
        );
    }

    async fn init_block_number(&self, name: &str, block_number: u64) {
        let mut guard = self.states.lock().await;
        guard
            .entry(name.to_string())
            .or_insert_with(|| ProviderState {
                fetched_block_number: block_number,
                last_had_update: false,
            });
    }
}

pub struct RelayerWrapper {
    manager: RelayerManager,
    tracker: ProviderProgressTracker,
}

// 初始化的时候之类也要拿到所有的provider和他对应的onchain number
// 接下来在fetch的时候  需要记录block number. 如果没有推进到下一个block number 不应该有动静
// 当然这里需要event里面记录上block number才能做到 我感觉是可以的
// 比如初始化的时候block number从100开始. 现在一次fetch返回的是(logs, 150)
// 那么下一次fetch的时候 要先活动active provider上的数据拿到最新的链上的block number. 如果还是没变那就不应该推进
// 如果变了那可以从150开始 因为150是已经被get logs过的. 因为中间的event可能是140这个block发出来的.
// 所以相当于是每次要记录上一次请求是从多少开始的，如果返回的logs是空，就把游标改成随着空logs返回的block number即可. 要把relayer实现在这才行
// 甚至要监听用的URL都应该从这里直接传递给jwk 的epoch manager 不然一点都不好写 就全部用全局变量就行

impl RelayerWrapper {
    pub fn new() -> Self {
        let manager = RelayerManager::new();
        Self {
            manager,
            tracker: ProviderProgressTracker::new(),
        }
    }

    async fn get_active_providers() -> Vec<OIDCProvider> {
        let block_number = get_block_buffer_manager().latest_commit_block_number().await;
        let config_bytes = GLOBAL_CONFIG_STORAGE
            .get()
            .unwrap()
            .fetch_config_bytes(OnChainConfig::JWKConsensusConfig, block_number)
            .unwrap();
        
        let bytes: Bytes = config_bytes.try_into().unwrap();
        let jwk_config =
            bcs::from_bytes::<gaptos::api_types::on_chain_config::jwks::JWKConsensusConfig>(&bytes)
                .unwrap();

        jwk_config.oidc_providers
    }

    fn should_block_poll(
        state: &ProviderState,
        onchain_block_number: u64,
    ) -> bool {
        state.last_had_update && state.fetched_block_number == onchain_block_number
    }

    async fn poll_and_update_state(
        &self,
        uri: &str,
        onchain_block_number: u64,
        state: &ProviderState,
    ) -> Result<PollResult, ExecError> {
        info!(
            "Polling uri: {} (onchain: {}, last_fetched: {}, last_had_update: {})",
            uri, onchain_block_number, state.fetched_block_number, state.last_had_update
        );

        let result = self
            .manager
            .poll_uri(uri)
            .await
            .map_err(|e| ExecError::Other(e.to_string()))?;

        let has_update = result.updated;
        self.tracker
            .update_state(uri, result.max_block_number, has_update)
            .await;

        info!(
            "Poll completed for uri: {} - block_number: {}, has_update: {}",
            uri, result.max_block_number, has_update
        );

        Ok(result)
    }
}

#[async_trait]
impl Relayer for RelayerWrapper {
    async fn add_uri(&self, uri: &str, rpc_url: &str) -> Result<(), ExecError> {
        info!("Adding URI: {}, RPC URL: {}", uri, rpc_url);

        let active_providers = Self::get_active_providers().await;
        if let Some(provider) = active_providers.iter().find(|p| p.name == uri) {
            let block_number = provider.onchain_block_number.unwrap_or(0);
            self.tracker.init_block_number(&provider.name, block_number).await;
        }

        self.manager
            .add_uri(uri, rpc_url)
            .await
            .map_err(|e| ExecError::Other(e.to_string()))
    }

    // TODO: All URIs starting with gravity:// are definitely UnsupportedJWK
    async fn get_last_state(&self, uri: &str) -> Result<PollResult, ExecError> {
        let active_providers = Self::get_active_providers().await;
        
        let provider = active_providers
            .iter()
            .find(|p| p.name == uri)
            .ok_or_else(|| {
                ExecError::Other(format!("Provider {} not found in active providers", uri))
            })?;

        let onchain_block_number = provider.onchain_block_number.unwrap_or(0);
        let state = self.tracker.get_state(uri).await;

        info!(
            "get_last_state - uri: {}, onchain: {}, state: {:?}",
            uri, onchain_block_number, state
        );

        if Self::should_block_poll(&state, onchain_block_number) {
            warn!(
                "Blocking poll for uri: {} - waiting for consumption (onchain block unchanged)",
                uri
            );
            return Err(ExecError::Other(format!(
                "Block number hasn't progressed for uri: {} (waiting for consumption)",
                uri
            )));
        }

        // 执行 poll 并更新状态
        self.poll_and_update_state(uri, onchain_block_number, &state).await
    }
}
