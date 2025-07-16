use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use alloy_consensus::Transaction;
use alloy_primitives::{Address, TxHash};
use alloy_eips::Encodable2718;
use greth::reth_transaction_pool::TransactionPool;
use tracing::{debug, info, warn};

use block_buffer_manager::get_block_buffer_manager;
use gaptos::api_types::account::ExternalChainId;
use gaptos::api_types::u256_define::TxnHash;
use gaptos::api_types::{VerifiedTxn, VerifiedTxnWithAccountSeqNum};

use greth::reth_db::DatabaseEnv;
use greth::reth_node_api::NodeTypesWithDBAdapter;
use greth::reth_node_ethereum::EthereumNode;
use greth::reth_provider::providers::BlockchainProvider;
use greth::reth_provider::AccountReader;
use greth::reth_transaction_pool::{
    Pool, ValidPoolTransaction,
    EthPooledTransaction,
    TransactionValidationTaskExecutor,
    EthTransactionValidator,
    CoinbaseTipOrdering,
    blobstore::DiskFileBlobStore,
};

use crate::reth_cli::convert_account;

// --- Type Aliases (Unchanged) ---
type RethPool = Pool<
    TransactionValidationTaskExecutor<
        EthTransactionValidator<
            RethProvider,
            EthPooledTransaction,
        >,
    >,
    CoinbaseTipOrdering<EthPooledTransaction>,
    DiskFileBlobStore,
>;
type RethProvider = BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>;
type RethValidTx = Arc<ValidPoolTransaction<EthPooledTransaction>>;

/// A stateful mempool processor that handles transactions in a more intuitive, nonce-ordered manner.
pub struct MempoolProcessor {
    // --- Core Dependencies ---
    provider: RethProvider,
    pool: RethPool,
    chain_id: u64,

    // --- State Management ---
    txn_listener: Option<tokio::sync::mpsc::Receiver<TxHash>>,
    
    /// Buffer for pending transactions, keyed by sender and sorted by nonce.
    /// The `Instant` is used for state cleanup (TTL).
    pending_txs: HashMap<Address, BTreeMap<u64, (RethValidTx, Instant)>>,
    
    /// Tracks the next expected nonce for each sender.
    nonce_tracker: HashMap<Address, u64>,
    
    /// Tracks dispatched transaction hashes to prevent reprocessing.
    processed_tx_hashes: HashMap<TxHash, Instant>,
    
    // --- Configuration ---
    process_interval: Duration, 
    state_cleanup_interval: Duration, 
    pending_tx_ttl: Duration, // TTL for individual transactions in `pending_txs`.
    processed_tx_ttl: Duration, // TTL for the processed transaction hashes.
}

impl MempoolProcessor {
    pub fn new(provider: RethProvider, pool: RethPool, chain_id: u64, txn_listener: tokio::sync::mpsc::Receiver<TxHash>) -> Self {
        Self {
            provider,
            pool,
            chain_id,
            txn_listener: Some(txn_listener),
            pending_txs: HashMap::new(),
            nonce_tracker: HashMap::new(),
            processed_tx_hashes: HashMap::new(),
            process_interval: Duration::from_millis(200),
            state_cleanup_interval: Duration::from_secs(60),
            pending_tx_ttl: Duration::from_secs(600), // Pending tx expires after 10 minutes.
            processed_tx_ttl: Duration::from_secs(3600), // Processed hash record expires after 1 hour.
        }
    }

    /// Starts the main processing loop for the mempool.
    pub async fn start_mempool(mut self) -> Result<(), String> {
        info!("▶️ Refactored MempoolProcessor is running...");
        let mut txn_listener = self.txn_listener.take()
            .ok_or("Processor already started or not initialized correctly")?;

        let mut process_timer = tokio::time::interval(self.process_interval);
        let mut cleanup_timer = tokio::time::interval(self.state_cleanup_interval);

        loop {
            tokio::select! {
                // Prioritize handling new incoming transaction hashes to fill the buffer quickly.
                biased;
                Some(hash) = txn_listener.recv() => {
                    self.add_txn_to_pending_buffer(hash).await;
                }

                // Periodically process transactions from the pending buffer.
                _ = process_timer.tick() => {
                    if let Err(e) = self.process_and_dispatch_pending().await {
                        warn!("Error processing pending transactions: {}", e);
                    }
                }

                // Periodically clean up stale state to prevent memory leaks.
                _ = cleanup_timer.tick() => {
                    self.cleanup_stale_state();
                }
            }
        }
    }

