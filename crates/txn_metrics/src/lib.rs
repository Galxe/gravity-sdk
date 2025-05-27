use std::{
    collections::{hash_map::DefaultHasher, HashSet}, // Needed for DashMap entry key hashing if not directly supported
    hash::{Hash, Hasher},
    sync::OnceLock,
    time::SystemTime,
};

use aptos_consensus_types::{
    common::{Payload, ProofWithData},
    proof_of_store::BatchId,
};
use dashmap::DashMap;
use gaptos::{ // Assuming these are correct and available in your gaptos crate
    aptos_crypto::HashValue,
    aptos_metrics_core::{register_histogram, Histogram},
    aptos_types::transaction::SignedTransaction,
};

// --- Histograms (All "Added to X" focus) ---

static TXN_ADDED_TO_BATCH_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_batch_histogram() -> &'static Histogram {
    TXN_ADDED_TO_BATCH_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_batch_time_seconds",
            "Time from transaction added to included in a batch (seconds)",
            vec![0.0, 0.01, 0.02, 0.03, 0.05, 0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_PROOF_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_proof_histogram() -> &'static Histogram {
    TXN_ADDED_TO_PROOF_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_proof_time_seconds",
            "Time from transaction added to proof generated (seconds)",
            vec![0.0, 0.01, 0.02, 0.03, 0.05, 0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_BLOCK_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_block_histogram() -> &'static Histogram {
    TXN_ADDED_TO_BLOCK_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_block_time_seconds",
            "Total time from transaction added to included in a block (seconds)",
            vec![0.0, 0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0]
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_COMMITTED_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_committed_histogram() -> &'static Histogram {
    TXN_ADDED_TO_COMMITTED_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_committed_time_seconds",
            "Total time from transaction added to committed (seconds)",
            vec![0.0, 0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0]
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_EXECUTING_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_executing_histogram() -> &'static Histogram {
    TXN_ADDED_TO_EXECUTING_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_executing_time_seconds",
            "Time from transaction added to executing (seconds)",
            vec![0.0, 0.01, 0.02, 0.03, 0.05, 0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_EXECUTED_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> = 
    std::sync::OnceLock::new();
fn get_txn_added_to_executed_histogram() -> &'static Histogram {
    TXN_ADDED_TO_EXECUTED_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_executed_time_seconds",
            "Time from transaction added to executed (seconds)",
            vec![0.0, 0.01, 0.02, 0.03, 0.05, 0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_BLOCK_COMMITTED_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_block_committed_histogram() -> &'static Histogram {
    TXN_ADDED_TO_BLOCK_COMMITTED_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_block_committed_time_seconds",
            "Time from transaction added to block committed (seconds)",
            vec![0.0, 0.01, 0.02, 0.03, 0.05, 0.1, 0.2, 0.3, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]
        )
        .unwrap()
    })
}

pub struct TxnLifeTime {
    txn_initial_add_time: DashMap<HashValue, SystemTime>,
    txn_batch_id: DashMap<BatchId, HashSet<HashValue>>, // Tracks txns in a batch
    txn_block_id: DashMap<HashValue, HashSet<HashValue>>, // Tracks txns in a block (block_id is HashValue)
}

static INSTANCE: OnceLock<TxnLifeTime> = OnceLock::new();

