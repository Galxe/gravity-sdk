use serde::{Deserialize, Serialize};

type ShardId = usize;
use anyhow::Result;
use aptos_schemadb::{
    define_schema,
    schema::{KeyCodec, ValueCodec},
};
use aptos_types::transaction::Version;

use super::{StateSnapshotProgress, DB_METADATA_CF_NAME};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) enum DbMetadataValue {
    Version(Version),
    StateSnapshotProgress(StateSnapshotProgress),
}

impl DbMetadataValue {
    pub fn expect_version(self) -> Version {
        match self {
            Self::Version(version) => version,
            _ => unreachable!("expected Version, got {:?}", self),
        }
    }

    pub fn expect_state_snapshot_progress(self) -> StateSnapshotProgress {
        match self {
            Self::StateSnapshotProgress(progress) => progress,
            _ => unreachable!("expected KeyHashAndUsage, got {:?}", self),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DbMetadataKey {
    LedgerPrunerProgress,
    StateMerklePrunerProgress,
    EpochEndingStateMerklePrunerProgress,
    StateKvPrunerProgress,
    StateSnapshotKvRestoreProgress(Version),
    LedgerCommitProgress,
    StateKvCommitProgress,
    OverallCommitProgress,
    StateKvShardCommitProgress(ShardId),
    StateMerkleCommitProgress,
    StateMerkleShardCommitProgress(ShardId),
    EventPrunerProgress,
    TransactionAccumulatorPrunerProgress,
    TransactionInfoPrunerProgress,
    TransactionPrunerProgress,
    WriteSetPrunerProgress,
    StateMerkleShardPrunerProgress(ShardId),
    EpochEndingStateMerkleShardPrunerProgress(ShardId),
    StateKvShardPrunerProgress(ShardId),
    StateMerkleShardRestoreProgress(ShardId, Version),
    TransactionAuxiliaryDataPrunerProgress,
}

define_schema!(
    DbMetadataSchema,
    DbMetadataKey,
    DbMetadataValue,
    DB_METADATA_CF_NAME
);

impl KeyCodec<DbMetadataSchema> for DbMetadataKey {
    fn encode_key(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(self)?)
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}

impl ValueCodec<DbMetadataSchema> for DbMetadataValue {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(bcs::to_bytes(self)?)
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Ok(bcs::from_bytes(data)?)
    }
}
