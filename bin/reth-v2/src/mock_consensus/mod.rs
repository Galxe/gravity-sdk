use std::{collections::VecDeque, hash::{DefaultHasher, Hash, Hasher}, sync::Arc};

use alloy_rpc_types_engine::PayloadAttributes;
use api_types::{BlockId, ExecutionApiV2, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn, VerifiedTxnWithAccountSeqNum};

pub struct MockConsensus {
    exec_api: Arc<dyn ExecutionApiV2>,
    parent_meta: ExternalBlockMeta,
    pending_txns: VecDeque<VerifiedTxnWithAccountSeqNum>,
    block_number_water_mark: u64,
}

impl MockConsensus {
    async fn construct_block(&mut self, txns: &mut Vec<VerifiedTxn>, attr: ExternalPayloadAttr) -> Option<ExternalBlock> {
        loop {
            if let Some(txn) = self.pending_txns.pop_front() {
                let res = self.exec_api.check_block_txns(attr.clone(), vec![txn.txn.clone()]).await.unwrap();
                if res {
                    txns.push(txn.txn);
                } else {
                    let mut hasher = DefaultHasher::new();
                    txns.hash(&mut hasher);
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
            }
        }
    }

    pub async fn run(mut self) {
        let mut block_txns = vec![];
        let mut attr = ExternalPayloadAttr { ts: 0 };
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let txns = self.exec_api.recv_pending_txns().await.unwrap();
            self.pending_txns.extend(txns);
            if self.pending_txns.is_empty() {
                continue;
            }
            if let Some(block) = self.construct_block(&mut block_txns, attr.clone()).await {
                let head = block.block_meta.clone();
                self.exec_api.send_ordered_block(self.parent_meta.block_id, block).await.unwrap();
                attr.ts += 1;
                block_txns.clear();
                let _ = self.exec_api.recv_executed_block_hash(head.clone()).await.unwrap();
                self.exec_api.commit_block(head.block_id.clone()).await.unwrap();
                self.parent_meta = head;
            }

        }
    }
}