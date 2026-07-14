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
use greth::reth_pipe_exec_layer_relayer::{parse_oracle_uri, OracleRelayerManager};
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

/// Keep only scheme and host for logging; redact userinfo, path, query, and fragment.
fn sanitize_url(url: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return "***".to_string();
    };
    let Some(host) = parsed.host_str() else {
        return "***".to_string();
    };
    let host = if host.contains(':') { format!("[{host}]") } else { host.to_string() };
    let port = parsed.port().map(|value| format!(":{value}")).unwrap_or_default();
    format!("{}://{}{} /***", parsed.scheme(), host, port)
}

#[derive(Debug, Clone, Default)]
struct ProviderState {
    /// Last nonce we returned from polling
    fetched_nonce: Option<u128>,
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
                    info!(
                        file = %path.file_name().and_then(|name| name.to_str()).unwrap_or("<config>"),
                        "Loaded relayer config"
                    );
                    Some(cfg)
                }
                Err(e) => {
                    warn!("Failed to load relayer config: {}. Using empty config.", e);
                    None
                }
            })
            .unwrap_or_default();
        for (uri, url) in &config.uri_mappings {
            info!("relayer config: URI {} -> {}", uri, sanitize_url(url));
        }

        // update reth commit and use
        let manager = OracleRelayerManager::new(datadir);
        Self { manager, tracker: ProviderProgressTracker::new(), config }
    }

    /// Fetch oracle source states from on-chain storage
    async fn get_oracle_source_states() -> Vec<OracleSourceState> {
        let block_number = get_block_buffer_manager().latest_commit_block_number().await;
        info!("get_oracle_source_states latest commit block number: {}", block_number);

        let config_bytes = match GLOBAL_CONFIG_STORAGE
            .get()
            .expect("GLOBAL_CONFIG_STORAGE not initialized")
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

    /// Parse only the source identity used to reconcile on-chain state.
    ///
    /// The reth relayer validates the task type and provider-specific parameters when the URI is
    /// added. Keeping this helper limited to `(source_type, source_id)` avoids duplicating those
    /// rules in the SDK wrapper.
    fn parse_source_from_uri(uri: &str) -> Option<(u32, u64)> {
        let task = parse_oracle_uri(uri).ok()?;
        Some((task.source_type, task.source_id))
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
    fn should_block_poll(state: &ProviderState, onchain_nonce: Option<u128>) -> bool {
        if let Some(fetched) = state.fetched_nonce {
            if let Some(onchain) = onchain_nonce {
                // Block if we returned data and on-chain nonce hasn't caught up
                return state.last_had_update && fetched > onchain;
            }
        }
        false
    }

    /// Submission-side dedup guard (gravity-audit subtask 2). Returns true when the observed
    /// oracle nonce is already committed on-chain (`observed <= onchain`), so surfacing it as an
    /// update would inject a `recordBatch` system-tx that re-submits an already-recorded nonce
    /// (provided <= currentNonce) and NECESSARILY reverts (NonceNotSequential). The revert is
    /// benign (logged, block proceeds) but wastes a full propose/certify/execute round
    /// (~2.5x/day on mainnet). A legitimately-needed observation always carries `observed >
    /// onchain`, so suppressing this never stalls a real update. Missing data is never suppressed.
    fn is_already_committed(observed_nonce: Option<u128>, onchain_nonce: Option<u128>) -> bool {
        matches!(
            (observed_nonce, onchain_nonce),
            (Some(observed), Some(onchain)) if observed <= onchain
        )
    }

    /// Apply the subtask-2 submission-side guard to a freshly polled result: if the observation's
    /// nonce is already committed on-chain, mark it not-updated so `JWKObserver` drops it and the
    /// doomed `recordBatch` vtxn is never proposed. A genuinely-new observation (nonce > onchain)
    /// passes through untouched.
    fn guard_already_committed(
        uri: &str,
        mut result: PollResult,
        onchain_nonce: Option<u128>,
    ) -> PollResult {
        if result.updated && Self::is_already_committed(result.nonce, onchain_nonce) {
            info!(
                "Suppressing already-committed oracle observation for uri {}: observed nonce {:?} <= onchain {:?}",
                uri, result.nonce, onchain_nonce
            );
            result.updated = false;
        }
        result
    }

    async fn poll_and_update_state(
        &self,
        uri: &str,
        onchain_nonce: Option<u128>,
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
            .poll_uri(uri, onchain_nonce, onchain_block_number)
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
        let oracle_state = Self::find_oracle_state_for_uri(uri, &oracle_states).ok_or_else(|| {
            let available_sources = oracle_states
                .iter()
                .map(|state| format!("{}:{}", state.source_type, state.source_id))
                .collect::<Vec<_>>()
                .join(", ");
                ExecError::Other(format!(
                    "Oracle state not found for URI: {uri}. Available source identities: [{available_sources}]"
                ))
            })?;

        // Extract nonce and block_number from oracle state
        let onchain_nonce = oracle_state.latest_nonce;
        let onchain_block_number =
            oracle_state.latest_record.as_ref().map(|r| r.block_number).unwrap_or(0);

        info!(
            "Adding URI: {}, RPC URL: {}, onchain_nonce: {}, onchain_block: {}",
            uri,
            sanitize_url(actual_url),
            onchain_nonce,
            onchain_block_number
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
            return Err(ExecError::Other(format!(
                "No cached result for uri: {uri} - polling despite block condition"
            )));
        }

        let result =
            self.poll_and_update_state(uri, onchain_nonce, onchain_block_number, &state).await?;

        // Subtask 2: never surface an observation whose nonce is already committed on-chain — it
        // would inject a guaranteed-revert oracle `recordBatch`. See `guard_already_committed`.
        Ok(Self::guard_already_committed(uri, result, onchain_nonce))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_already_committed_suppresses_committed_nonce() {
        // provided == currentNonce — the observed steady-state "provided = currentNonce" revert
        assert!(RelayerWrapper::is_already_committed(Some(5), Some(5)));
        // provided < currentNonce
        assert!(RelayerWrapper::is_already_committed(Some(4), Some(5)));
        // the next real observation (nonce == onchain + 1) must still flow
        assert!(!RelayerWrapper::is_already_committed(Some(6), Some(5)));
        // a later observation must flow
        assert!(!RelayerWrapper::is_already_committed(Some(100), Some(5)));
        // unknown data is never suppressed (fail-open on missing, never suppress a real update)
        assert!(!RelayerWrapper::is_already_committed(None, Some(5)));
        assert!(!RelayerWrapper::is_already_committed(Some(5), None));
        assert!(!RelayerWrapper::is_already_committed(None, None));
    }

    fn poll_result(nonce: u128, updated: bool) -> PollResult {
        PollResult { jwk_structs: vec![], max_block_number: 100, nonce: Some(nonce), updated }
    }

    #[test]
    fn guard_suppresses_an_already_committed_duplicate() {
        // Artificial duplicate: an observation whose nonce (7) is already committed on-chain (7).
        // Re-submitting it would revert NonceNotSequential — the guard must drop it.
        let dup = RelayerWrapper::guard_already_committed(
            "gravity://0/1/events",
            poll_result(7, true),
            Some(7),
        );
        assert!(!dup.updated, "already-committed observation (7 <= 7) must be suppressed");

        // A stale replay below the committed nonce is likewise suppressed.
        let stale = RelayerWrapper::guard_already_committed(
            "gravity://0/1/events",
            poll_result(6, true),
            Some(7),
        );
        assert!(!stale.updated, "stale observation (6 <= 7) must be suppressed");

        // The next genuine observation (nonce 8 > onchain 7) must pass through untouched.
        let fresh = RelayerWrapper::guard_already_committed(
            "gravity://0/1/events",
            poll_result(8, true),
            Some(7),
        );
        assert!(fresh.updated, "a genuinely-new observation (8 > 7) must NOT be suppressed");
    }

    #[test]
    fn test_parse_price_feed_source_uri() {
        let uri = "gravity://3/1001/price_feed?provider=binance_index_kline_v1";
        assert_eq!(RelayerWrapper::parse_source_from_uri(uri), Some((3, 1001)));
    }

    #[test]
    fn test_parse_blockchain_source_uri() {
        let uri = "gravity://0/31337/events?contract=0x0000000000000000000000000000000000000001";
        assert_eq!(RelayerWrapper::parse_source_from_uri(uri), Some((0, 31337)));
    }

    #[test]
    fn test_parse_source_accepts_identity_only_uri() {
        assert_eq!(RelayerWrapper::parse_source_from_uri("gravity://3/1001"), Some((3, 1001)));
    }

    #[test]
    fn test_parse_source_rejects_unroutable_uri() {
        assert_eq!(RelayerWrapper::parse_source_from_uri("gravity://price/1001"), None);
        assert_eq!(RelayerWrapper::parse_source_from_uri("https://oracle.example/3/1001"), None);
    }

    #[test]
    fn test_sanitize_url_removes_credentials_and_path() {
        let sanitized = sanitize_url("https://user:secret@polygon.example:8443/v1/key?token=abc");
        assert_eq!(sanitized, "https://polygon.example:8443 /***");
        assert!(!sanitized.contains("user"));
        assert!(!sanitized.contains("secret"));
        assert!(!sanitized.contains("token"));
    }
}
