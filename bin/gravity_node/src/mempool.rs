use std::{collections::HashMap, sync::Arc};

use alloy_eips::Encodable2718;
use alloy_primitives::Address;
use block_buffer_manager::TxPool;
use gaptos::api_types::{account::{ExternalAccountAddress, ExternalChainId}, u256_define::{HashValue, TxnHash}, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use greth::reth_transaction_pool::{EthPooledTransaction, TransactionPool, ValidPoolTransaction};
use tokio::sync::Mutex;
use dashmap::DashMap;
use crate::RethTransactionPool;

pub struct Mempool {
    pool: RethTransactionPool,
    txn_cache: Arc<DashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>>,
}


impl Mempool {
    pub fn new(pool: RethTransactionPool) -> Self {
        Self { 
            pool,
            txn_cache: Arc::new(DashMap::new()),
        }
    }

    pub fn tx_cache(&self) -> Arc<DashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>> {
        self.txn_cache.clone()
    }
}

pub fn convert_account(acc: Address) -> ExternalAccountAddress {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(acc.as_slice());
    ExternalAccountAddress::new(bytes)
}

fn to_verified_txn(pool_txn: Arc<ValidPoolTransaction<EthPooledTransaction>>) -> VerifiedTxn {
    let sender = pool_txn.sender();
    let nonce = pool_txn.nonce();
    let txn = pool_txn.transaction.transaction().inner();
    VerifiedTxn {
        bytes: txn.encoded_2718(),
        sender: convert_account(sender),
        sequence_number: nonce,
        chain_id: ExternalChainId::new(0),
        committed_hash: TxnHash::from_bytes(txn.hash().as_slice()).into(),
    }
}


impl TxPool for Mempool {
    fn best_txns(&self, filter: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        let txn_cache = self.txn_cache.clone();
        let iter = self.pool.best_transactions()
            .filter_map(move |pool_txn| {
                let sender = convert_account(pool_txn.sender());
                let nonce = pool_txn.nonce();
                let hash = TxnHash::from_bytes(pool_txn.hash().as_slice());
                if let Some(filter) = &filter {
                    if !filter((sender, nonce, hash)) {
                        return None;
                    }
                }
                let verified_txn = to_verified_txn(pool_txn.clone());
                txn_cache.insert((verified_txn.sender.clone(), verified_txn.sequence_number), pool_txn);
                Some(verified_txn)
            });
        Box::new(iter)
    }
}