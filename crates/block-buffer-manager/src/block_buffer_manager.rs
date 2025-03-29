use log::info;
use std::{
    cell::OnceCell,
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, OnceLock},
    time::Duration,
};
use tokio::{sync::Mutex, time::Instant};

use api_types::{
    compute_res::ComputeRes, u256_define::BlockId, ExternalBlock,
    VerifiedTxn, VerifiedTxnWithAccountSeqNum,
};
use itertools::Itertools;

pub struct TxnBuffer {
    txns: Mutex<Vec<VerifiedTxnWithAccountSeqNum>>,
}

pub struct BlockIdNumHash {
    pub block_id: BlockId,
    pub num: u64,
    pub hash: Option<[u8; 32]>,
}

pub enum BlockState {
    Ordered { block: ExternalBlock, parent_id: BlockId },
    Computed((u64, ComputeRes)),
    Commited { hash: Option<[u8; 32]>, num: u64 },
}

pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockId, BlockState>,
    latest_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
}

impl BlockBufferManager {
    pub fn new() -> Self {
        let (sender, _recv) = tokio::sync::broadcast::channel(1024);
        Self {
            txn_buffer: TxnBuffer { txns: Mutex::new(Vec::new()) },
            block_state_machine: Mutex::new(BlockStateMachine {
                sender,
                blocks: HashMap::new(),
                latest_block_number: 0,
                latest_finalized_block_number: 0,
                block_number_to_block_id: HashMap::new(),
            }),
        }
    }

    pub async fn init(&self, latest_block_number: u64, block_number_to_block_id: HashMap<u64, BlockId>) {
        let mut block_state_machine = self.block_state_machine.lock().await;
        // When init, the latest_finalized_block_number is the same as latest_block_number
        block_state_machine.latest_block_number = latest_block_number;
        block_state_machine.latest_finalized_block_number = latest_block_number;
        block_state_machine.block_number_to_block_id = block_number_to_block_id;
    }

    // Helper method to wait for changes
    async fn wait_for_change(&self, timeout: Duration) -> Result<(), anyhow::Error> {
        let mut receiver = {
            let block_state_machine = self.block_state_machine.lock().await;
            block_state_machine.sender.subscribe()
        };

        tokio::select! {
            _ = receiver.recv() => Ok(()),
            _ = tokio::time::sleep(timeout) => Err(anyhow::anyhow!("Timeout waiting for change"))
        }
    }

    pub async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, anyhow::Error> {
        unimplemented!()
    }

    pub async fn push_txns(&self, txn: Vec<VerifiedTxnWithAccountSeqNum>) {
        let mut txns = self.txn_buffer.txns.lock().await;
        txns.extend(txn);
    }

    pub async fn push_txn(&self, txn: VerifiedTxnWithAccountSeqNum) {
        info!("push_txn {:?}", txn.txn.seq_number());
        let mut txns = self.txn_buffer.txns.lock().await;
        txns.push(txn);
    }

    pub async fn pop_txns(
        &self,
        max_size: usize,
    ) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
        let mut txns = self.txn_buffer.txns.lock().await;

