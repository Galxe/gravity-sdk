use axum::{extract::{Path, State}, http::StatusCode, response::Json as JsonResponse};
use serde::{Deserialize, Serialize};
use aptos_consensus::consensusdb::{ConsensusDB, BlockSchema, BlockNumberSchema, EpochByBlockNumberSchema, LedgerInfoSchema};
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::{error, info};
use gaptos::api_types::config_storage::{OnChainConfig, GLOBAL_CONFIG_STORAGE};
use gaptos::aptos_types::on_chain_config::{OnChainConfig as OnChainConfigTrait, ValidatorSet};
use bytes::Bytes;
use crate::https::dkg::DkgState;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug)]
pub struct LedgerInfoResponse {
    pub epoch: u64,
    pub round: u64,
    pub block_number: u64,
    pub block_hash: String, // hex encoded
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockInfo {
    pub epoch: u64,
    pub round: u64,
    pub block_number: Option<u64>,
    pub block_id: String, // hex encoded
    pub parent_id: String, // hex encoded
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QCInfo {
    pub epoch: u64,
    pub round: u64,
    pub block_number: Option<u64>,
    pub certified_block_id: String, // hex encoded
    pub commit_info_block_id: String, // hex encoded - commit_info().id()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidatorCountResponse {
    pub epoch: u64,
    pub block_number: u64,
    pub validator_count: usize,
}

/// Get ledger info by epoch
/// Example: GET /consensus/ledger_info/:epoch
pub async fn get_ledger_info_by_epoch(
    State(dkg_state): State<Arc<DkgState>>,
    Path(epoch): Path<u64>,
) -> Result<(StatusCode, JsonResponse<LedgerInfoResponse>), (StatusCode, JsonResponse<ErrorResponse>)> {
    info!("Getting ledger info for epoch={}", epoch);

    let consensus_db = match dkg_state.consensus_db() {
        Some(db) => db,
        None => {
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, "ConsensusDB is not initialized"));
        }
    };

    // Get all epoch by block number mappings
    let all_epoch_blocks = match consensus_db.get_all::<EpochByBlockNumberSchema>() {
        Ok(blocks) => blocks,
        Err(e) => {
            error!("Failed to get epoch by block number: {:?}", e);
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to get epoch by block number: {:?}", e)));
        }
    };

    // Find the block number for the target epoch
    let target_block_number = all_epoch_blocks
        .into_iter()
        .find(|(_, epoch_)| *epoch_ == epoch)
        .map(|(block_number, _)| block_number)
        .ok_or_else(|| {
            error!("Cannot find block number for epoch {}", epoch);
            error_response(StatusCode::NOT_FOUND, &format!("Cannot find block number for epoch {}", epoch))
        })?;

    // Get the ledger info for the target block number
    match consensus_db.get::<LedgerInfoSchema>(&target_block_number) {
        Ok(Some(ledger_info)) => {
            let ledger_info_inner = ledger_info.ledger_info();
            let response = LedgerInfoResponse {
                epoch: ledger_info_inner.epoch(),
                round: ledger_info_inner.round(),
                block_number: ledger_info_inner.block_number(),
                block_hash: hex::encode(ledger_info_inner.block_hash().as_ref()),
            };
            info!("Successfully retrieved ledger info for epoch={}, block_number={}", epoch, target_block_number);
            Ok((StatusCode::OK, JsonResponse(response)))
        }
        Ok(None) => {
            error!("Ledger info not found for block_number={} (epoch={})", target_block_number, epoch);
            Err(error_response(StatusCode::NOT_FOUND, &format!("Ledger info not found for block_number={} (epoch={})", target_block_number, epoch)))
        }
        Err(e) => {
            error!("Failed to get ledger info for block_number={}: {:?}", target_block_number, e);
            Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to get ledger info: {:?}", e)))
        }
    }
}

