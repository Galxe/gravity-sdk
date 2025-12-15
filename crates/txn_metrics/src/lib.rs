use std::{
    collections::{hash_map::DefaultHasher, HashSet}, // Needed for DashMap entry key hashing if not directly supported
    hash::{Hash, Hasher},
    sync::OnceLock,
    time::SystemTime,
};

use gaptos::aptos_types::account_address::AccountAddress;

use aptos_consensus_types::{
    common::{Payload, ProofWithData},
    proof_of_store::BatchId,
};
use dashmap::DashMap;
use gaptos::{
    // Assuming these are correct and available in your gaptos crate
    aptos_crypto::HashValue,
    aptos_metrics_core::{register_histogram, Histogram},
    aptos_types::transaction::SignedTransaction,
};

const BUCKETS: [f64; 46] = [
    0.0001, 0.0002, 0.0003, 0.0004, 0.0005, 0.0006, 0.0007, 0.0008, 0.0009, 0.001, 0.002, 0.003,
    0.004, 0.005, 0.006, 0.007, 0.008, 0.009, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08, 0.09,
    0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0,
];

// --- Histograms (All "Added to X" focus) ---

static TXN_ADDED_TO_BATCH_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_batch_histogram() -> &'static Histogram {
    TXN_ADDED_TO_BATCH_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_batch_time_seconds",
            "Time from transaction added to included in a batch (seconds)",
            BUCKETS.to_vec()
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_BEFORE_BATCH_PERSIST_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_before_batch_persist_histogram() -> &'static Histogram {
    TXN_ADDED_TO_BEFORE_BATCH_PERSIST_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_before_batch_persist_time_seconds",
            "Time from transaction added to proof generated (seconds)",
            BUCKETS.to_vec()
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_AFTER_BATCH_PERSIST_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_batch_persist_histogram() -> &'static Histogram {
    TXN_ADDED_TO_AFTER_BATCH_PERSIST_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_after_batch_persist_time_seconds",
            "Time from transaction added to proof generated (seconds)",
            BUCKETS.to_vec()
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
            BUCKETS.to_vec()
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
            BUCKETS.to_vec()
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
            BUCKETS.to_vec()
        )
        .unwrap()
    })
}

static TXN_ADDED_TO_BROADCAST_BATCH_TIME_HISTOGRAM: std::sync::OnceLock<Histogram> =
    std::sync::OnceLock::new();
fn get_txn_added_to_broadcast_batch_histogram() -> &'static Histogram {
    TXN_ADDED_TO_BROADCAST_BATCH_TIME_HISTOGRAM.get_or_init(|| {
        register_histogram!(
            "aptos_txn_added_to_broadcast_batch_time_seconds",
            "Time from transaction added to broadcast batch (seconds)",
            BUCKETS.to_vec()
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
            BUCKETS.to_vec()
        )
        .unwrap()
    })
}

// Capacity limits to prevent unbounded memory growth
const MAX_TXN_INITIAL_ADD_TIME_CAPACITY: usize = 100_000;
const MAX_TXN_BATCH_ID_CAPACITY: usize = 10_000;
const MAX_TXN_BLOCK_ID_CAPACITY: usize = 1_000;

// Key type for transaction tracking: (AccountAddress, sequence_number)
type TxnKey = (AccountAddress, u64);

pub struct TxnLifeTime {
    // Primary storage: (address, nonce) -> initial add time
    txn_initial_add_time: DashMap<TxnKey, SystemTime>,
    // Reverse mapping: hash -> (address, nonce) for batch/block recording
    txn_hash_to_key: DashMap<HashValue, TxnKey>,
    // Tracks txns in a batch
    txn_batch_id: DashMap<BatchId, HashSet<TxnKey>>,
    // Tracks txns in a block (block_id is HashValue)
    txn_block_id: DashMap<HashValue, HashSet<TxnKey>>,
}

static INSTANCE: OnceLock<TxnLifeTime> = OnceLock::new();

/// Check if TxnLife feature is enabled via environment variable.
/// Reads `TXN_LIFE_ENABLED` env var. Defaults to false (disabled).
static TXN_LIFE_ENABLED: OnceLock<bool> = OnceLock::new();

