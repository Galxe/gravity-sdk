use std::{collections::HashMap};
use log::info;
use tokio::sync::{broadcast::Receiver, Mutex};

use api_types::{compute_res::ComputeRes, u256_define::BlockId, ExternalBlock, ExternalBlockMeta, VerifiedTxn, VerifiedTxnWithAccountSeqNum};

pub struct TxnBuffer {
    txns: Mutex<Vec<VerifiedTxnWithAccountSeqNum>>,
}
pub enum BlockState {
    Ordered((BlockId, ExternalBlock)),
    Computed(ComputeRes),
    Commited,
}

pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockId, BlockState>
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
}

impl BlockBufferManager {

    pub fn new() -> Self {
        let (sender, recv) = tokio::sync::broadcast::channel(1024);
        Self {
            txn_buffer: TxnBuffer { txns: Mutex::new(Vec::new()) },
            block_state_machine: Mutex::new(BlockStateMachine {
                sender,
                blocks: HashMap::new(),
            }),
        }
    }

    pub async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, anyhow::Error> {
        todo!()
    }

    pub async fn push_txns(&self, txn: Vec<VerifiedTxnWithAccountSeqNum>) {
        let mut txns = self.txn_buffer.txns.lock().await;
        txns.extend(txn);
    }

    pub async fn push_txn(&self, txn: VerifiedTxnWithAccountSeqNum) {
        let mut txns = self.txn_buffer.txns.lock().await;
        txns.push(txn);
    }

    pub async fn pop_txns(&self, max_size: usize) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
        let mut txns = self.txn_buffer.txns.lock().await;
        let len = txns.len();
        if len <= max_size && !txns.is_empty() {
            let result = std::mem::take(&mut *txns);
            return Ok(result)
        } else {
            let result = txns.split_off(max_size - len);
            return Ok(result)
        }
    }

    pub async fn push_commit_blocks(&self, block_ids: Vec<BlockId>) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        for block_id in block_ids {
            block_state_machine.blocks.insert(block_id, BlockState::Commited);
        }
        block_state_machine.sender.send(()).unwrap();
        Ok(())
    }

    pub async fn push_compute_res(&self, block_id: BlockId, compute_res: ComputeRes) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.blocks.insert(block_id, BlockState::Computed(compute_res));
        block_state_machine.sender.send(()).unwrap();
        Ok(())
    }

    pub async fn pop_commit_blocks(&self) -> Result<Vec<(BlockId, )>, anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        let mut block_ids = Vec::new();
        for (block_id, block_state) in block_state_machine.blocks.iter() {
            if let BlockState::Commited = block_state {
                block_ids.push(*block_id);
            }
        }
        block_state_machine.blocks.retain(|_, block_state| {
            !matches!(block_state, BlockState::Commited)
        });
        block_state_machine.sender.send(()).unwrap();
        Ok(block_ids)
    }

    pub async fn push_ordered_blocks(&self, parent_id: BlockId, blocks: ExternalBlock) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        let block_id = blocks.block_meta.block_id;
        block_state_machine.blocks.insert(block_id, BlockState::Ordered((parent_id, blocks)));
        block_state_machine.sender.send(()).unwrap();
        Ok(())
    }

    pub async fn pop_ordered_blocks(&self) -> Result<(ExternalBlock, BlockId), anyhow::Error> {
        todo!()
    }

    pub async fn get_executed_res(&self, block_id: BlockId) -> Result<ComputeRes, anyhow::Error> {
        pub enum Result {
            ComputeResult(ComputeRes),
            WaitChange(Receiver<()>)
        }
        loop {
            let recv = {
                let block_state_machine = self.block_state_machine.lock().await;
                info!("get_executed_res {:?}", block_id);
                if let Some(block) = block_state_machine.blocks.get(&block_id) {
                    let res = match block {
                        BlockState::Computed(res) => Result::ComputeResult(res.clone()),
                        BlockState::Ordered(_) => Result::WaitChange(block_state_machine.sender.subscribe()),
                        BlockState::Commited => {
                            panic!("There is no Ordered Block but try to get executed result for block {:?}", block_id);
                        }
                    };
                    info!("get_executed_res done with res {:?}", block_id);
                    res
                } else {
                    panic!("There is no Ordered Block but try to get executed result for block {:?}", block_id)
                }
            };
            match recv {
                Result::ComputeResult(res) => return Ok(res),
                Result::WaitChange(mut recv) => {
                    if let Err(e) = recv.recv().await {
                        panic!("Failed to get executed result for block {:?}", block_id)
                    }
                    continue;
                }
            }
        }
        
    }
}