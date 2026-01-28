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
use greth::reth_pipe_exec_layer_relayer::OracleRelayerManager;
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
    /// Last nonce we returned from polling
    fetched_nonce: Option<u64>,
    /// Whether the last poll returned new data
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

    async fn update_state(&self, name: &str, nonce: Option<u64>, had_update: bool) {
        let mut guard = self.states.lock().await;
        guard.insert(
            name.to_string(),
            ProviderState { fetched_nonce: nonce, last_had_update: had_update },
        );
    }
}

pub struct RelayerWrapper {
    manager: OracleRelayerManager,
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

        let manager = OracleRelayerManager::new();
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

    /// Block poll if we returned data and on-chain hasn't caught up
    fn should_block_poll(state: &ProviderState, onchain_nonce: Option<u64>) -> bool {
        if let Some(fetched) = state.fetched_nonce {
            if let Some(onchain) = onchain_nonce {
                // Block if we returned data and on-chain nonce hasn't caught up
                return state.last_had_update && fetched > onchain;
            }
        }
        false
    }

    async fn poll_and_update_state(
        &self,
        uri: &str,
        onchain_nonce: Option<u64>,
        state: &ProviderState,
    ) -> Result<PollResult, ExecError> {
        info!(
            "Polling uri: {} (onchain_nonce: {:?}, fetched_nonce: {:?}, last_had_update: {})",
            uri, onchain_nonce, state.fetched_nonce, state.last_had_update
        );

        let result =
            self.manager.poll_uri(uri).await.map_err(|e| ExecError::Other(e.to_string()))?;

        let has_update = result.updated;
        // Track the nonce we returned (from PollResult)
        self.tracker.update_state(uri, result.nonce, has_update).await;

        info!(
            "Poll completed for uri: {} - nonce: {:?}, has_update: {}",
            uri, result.nonce, has_update
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

        // Get onchain_nonce from active providers for warm-start
        let active_providers = Self::get_active_providers().await;
        let onchain_nonce = active_providers
            .iter()
            .find(|p| p.name == uri)
            .and_then(|p| p.onchain_nonce)
            .unwrap_or(0) as u128;

        info!("Adding URI: {}, RPC URL: {}, onchain_nonce: {}", uri, actual_url, onchain_nonce);

        // Pass onchain_nonce to manager for warm-start
        self.manager
            .add_uri(uri, actual_url, onchain_nonce)
            .await
            .map_err(|e| ExecError::Other(e.to_string()))
    }

    // All URIs starting with gravity:// are definitely UnsupportedJWK
    async fn get_last_state(&self, uri: &str) -> Result<PollResult, ExecError> {
        let active_providers = Self::get_active_providers().await;

        let provider = active_providers.iter().find(|p| p.name == uri).ok_or_else(|| {
            ExecError::Other(format!("Provider {uri} not found in active providers"))
        })?;

        // Get on-chain nonce for comparison
        let onchain_nonce = provider.onchain_nonce;
        let state = self.tracker.get_state(uri).await;

        info!(
            "get_last_state - uri: {}, onchain_nonce: {:?}, fetched_nonce: {:?}, last_had_update: {}",
            uri, onchain_nonce, state.fetched_nonce, state.last_had_update
        );

        if Self::should_block_poll(&state, onchain_nonce) {
            warn!(
                "Blocking poll for uri: {} - waiting for consumption (fetched_nonce: {:?} > onchain_nonce: {:?})",
                uri, state.fetched_nonce, onchain_nonce
            );
            return Err(ExecError::Other(format!(
                "Nonce hasn't progressed for uri: {uri} (waiting for consumption)"
            )));
        }

        self.poll_and_update_state(uri, onchain_nonce, &state).await
    }
}