    /// Fetches a transaction from the Reth pool and adds it to the `pending_txs` buffer.
    async fn add_txn_to_pending_buffer(&mut self, tx_hash: TxHash) {
        // Ignore if already processed.
        if self.processed_tx_hashes.contains_key(&tx_hash) {
            return;
        }
        
        if let Some(tx) = self.pool.get(&tx_hash) {
            let sender = tx.sender();
            let nonce = tx.nonce();
            debug!("Received Tx: Sender {}, Nonce {}, Hash {}", sender, nonce, tx_hash);
            
            self.pending_txs
                .entry(sender)
                .or_default()
                .insert(nonce, (tx, Instant::now()));
        }
    }

    /// Core processing logic. This function finds continuous sequences of transactions for each sender,
    /// collects them, and dispatches them to the execution engine.
    async fn process_and_dispatch_pending(&mut self) -> Result<(), String> {
        if self.pending_txs.is_empty() {
            return Ok(());
        }
        
        let mut dispatchable_txs = Vec::new();
        let mut senders_to_dispatch = HashSet::new();
        let senders: Vec<Address> = self.pending_txs.keys().cloned().collect();

        // Iterate over all senders with pending txs.
        for sender in senders {
            let tx_map = self.pending_txs.entry(sender).or_default();

            // Determine the next nonce we should process for this sender.
            let next_nonce = match self.nonce_tracker.get(&sender).cloned() {
                Some(nonce) => nonce,
                None => {
                    // Fetch from chain if not tracked locally.
                    let onchain_nonce = Self::fetch_onchain_nonce(&self.provider, &sender).await?;
                    self.nonce_tracker.insert(sender, onchain_nonce);
                    onchain_nonce
                }
            };

            let mut current_nonce = next_nonce;

            // Loop to find a continuous sequence of nonces, with proactive gap-filling.
            loop {
                // Step 1: Check the local buffer first (most efficient).
                if let Some((tx, _)) = tx_map.get(&current_nonce) {
                    if !self.processed_tx_hashes.contains_key(tx.hash()) {
                        dispatchable_txs.push(tx.clone());
                        senders_to_dispatch.insert(sender);
                    } else {
                        warn!("Found already processed tx for sender {} nonce {} in pending list. Skipping.", sender, current_nonce);
                    }
                    current_nonce += 1;
                    continue; // Continue to the next nonce.
                }

                // Step 2: If not in local buffer, query the main pool to fill potential gaps from missed events.
                if let Some(found_tx) = self.pool.get_transaction_by_sender_and_nonce(sender, current_nonce) {
                    info!("Actively filled gap for sender {} with nonce {} from main pool.", sender, current_nonce);
                    
                    dispatchable_txs.push(found_tx.clone());
                    senders_to_dispatch.insert(sender);

                    // Add to our buffer for consistency and to avoid future lookups.
                    tx_map.insert(current_nonce, (found_tx, Instant::now()));
                    
                    current_nonce += 1;
                    continue; // Continue to the next nonce.
                }

                // Step 3: If not in local buffer or main pool, the gap is real. We must wait.
                break; // Break the loop for the current sender and move to the next.
            }
            
            // Update the nonce tracker if we've processed any transactions.
            if current_nonce > next_nonce {
                self.nonce_tracker.insert(sender, current_nonce);
            }
        }
        
        if dispatchable_txs.is_empty() {
            return Ok(());
        }
        
        // Batch prefetch the latest on-chain nonces for dispatchable senders.
        let onchain_nonces = self.prefetch_onchain_nonces(&senders_to_dispatch).await?;
        
        // Prepare and dispatch the transactions.
        let (vtxns_to_send, total_gas) = self.prepare_transactions_for_engine(&dispatchable_txs, &onchain_nonces)?;

        if !vtxns_to_send.is_empty() {
            get_block_buffer_manager().push_txns(&mut vtxns_to_send.clone(), total_gas).await;
            info!("🚀 Dispatched {} transactions.", vtxns_to_send.len());
            
            // Mark as processed and remove from the pending buffer.
            for tx in &dispatchable_txs {
                self.processed_tx_hashes.insert(*tx.hash(), Instant::now());
                if let Some(sender_map) = self.pending_txs.get_mut(&tx.sender()) {
                    sender_map.remove(&tx.nonce());
                }
            }
        }
        
        Ok(())
    }

