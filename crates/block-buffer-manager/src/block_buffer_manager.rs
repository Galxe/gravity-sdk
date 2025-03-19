use std::{collections::HashMap};
use tokio::sync::Mutex;

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

    pub async fn push_txns(&self, txn: Vec<VerifiedTxnWithAccountSeqNum>) {
        let mut txns = self.txn_buffer.txns.lock().await;
        txns.extend(txn);
    }

    pub async fn pop_txns(&self, max_size: usize) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
        todo!()
    }

    pub async fn push_commit_blocks(&self, block_ids: Vec<BlockId>) -> Result<(), anyhow::Error> {
        todo!()
    }

    pub async fn push_ordered_blocks(&self, parent_id: BlockId, blocks: ExternalBlock) -> Result<(), anyhow::Error> {
        todo!()
    }

    pub async fn get_executed_res(&self, block_id: BlockId) -> Result<ComputeRes, anyhow::Error> {
        todo!()
    }

    pub async fn pop_wait_commit(&self) -> Result<(), anyhow::Error> {
        todo!()
    }
}