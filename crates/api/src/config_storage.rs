use bytes::Bytes;
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
            OnChainConfig::DKGState |
            OnChainConfig::RandomnessConfig => {
                self.config_storage.fetch_config_bytes(config_name, block_number)
            }
            OnChainConfig::ConsensusConfig => {
                // let bytes = vec![
                //     3, 1, 1, 10, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 10,
                //     0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
                // ];
                // The following bytes is serialized from the following struct:
                // V4 { alg: JolteonV2 { main: ConsensusConfigV1 { decoupled_execution: true,
                // back_pressure_limit: 10, exclude_round: 40, proposer_election_type:
                // RotatingProposer(1), max_failed_authors_to_store: 10 }, quorum_store_enabled:
                // true, order_vote_enabled: false }, vtxn: V1 { per_block_limit_txn_count: 2,
                // per_block_limit_total_bytes: 2097152 }, window_size: None }
                let bytes = vec![
                    3, 1, 1, 10, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 10,
                    0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0,
                    0,
                ];
                let bytes = Bytes::from(bytes);
                let res: OnChainConfigResType = bytes.into();
                Some(res)
            }
            _ => {
                // Return None so the caller can use default config for dev debug
                None
            }
        }
    }
}
