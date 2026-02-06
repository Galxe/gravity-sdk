use std::{collections::HashMap, path::PathBuf};

use async_trait::async_trait;
use block_buffer_manager::get_block_buffer_manager;
use bytes::Bytes;
use gaptos::api_types::{
    config_storage::{OnChainConfig, GLOBAL_CONFIG_STORAGE},
    on_chain_config::oracle_state::OracleSourceState,
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
    /// Cached last poll result for re-sending when blocked
    last_result: Option<PollResult>,
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

    async fn update_state(&self, name: &str, result: &PollResult) {
        let mut guard = self.states.lock().await;
        guard.insert(
            name.to_string(),
            ProviderState {
                fetched_nonce: result.nonce,
                last_had_update: result.updated,
                last_result: Some(result.clone()),
            },
        );
    }
}

pub struct RelayerWrapper {
    manager: OracleRelayerManager,
    tracker: ProviderProgressTracker,
    config: RelayerConfig,
}

impl RelayerWrapper {
    pub fn new(config_path: Option<PathBuf>, datadir: PathBuf) -> Self {
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

        let manager = OracleRelayerManager::new(Some(datadir));
        Self { manager, tracker: ProviderProgressTracker::new(), config }
    }

    /// Fetch oracle source states from on-chain storage
    async fn get_oracle_source_states() -> Vec<OracleSourceState> {
        let block_number = get_block_buffer_manager().latest_commit_block_number().await;
        info!("get_oracle_source_states latest commit block number: {}", block_number);

        let config_bytes = match GLOBAL_CONFIG_STORAGE
            .get()
            .unwrap()
            .fetch_config_bytes(OnChainConfig::OracleState, block_number.into())
        {
            Some(bytes) => bytes,
            None => {
                warn!("Failed to fetch OracleState config");
                return vec![];
            }
        };

        let bytes: Bytes = match config_bytes.try_into() {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to convert OracleState config bytes: {:?}", e);
                return vec![];
            }
        };

        match bcs::from_bytes::<Vec<OracleSourceState>>(&bytes) {
            Ok(states) => {
                info!("Fetched {} oracle source states", states.len());
                states
            }
            Err(e) => {
                warn!("Failed to deserialize OracleSourceStates: {:?}", e);
                vec![]
            }
        }
    }

    /// Parse URI to extract source_type and source_id
    /// URI format: gravity://<source_type>/<source_id>/<task_type>?<params>
    fn parse_source_from_uri(uri: &str) -> Option<(u32, u64)> {
        if !uri.starts_with("gravity://") {
            return None;
        }

        let rest = &uri[10..]; // len("gravity://") = 10
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() < 2 {
            return None;
        }

        let source_type: u32 = parts[0].parse().ok()?;
        // Remove query string from source_id if present
        let source_id_str = parts[1].split('?').next()?;
        let source_id: u64 = source_id_str.parse().ok()?;

        Some((source_type, source_id))
    }

    /// Find oracle state for a URI by matching source_type and source_id
    fn find_oracle_state_for_uri<'a>(
        uri: &str,
        states: &'a [OracleSourceState],
    ) -> Option<&'a OracleSourceState> {
        let (source_type, source_id) = Self::parse_source_from_uri(uri)?;
        states.iter().find(|s| s.source_type == source_type && s.source_id == source_id)
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
        onchain_block_number: Option<u64>,
        state: &ProviderState,
    ) -> Result<PollResult, ExecError> {
        info!(
            "Polling uri: {} (onchain_nonce: {:?}, onchain_block: {:?}, fetched_nonce: {:?}, last_had_update: {})",
            uri, onchain_nonce, onchain_block_number, state.fetched_nonce, state.last_had_update
        );

        // Pass onchain state to poll_uri for reconciliation
        let result = self
            .manager
            .poll_uri(uri, onchain_nonce.map(|n| n as u128), onchain_block_number)
            .await
            .map_err(|e| ExecError::Other(e.to_string()))?;

        info!(
            "Poll completed for uri: {}, block number: {:?} - nonce: {:?}, has_update: {}, data len: {}",
            uri, result.max_block_number, result.nonce, result.updated, result.jwk_structs.len()
        );

        // Cache the result for potential re-sending when blocked
        self.tracker.update_state(uri, &result).await;

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

        // Get onchain state for this URI using source_type/source_id from URI
        let oracle_states = Self::get_oracle_source_states().await;
        info!("Oracle states: {:?}", oracle_states);
        let oracle_state =
            Self::find_oracle_state_for_uri(uri, &oracle_states).ok_or_else(|| {
                ExecError::Other(format!(
                    "Oracle state not found for URI: {uri}. Available states: {oracle_states:?}"
                ))
            })?;

        // Extract nonce and block_number from oracle state
        let onchain_nonce = oracle_state.latest_nonce as u128;
        let onchain_block_number =
            oracle_state.latest_record.as_ref().map(|r| r.block_number).unwrap_or(0);

        info!(
            "Adding URI: {}, RPC URL: {}, onchain_nonce: {}, onchain_block: {}",
            uri, actual_url, onchain_nonce, onchain_block_number
        );

        // Pass onchain state to manager for warm-start
        self.manager
            .add_uri(uri, actual_url, onchain_nonce, onchain_block_number)
            .await
            .map_err(|e| ExecError::Other(e.to_string()))
    }

    // All URIs starting with gravity:// are definitely UnsupportedJWK
    async fn get_last_state(&self, uri: &str) -> Result<PollResult, ExecError> {
        // Get onchain state for this URI using source_type/source_id from URI
        let oracle_states = Self::get_oracle_source_states().await;
        let oracle_state = Self::find_oracle_state_for_uri(uri, &oracle_states);

        // Extract nonce and block_number for reconciliation
        let (onchain_nonce, onchain_block_number) = if let Some(state) = oracle_state {
            let nonce = Some(state.latest_nonce);
            let block = state.latest_record.as_ref().map(|r| r.block_number);
            (nonce, block)
        } else {
            (None, None)
        };

        let state = self.tracker.get_state(uri).await;

        info!(
            "get_last_state - uri: {}, onchain_nonce: {:?}, onchain_block: {:?}, fetched_nonce: {:?}, last_had_update: {}",
            uri, onchain_nonce, onchain_block_number, state.fetched_nonce, state.last_had_update
        );

        if Self::should_block_poll(&state, onchain_nonce) {
            // Re-send the cached result instead of polling again
            if let Some(cached) = &state.last_result {
                info!(
                    "Returning cached result for uri: {} (fetched_nonce: {:?} > onchain_nonce: {:?})",
                    uri, state.fetched_nonce, onchain_nonce
                );
                return Ok(cached.clone());
            }
            // No cached result available, fall through to poll
            panic!("No cached result for uri: {uri} - polling despite block condition");
        }

        self.poll_and_update_state(uri, onchain_nonce, onchain_block_number, &state).await
    }
}
