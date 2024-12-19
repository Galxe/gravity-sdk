use std::collections::{HashMap, BTreeMap};

use api_types::{account::ExternalAccountAddress, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use reth_payload_builder::error;
use tracing::{info, warn};

/// 内存池的核心实现
/// 为每个账户维护一个有序的交易队列
pub struct Mempool {
    /// 账户交易存储: AccountAddress -> (sequence_number -> transaction)
    txns: HashMap<ExternalAccountAddress, BTreeMap<u64, VerifiedTxnWithAccountSeqNum>>,
    /// 账户当前序号跟踪: AccountAddress -> current_sequence_number
    current_sequence_numbers: HashMap<ExternalAccountAddress, u64>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            txns: HashMap::new(),
            current_sequence_numbers: HashMap::new(),
        }
    }

    /// 添加交易到内存池
    /// 如果交易序号小于账户当前序号，则忽略
    pub fn add(&mut self, txn: VerifiedTxnWithAccountSeqNum) {
        let account = txn.txn.sender.clone();
        let seq_num = txn.txn.sequence_number;
        info!("add txn to mempool: {:?}, seq_num: {}", account, seq_num);
        // 如果序号小于当前序号，直接返回
        if seq_num < self.get_current_sequence_number(&account) {
            warn!("txn sequence number is less than current sequence number");
            return;
        }

        self.txns
            .entry(account)
            .or_default()
            .insert(seq_num, txn);
    }

    /// 获取下一个ready的交易
    /// 交易必须是账户当前序号的下一个交易
    pub fn get_next(&self) -> Option<(&ExternalAccountAddress, &VerifiedTxnWithAccountSeqNum)> {
        self.txns.iter()
            .find_map(|(account, txns)| {
                let current_seq = self.get_current_sequence_number(account);
                txns.get(&current_seq)
                    .map(|txn| (account, txn))
            })
    }

    /// 提交交易，移除交易并更新账户序号
    pub fn commit(&mut self, account: &ExternalAccountAddress, sequence_number: u64) {
        if let Some(txns) = self.txns.get_mut(account) {
            // 移除交易
            txns.remove(&sequence_number);
            
            // 更新序号
            self.current_sequence_numbers.insert(account.clone(), sequence_number + 1);
            
            // 如果账户没有更多交易，清理存储
            if txns.is_empty() {
                self.txns.remove(account);
            }
        }
    }

    /// 获取账户当前序号
    pub fn get_current_sequence_number(&self, account: &ExternalAccountAddress) -> u64 {
        *self.current_sequence_numbers.get(account).unwrap_or(&0)
    }

    /// 设置账户当前序号
    pub fn set_current_sequence_number(&mut self, account: ExternalAccountAddress, sequence_number: u64) {
        self.current_sequence_numbers.insert(account, sequence_number);
    }

    /// 获取内存池大小
    pub fn size(&self) -> usize {
        self.txns.values().map(|txns| txns.len()).sum()
    }
}

/// 实现迭代器，按序号顺序返回ready的交易
impl Mempool {
    pub fn iter_ready(&self) -> impl Iterator<Item = &VerifiedTxnWithAccountSeqNum> {
        self.txns.iter().filter_map(|(account, txns)| {
            let current_seq = self.get_current_sequence_number(account);
            txns.get(&current_seq)
        })
    }
}