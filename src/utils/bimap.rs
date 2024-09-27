use std::collections::HashMap;

use aptos_crypto::HashValue;

#[derive(Debug)]
pub struct BiMap {
    payload_to_block: HashMap<[u8; 32], HashValue>,
    block_to_payload: HashMap<HashValue, [u8; 32]>,
}
impl BiMap {
    pub fn new() -> Self {
        BiMap {
            payload_to_block: HashMap::new(),
            block_to_payload: HashMap::new(),
        }
    }
    pub fn insert(&mut self, payload_id: [u8; 32], block_id: HashValue) {
        self.payload_to_block.insert(payload_id, block_id);
        self.block_to_payload.insert(block_id, payload_id);
    }
    pub fn get_block_id(&self, payload_id: &[u8; 32]) -> Option<&HashValue> {
        self.payload_to_block.get(payload_id)
    }
    pub fn get_payload_id(&self, block_id: &HashValue) -> Option<&[u8; 32]> {
        self.block_to_payload.get(block_id)
    }
    pub fn remove(&mut self, payload_id: &[u8; 32], block_id: &HashValue) {
        self.payload_to_block.remove(payload_id);
        self.block_to_payload.remove(block_id);
    }
}