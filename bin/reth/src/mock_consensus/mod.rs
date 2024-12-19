use std::{
    collections::VecDeque,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::SystemTime,
};

use api_types::{
    BlockId, ExecutionApiV2, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn,
    VerifiedTxnWithAccountSeqNum,
};
use mempool::Mempool;
use tracing::info;

pub mod mempool;

pub struct MockConsensus {
    exec_api: Arc<dyn ExecutionApiV2>,
    parent_meta: ExternalBlockMeta,
    pending_txns: Mempool,
    block_number_water_mark: u64,
    gensis: [u8; 32],
}

impl MockConsensus {
    pub fn new(exec_api: Arc<dyn ExecutionApiV2>, gensis: [u8; 32]) -> Self {
        let parent_meta = ExternalBlockMeta { block_id: BlockId(gensis), block_number: 0, ts: 0 };
        Self {
            exec_api,
            parent_meta,
            pending_txns: Mempool::new(),
            block_number_water_mark: 0,
            gensis,
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
                ts: attr.ts,
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
            let time_gap = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
                - attr.ts;
            if time_gap > 1 {
                return self.construct_block(txns, attr);
            }
            let txn: Option<VerifiedTxnWithAccountSeqNum> = self.pending_txns.iter_ready().next().cloned();
            if let Some(txn) = txn {
                let res = self
                    .exec_api
                    .check_block_txns(attr.clone(), vec![txn.txn.clone()])
                    .await
                    .unwrap();
                if res {
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
            let txns = self.exec_api.recv_pending_txns().await.unwrap();
            for txn in txns {
                self.pending_txns.add(txn);
            }
            info!("pending txns size is {:?}", block_txns.len());
            let block = self.check_and_construct_block(&mut block_txns, attr.clone()).await;
            if let Some(block) = block {
                let head = block.block_meta.clone();
                let commit_txns = block.txns.clone();
                self.exec_api.send_ordered_block(self.parent_meta.block_id, block).await.unwrap();
                attr.ts =
                    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                
                block_txns.clear();
                let _ = self.exec_api.recv_executed_block_hash(head.clone()).await.unwrap();
                for txn in commit_txns {
                    self.pending_txns.commit(&txn.sender, txn.sequence_number);
                }
                self.exec_api.commit_block(head.block_id.clone()).await.unwrap();
                self.parent_meta = head;
            }
        }
    }
}
