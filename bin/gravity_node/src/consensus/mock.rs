use std::{
    collections::HashMap, hash::{DefaultHasher, Hash, Hasher}, sync::Arc, time::SystemTime
};

use super::mempool::Mempool;
use api_types::{
    compute_res::{ComputeRes, TxnStatus}, u256_define::BlockId, ExecutionChannel, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn
};

use alloy_primitives::B256;
use block_buffer_manager::{block_buffer_manager::BlockHashRef, get_block_buffer_manager};
use tracing::debug;

pub struct MockConsensus {
    parent_meta: ExternalBlockMeta,
    pending_txns: Mempool,
    block_number_water_mark: u64,
}

impl MockConsensus {
    pub async fn new() -> Self {
        let genesis_block_id = [
            141, 91, 216, 66, 168, 139, 218, 32, 132, 186, 161, 251, 250, 51, 34, 197, 38, 71, 196,
            135, 49, 116, 247, 25, 67, 147, 163, 137, 28, 58, 62, 73,
        ];
        let parent_meta = ExternalBlockMeta {
            block_id: BlockId(genesis_block_id),
            block_number: 0,
            usecs: 0,
            randomness: None,
            block_hash: None,
        };
        let mut block_number_to_block_id = HashMap::new();
        block_number_to_block_id.insert(0u64, BlockId(genesis_block_id));
        get_block_buffer_manager().init(0, block_number_to_block_id).await;
        Self {
            parent_meta,
            pending_txns: Mempool::new(),
            block_number_water_mark: 0,
        }
    }

    fn construct_block(
        &mut self,
        txns: &mut Vec<VerifiedTxn>,
        attr: ExternalPayloadAttr,
    ) -> Option<ExternalBlock> {
        let mut hasher = DefaultHasher::new();
        txns.hash(&mut hasher);
        attr.hash(&mut hasher);
        let block_id = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&block_id.to_be_bytes());
        self.block_number_water_mark += 1;
        return Some(ExternalBlock {
            block_meta: ExternalBlockMeta {
                block_id: BlockId(bytes),
                block_number: self.block_number_water_mark,
                usecs: attr.ts,
                randomness: None,
                block_hash: None,
            },
            txns: txns.drain(..).collect(),
        });
    }

    async fn check_and_construct_block(
        &mut self,
        txns: &mut Vec<VerifiedTxn>,
        attr: ExternalPayloadAttr,
    ) -> Option<ExternalBlock> {
        loop {
            let time_gap =
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
                    - attr.ts;
            if time_gap > 1 {
                return self.construct_block(txns, attr);
            }
            let txn = self.pending_txns.get_next();
            if let Some((_, txn)) = txn {
                if txns.len() < 5000 {
                    txns.push(txn.txn);
                } else {
                    return self.construct_block(txns, attr);
                }
            }
        }
    }

    pub async fn run(mut self) {
        let mut block_txns = vec![];
        let mut attr = ExternalPayloadAttr {
            ts: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
        };
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let txns = get_block_buffer_manager().pop_txns(usize::MAX).await.unwrap();
            for txn in txns {
                self.pending_txns.add(txn);
            }
            debug!("pending txns size is {:?}", block_txns.len());
            let block = self.check_and_construct_block(&mut block_txns, attr.clone()).await;
            if let Some(block) = block {
                let head = block.block_meta.clone();
                let commit_txns = block.txns.clone();
                get_block_buffer_manager().set_ordered_blocks(self.parent_meta.block_id, block).await.unwrap();
                attr.ts =
                    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

                block_txns.clear();
                let res = get_block_buffer_manager().get_executed_res(head.block_id.clone(),
                    head.block_number,
                    ).await.unwrap();
                for txn in commit_txns {
                    self.pending_txns.commit(&txn.sender, txn.sequence_number);
                }
                get_block_buffer_manager().set_commit_blocks(vec![
                    BlockHashRef {
                        block_id: head.block_id.clone(),
                        num: head.block_number,
                        hash: None,
                    }
                ]).await.unwrap();
                self.parent_meta = head;
            }
        }
    }
}
