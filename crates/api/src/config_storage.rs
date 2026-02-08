use gaptos::{
    api_types::config_storage::{BlockNumber, ConfigStorage, OnChainConfig, OnChainConfigResType},
    aptos_logger::info,
};
use std::sync::Arc;

pub struct ConfigStorageWrapper {
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
        config_name: OnChainConfig,
        block_number: BlockNumber,
    ) -> Option<OnChainConfigResType> {
        println!("fetch_config_bytes: {config_name:?}, block_number: {block_number:?}");

        info!("fetch_config_bytes: {:?}, block_number: {:?}", config_name, block_number);
        match config_name {
            OnChainConfig::Epoch |
            OnChainConfig::ValidatorSet |
            OnChainConfig::JWKConsensusConfig |
            OnChainConfig::ObservedJWKs |
            OnChainConfig::RandomnessConfig |
            OnChainConfig::OracleState |
            OnChainConfig::DKGState |
            OnChainConfig::ConsensusConfig => {
                self.config_storage.fetch_config_bytes(config_name, block_number)
            }
            _ => {
                // Return None so the caller can use default config for dev debug
                None
            }
        }
    }
}
