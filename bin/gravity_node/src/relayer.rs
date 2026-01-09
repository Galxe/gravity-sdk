use std::{collections::HashMap, path::PathBuf};

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
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Relayer configuration that maps URIs to their RPC URLs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayerConfig {
    /// Map from URI to RPC URL
    pub uri_mappings: HashMap<String, String>,
}

impl RelayerConfig {
    /// Load configuration from a JSON file
    pub fn from_file(path: &PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read relayer config file: {e}"))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse relayer config JSON: {e}"))
    }

    /// Get RPC URL for a given URI
    pub fn get_url(&self, uri: &str) -> Option<&str> {
        self.uri_mappings.get(uri).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, Default)]
struct ProviderState {
    /// Last fetched block number
    fetched_block_number: u64,
    /// Whether the last poll returned data
    last_had_update: bool,
}

struct ProviderProgressTracker {
    states: Mutex<HashMap<String, ProviderState>>,
}

impl ProviderProgressTracker {
    fn new() -> Self {
        Self { states: Mutex::new(HashMap::new()) }
    }

    async fn get_state(&self, name: &str) -> ProviderState {
        let guard = self.states.lock().await;
        guard.get(name).cloned().unwrap_or_default()
    }

    async fn update_state(&self, name: &str, block_number: u64, had_update: bool) {
        let mut guard = self.states.lock().await;
        guard.insert(
            name.to_string(),
            ProviderState { fetched_block_number: block_number, last_had_update: had_update },
        );
    }

    async fn init_block_number(&self, name: &str, block_number: u64) {
        let mut guard = self.states.lock().await;
        guard.entry(name.to_string()).or_insert_with(|| ProviderState {
            fetched_block_number: block_number,
            last_had_update: false,
        });
    }
}

pub struct RelayerWrapper {
    manager: RelayerManager,
    tracker: ProviderProgressTracker,
    config: RelayerConfig,
}

impl RelayerWrapper {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let config = config_path
            .and_then(|path| match RelayerConfig::from_file(&path) {
                Ok(cfg) => {
                    info!("Loaded relayer config from {:?}", path);
                    Some(cfg)
                }
                Err(e) => {
                    warn!("Failed to load relayer config: {}. Using empty config.", e);
                    None
                }
            })
            .unwrap_or_default();
        info!("relayer config: {:?}", config);

        let manager = RelayerManager::new();
        Self { manager, tracker: ProviderProgressTracker::new(), config }
    }

    async fn get_active_providers() -> Vec<OIDCProvider> {
        let block_number = get_block_buffer_manager().latest_commit_block_number().await;
        let config_bytes = GLOBAL_CONFIG_STORAGE
            .get()
            .unwrap()
            .fetch_config_bytes(OnChainConfig::JWKConsensusConfig, block_number.into())
            .unwrap();

        let bytes: Bytes = config_bytes.try_into().unwrap();
        let jwk_config =
            bcs::from_bytes::<gaptos::api_types::on_chain_config::jwks::JWKConsensusConfig>(&bytes)
                .unwrap();

        jwk_config.oidc_providers
    }

    fn should_block_poll(state: &ProviderState, onchain_block_number: u64) -> bool {
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

        let result =
            self.manager.poll_uri(uri).await.map_err(|e| ExecError::Other(e.to_string()))?;

        let has_update = result.updated;
        self.tracker.update_state(uri, result.max_block_number, has_update).await;

        info!(
            "Poll completed for uri: {} - block_number: {}, has_update: {}",
            uri, result.max_block_number, has_update
        );

        Ok(result)
    }
}

#[async_trait]
impl Relayer for RelayerWrapper {
    async fn add_uri(&self, uri: &str, _rpc_url: &str) -> Result<(), ExecError> {
        // Use local config URL if available, otherwise fall back to the provided rpc_url
        let actual_url = self
            .config
            .get_url(uri)
            .ok_or_else(|| ExecError::Other(format!("Provider {uri} not found in local config")))?;

        info!(
            "Adding URI: {}, RPC URL: {} ({})",
            uri,
            actual_url,
            if self.config.get_url(uri).is_some() { "from local config" } else { "from parameter" }
        );

        let active_providers = Self::get_active_providers().await;
        let from_block = if let Some(provider) = active_providers.iter().find(|p| p.name == uri) {
            let block_number = provider.onchain_block_number.unwrap_or(0);
            self.tracker.init_block_number(&provider.name, block_number).await;
            block_number
        } else {
            return Err(ExecError::Other(format!("Provider {uri} not found in active providers")));
        };

        self.manager
            .add_uri(uri, actual_url, from_block)
            .await
            .map_err(|e| ExecError::Other(e.to_string()))
    }

    // All URIs starting with gravity:// are definitely UnsupportedJWK
    async fn get_last_state(&self, uri: &str) -> Result<PollResult, ExecError> {
        let active_providers = Self::get_active_providers().await;

        let provider = active_providers.iter().find(|p| p.name == uri).ok_or_else(|| {
            ExecError::Other(format!("Provider {uri} not found in active providers"))
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
                "Block number hasn't progressed for uri: {uri} (waiting for consumption)"
            )));
        }

        self.poll_and_update_state(uri, onchain_block_number, &state).await
    }
}
