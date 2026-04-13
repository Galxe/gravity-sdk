use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{reth_cli::TxnCache, RethTransactionPool};
use alloy_consensus::{transaction::SignerRecoverable, Transaction};
use alloy_eips::{Decodable2718, Encodable2718};
use alloy_primitives::Address;
use block_buffer_manager::TxPool;
use dashmap::DashMap;
use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId},
    u256_define::TxnHash,
    VerifiedTxn,
};
use greth::{
    reth_primitives::{Recovered, TransactionSigned},
    reth_transaction_pool::{
        BestTransactions, EthPooledTransaction, PoolTransaction, TransactionPool,
        ValidPoolTransaction,
    },
};

/// Cache TTL for best transactions (in milliseconds)
/// Can be configured via MEMPOOL_CACHE_TTL_MS environment variable
fn cache_ttl() -> Duration {
    static CACHE_TTL: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *CACHE_TTL.get_or_init(|| {
        let ms = std::env::var("MEMPOOL_CACHE_TTL_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1000); // Default 1000ms (1 second)
        Duration::from_millis(ms)
    })
}

/// Cached best transactions with TTL
struct CachedBest {
    best_txns: Option<
        Box<dyn BestTransactions<Item = Arc<ValidPoolTransaction<EthPooledTransaction>>> + 'static>,
    >,
    created_at: Instant,
    /// Track the last yielded nonce per sender to enforce nonce ordering.
    /// Cleared when the iterator is recreated (on TTL expiry).
    last_nonces: HashMap<Address, u64>,
}

impl CachedBest {
    fn new() -> Self {
        Self {
            best_txns: None,
            created_at: Instant::now() - cache_ttl() - Duration::from_millis(1), // Start expired
            last_nonces: HashMap::new(),
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > cache_ttl()
    }
}

pub struct Mempool {
    pool: RethTransactionPool,
    txn_cache: TxnCache,
    cached_best: Arc<std::sync::Mutex<CachedBest>>,
    runtime: tokio::runtime::Runtime,
    enable_broadcast: bool,
    chain_id: u64,
}

impl Mempool {
    pub fn new(pool: RethTransactionPool, enable_broadcast: bool, chain_id: u64) -> Self {
        Self {
            pool,
            txn_cache: Arc::new(DashMap::new()),
            cached_best: Arc::new(std::sync::Mutex::new(CachedBest::new())),
            runtime: tokio::runtime::Runtime::new().unwrap(),
            enable_broadcast,
            chain_id,
        }
    }

    pub fn tx_cache(&self) -> TxnCache {
        self.txn_cache.clone()
    }
}

pub fn convert_account(acc: Address) -> ExternalAccountAddress {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(acc.as_slice());
    ExternalAccountAddress::new(bytes)
}

fn to_verified_txn(
    pool_txn: Arc<ValidPoolTransaction<EthPooledTransaction>>,
    chain_id: u64,
) -> VerifiedTxn {
    let sender = pool_txn.sender();
    let nonce = pool_txn.nonce();
    let txn = pool_txn.transaction.transaction().inner();
    VerifiedTxn::new(
        txn.encoded_2718(),
        convert_account(sender),
        nonce,
        ExternalChainId::new(chain_id),
    )
}

fn to_verified_txn_from_reth_txn(
    pool_txn: Recovered<TransactionSigned>,
    chain_id: u64,
) -> VerifiedTxn {
    let sender = pool_txn.signer();
    let nonce = pool_txn.inner().nonce();
    let txn = pool_txn.inner();
    VerifiedTxn::new(
        txn.encoded_2718(),
        convert_account(sender),
        nonce,
        ExternalChainId::new(chain_id),
    )
}

impl TxPool for Mempool {
    fn best_txns(
        &self,
        filter: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
        limit: usize,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        let mut best_txns = self.cached_best.lock().unwrap();
        if best_txns.is_expired() || best_txns.best_txns.is_none() {
            *best_txns = CachedBest {
                best_txns: Some(self.pool.best_transactions()),
                created_at: Instant::now(),
                last_nonces: HashMap::new(),
            };
        }
        let txn_cache = self.txn_cache.clone();
        let chain_id = self.chain_id;
        // Take last_nonces out to avoid borrow conflict with best_txns iterator
        let mut last_nonces = std::mem::take(&mut best_txns.last_nonces);
        let result: Vec<_> = best_txns
            .best_txns
            .as_mut()
            .unwrap()
            .filter_map(|pool_txn| {
                let sender = pool_txn.sender();
                let nonce = pool_txn.nonce();

                // Enforce nonce ordering: skip transactions that are not consecutive
                if let Some(&last) = last_nonces.get(&sender) {
                    if nonce != last + 1 {
                        return None;
                    }
                }

                // transactions from poisoning nonce tracking
                let sender_addr = convert_account(sender);
                if let Some(ref f) = filter {
                    let hash = TxnHash::from_bytes(pool_txn.hash().as_slice());
                    if !f((sender_addr.clone(), nonce, hash)) {
                        return None;
                    }
                }

                // Only record nonce after filter passes
                last_nonces.insert(sender, nonce);

                let verified_txn = to_verified_txn(pool_txn.clone(), chain_id);
                let tx_hash: [u8; 32] = pool_txn.transaction.transaction().inner().hash().0;
                txn_cache.insert(tx_hash, pool_txn);
                Some(verified_txn)
            })
            .take(limit)
            .collect();
        // Put last_nonces back
        best_txns.last_nonces = last_nonces;
        if result.is_empty() {
            *best_txns = CachedBest {
                best_txns: None,
                created_at: Instant::now(),
                last_nonces: HashMap::new(),
            };
        }
        Box::new(result.into_iter())
    }

    fn get_broadcast_txns(
        &self,
        filter: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        if !self.enable_broadcast {
            return Box::new(std::iter::empty());
        }
        let all_txns = self.pool.pending_transactions();
        let iter = all_txns
            .iter()
            .filter_map(move |txn| {
                let sender = convert_account(txn.sender());
                let nonce = txn.nonce();
                let hash = TxnHash::from_bytes(txn.hash().as_slice());
                if let Some(filter) = &filter {
                    if !filter((sender, nonce, hash)) {
                        return None;
                    }
                }
                let verified_txn = to_verified_txn_from_reth_txn(
                    txn.transaction.transaction().clone(),
                    self.chain_id,
                );
                Some(verified_txn)
            })
            .collect::<Vec<_>>();
        Box::new(iter.into_iter())
    }

    fn add_external_txn(&self, txn: VerifiedTxn) -> bool {
        let txn = TransactionSigned::decode_2718(&mut txn.bytes().as_slice());
        match txn {
            Ok(txn) => {
                let signer = match txn.recover_signer() {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to recover signer for external transaction: {e}");
                        return false;
                    }
                };
                let len = txn.encode_2718_len();
                let recovered = Recovered::new_unchecked(txn, signer);
                let pool_txn = EthPooledTransaction::new(recovered, len);
                let pool = self.pool.clone();
                let address = pool_txn.sender();
                let to = pool_txn.to();
                self.runtime.spawn(async move {
                    let res = pool.add_external_transaction(pool_txn).await;
                    if let Err(e) = res {
                        tracing::error!(
                            "Failed to add transaction: {:?} {:?} {:?}",
                            address,
                            to,
                            e
                        );
                    }
                });
                true
            }
            Err(e) => {
                tracing::error!("Failed to decode transaction: {}", e);
                false
            }
        }
    }

    fn remove_txns(&self, txns: Vec<VerifiedTxn>) {
        if txns.is_empty() {
            return;
        }

        let mut eth_txn_hashes = Vec::with_capacity(txns.len());
        for txn in txns {
            let txn = TransactionSigned::decode_2718(&mut txn.bytes().as_slice());
            match txn {
                Ok(txn) => {
                    eth_txn_hashes.push(*txn.hash());
                }
                Err(e) => tracing::error!("Failed to decode transaction: {}", e),
            }
        }
        self.pool.remove_transactions(eth_txn_hashes);
    }
}