/// Get block by epoch and round
/// Example: GET /consensus/block/:epoch/:round
pub async fn get_block(
    State(dkg_state): State<Arc<DkgState>>,
    Path((epoch, round)): Path<(u64, u64)>,
) -> Result<(StatusCode, JsonResponse<BlockInfo>), (StatusCode, JsonResponse<ErrorResponse>)> {
    info!("Getting block for epoch={}, round={}", epoch, round);

    let consensus_db = match dkg_state.consensus_db() {
        Some(db) => db,
        None => {
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, "ConsensusDB is not initialized"));
        }
    };

    // Get block by epoch and round
    match get_block_by_round(&consensus_db, epoch, round) {
        Some(block_info) => {
            info!("Successfully retrieved block for epoch={}, round={}", epoch, round);
            Ok((StatusCode::OK, JsonResponse(block_info)))
        }
        None => {
            error!("Block not found for epoch={}, round={}", epoch, round);
            Err(error_response(StatusCode::NOT_FOUND, &format!("Block not found for epoch={}, round={}", epoch, round)))
        }
    }
}

/// Get QC by epoch and round
/// Example: GET /consensus/qc/:epoch/:round
pub async fn get_qc(
    State(dkg_state): State<Arc<DkgState>>,
    Path((epoch, round)): Path<(u64, u64)>,
) -> Result<(StatusCode, JsonResponse<QCInfo>), (StatusCode, JsonResponse<ErrorResponse>)> {
    info!("Getting QC for epoch={}, round={}", epoch, round);

    let consensus_db = match dkg_state.consensus_db() {
        Some(db) => db,
        None => {
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, "ConsensusDB is not initialized"));
        }
    };

    // Get QC by epoch and round
    match get_qc_by_round(&consensus_db, epoch, round) {
        Some(qc_info) => {
            info!("Successfully retrieved QC for epoch={}, round={}", epoch, round);
            Ok((StatusCode::OK, JsonResponse(qc_info)))
        }
        None => {
            error!("QC not found for epoch={}, round={}", epoch, round);
            Err(error_response(StatusCode::NOT_FOUND, &format!("QC not found for epoch={}, round={}", epoch, round)))
        }
    }
}

