use std::{collections::HashMap, sync::Mutex};

use api_types::{compute_res::ComputeRes, u256_define::BlockId, ExternalBlock, ExternalBlockMeta, VerifiedTxnWithAccountSeqNum};

pub struct TxnBuffer {
    txns: Mutex<Vec<VerifiedTxnWithAccountSeqNum>>,
}


pub enum BlockState {
    Ordered(ExternalBlock),
    Computed(ComputeRes),
    Commited,
    
}

pub struct BlockStateMachine {
    block_id_to_meta: HashMap<BlockId, ExternalBlockMeta>,
    block_num_to_meta: HashMap<u64, ExternalBlockMeta>,
    blocks: HashMap<ExternalBlockMeta, BlockState>
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,

}

impl BlockBufferManager {

    pub fn new() -> Self {
        Self {
            txn_buffer: TxnBuffer { txns: Mutex::new(Vec::new()) },
            block_state_machine: Mutex::new(BlockStateMachine {
                block_id_to_meta: HashMap::new(),
                block_num_to_meta: HashMap::new(),
                blocks: HashMap::new(),
            }),
        }
    }

    pub fn push_txns(&self, txn: Vec<VerifiedTxnWithAccountSeqNum>) {
        let mut txns = self.txn_buffer.txns.lock().unwrap();
        txns.extend(txn);
    }

    pub fn pop_txns(&self, count: usize) -> Vec<VerifiedTxnWithAccountSeqNum> {
        let mut txns = self.txn_buffer.txns.lock().unwrap();
        txns.drain(0..count).collect()
    }

    pub fn push_ordered_block(&self, block: ExternalBlock) {
        let mut blocks = self.block_state_machine.lock().unwrap();
        blocks.block_id_to_meta.insert(block.block_meta.block_id.clone(), block.block_meta.clone());
        blocks.block_num_to_meta.insert(block.block_meta.block_number.clone(), block.block_meta.clone());
        blocks.blocks.insert(block.block_meta.clone(), BlockState::Ordered(block));

    }

    pub fn get_block_by_id(&self, block_id: BlockId) -> Option<ExternalBlock> {
        let blocks = self.block_state_machine.lock().unwrap();
        let meta = blocks.block_id_to_meta.get(&block_id);
        if let Some(meta) = meta {
            blocks.blocks.get(meta).and_then(|state| match state {
                BlockState::Ordered(block) => Some(block.clone()),
                _ => None
            })
        } else {
            None
        }
    }

    pub fn get_block_by_number(&self, block_number: u64) -> Option<ExternalBlock> {
        let blocks = self.block_state_machine.lock().unwrap();
        let meta = blocks.block_num_to_meta.get(&block_number);
        if let Some(meta) = meta {
            blocks.blocks.get(meta).and_then(|state| match state {
                BlockState::Ordered(block) => Some(block.clone()),
                _ => None
            })
        } else {
            None
        }
    }

    pub fn push_computed_block_with_id(&self, block: BlockId, compute_res: ComputeRes) {
        let mut blocks = self.block_state_machine.lock().unwrap();
        let meta = blocks.block_id_to_meta.get(&block).cloned();
        if let Some(meta) = meta {
            blocks.blocks.insert(meta.clone(), BlockState::Computed(compute_res));
        }
    }

    pub fn get_computed_block_with_id(&self, block: BlockId) -> Option<ComputeRes> {
        let blocks = self.block_state_machine.lock().unwrap();
        let meta = blocks.block_id_to_meta.get(&block).cloned();
        if let Some(meta) = meta {
            blocks.blocks.get(&meta).and_then(|state| match state {
                BlockState::Computed(compute_res) => Some(compute_res.clone()),
                _ => None
            })
        } else {
            None
        }
    }

    pub fn push_computed_block_with_number(&self, block: u64, compute_res: ComputeRes) {
        let mut blocks = self.block_state_machine.lock().unwrap();
        let meta = blocks.block_num_to_meta.get(&block).cloned();
        if let Some(meta) = meta {
            blocks.blocks.insert(meta.clone(), BlockState::Computed(compute_res));
        }
    }

    pub fn get_computed_block_with_number(&self, block: u64) -> Option<ComputeRes> {
        let blocks = self.block_state_machine.lock().unwrap();
        let meta = blocks.block_num_to_meta.get(&block).cloned();
        if let Some(meta) = meta {
            blocks.blocks.get(&meta).and_then(|state| match state {
                BlockState::Computed(compute_res) => Some(compute_res.clone()),
                _ => None
            })
        } else {
            None
        }
    }
}