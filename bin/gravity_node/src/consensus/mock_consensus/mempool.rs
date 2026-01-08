use std::collections::{BTreeMap, HashMap};

use block_buffer_manager::TxPool;
use gaptos::api_types::{
    account::ExternalAccountAddress, u256_define::TxnHash, VerifiedTxn,
    VerifiedTxnWithAccountSeqNum,
};
use tracing::*;

pub struct Mempool {
    pool_txns: Box<dyn TxPool>,
    /// AccountAddress -> current_sequence_number
    commit_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
    next_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
}

pub struct TxnId {
    pub sender: ExternalAccountAddress,
    pub seq_num: u64,
}

impl Mempool {
    pub fn new(pool_txns: Box<dyn TxPool>) -> Self {
        Self {
            pool_txns,
            next_sequence_numbers: HashMap::new(),
            commit_sequence_numbers: HashMap::new(),
        }
    }

    pub fn reset_epoch(&mut self) {
        self.next_sequence_numbers.clear();
        for (account, seq_num) in self.commit_sequence_numbers.iter() {
            self.next_sequence_numbers.insert(account.clone(), *seq_num + 1);
        }
    }

    pub fn get_txns(&mut self, block_txns: &mut Vec<VerifiedTxn>, max_block_size: usize) -> bool {
        let mut has_new_txn = false;
        let next_txns = self.next_sequence_numbers.clone();
        let filter = Box::new(move |txn: (ExternalAccountAddress, u64, TxnHash)| {
            let next_nonce = *next_txns.get(&txn.0).unwrap_or(&0);
            next_nonce <= txn.1
        });
        for txn in self.pool_txns.best_txns(Some(filter)) {
            let account = txn.sender();
            let nonce = txn.seq_number();
            self.next_sequence_numbers.insert(account.clone(), nonce + 1);
            block_txns.push(txn);
            has_new_txn = true;
            if block_txns.len() >= max_block_size {
                break;
            }
        }

        has_new_txn
    }

    pub fn commit_txns(&mut self, _txns: &[TxnId]) {}

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
}