impl TxnLifeTime {
    pub fn get_txn_life_time() -> &'static TxnLifeTime {
        INSTANCE.get_or_init(|| TxnLifeTime {
            txn_initial_add_time: DashMap::new(),
            txn_batch_id: DashMap::new(),
            txn_block_id: DashMap::new(),
        })
    }

    pub fn record_added(&self, txn_hash: HashValue) {
        let now = SystemTime::now();
        // Only insert initial add time if it's not already there.
        // Useful if record_added might be called multiple times for the same transaction.
        self.txn_initial_add_time.entry(txn_hash).or_insert(now);
    }

    pub fn record_batch(&self, batch_id: BatchId, batch: &Vec<SignedTransaction>) {
        let now = SystemTime::now();
        let mut current_batch_txn_hashes = HashSet::with_capacity(batch.len());
        for txn in batch.iter() {
            let txn_hash = txn.committed_hash();
            if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
                if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                    get_txn_added_to_batch_histogram().observe(duration.as_secs_f64());
                }
            }
            current_batch_txn_hashes.insert(txn_hash);
        }
        if !current_batch_txn_hashes.is_empty() {
            self.txn_batch_id.insert(batch_id, current_batch_txn_hashes);
        }
    }

    pub fn record_proof(&self, batch_id: BatchId) {
        let now = SystemTime::now();
        if let Some(txn_hashes_entry) = self.txn_batch_id.get(&batch_id) {
            for &txn_hash in txn_hashes_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_proof_histogram().observe(duration.as_secs_f64());
                    }
                }
                // No state update needed in txn_time anymore
            }
        }
    }

    // Helper function to avoid code duplication in record_block paths for "Added to Block"
    fn observe_added_to_block(&self, txn_hash: HashValue, block_time: SystemTime) {
        if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
            if let Ok(duration) = block_time.duration_since(*initial_add_time_entry.value()) {
                get_txn_added_to_block_histogram().observe(duration.as_secs_f64());
            }
        }
    }
    
    fn process_proof_with_data(&self, proof_with_data: &ProofWithData, block_id: HashValue, block_record_time: SystemTime) {
        let mut current_block_txn_hashes = vec![];
        for p in &proof_with_data.proofs {
            let batch_id = p.batch_id();
            if let Some(txn_hashes_entry) = self.txn_batch_id.get(&batch_id) {
                for &txn_hash in txn_hashes_entry.value().iter() {
                    self.observe_added_to_block(txn_hash, block_record_time);
                    current_block_txn_hashes.push(txn_hash);
                }
            }
        }
        if !current_block_txn_hashes.is_empty() {
             // Use entry().or_default().extend() to append if block_id already has txns from other sources (e.g. hybrid)
            self.txn_block_id.entry(block_id).or_default().extend(current_block_txn_hashes);
        }
    }

    pub fn record_block(&self, payload: Option<&Payload>, block_id: HashValue) {
        let now = SystemTime::now(); // Time this block is being processed/recorded
        if let Some(payload) = payload {
            match payload {
                Payload::DirectMempool(txns) => {
                    let mut current_block_txn_hashes = HashSet::with_capacity(txns.len());
                    for txn in txns.iter() {
                        let txn_hash = txn.committed_hash();
                        self.observe_added_to_block(txn_hash, now);
                        current_block_txn_hashes.insert(txn_hash);
                    }
                    if !current_block_txn_hashes.is_empty() {
                        self.txn_block_id.insert(block_id, current_block_txn_hashes);
                    }
                }
                Payload::InQuorumStore(proof_with_data) => {
                    self.process_proof_with_data(proof_with_data, block_id, now);
                }
                Payload::InQuorumStoreWithLimit(proof_with_data_with_txn_limit) => {
                    self.process_proof_with_data(
                        &proof_with_data_with_txn_limit.proof_with_data,
                        block_id,
                        now,
                    );
                }
                Payload::QuorumStoreInlineHybrid(vec_payload, proof_with_data, _) => {
                    // Process proof part first
                    self.process_proof_with_data(proof_with_data, block_id, now);
                    
                    // Process inline transactions part
                    let mut inline_txn_hashes = vec![];
                    for (_, txn_hashes_in_vec) in vec_payload {
                        for txn in txn_hashes_in_vec.iter() {
                            let txn_hash = txn.committed_hash();
                            self.observe_added_to_block(txn_hash, now);
                            inline_txn_hashes.push(txn_hash);
                        }
                    }
                    if !inline_txn_hashes.is_empty() {
                        self.txn_block_id.entry(block_id).or_default().extend(inline_txn_hashes);
                    }
                }
                Payload::OptQuorumStore(_) => {
                    
                }
            }
        }
    }

    pub fn record_executing(&self, block_id: HashValue) {
        let now = SystemTime::now();
        if let Some(txn_hashes_entry) = self.txn_block_id.get(&block_id) {
            for &txn_hash in txn_hashes_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_executing_histogram().observe(duration.as_secs_f64());
                    }
                }
            }
        }
    }

    pub fn record_executed(&self, block_id: HashValue) {
        let now = SystemTime::now();
        if let Some(txn_hashes_entry) = self.txn_block_id.get(&block_id) {
            for &txn_hash in txn_hashes_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_executed_histogram().observe(duration.as_secs_f64());
                    }
                }
            }
        }
    }

    pub fn record_block_committed(&self, block_id: HashValue) {
        let now = SystemTime::now();
        if let Some(txn_hashes_entry) = self.txn_block_id.get(&block_id) {
            for &txn_hash in txn_hashes_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_block_committed_histogram().observe(duration.as_secs_f64());
                    }
                }
            }
        }
    }

    pub fn record_committed(&self, txn_hash: HashValue) {
        let now = SystemTime::now();
        if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_hash) {
            if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                get_txn_added_to_committed_histogram().observe(duration.as_secs_f64());
            }
        }
        self.txn_initial_add_time.remove(&txn_hash);
    
        for mut entry in self.txn_batch_id.iter_mut() {
            entry.value_mut().remove(&txn_hash);
        }
        self.txn_batch_id.retain(|_batch_id, txn_set| !txn_set.is_empty());
    
        for mut entry in self.txn_block_id.iter_mut() {
            entry.value_mut().remove(&txn_hash);
        }
        self.txn_block_id.retain(|_block_id, txn_set| !txn_set.is_empty());
    }
}