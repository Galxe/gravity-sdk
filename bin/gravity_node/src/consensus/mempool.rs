use std::{collections::{BTreeMap, HashMap, HashSet}, sync::Arc};

use api_types::{account::ExternalAccountAddress, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use tracing::*;

pub struct Mempool {
    /// AccountAddress -> (sequence_number -> transaction)
    pub pool_txns: Arc<tokio::sync::Mutex<HashMap<ExternalAccountAddress, BTreeMap<u64, VerifiedTxnWithAccountSeqNum>>>>,
    /// AccountAddress -> current_sequence_number
        commit_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
        next_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
    }

    impl Mempool {
    pub fn new() -> Self {
        Self {
            pool_txns: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            next_sequence_numbers: HashMap::new(),
            commit_sequence_numbers: HashMap::new(),
        }
    }

    pub async fn add(pool_txns: &Arc<tokio::sync::Mutex<HashMap<ExternalAccountAddress, BTreeMap<u64, VerifiedTxnWithAccountSeqNum>>>>, txns: Vec<VerifiedTxnWithAccountSeqNum>) {
        let mut pool_txns = pool_txns.lock().await;
        for txn in txns {
            let account = txn.txn.sender.clone();
            let account_seq = txn.account_seq_num;
            let seq_num = txn.txn.sequence_number;
            trace!("add txn to mempool: {:?}, seq, seq_num: {}", account_seq, seq_num);
            pool_txns.entry(account).or_default().insert(seq_num, txn);
        }
    }

    pub async fn get_txns(&mut self, block_txns: &mut Vec<VerifiedTxn>) -> bool {
        let mut has_new_txn = false;
        let pool_txns = self.pool_txns.lock().await;
        for (account, txns) in pool_txns.iter() {
            let next_nonce = self.next_sequence_numbers.get(account).unwrap_or(&0);
            let txn = txns.get(&next_nonce).map(|txn| (account.clone(), txn.clone()));
            if let Some(txn) = txn {
                block_txns.push(txn.1.txn.clone());
                self.next_sequence_numbers.insert(txn.0.clone(), txn.1.txn.sequence_number + 1);
                has_new_txn = true;
            }
        }

        has_new_txn
    }

    pub async fn commit(&mut self, txns: &Vec<VerifiedTxn>) {
        for txn in txns {
            let account = txn.sender.clone();
            let seq_num = txn.sequence_number;
            let mut pool_txns = self.pool_txns.lock().await;
            if let Some(txns) = pool_txns.get_mut(&account) {
                txns.remove(&seq_num);
            }
        }
    }

    pub fn get_current_sequence_number(&self, account: &ExternalAccountAddress) -> u64 {
        *self.commit_sequence_numbers.get(account).unwrap_or(&0)
    }

    pub fn set_current_sequence_number(
        &mut self,
        account: ExternalAccountAddress,
        sequence_number: u64,
    ) {
        self.commit_sequence_numbers.insert(account, sequence_number);
    }

    pub async fn size(&self) -> usize {
        let pool_txns = self.pool_txns.lock().await;
        pool_txns.values().map(|txns| txns.len()).sum()
    }
}
