// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod consensusdb_test;
mod ledger_db;
mod schema;

use crate::error::DbError;
use anyhow::Result;
use aptos_consensus_types::{block::Block, quorum_cert::QuorumCert};
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::prelude::*;
use gaptos::aptos_schemadb::{
    schema::{KeyCodec, Schema},
    Options, batch::SchemaBatch, DB, DEFAULT_COLUMN_FAMILY_NAME,
};
use gaptos::aptos_storage_interface::AptosDbError;
use ledger_db::LedgerDb;
use rocksdb::ReadOptions;
pub use schema::{
    block::BlockNumberSchema,
    block::BlockSchema,
    dag::{CertifiedNodeSchema, DagVoteSchema, NodeSchema},
    quorum_certificate::QCSchema,
};
use schema::{
    block::BLOCK_NUMBER_CF_NAME,
    single_entry::{SingleEntryKey, SingleEntrySchema},
    BLOCK_CF_NAME, CERTIFIED_NODE_CF_NAME, DAG_VOTE_CF_NAME, LEDGER_INFO_CF_NAME, NODE_CF_NAME,
    QC_CF_NAME, SINGLE_ENTRY_CF_NAME,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    iter::Iterator,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

/// The name of the consensus db file
pub const CONSENSUS_DB_NAME: &str = "consensus_db";
const RECENT_BLOCKS_RANGE: u64 = 256;

/// Creates new physical DB checkpoint in directory specified by `checkpoint_path`.
pub fn create_checkpoint<P: AsRef<Path> + Clone>(db_path: P, checkpoint_path: P) -> Result<()> {
    let start = Instant::now();
    let consensus_db_checkpoint_path = checkpoint_path.as_ref().join(CONSENSUS_DB_NAME);
    std::fs::remove_dir_all(&consensus_db_checkpoint_path).unwrap_or(());
    ConsensusDB::new(db_path, &PathBuf::new())
        .db
        .create_checkpoint(&consensus_db_checkpoint_path)?;
    info!(
        path = consensus_db_checkpoint_path,
        time_ms = %start.elapsed().as_millis(),
        "Made ConsensusDB checkpoint."
    );
    Ok(())
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GravityNodeConfig {
    pub consensus_public_key: String,
    pub account_address: String,
    pub network_public_key: String,
    pub trusted_peers_map: Vec<String>,
    pub public_ip_address: String,
    pub voting_power: u64,
}

pub type GravityNodeConfigSet = BTreeMap<String, GravityNodeConfig>;

/// Loads a config configuration file
fn load_file(path: &Path) -> GravityNodeConfigSet {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_yaml::from_str(&contents).unwrap()
}

pub struct ConsensusDB {
    db: Arc<DB>,
    pub node_config_set: GravityNodeConfigSet,
    pub ledger_db: LedgerDb,
}

impl ConsensusDB {
    pub fn new<P: AsRef<Path> + Clone>(db_root_path: P, node_config_path: &PathBuf) -> Self {
        let column_families = vec![
            /* UNUSED CF = */ DEFAULT_COLUMN_FAMILY_NAME,
            BLOCK_CF_NAME,
            QC_CF_NAME,
            SINGLE_ENTRY_CF_NAME,
            NODE_CF_NAME,
            CERTIFIED_NODE_CF_NAME,
            DAG_VOTE_CF_NAME,
            LEDGER_INFO_CF_NAME,
            BLOCK_NUMBER_CF_NAME,
            "ordered_anchor_id", // deprecated CF
        ];

        let path = db_root_path.as_ref().join(CONSENSUS_DB_NAME);
        println!("consensun path : {:?}", path);
        let instant = Instant::now();
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let db = Arc::new(
            DB::open(path.clone(), "consensus", column_families, &opts)
                .expect("ConsensusDB open failed; unable to continue"),
        );

        info!("Opened ConsensusDB at {:?} in {} ms", path, instant.elapsed().as_millis());
        let mut node_config_set = BTreeMap::new();
        if node_config_path.to_str().is_some() && !node_config_path.to_str().unwrap().is_empty() {
            node_config_set = load_file(node_config_path.as_path());
        }

        let ledger_db = LedgerDb::new(db.clone());

        Self { db, node_config_set, ledger_db }
    }

    pub fn get_data(
        &self,
        latest_block_number: u64,
    ) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>, Vec<Block>, Vec<QuorumCert>)> {
        let last_vote = self.get_last_vote()?;
        let highest_2chain_timeout_certificate = self.get_highest_2chain_timeout_certificate()?;
        let block_number_to_block_id = self
            .get_all::<BlockNumberSchema>()?
            .into_iter()
            .filter(|(_, block_number)| block_number >= &latest_block_number)
            .map(|(block_id, block_number)| (block_number, block_id))
            .collect::<HashMap<u64, HashValue>>();
        let block_id_to_block_number = block_number_to_block_id
            .iter()
            .map(|(block_number, block_id)| (*block_id, *block_number))
            .collect::<HashMap<HashValue, u64>>();
        let start_round = if block_number_to_block_id.contains_key(&latest_block_number) {
            self.get::<BlockSchema>(&block_number_to_block_id[&latest_block_number])?
                .unwrap()
                .round()
        } else {
            0
        };
        let mut consensus_blocks: Vec<_> = self
            .get_all::<BlockSchema>()?
            .into_iter()
            .map(|(_, block)| block)
            .filter(|block| block.round() >= start_round)
            .collect();
        consensus_blocks.iter_mut().for_each(|block| {
            if block.block_number().is_none() {
                if let Some(block_number) = block_id_to_block_number.get(&block.id()) {
                    block.set_block_number(*block_number);
                }
            }
        });
        let consensus_qcs: Vec<_> = self
            .get_all::<QCSchema>()?
            .into_iter()
            .map(|(_, qc)| qc)
            .filter(|qc| qc.certified_block().round() >= start_round)
            .collect();
        info!("consensus_blocks size : {}, consensus_qcs size : {}, block_number_to_block_id size : {}",
                 consensus_blocks.len(), consensus_qcs.len(), block_number_to_block_id.len());
        println!("qcs : {:?}", consensus_qcs);
        Ok((last_vote, highest_2chain_timeout_certificate, consensus_blocks, consensus_qcs))
    }

    pub fn save_highest_2chain_timeout_certificate(&self, tc: Vec<u8>) -> Result<(), DbError> {
        let mut batch = SchemaBatch::new();
        batch.put::<SingleEntrySchema>(&SingleEntryKey::Highest2ChainTimeoutCert, &tc)?;
        self.commit(batch)?;
        Ok(())
    }

    pub fn save_vote(&self, last_vote: Vec<u8>) -> Result<(), DbError> {
        let mut batch = SchemaBatch::new();
        batch.put::<SingleEntrySchema>(&SingleEntryKey::LastVote, &last_vote)?;
        self.commit(batch)
    }

    pub fn save_blocks_and_quorum_certificates(
        &self,
        block_data: Vec<Block>,
        qc_data: Vec<QuorumCert>,
    ) -> Result<(), DbError> {
        if block_data.is_empty() && qc_data.is_empty() {
            return Ok(());
        }
        let mut batch = SchemaBatch::new();
        block_data.iter().try_for_each(|block| batch.put::<BlockSchema>(&block.id(), block))?;
        qc_data.iter().try_for_each(|qc| batch.put::<QCSchema>(&qc.certified_block().id(), qc))?;
        self.commit(batch)
    }

    pub fn save_block_numbers(&self, block_numbers: Vec<(u64, HashValue)>) -> Result<(), DbError> {
        if block_numbers.is_empty() {
            return Ok(());
        }
        let mut batch = SchemaBatch::new();
        block_numbers.iter().try_for_each(|(block_number, block_id)| {
            batch.put::<BlockNumberSchema>(block_id, block_number)
        })?;
        self.commit(batch)
    }

    pub fn delete_blocks_and_quorum_certificates(
        &self,
        block_ids: Vec<HashValue>,
    ) -> Result<(), DbError> {
        if block_ids.is_empty() {
            return Err(anyhow::anyhow!("Consensus block ids is empty!").into());
        }
        let mut batch = SchemaBatch::new();
        block_ids.iter().try_for_each(|hash| {
            batch.delete::<BlockSchema>(hash)?;
            batch.delete::<QCSchema>(hash)
        })?;
        self.commit(batch)
    }

    /// Write the whole schema batch including all data necessary to mutate the ledger
    /// state of some transaction by leveraging rocksdb atomicity support.
    fn commit(&self, batch: SchemaBatch) -> Result<(), DbError> {
        self.db.write_schemas(batch)?;
        Ok(())
    }

    /// Get latest timeout certificates (we only store the latest highest timeout certificates).
    fn get_highest_2chain_timeout_certificate(&self) -> Result<Option<Vec<u8>>, DbError> {
        Ok(self.db.get::<SingleEntrySchema>(&SingleEntryKey::Highest2ChainTimeoutCert)?)
    }

    pub fn delete_highest_2chain_timeout_certificate(&self) -> Result<(), DbError> {
        let mut batch = SchemaBatch::new();
        batch.delete::<SingleEntrySchema>(&SingleEntryKey::Highest2ChainTimeoutCert)?;
        self.commit(batch)
    }

    /// Get serialized latest vote (if available)
    fn get_last_vote(&self) -> Result<Option<Vec<u8>>, DbError> {
        Ok(self.db.get::<SingleEntrySchema>(&SingleEntryKey::LastVote)?)
    }

    pub fn delete_last_vote_msg(&self) -> Result<(), DbError> {
        let mut batch = SchemaBatch::new();
        batch.delete::<SingleEntrySchema>(&SingleEntryKey::LastVote)?;
        self.commit(batch)?;
        Ok(())
    }

    pub fn put<S: Schema>(&self, key: &S::Key, value: &S::Value) -> Result<(), DbError> {
        let mut batch = SchemaBatch::new();
        batch.put::<S>(key, value)?;
        self.commit(batch)?;
        Ok(())
    }

    pub fn delete<S: Schema>(&self, keys: Vec<S::Key>) -> Result<(), DbError> {
        let mut batch = SchemaBatch::new();
        keys.iter().try_for_each(|key| batch.delete::<S>(key))?;
        self.commit(batch)
    }

    pub fn get_all<S: Schema>(&self) -> Result<Vec<(S::Key, S::Value)>, DbError> {
        let mut iter = self.db.iter::<S>()?;
        iter.seek_to_first();
        Ok(iter.collect::<Result<Vec<(S::Key, S::Value)>, AptosDbError>>()?)
    }

    pub fn get<S: Schema>(&self, key: &S::Key) -> Result<Option<S::Value>, DbError> {
        Ok(self.db.get::<S>(key)?)
    }

    pub fn get_block(&self, block_id: &HashValue) -> Result<Option<Block>, DbError> {
        let block = self.get::<BlockSchema>(block_id)?;
        if let Some(block) = &block {
            let block_number = self.get::<BlockNumberSchema>(block_id)?;
            match block_number {
                Some(block_number) => block.set_block_number(block_number),
                None => (),
            }
        }
        Ok(block)
    }
}

include!("include/reader.rs");
include!("include/writer.rs");

#[cfg(test)]
mod test {
    use gaptos::aptos_crypto::ed25519::Ed25519PrivateKey;
    use gaptos::aptos_crypto::ed25519::Ed25519PublicKey;
    use gaptos::aptos_crypto::test_utils::KeyPair;
    use gaptos::aptos_crypto::{bls12381, x25519, PrivateKey};

    #[test]
    fn gen_account_private_key() {
        let current_dir = env!("CARGO_MANIFEST_DIR").to_string() + "/../../deploy_utils/";
        let path = current_dir.clone() + "four_nodes_config.json";
        let node_config_set = load_file(Path::new(&path));
        node_config_set.iter().for_each(|(addr, config)| {
            let mut rng = thread_rng();
            let kp = KeyPair::<Ed25519PrivateKey, Ed25519PublicKey>::generate(&mut rng);
            println!(
                "{} private key {}, public key {}",
                addr,
                hex::encode(kp.private_key.to_bytes().as_slice()).as_str(),
                kp.public_key.to_string()
            )
        });
    }

    use gaptos::aptos_crypto::{Uniform, ValidCryptoMaterial};
    use rand::thread_rng;
    use std::path::Path;

    use super::load_file;

    #[test]
    fn println_consensus_pri_key() {
        for _ in 0..2 {
            let mut rng = thread_rng();
            let private_key = bls12381::PrivateKey::generate(&mut rng);
            println!(
                "consensus private key {:?}, public key {}",
                private_key.to_bytes(),
                private_key.public_key().to_string()
            );
        }
    }

    #[test]
    fn println_network_pri_key() {
        for _ in 0..2 {
            let mut rng = thread_rng();
            let private_key = x25519::PrivateKey::generate(&mut rng);
            println!(
                "network private key {:?}, public key {}",
                private_key.to_bytes(),
                private_key.public_key().to_string()
            );
        }
    }
}
