use bytes::Bytes;
use gaptos::{
    api_types::config_storage::{OnChainConfig, GLOBAL_CONFIG_STORAGE},
    aptos_logger::{error, info},
    aptos_storage_interface::DbReader,
    aptos_types::{
        dkg::DKGState,
        on_chain_config::OnChainConfig as OnChainConfigTrait,
    },
};
use axum::{http::StatusCode, response::{Json as JsonResponse, IntoResponse}};
use serde::{Deserialize, Serialize};
use aptos_consensus::consensusdb::ConsensusDB;
use std::sync::Arc;

pub struct DkgState {
    consensus_db: Option<Arc<ConsensusDB>>,
}

impl DkgState {
    pub fn new(consensus_db: Option<Arc<ConsensusDB>>) -> Self {
        Self { consensus_db }
    }

    pub fn consensus_db(&self) -> Option<&Arc<ConsensusDB>> {
        self.consensus_db.as_ref()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DKGStateResponse {
    pub last_completed: Option<DKGSessionStateInfo>,
    pub in_progress: Option<DKGSessionStateInfo>,
    pub block_number: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DKGSessionStateInfo {
    pub metadata: DKGSessionMetadataInfo,
    pub start_time_us: u64,
    pub transcript_length: usize,
    pub target_epoch: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DKGSessionMetadataInfo {
    pub dealer_epoch: u64,
    pub target_num_validators: usize,
    pub num_dealers: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DKGStatusResponse {
    pub epoch: u64,
    pub round: u64,
    pub block_number: u64,
    pub participating_nodes: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RandomnessResponse {
    pub block_number: u64,
    pub randomness: Option<String>, // hex encoded
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

impl DkgState {
    /// Get DKG status (epoch, round, block, participating nodes)
    /// Example: curl https://127.0.0.1:1024/dkg/status
    pub async fn get_dkg_status(&self) -> impl IntoResponse {
        info!("Getting DKG status");

        // Get ConsensusDB
        let consensus_db = match self.consensus_db.as_ref() {
            Some(db) => db,
            None => {
                error!("ConsensusDB is not initialized");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(ErrorResponse {
                        error: "ConsensusDB is not initialized".to_string(),
                    }),
                ).into_response();
            }
        };

        // Get latest ledger info using DbReader trait
        let latest_ledger_info = match DbReader::get_latest_ledger_info(consensus_db.as_ref()) {
            Ok(info) => info,
            Err(e) => {
                error!("Failed to get latest ledger info: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(ErrorResponse {
                        error: format!("Failed to get latest ledger info: {:?}", e),
                    }),
                ).into_response();
            }
        };

        let ledger_info = latest_ledger_info.ledger_info();
        let epoch = ledger_info.epoch();
        let round = ledger_info.round();
        let block = ledger_info.block_number();

        // Get participating nodes count from DKGState last_completed session
        let participating_nodes = if let Some(config_storage) = GLOBAL_CONFIG_STORAGE.get() {
            if let Some(config_bytes) = config_storage.fetch_config_bytes(OnChainConfig::DKGState, block.into()) {
                match config_bytes.try_into() {
                    Ok(bytes) => {
                        let bytes: Bytes = bytes;
                        match <DKGState as OnChainConfigTrait>::deserialize_into_config(bytes.as_ref()) {
                            Ok(dkg_state) => {
                                // participating_nodes is the count of target_validator_set from last_completed session
                                if let Some(session) = &dkg_state.last_completed {
                                    session.metadata.target_validator_set.len()
                                } else {
                                    error!("No last_completed DKG session found at block {}", block);
                                    return (
                                        StatusCode::NOT_FOUND,
                                        JsonResponse(ErrorResponse {
                                            error: format!("No last_completed DKG session found at block {}", block),
                                        }),
                                    ).into_response();
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize DKG state: {:?}", e);
                                return (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    JsonResponse(ErrorResponse {
                                        error: format!("Failed to deserialize DKG state: {:?}", e),
                                    }),
                                ).into_response();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to convert config bytes: {:?}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            JsonResponse(ErrorResponse {
                                error: format!("Failed to convert config bytes: {:?}", e),
                            }),
                        ).into_response();
                    }
                }
            } else {
                error!("Failed to fetch DKG state from config storage at block {}", block);
                return (
                    StatusCode::NOT_FOUND,
                    JsonResponse(ErrorResponse {
                        error: format!("Failed to fetch DKG state from config storage at block {}", block),
                    }),
                ).into_response();
            }
        } else {
            error!("GLOBAL_CONFIG_STORAGE is not initialized");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(ErrorResponse {
                    error: "GLOBAL_CONFIG_STORAGE is not initialized".to_string(),
                }),
            ).into_response();
        };

        let response = DKGStatusResponse {
            epoch,
            round,
            block_number: block,
            participating_nodes,
        };

        info!("Successfully retrieved DKG status: epoch={}, round={}, block={}, nodes={}", 
              epoch, round, block, participating_nodes);
        JsonResponse(response).into_response()
    }

    /// Get randomness for a specific block number
    /// Example: curl "https://127.0.0.1:1024/dkg/randomness/100"
    pub async fn get_randomness(
        &self,
        block_number: u64,
    ) -> impl IntoResponse {
        info!("Getting randomness for block {}", block_number);

        // Get ConsensusDB
        let consensus_db = match self.consensus_db.as_ref() {
            Some(db) => db,
            None => {
                error!("ConsensusDB is not initialized");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(ErrorResponse {
                        error: "ConsensusDB is not initialized".to_string(),
                    }),
                ).into_response();
            }
        };

        match consensus_db.get_randomness(block_number) {
            Ok(Some(randomness)) => {
                let response = RandomnessResponse {
                    block_number,
                    randomness: Some(hex::encode(&randomness)),
                };
                info!("Successfully retrieved randomness for block {}", block_number);
                JsonResponse(response).into_response()
            }
            Ok(None) => {
                // Return 200 with None randomness instead of 404
                // This is more RESTful: the resource exists, but has no randomness data
                let response = RandomnessResponse {
                    block_number,
                    randomness: None,
                };
                info!("No randomness found for block {}", block_number);
                JsonResponse(response).into_response()
            }
            Err(e) => {
                error!("Failed to get randomness for block {}: {:?}", block_number, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(ErrorResponse {
                        error: format!("Failed to get randomness for block {}: {:?}", block_number, e),
                    }),
                ).into_response()
            }
        }
    }
}
