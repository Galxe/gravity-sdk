use std::collections::HashMap;

use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_types::block_executor::partitioner::ExecutableBlock;

pub struct MockBlockTree {
    pub id_to_block: HashMap<HashValue, ExecutableBlock>,
    pub commited_blocks: Vec<HashValue>,
}

impl MockBlockTree {
    pub fn new() -> Self {
        Self {
            id_to_block: HashMap::new(),
            commited_blocks: vec![],
        }
    }



}