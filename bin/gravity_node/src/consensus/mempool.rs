use std::collections::{BTreeMap, HashMap, HashSet};

use api_types::{account::ExternalAccountAddress, VerifiedTxnWithAccountSeqNum};
use tracing::*;

pub struct Mempool {
    /// AccountAddress -> (sequence_number -> transaction)
    txns: HashMap<ExternalAccountAddress, BTreeMap<u64, VerifiedTxnWithAccountSeqNum>>,
    /// AccountAddress -> current_sequence_number
    commit_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
    next_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
    /// (account, sequence_number)
    processed_txns: HashSet<(ExternalAccountAddress, u64)>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            txns: HashMap::new(),
            next_sequence_numbers: HashMap::new(),
            commit_sequence_numbers: HashMap::new(),
            processed_txns: HashSet::new(),
        }
    }

    pub fn add(&mut self, txn: VerifiedTxnWithAccountSeqNum) {
        let account = txn.txn.sender.clone();
        let account_seq = txn.account_seq_num;
        let seq_num = txn.txn.sequence_number;
        trace!("add txn to mempool: {:?}, seq, seq_num: {}", account_seq, seq_num);

        if seq_num < self.get_current_sequence_number(&account) {
            warn!("txn sequence number is less than current sequence number");
            return;
        }

        self.txns.entry(account).or_default().insert(seq_num, txn);
    }

    pub fn get_next(&mut self) -> Option<(ExternalAccountAddress, VerifiedTxnWithAccountSeqNum)> {
        let mut next = None;
        for (account, txns) in self.txns.iter() {
            let next_nonce = self.next_sequence_numbers.get(account).unwrap_or(&0);
            if self.processed_txns.contains(&(account.clone(), *next_nonce)) {
                continue;
            }
            let txn = txns.get(&next_nonce).map(|txn| (account.clone(), txn.clone()));
            if let Some(txn) = txn {
                next = Some(txn);
            }
        }

        if let Some((account, txn)) = &next {
            self.processed_txns.insert((account.clone(), txn.txn.sequence_number + 1));
            self.next_sequence_numbers.insert(account.clone(), txn.txn.sequence_number);
        }

        next
    }

    pub fn commit(&mut self, account: &ExternalAccountAddress, sequence_number: u64) {
        if let Some(txns) = self.txns.get_mut(account) {
            txns.remove(&sequence_number);

            self.commit_sequence_numbers.insert(account.clone(), sequence_number + 1);

            if txns.is_empty() {
                self.txns.remove(account);
                self.processed_txns.retain(|(a, _)| a != account);
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

    pub fn size(&self) -> usize {
        self.txns.values().map(|txns| txns.len()).sum()
    }
}
