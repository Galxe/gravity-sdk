use api_types::{BlockId, ExternalPayloadAttr};
use reth::primitives::B256;
use web3::types::Transaction;
use tracing::info;

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
    block_id_to_hash: std::collections::HashMap<BlockId, B256>,
    block_hash_to_id: std::collections::HashMap<B256, BlockId>,
    block_id_parent: std::collections::HashMap<BlockId, BlockId>,
    building_block: std::collections::HashMap<ExternalPayloadAttr, BuildingState>,
}

impl State {
    pub fn new() -> Self {
        State {
            status: Status::Executing,
            block_id_to_hash: std::collections::HashMap::new(),
            block_hash_to_id: std::collections::HashMap::new(),
            block_id_parent: std::collections::HashMap::new(),
            building_block: std::collections::HashMap::new(),
        }
    }

    pub fn check_new_txn(&mut self, attr: &ExternalPayloadAttr, txn: Transaction) -> bool {
        let building_state =
            self.building_block.entry(attr.clone()).or_insert(BuildingState { gas_used: 0 });
        building_state.gas_used += txn.gas.as_u64();
        if building_state.gas_used > 1000000 {
            return false;
        }
        true
    }

    pub fn insert_new_block(&mut self, block_id: BlockId, block_hash: B256) {
        info!("insert_new_block: {:?} {:?}", block_id, block_hash);
        self.block_hash_to_id.insert(block_hash, block_id.clone());
        self.block_id_to_hash.insert(block_id, block_hash);
    }

    pub fn get_block_hash(&self, block_id: BlockId) -> Option<B256> {
        self.block_id_to_hash.get(&block_id).cloned()
    }
}
