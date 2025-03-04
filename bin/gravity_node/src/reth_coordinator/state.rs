use std::sync::atomic::{AtomicU64, Ordering};

use api_types::account::ExternalAccountAddress;
use api_types::VerifiedTxn;
use api_types::{u256_define::BlockId, ExternalPayloadAttr};
use greth::reth::primitives::B256;
use greth::reth_primitives::revm_primitives::bitvec::store::BitStore;
use greth::reth_primitives::revm_primitives::bitvec::view::BitViewSized;
use greth::reth_primitives::Transaction;
use tracing::debug;
pub struct BuildingState {
    gas_used: u64,
}

pub enum Status {
    Executing,
    Executed,
    Committed,
}

pub struct State {
    status: Status,
    accout_seq_num: std::collections::HashMap<ExternalAccountAddress, i64>,
    block_id_to_hash: std::collections::HashMap<BlockId, B256>,
    block_hash_to_id: std::collections::HashMap<B256, BlockId>,
    block_id_parent: std::collections::HashMap<BlockId, BlockId>,
    building_block: std::collections::HashMap<ExternalPayloadAttr, BuildingState>,
    block_id_to_block_number: std::collections::HashMap<BlockId, u64>,
    latest_executed_block_number: u64,
    latest_committed_block_number: u64,
}

impl State {
    pub fn new(latest_block_numnber: u64) -> Self {
        State {
            status: Status::Executing,
            accout_seq_num: std::collections::HashMap::new(),
            block_id_to_hash: std::collections::HashMap::new(),
            block_hash_to_id: std::collections::HashMap::new(),
            block_id_parent: std::collections::HashMap::new(),
            building_block: std::collections::HashMap::new(),
            block_id_to_block_number: std::collections::HashMap::new(),
            latest_executed_block_number: latest_block_numnber,
            latest_committed_block_number: latest_block_numnber,
        }
    }

    pub fn update_account_seq_num(&mut self, txn: &VerifiedTxn) -> bool {
        let account = txn.sender.clone();
        let seq_num = self.accout_seq_num.entry(account).or_insert(-1);
        if *seq_num + 1 != txn.sequence_number as i64 {
            debug!("meet false seq_num: {:?} {:?}", seq_num, txn.sequence_number);
            return false;
        }
        *seq_num += 1;
        true
    }

    pub fn check_new_txn(&mut self, attr: &ExternalPayloadAttr, txn: Transaction) -> bool {
        let building_state =
            self.building_block.entry(attr.clone()).or_insert(BuildingState { gas_used: 0 });
        building_state.gas_used += txn.gas_limit();
        if building_state.gas_used > 1000000 {
            return false;
        }
        true
    }

    pub fn insert_new_block(&mut self, block_id: BlockId, block_hash: B256) {
        debug!("insert_new_block: {:?} {:?}", block_id, block_hash);
        self.block_hash_to_id.insert(block_hash, block_id.clone());
        self.block_id_to_hash.insert(block_id, block_hash);
    }

    pub fn get_block_hash(&self, block_id: BlockId) -> Option<B256> {
        self.block_id_to_hash.get(&block_id).cloned()
    }

    pub fn insert_block_number(&mut self, block_id: BlockId, block_numnber: u64) {
        self.block_id_to_block_number.insert(block_id, block_numnber);
    }

    pub fn get_block_number(&self, block_id: &BlockId) -> u64 {
        self.block_id_to_block_number.get(block_id).unwrap().clone()
    }

    pub fn cas_executed_block_number(&mut self, executed_block_number: u64) -> bool {
        assert!(executed_block_number <= self.latest_executed_block_number + 1);
        if executed_block_number == self.latest_executed_block_number + 1 {
            self.latest_executed_block_number += 1;
            return true;
        }
        false
    }

    pub fn cas_committed_block_number(&mut self, committed_block_number: u64) -> bool {
        assert!(committed_block_number <= self.latest_committed_block_number + 1);
        if committed_block_number == self.latest_committed_block_number + 1 {
            self.latest_committed_block_number += 1;
            return true;
        }
        false
    }

    pub fn latest_executed_block_number(&self) -> u64 {
        self.latest_executed_block_number
    }

    pub fn latest_committed_block_number(&self) -> u64 {
        self.latest_committed_block_number
    }
}
