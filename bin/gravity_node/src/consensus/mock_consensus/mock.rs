use tracing::{info, warn};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Instant, SystemTime},
};

use super::mempool::{Mempool, TxnId};
use gaptos::api_types::{
    account::ExternalAccountAddress, u256_define::BlockId, ExternalBlock, ExternalBlockMeta,
    ExternalPayloadAttr, VerifiedTxn,
};

use block_buffer_manager::{block_buffer_manager::BlockHashRef, get_block_buffer_manager};

pub struct MockConsensus {
    pool: Arc<tokio::sync::Mutex<Mempool>>,
    genesis_block_id: BlockId,
    executed_jam_wait: Arc<(Mutex<u64>, Condvar)>,
}

static ORDERED_INTERVAL_MS: OnceLock<u64> = OnceLock::new();
fn get_ordered_interval_ms() -> u64 {
    *ORDERED_INTERVAL_MS.get_or_init(|| {
        std::env::var("MOCK_SET_ORDERED_INTERVAL_MS")
            .unwrap_or_else(|_| "200".to_string())
            .parse()
            .unwrap_or(200)
    })
}

fn get_max_txn_num() -> usize {
    std::env::var("MOCK_MAX_BLOCK_SIZE")
        .unwrap_or_else(|_| "7000".to_string())
        .parse()
        .unwrap_or(7000)
}

static MAX_EXECUTED_GAP: OnceLock<u64> = OnceLock::new();
fn get_max_executed_gap() -> u64 {
    *MAX_EXECUTED_GAP.get_or_init(|| {
        std::env::var("MAX_EXECUTED_GAP")
            .unwrap_or_else(|_| "16".to_string())
            .parse()
            .unwrap_or(16)
    })
}

impl MockConsensus {
    pub async fn new() -> Self {
        let genesis_block_id = BlockId([
            141, 91, 216, 66, 168, 139, 218, 32, 132, 186, 161, 251, 250, 51, 34, 197, 38, 71, 196,
            135, 49, 116, 247, 25, 67, 147, 163, 137, 28, 58, 62, 73,
        ]);
        let mut block_number_to_block_id = HashMap::new();
        block_number_to_block_id.insert(0u64, genesis_block_id.clone());
        get_block_buffer_manager().init(0, block_number_to_block_id).await;

        Self {
            pool: Arc::new(tokio::sync::Mutex::new(Mempool::new())),
            genesis_block_id,
            executed_jam_wait: Arc::new((Mutex::new(0), Condvar::new()))
        }
    }

    fn construct_block(
        block_number: u64,
        txns: Vec<VerifiedTxn>,
        attr: ExternalPayloadAttr,
    ) -> ExternalBlock {
        let mut hasher = DefaultHasher::new();
        txns.hash(&mut hasher);
        attr.hash(&mut hasher);
        let block_id = hasher.finish();
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&block_id.to_be_bytes());
        return ExternalBlock {
            block_meta: ExternalBlockMeta {
                block_id: BlockId(bytes),
                block_number,
                usecs: attr.ts,
                epoch: 0,
                randomness: None,
                block_hash: None,
                proposer: None,
            },
            txns,
        };
    }

    async fn check_and_construct_block(
        pool: &tokio::sync::Mutex<Mempool>,
        block_number: u64,
        attr: ExternalPayloadAttr,
    ) -> ExternalBlock {
        let max_txn_num: usize = get_max_txn_num();
        let mut txns = Vec::with_capacity(max_txn_num);
        loop {
            let time_gap =
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() -
                    attr.ts;
            if time_gap > 1 {
                return Self::construct_block(block_number, txns, attr);
            }
            let has_new_txn = pool.lock().await.get_txns(&mut txns, max_txn_num);
            if !has_new_txn {
                if txns.len() > 0 {
                    return Self::construct_block(block_number, txns, attr);
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    continue;
                }
            }

            if txns.len() > max_txn_num {
                return Self::construct_block(block_number, txns, attr);
            }
        }
    }

    pub async fn run(self) {
        tokio::spawn({
            let pool = self.pool.clone();
            async move {
                loop {
                    let txns = get_block_buffer_manager().pop_txns(usize::MAX, u64::MAX).await.unwrap();
                    let mut pool = pool.lock().await;
                    pool.add_txns(txns);
                    drop(pool);
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        get_ordered_interval_ms(),
                    ))
                    .await;
                }
            }
        });

        let (block_meta_tx, mut block_meta_rx) = tokio::sync::mpsc::channel(8);
        tokio::spawn({
            let pool = self.pool.clone();
            let mut parent_id = self.genesis_block_id;
            let executed_jam_wait = self.executed_jam_wait.clone();
            async move {
                let mut block_number = 0;
                loop {
                    block_number += 1;
                    let attr = ExternalPayloadAttr {
                        ts: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    let block =
                        Self::check_and_construct_block(&pool, block_number, attr.clone()).await;

                    let head_meta = block.block_meta.clone();
                    get_block_buffer_manager().set_ordered_blocks(parent_id, block).await.unwrap();
                    parent_id = head_meta.block_id.clone();
                    let _ = block_meta_tx.send(head_meta).await;
                    // wait if there's large gap between executed block and ordered block
                    {
                        let (lock, cvar) = executed_jam_wait.as_ref();
                        let mut executed_number = lock.lock().unwrap();
                        let large_gap = block_number - *executed_number;
                        let start = Instant::now();
                        while (block_number - *executed_number) > get_max_executed_gap() {
                            executed_number = cvar.wait(executed_number).unwrap();
                        }
                        if large_gap > get_max_executed_gap() {
                            info!("large executed gap = {}, wait more {}ms", large_gap, start.elapsed().as_millis());
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        get_ordered_interval_ms(),
                    ))
                    .await;
                }
            }
        });

        let (commit_txns_tx, mut commit_txns_rx) =
            tokio::sync::mpsc::unbounded_channel::<Vec<TxnId>>();
        tokio::spawn({
            let pool = self.pool.clone();
            async move {
                while let Some(txns) = commit_txns_rx.recv().await {
                    pool.lock().await.commit_txns(&txns);
                }
            }
        });

        while let Some(block_meta) = block_meta_rx.recv().await {
            let block_id = block_meta.block_id;
            let block_number = block_meta.block_number;

            let res = loop {
                match get_block_buffer_manager().get_executed_res(block_id, block_number).await {
                    Ok(r) => {
                        break r;
                    },
                    Err(e) => {
                        let msg = format!("{}", e);
                        warn!("get executed result failed: {}", msg);
                        if !msg.contains("get_executed_res timeout") {
                            panic!("get executed result failed: {}", msg);
                        }
                    },
                }
            };

            {
                let (lock, cvar) = self.executed_jam_wait.as_ref();
                let mut executed_number = lock.lock().unwrap();
                *executed_number = block_number;
                cvar.notify_all();
            }

            get_block_buffer_manager()
                .set_commit_blocks(vec![BlockHashRef {
                    block_id,
                    num: block_number,
                    hash: Some(res.execution_output.data),
                    persist_notifier: None,
                }])
                .await
                .unwrap();

            let committed_txns = res
                .execution_output
                .txn_status
                .as_ref()
                .as_ref()
                .unwrap()
                .iter()
                .map(|tx| TxnId {
                    sender: ExternalAccountAddress::new(tx.sender),
                    seq_num: tx.nonce,
                })
                .collect::<Vec<_>>();
            let _ = commit_txns_tx.send(committed_txns);
        }
    }
}