fn is_txn_life_enabled() -> bool {
    *TXN_LIFE_ENABLED.get_or_init(|| {
        std::env::var("TXN_LIFE_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

impl TxnLifeTime {
    pub fn get_txn_life_time() -> &'static TxnLifeTime {
        INSTANCE.get_or_init(|| TxnLifeTime {
            txn_initial_add_time: DashMap::new(),
            txn_hash_to_key: DashMap::new(),
            txn_batch_id: DashMap::new(),
            txn_block_id: DashMap::new(),
        })
    }

    pub fn record_added(&self, txn: &SignedTransaction) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        let txn_key = (txn.sender(), txn.sequence_number());
        let txn_hash = txn.committed_hash();

        // Check capacity before inserting
        if self.txn_initial_add_time.len() >= MAX_TXN_INITIAL_ADD_TIME_CAPACITY {
            self.cleanup_old_entries();
        }

        // Store the mapping from hash to key
        self.txn_hash_to_key.insert(txn_hash, txn_key);

        // Only insert initial add time if it's not already there.
        // Useful if record_added might be called multiple times for the same transaction.
        self.txn_initial_add_time.entry(txn_key).or_insert(now);
    }

    pub fn record_batch(&self, batch_id: BatchId, batch: &Vec<SignedTransaction>) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        let mut current_batch_txn_keys = HashSet::with_capacity(batch.len());
        for txn in batch.iter() {
            let txn_key = (txn.sender(), txn.sequence_number());
            let txn_hash = txn.committed_hash();

            // Update hash to key mapping
            self.txn_hash_to_key.insert(txn_hash, txn_key);

            if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                    get_txn_added_to_batch_histogram().observe(duration.as_secs_f64());
                }
            }
            current_batch_txn_keys.insert(txn_key);
        }
        if !current_batch_txn_keys.is_empty() {
            // Check capacity before inserting
            if self.txn_batch_id.len() >= MAX_TXN_BATCH_ID_CAPACITY {
                self.cleanup_old_entries();
            }
            self.txn_batch_id.insert(batch_id, current_batch_txn_keys);
        }
    }

    pub fn record_broadcast_batch(&self, batch_id: BatchId) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_batch_id.get(&batch_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_broadcast_batch_histogram()
                            .observe(duration.as_secs_f64());
                    }
                }
                // No state update needed in txn_time anymore
            }
        }
    }

    pub fn record_before_persist(&self, batch_id: BatchId) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_batch_id.get(&batch_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_before_batch_persist_histogram()
                            .observe(duration.as_secs_f64());
                    }
                }
                // No state update needed in txn_time anymore
            }
        }
    }

    pub fn record_after_persist(&self, batch_id: BatchId) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_batch_id.get(&batch_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_batch_persist_histogram().observe(duration.as_secs_f64());
                    }
                }
                // No state update needed in txn_time anymore
            }
        }
    }

    pub fn record_proof(&self, batch_id: BatchId) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_batch_id.get(&batch_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_proof_histogram().observe(duration.as_secs_f64());
                    }
                }
                // No state update needed in txn_time anymore
            }
        }
    }

    // Helper function to avoid code duplication in record_block paths for "Added to Block"
    fn observe_added_to_block(&self, txn_key: TxnKey, block_time: SystemTime) {
        if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
            if let Ok(duration) = block_time.duration_since(*initial_add_time_entry.value()) {
                get_txn_added_to_block_histogram().observe(duration.as_secs_f64());
            }
        }
    }

    fn process_proof_with_data(
        &self,
        proof_with_data: &ProofWithData,
        block_id: HashValue,
        block_record_time: SystemTime,
    ) {
        let mut current_block_txn_keys = vec![];
        for p in &proof_with_data.proofs {
            let batch_id = p.batch_id();
            if let Some(txn_keys_entry) = self.txn_batch_id.get(&batch_id) {
                for &txn_key in txn_keys_entry.value().iter() {
                    self.observe_added_to_block(txn_key, block_record_time);
                    current_block_txn_keys.push(txn_key);
                }
            }
        }
        if !current_block_txn_keys.is_empty() {
            // Use entry().or_default().extend() to append if block_id already has txns from other sources (e.g. hybrid)
            self.txn_block_id.entry(block_id).or_default().extend(current_block_txn_keys);
        }
    }

    pub fn record_block(&self, payload: Option<&Payload>, block_id: HashValue) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now(); // Time this block is being processed/recorded
        if let Some(payload) = payload {
            match payload {
                Payload::DirectMempool(txns) => {
                    let mut current_block_txn_keys = HashSet::with_capacity(txns.len());
                    for txn in txns.iter() {
                        let txn_key = (txn.sender(), txn.sequence_number());
                        let txn_hash = txn.committed_hash();
                        self.txn_hash_to_key.insert(txn_hash, txn_key);
                        self.observe_added_to_block(txn_key, now);
                        current_block_txn_keys.insert(txn_key);
                    }
                    if !current_block_txn_keys.is_empty() {
                        self.txn_block_id.insert(block_id, current_block_txn_keys);
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
                    let mut inline_txn_keys = vec![];
                    for (_, txn_hashes_in_vec) in vec_payload {
                        for txn in txn_hashes_in_vec.iter() {
                            let txn_key = (txn.sender(), txn.sequence_number());
                            let txn_hash = txn.committed_hash();
                            self.txn_hash_to_key.insert(txn_hash, txn_key);
                            self.observe_added_to_block(txn_key, now);
                            inline_txn_keys.push(txn_key);
                        }
                    }
                    if !inline_txn_keys.is_empty() {
                        self.txn_block_id.entry(block_id).or_default().extend(inline_txn_keys);
                    }
                }
                Payload::OptQuorumStore(_) => {}
            }
        }
    }

    pub fn record_executing(&self, block_id: HashValue) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_block_id.get(&block_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_executing_histogram().observe(duration.as_secs_f64());
                    }
                }
            }
        }
    }

    pub fn record_executed(&self, block_id: HashValue) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_block_id.get(&block_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_executed_histogram().observe(duration.as_secs_f64());
                    }
                }
            }
        }
    }

    pub fn record_block_committed(&self, block_id: HashValue) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        if let Some(txn_keys_entry) = self.txn_block_id.get(&block_id) {
            for &txn_key in txn_keys_entry.value().iter() {
                if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
                    if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                        get_txn_added_to_block_committed_histogram()
                            .observe(duration.as_secs_f64());
                    }
                }
            }
        }
    }

    pub fn record_committed(&self, sender: &AccountAddress, sequence_number: u64) {
        if !is_txn_life_enabled() {
            return;
        }
        let now = SystemTime::now();
        let txn_key = (*sender, sequence_number);

        if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
            if let Ok(duration) = now.duration_since(*initial_add_time_entry.value()) {
                get_txn_added_to_committed_histogram().observe(duration.as_secs_f64());
            }
        }

        // Remove from primary storage
        self.txn_initial_add_time.remove(&txn_key);

        // Remove from hash mapping (find and remove all hashes for this key)
        self.txn_hash_to_key.retain(|_hash, key| key != &txn_key);

        // Remove from batch tracking
        for mut entry in self.txn_batch_id.iter_mut() {
            entry.value_mut().remove(&txn_key);
        }
        self.txn_batch_id.retain(|_batch_id, txn_set| !txn_set.is_empty());

        // Remove from block tracking
        for mut entry in self.txn_block_id.iter_mut() {
            entry.value_mut().remove(&txn_key);
        }
        self.txn_block_id.retain(|_block_id, txn_set| !txn_set.is_empty());
    }

    /// Cleanup old entries when capacity limits are exceeded
    /// This removes the oldest 20% of entries based on insertion time
    fn cleanup_old_entries(&self) {
        let now = SystemTime::now();

        // Cleanup txn_initial_add_time if over capacity
        if self.txn_initial_add_time.len() >= MAX_TXN_INITIAL_ADD_TIME_CAPACITY {
            // Collect entries older than 60 seconds
            let mut old_keys = Vec::new();
            for entry in self.txn_initial_add_time.iter() {
                if let Ok(duration) = now.duration_since(*entry.value()) {
                    if duration.as_secs() > 60 {
                        old_keys.push(*entry.key());
                    }
                }
            }

            // Remove old entries
            for key in old_keys {
                self.txn_initial_add_time.remove(&key);
                // Also remove from hash mapping
                self.txn_hash_to_key.retain(|_hash, k| k != &key);
            }
        }

        // Cleanup txn_batch_id if over capacity
        if self.txn_batch_id.len() >= MAX_TXN_BATCH_ID_CAPACITY {
            // Remove batches that reference non-existent transactions
            self.txn_batch_id.retain(|_batch_id, txn_set| {
                txn_set.retain(|key| self.txn_initial_add_time.contains_key(key));
                !txn_set.is_empty()
            });
        }

        // Cleanup txn_block_id if over capacity
        if self.txn_block_id.len() >= MAX_TXN_BLOCK_ID_CAPACITY {
            // Remove blocks that reference non-existent transactions
            self.txn_block_id.retain(|_block_id, txn_set| {
                txn_set.retain(|key| self.txn_initial_add_time.contains_key(key));
                !txn_set.is_empty()
            });
        }
    }
}