    /// Prepares transactions for dispatch to the execution engine.
    fn prepare_transactions_for_engine(
        &self, 
        pool_txns: &[RethValidTx],
        onchain_nonces: &HashMap<Address, u64>
    ) -> Result<(Vec<VerifiedTxnWithAccountSeqNum>, u64), String> {
        let mut vtxn_buffer = Vec::with_capacity(pool_txns.len());
        let mut total_gas_limit = 0;
    
        for pool_txn in pool_txns {
            let sender = pool_txn.sender();
            let account_nonce = *onchain_nonces.get(&sender)
                .ok_or_else(|| format!("Logic error: On-chain nonce not found for sender {}", sender))?;
            
            let txn = pool_txn.transaction.transaction().inner();
            total_gas_limit += txn.gas_limit();
            
            let vtxn = VerifiedTxnWithAccountSeqNum {
                txn: VerifiedTxn {
                    bytes: txn.encoded_2718(),
                    sender: convert_account(sender),
                    sequence_number: pool_txn.nonce(),
                    chain_id: ExternalChainId::new(self.chain_id),
                    committed_hash: TxnHash::from_bytes(txn.hash().as_slice()).into(),
                },
                account_seq_num: account_nonce,
            };
            vtxn_buffer.push(vtxn);
        }
        Ok((vtxn_buffer, total_gas_limit))
    }

    /// Fetches the on-chain nonce for a single address directly from the provider.
    async fn fetch_onchain_nonce(
        provider: &RethProvider, 
        sender: &Address
    ) -> Result<u64, String> {
        let res = provider.basic_account(sender)
            .map_err(|e| format!("Failed to get basic account for {}: {:?}", sender, e))?
            .map_or(0, |acc| acc.nonce);
        Ok(res)
    }

    /// Prefetches on-chain nonces for a set of addresses.
    async fn prefetch_onchain_nonces(&mut self, senders: &HashSet<Address>) -> Result<HashMap<Address, u64>, String> {
        let mut onchain_nonces = HashMap::new();
        for &sender in senders {
            let nonce = Self::fetch_onchain_nonce(&self.provider, &sender).await?;
            onchain_nonces.insert(sender, nonce);
        }
        Ok(onchain_nonces)
    }

    /// Cleans up stale internal state to prevent unbounded memory growth.
    fn cleanup_stale_state(&mut self) {
        let now = Instant::now();
        
        // 1. Clean up processed transaction hashes that have passed their TTL.
        let old_processed_count = self.processed_tx_hashes.len();
        self.processed_tx_hashes.retain(|_, timestamp| {
            now.duration_since(*timestamp) < self.processed_tx_ttl
        });
        
        // 2. Clean up stuck pending transactions that have been in the buffer for too long.
        let mut empty_senders = Vec::new();
        let mut cleaned_tx_count = 0;
        for (sender, tx_map) in self.pending_txs.iter_mut() {
            tx_map.retain(|_, (_, timestamp)| {
                let retain = now.duration_since(*timestamp) < self.pending_tx_ttl;
                if !retain { cleaned_tx_count += 1; }
                retain
            });

            // If a sender has no more pending txs after cleanup, mark them for removal.
            if tx_map.is_empty() {
                empty_senders.push(*sender);
            }
        }

        // 3. Remove entries for idle senders from all state maps to prevent stale state issues.
        for sender in empty_senders {
            self.pending_txs.remove(&sender);
            // This is the crucial fix: also remove the stale nonce tracker.
            self.nonce_tracker.remove(&sender);
        }
        
        if cleaned_tx_count > 0 || old_processed_count != self.processed_tx_hashes.len() {
            info!(
                "State cleanup complete. Processed hashes: {} -> {}. Cleaned {} stale pending txs.",
                old_processed_count, self.processed_tx_hashes.len(), cleaned_tx_count
            );
        }
    }
}