use std::collections::{HashMap, BTreeMap, HashSet};

use api_types::{account::ExternalAccountAddress, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use reth_payload_builder::error;
use tracing::{info, warn};

pub struct Mempool {
    /// 账户交易存储: AccountAddress -> (sequence_number -> transaction)
    txns: HashMap<ExternalAccountAddress, BTreeMap<u64, VerifiedTxnWithAccountSeqNum>>,
    /// 账户当前序号跟踪: AccountAddress -> current_sequence_number
    current_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
    /// 已处理的交易: (account, sequence_number)
    processed_txns: HashSet<(ExternalAccountAddress, u64)>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            txns: HashMap::new(),
            current_sequence_numbers: HashMap::new(),
            processed_txns: HashSet::new(),
        }
    }

    pub fn add(&mut self, txn: VerifiedTxnWithAccountSeqNum) {
        let account = txn.txn.sender.clone();
        let account_seq = txn.account_seq_num;
        let seq_num = txn.txn.sequence_number;
        info!("add txn to mempool: {:?}, seq, seq_num: {}", account_seq, seq_num);
        
        if seq_num < self.get_current_sequence_number(&account) {
            warn!("txn sequence number is less than current sequence number");
            return;
        }

        self.txns
            .entry(account)
            .or_default()
            .insert(seq_num, txn);
    }

    pub fn get_next(&mut self) -> Option<(ExternalAccountAddress, VerifiedTxnWithAccountSeqNum)> {
        let next = self.txns.iter()
            .find_map(|(account, txns)| {
                let current_seq = self.get_current_sequence_number(account);
                if self.processed_txns.contains(&(account.clone(), current_seq)) {
                    return None;
                }
                txns.get(&current_seq)
                    .map(|txn| (account.clone(), txn.clone()))
            });
        
        // 如果找到了交易，立即标记为已处理
        if let Some((account, txn)) = &next {
            self.processed_txns.insert((account.clone(), txn.txn.sequence_number));
        }
        
        next
    }

    pub fn commit(&mut self, account: &ExternalAccountAddress, sequence_number: u64) {
        if let Some(txns) = self.txns.get_mut(account) {
            // 移除交易
            txns.remove(&sequence_number);
            
            // 更新序号
            self.current_sequence_numbers.insert(account.clone(), sequence_number + 1);
            
            // 如果账户没有更多交易，清理存储
            if txns.is_empty() {
                self.txns.remove(account);
                // 清理已处理记录
                self.processed_txns.retain(|(a, _)| a != account);
            }
        }
    }

    pub fn get_current_sequence_number(&self, account: &ExternalAccountAddress) -> u64 {
        *self.current_sequence_numbers.get(account).unwrap_or(&0)
    }

    pub fn set_current_sequence_number(&mut self, account: ExternalAccountAddress, sequence_number: u64) {
        self.current_sequence_numbers.insert(account, sequence_number);
    }

    pub fn size(&self) -> usize {
        self.txns.values().map(|txns| txns.len()).sum()
    }
}