        if txns.len() <= max_size {
            let result = std::mem::take(&mut *txns);
            return Ok(result);
        } else {
            // take 0..max_size
            let result = txns.drain(0..max_size).collect();
            return Ok(result);
        }
    }

    pub async fn set_ordered_blocks(
        &self,
        parent_id: BlockId,
        block: ExternalBlock,
    ) -> Result<(), anyhow::Error> {
        info!(
            "push_ordered_blocks {:?} num {:?}",
            block.block_meta.block_id, block.block_meta.block_number
        );
        let mut block_state_machine = self.block_state_machine.lock().await;
        let block_id = block.block_meta.block_id;
        block_state_machine.blocks.insert(block_id, BlockState::Ordered { block, parent_id });
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn get_ordered_blocks(
        &self,
        start_num: u64,
        max_size: Option<usize>,
    ) -> Result<Vec<(ExternalBlock, BlockId)>, anyhow::Error> {
        let start = Instant::now();
        let timeout = Duration::from_secs(5);
        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for ordered blocks"));
            }

            let block_state_machine = self.block_state_machine.lock().await;
            let result = block_state_machine
                .blocks
                .iter()
                .map(|(_id, block_state)| match block_state {
                    BlockState::Ordered { block, parent_id } => Some((block.clone(), *parent_id)),
                    _ => None,
                })
                .filter(|v| v.is_some())
                .map(|v| v.unwrap())
                .filter(|(b, _id)| b.block_meta.block_number >= start_num)
                .sorted_by_key(|(b, _id)| b.block_meta.block_number)
                .take(max_size.unwrap_or(usize::MAX))
                .collect::<Vec<_>>();
            if result.is_empty() {
                // Release lock before waiting
                drop(block_state_machine);

                // Wait for changes and try again
                match self.wait_for_change(Duration::from_millis(100)).await {
                    Ok(_) => continue,
                    Err(_) => continue, // Timeout on the wait, retry
                }
            } else {
                return Ok(result);
            }
        }
    }

    pub async fn get_executed_res(
        &self,
        block_id: BlockId,
        block_num: u64,
    ) -> Result<ComputeRes, anyhow::Error> {
        let start = Instant::now();
        let timeout = Duration::from_secs(5);

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("get_executed_res timeout for block {:?}", block_id));
            }

            let block_state_machine = self.block_state_machine.lock().await;
            info!("get_executed_res {:?}", block_id);

            if let Some(block) = block_state_machine.blocks.get(&block_id) {
                match block {
                    BlockState::Computed((num, res)) => {
                        info!(
                            "get_executed_res done with id {:?} num {:?} res {:?}",
                            block_id, *num, res
                        );
                        assert_eq!(*num, block_num);
                        return Ok(res.clone());
                    }
                    BlockState::Ordered { .. } => {
                        // Release lock before waiting
                        drop(block_state_machine);

                        // Wait for changes and try again
                        match self.wait_for_change(Duration::from_millis(100)).await {
                            Ok(_) => continue,
                            Err(_) => continue, // Timeout on the wait, retry
                        }
                    }
                    BlockState::Commited { .. } => {
                        panic!("There is no Ordered Block but try to get executed result for block {:?}", block_id);
                    }
                }
            } else {
                panic!(
                    "There is no Ordered Block but try to get executed result for block {:?}",
                    block_id
                )
            }
        }
    }

    pub async fn set_compute_res(
        &self,
        block_id: BlockId,
        block_hash: [u8; 32],
        block_num: u64,
    ) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        if let Some(BlockState::Ordered { block, parent_id: _ }) =
            block_state_machine.blocks.get(&block_id)
        {
            assert_eq!(block.block_meta.block_number, block_num);
            let txn_len = block.txns.len();
            block_state_machine.blocks.insert(
                block_id,
                BlockState::Computed((
                    block_num,
                    ComputeRes { data: block_hash, txn_num: txn_len as u64 },
                )),
            );
            let _ = block_state_machine.sender.send(());
            return Ok(());
        }
        panic!("There is no Ordered Block but try to push compute result for block {:?}", block_id)
    }

    pub async fn set_commit_blocks(
        &self,
        block_ids: Vec<BlockIdNumHash>,
    ) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        for block_id_num_hash in block_ids {
            info!(
                "push_commit_blocks id {:?} num {:?}",
                block_id_num_hash.block_id, block_id_num_hash.num
            );
            if let Some(state) = block_state_machine.blocks.get_mut(&block_id_num_hash.block_id) {
                match state {
                    BlockState::Computed((num, _)) => {
                        if *num == block_id_num_hash.num {
                            *state = BlockState::Commited {
                                hash: block_id_num_hash.hash,
                                num: block_id_num_hash.num,
                            };
                        } else {
                            panic!("There is no Ordered Block but try to push commit block for block {:?}", block_id_num_hash.block_id);
                        }
                    }
                    _ => {
                        panic!(
                            "There is no Ordered Block but try to push commit block for block {:?}",
                            block_id_num_hash.block_id
                        );
                    }
                }
            } else {
                panic!(
                    "There is no Ordered Block but try to push commit block for block {:?}",
                    block_id_num_hash.block_id
                );
            }
        }
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn get_commited_blocks(
        &self,
        start_num: u64,
        max_size: Option<usize>,
    ) -> Result<Vec<BlockIdNumHash>, anyhow::Error> {
        let start = Instant::now();
        let timeout = Duration::from_secs(2);

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for committed blocks"));
            }

            let mut block_state_machine = self.block_state_machine.lock().await;
            let result = block_state_machine
                .blocks
                .iter()
                .map(|(block_id, block_state)| match block_state {
                    BlockState::Commited { hash, num } => {
                        Some(BlockIdNumHash { block_id: *block_id, num: *num, hash: *hash })
                    }
                    _ => None,
                })
                .filter(|v| v.is_some())
                .map(|v| v.unwrap()) // Unwrap after filtering for Some values
                .filter(|v| v.num >= start_num)
                .sorted_by_key(|v| v.num) // Use sorted_by_key from itertools
                .take(max_size.unwrap_or(usize::MAX))
                .collect::<Vec<_>>();

            if result.is_empty() {
                // Release lock before waiting
                drop(block_state_machine);

                // Wait for changes and try again
                match self.wait_for_change(Duration::from_millis(100)).await {
                    Ok(_) => continue,
                    Err(_) => continue, // Timeout on the wait, retry
                }
            } else {
                block_state_machine.latest_finalized_block_number = std::cmp::max(block_state_machine.latest_finalized_block_number, result.last().unwrap().num);
                return Ok(result);
            }
        }
    }

    pub async fn remove_commited_blocks(
        &self,
        latest_persist_block_num: u64,
    ) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.latest_finalized_block_number = std::cmp::max(block_state_machine.latest_finalized_block_number, latest_persist_block_num);
        block_state_machine.blocks.retain(|_, block_state| match block_state {
            BlockState::Commited { num, .. } => *num > latest_persist_block_num,
            _ => true,
        });
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn latest_block_number(&self) -> u64 {
        let block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.latest_block_number
    }

    pub async fn block_number_to_block_id(&self) -> HashMap<u64, BlockId> {
        let block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.block_number_to_block_id.clone()
    }
}
