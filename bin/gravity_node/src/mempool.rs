use std::{collections::HashMap, sync::Arc};

use crate::RethTransactionPool;
use alloy_consensus::{transaction::SignerRecoverable, Transaction};
use alloy_eips::Decodable2718;
use alloy_eips::Encodable2718;
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
        EthPooledTransaction, TransactionPool, ValidPoolTransaction,
    },
};

pub struct Mempool {
    pool: RethTransactionPool,
    txn_cache: Arc<
        DashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>,
    >,
}

impl Mempool {
    pub fn new(pool: RethTransactionPool) -> Self {
        Self { pool, txn_cache: Arc::new(DashMap::new()) }
    }

    pub fn tx_cache(
        &self,
    ) -> Arc<DashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>>
    {
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

fn to_verified_txn_from_recovered_txn(pool_txn: Recovered<TransactionSigned>) -> VerifiedTxn {
    let sender = pool_txn.signer();
    let nonce = pool_txn.inner().nonce();
    let txn = pool_txn.inner();
    VerifiedTxn {
        bytes: txn.encoded_2718(),
        sender: convert_account(sender),
        sequence_number: nonce,
        chain_id: ExternalChainId::new(0),
        committed_hash: TxnHash::from_bytes(txn.hash().as_slice()).into(),
    }
}

#[async_trait::async_trait]
impl TxPool for Mempool {
    fn best_txns(
        &self,
        filter: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        let txn_cache = self.txn_cache.clone();
        let iter = self.pool.best_transactions().filter_map(move |pool_txn| {
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

    fn get_broadcast_txns(
        &self,
        filter: Option<Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>>,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        let all_txns = self.pool.all_transactions();
        let iter = all_txns
            .all()
            .filter_map(move |txn| {
                let sender = convert_account(txn.signer());
                let nonce = txn.inner().nonce();
                let hash = TxnHash::from_bytes(txn.inner().hash().as_slice());
                if let Some(filter) = &filter {
                    if !filter((sender, nonce, hash)) {
                        return None;
                    }
                }
                let verified_txn = to_verified_txn_from_recovered_txn(txn);
                Some(verified_txn)
            })
            .collect::<Vec<_>>();
        Box::new(iter.into_iter())
    }

    async fn add_external_txn(&self, txn: VerifiedTxn) -> bool {
        let txn = TransactionSigned::decode_2718(&mut txn.bytes.as_ref());
        match txn {
            Ok(txn) => {
                let signer = txn.recover_signer().unwrap();
                let len = txn.encode_2718_len();
                let recovered = Recovered::new_unchecked(txn, signer);
                let pool_txn = EthPooledTransaction::new(recovered, len);
                self.pool.add_external_transaction(pool_txn).await.is_ok()
            }
            Err(e) => {
                tracing::error!("Failed to decode transaction: {}", e);
                false
            }
        }
    }

    async fn remove_txns(&self, txns: Vec<VerifiedTxn>) {
        let mut eth_txn_hashes = Vec::with_capacity(txns.len());
        for txn in txns {
            let txn = TransactionSigned::decode_2718(&mut txn.bytes.as_ref());
            match txn {
                Ok(txn) => {
                    eth_txn_hashes.push(txn.hash().clone());
                }
                Err(e) => tracing::error!("Failed to decode transaction: {}", e),
            }
        }
        self.pool.remove_transactions(eth_txn_hashes);
    }
}