/// Get validator count by epoch
/// Example: GET /consensus/validator_count/:epoch
pub async fn get_validator_count_by_epoch(
    State(dkg_state): State<Arc<DkgState>>,
    Path(epoch): Path<u64>,
) -> Result<(StatusCode, JsonResponse<ValidatorCountResponse>), (StatusCode, JsonResponse<ErrorResponse>)> {
    info!("Getting validator count for epoch={}", epoch);

    let consensus_db = match dkg_state.consensus_db() {
        Some(db) => db,
        None => {
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, "ConsensusDB is not initialized"));
        }
    };

    // Get block number for the target epoch
    let all_epoch_blocks = match consensus_db.get_all::<EpochByBlockNumberSchema>() {
        Ok(blocks) => blocks,
        Err(e) => {
            error!("Failed to get epoch by block number: {:?}", e);
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to get epoch by block number: {:?}", e)));
        }
    };

    // Find the block number for the target epoch
    let target_block_number = all_epoch_blocks
        .into_iter()
        .find(|(_, epoch_)| *epoch_ == epoch)
        .map(|(block_number, _)| block_number)
        .ok_or_else(|| {
            error!("Cannot find block number for epoch {}", epoch);
            error_response(StatusCode::NOT_FOUND, &format!("Cannot find block number for epoch {}", epoch))
        })?;

    // Get validator set from config storage using block_number
    let validator_count = match GLOBAL_CONFIG_STORAGE.get() {
        Some(config_storage) => {
            match config_storage.fetch_config_bytes(OnChainConfig::ValidatorSet, target_block_number.into()) {
                Some(config_bytes) => {
                    match config_bytes.try_into() {
                        Ok(bytes) => {
                            let bytes: Bytes = bytes;
                            match ValidatorSet::deserialize_into_config(bytes.as_ref()) {
                                Ok(validator_set) => {
                                    let count = validator_set.active_validators.len();
                                    info!("Epoch {} validator count: {}", epoch, count);
                                    count
                                }
                                Err(e) => {
                                    error!("Failed to deserialize ValidatorSet: {:?}", e);
                                    return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to deserialize ValidatorSet: {:?}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to convert config bytes: {:?}", e);
                            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to convert config bytes: {:?}", e)));
                        }
                    }
                }
                None => {
                    error!("ValidatorSet not found for block_number {}", target_block_number);
                    return Err(error_response(StatusCode::NOT_FOUND, &format!("ValidatorSet not found for block_number {}", target_block_number)));
                }
            }
        }
        None => {
            error!("GLOBAL_CONFIG_STORAGE is not initialized");
            return Err(error_response(StatusCode::INTERNAL_SERVER_ERROR, "GLOBAL_CONFIG_STORAGE is not initialized"));
        }
    };

    let response = ValidatorCountResponse {
        epoch,
        block_number: target_block_number,
        validator_count,
    };

    Ok((StatusCode::OK, JsonResponse(response)))
}

/// Helper function to get block by epoch and round
fn get_block_by_round(consensus_db: &ConsensusDB, epoch: u64, round: u64) -> Option<BlockInfo> {
    let start_key = (epoch, HashValue::zero());
    let end_key = (epoch, HashValue::new([u8::MAX; HashValue::LENGTH]));

    // Get all blocks in this epoch and filter by round
    match consensus_db.get_range::<BlockSchema>(&start_key, &end_key) {
        Ok(blocks) => {
            // Find block with matching round
            for ((_, _), block) in blocks {
                if block.round() == round {
                    // Try to get block number if not set
                    let block_number = if block.block_number().is_none() {
                        consensus_db
                            .get::<BlockNumberSchema>(&(epoch, block.id()))
                            .ok()
                            .flatten()
                    } else {
                        block.block_number()
                    };

                    return Some(BlockInfo {
                        epoch: block.epoch(),
                        round: block.round(),
                        block_number,
                        block_id: hex::encode(block.id().as_ref()),
                        parent_id: hex::encode(block.parent_id().as_ref()),
                    });
                }
            }
            None
        }
        Err(e) => {
            error!("Failed to get blocks: {:?}", e);
            None
        }
    }
}

/// Helper function to get QC by epoch and round
fn get_qc_by_round(consensus_db: &ConsensusDB, epoch: u64, round: u64) -> Option<QCInfo> {
    let start_key = (epoch, HashValue::zero());
    let end_key = (epoch, HashValue::new([u8::MAX; HashValue::LENGTH]));

    // Get all QCs in this epoch and filter by round
    match consensus_db.get_qc_range(&start_key, &end_key) {
        Ok(qcs) => {
            // Find QC with matching round
            for qc in qcs {
                if qc.certified_block().round() == round {
                    // Try to get block number for the certified block
                    let block_number = consensus_db
                        .get::<BlockNumberSchema>(&(epoch, qc.certified_block().id()))
                        .ok()
                        .flatten();

                    return Some(QCInfo {
                        epoch: qc.certified_block().epoch(),
                        round: qc.certified_block().round(),
                        block_number,
                        certified_block_id: hex::encode(qc.certified_block().id().as_ref()),
                        commit_info_block_id: hex::encode(qc.commit_info().id().as_ref()),
                    });
                }
            }
            None
        }
        Err(e) => {
            error!("Failed to get QCs: {:?}", e);
            None
        }
    }
}

/// Helper function to create error response
fn error_response(status: StatusCode, message: &str) -> (StatusCode, JsonResponse<ErrorResponse>) {
    (
        status,
        JsonResponse(ErrorResponse {
            error: message.to_string(),
        }),
    )
}
