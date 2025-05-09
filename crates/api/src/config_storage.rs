use gaptos::api_types::config_storage::ConfigStorage;
use bytes::Bytes;
use std::sync::Arc;

struct ConfigStorageWrapper {
    config_storage: Arc<dyn ConfigStorage>,
}

impl ConfigStorageWrapper {
    pub fn new(config_storage: Arc<dyn ConfigStorage>) -> Self {
        Self { config_storage }
    }
}

impl ConfigStorage for ConfigStorageWrapper {
    fn fetch_config_bytes(
        &self,
        config_name: gaptos::api_types::config_storage::OnChainConfig,
        block_number: u64,
    ) -> Option<Bytes> {
        match config_name {
            gaptos::api_types::config_storage::OnChainConfig::ConsensusConfig => {
                self.config_storage.fetch_config_bytes(config_name, block_number)
            },
            _ => {
                // Return None so the caller can use default config for dev debug
                None
            }, 
        }
    }
}
