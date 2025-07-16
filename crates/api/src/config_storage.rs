use bytes::Bytes;
use gaptos::{api_types::config_storage::{ConfigStorage, OnChainConfig, OnChainConfigResType}, aptos_logger::info};
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
        block_number: u64,
    ) -> Option<OnChainConfigResType> {
        println!("fetch_config_bytes: {:?}, block_number: {:?}", config_name, block_number);

        info!("fetch_config_bytes: {:?}, block_number: {:?}", config_name, block_number);
        match config_name {
            OnChainConfig::Epoch | OnChainConfig::ValidatorSet => {
                self.config_storage.fetch_config_bytes(config_name, block_number)
            }
            OnChainConfig::ConsensusConfig => {
                let bytes = vec![
                    3, 1, 1, 10, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 10,
                    0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
                ];
                let bytes = Bytes::from(bytes);
                let res : OnChainConfigResType = bytes.into();
                Some(res)
            }
            _ => {
                // Return None so the caller can use default config for dev debug
                None
            }
        }
    }
}
