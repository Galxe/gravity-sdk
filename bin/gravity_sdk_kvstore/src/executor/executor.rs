use crate::{
    compute_transaction_hash, verify_signature, AccountId, AccountState, Block, State, StateRoot,
    Storage, Transaction, TransactionKind, TransactionReceipt, TransactionWithAccount,
};

use super::*;
use block_buffer_manager::get_block_buffer_manager;
use futures::channel::oneshot::{channel, Sender};
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::lock::Mutex;
use futures::{stream::FuturesUnordered, StreamExt};
use futures::{FutureExt, SinkExt};
use gaptos::api_types::compute_res::ComputeRes;
use gaptos::api_types::ExternalBlock;
use log::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PipelineExecutor;

impl PipelineExecutor {
    pub async fn run(start_num: u64, storage: Arc<dyn Storage>, state: Arc<RwLock<State>>) {
        let pending_blocks = Arc::new(Mutex::new(HashMap::new()));
        let pending_blocks_clone = pending_blocks.clone();
        tokio::spawn(async move {
            Self::execute_task(start_num, None, state, pending_blocks).await;
        });
        tokio::spawn(async move {
            Self::commit_task(start_num, None, storage, pending_blocks_clone).await;
        });
    }

    pub async fn execute_task(
        start_num: u64,
        max_size: Option<usize>,
        state: Arc<RwLock<State>>,
        pending_blocks: Arc<Mutex<HashMap<u64, (StateRoot, Block)>>>,
    ) {
        loop {
            let ordered_blocks =
                get_block_buffer_manager().get_ordered_blocks(start_num, max_size).await;
            if let Err(e) = ordered_blocks {
                warn!("failed to get ordered blocks: {}", e);
                continue;
            }
            let ordered_blocks = ordered_blocks.unwrap();
            for (block, _) in ordered_blocks {
                let block_num = block.block_meta.block_number;
                let block_id = block.block_meta.block_id;
                let exec_res = Self::execute_block(block, &state).await;
                let res = get_block_buffer_manager()
                    .set_compute_res(block_id, exec_res, block_num, Arc::new(None), vec![])
                    .await;
                if let Err(e) = res {
                    warn!("failed to set compute res: {}", e);
                }
            }
        }
    }

    async fn execute_block(block: ExternalBlock, state: &Arc<RwLock<State>>) -> [u8; 32] {
        // TODO: implement account dependencies when enable pipeline
        let mut state = state.write().await;

        for tx in block.txns {
            let tx_with_account = TransactionWithAccount::from(tx);
            let receipt = Self::execute_transaction(&tx_with_account.txn, &state).unwrap();
            for (account_id, state_update) in receipt.state_updates {
                state.update_account_state(&account_id, state_update).await.unwrap();
            }
        }
        state.get_state_root().0
    }

    fn execute_transaction(tx: &Transaction, state: &State) -> Result<TransactionReceipt, String> {
        let sender = verify_signature(tx)?;
        let sender_id = AccountId(sender.clone());
        let mut updates = vec![];
        trace!("Executing transaction from {} tx {:?}, state is {:?}", sender, tx.unsigned, state);

        let mut sender_state = state
            .get_account(&sender_id.0)
            .map(|account| account.clone())
            .or_else(|| state.get_account(&sender))
            .map(|account| AccountState {
                nonce: account.nonce,
                balance: account.balance,
                kv_store: account.kv_store.clone(),
            })
            .unwrap_or_else(|| AccountState {
                nonce: 0,
                balance: 5000000000,
                kv_store: HashMap::new(),
            });

        if tx.unsigned.nonce != sender_state.nonce {
            return Err(format!(
                "Invalid nonce, tx nonce {}, tx {:?}, state nonce {}, whole state {:?}",
                tx.unsigned.nonce, tx, sender_state.nonce, state,
            ));
        }
        sender_state.nonce += 1;

        match &tx.unsigned.kind {
            TransactionKind::Transfer { receiver, amount } => {
                if sender_state.balance < *amount {
                    return Err(format!("Insufficient balance"));
                }

                let mut receiver_state = if let Some(account) = state.get_account(receiver) {
                    AccountState {
                        nonce: account.nonce,
                        balance: account.balance,
                        kv_store: account.kv_store.clone(),
                    }
                } else {
                    AccountState { nonce: 0, balance: 0, kv_store: HashMap::new() }
                };
                sender_state.balance -= amount;
                receiver_state.balance += amount;
                updates.push((AccountId(receiver.clone()), receiver_state));
            }
            TransactionKind::SetKV { key, value } => {
                sender_state.kv_store.insert(key.clone(), value.clone());
            }
        }
        updates.push((sender_id, sender_state));

        Ok(TransactionReceipt {
            transaction: tx.clone(),
            transaction_hash: compute_transaction_hash(&tx.unsigned),
            status: true,
            state_updates: updates,
            gas_used: 21000, // to simplify, we use one fiexd gas num
            logs: Vec::new(),
        })
    }

    pub async fn commit_task(
        start_num: u64,
        max_size: Option<usize>,
        storage: Arc<dyn Storage>,
        pending_blocks: Arc<Mutex<HashMap<u64, (StateRoot, Block)>>>,
    ) {
        loop {
            let committed_blocks =
                get_block_buffer_manager().get_committed_blocks(start_num, max_size).await;
            if let Err(e) = committed_blocks {
                warn!("failed to get committed blocks: {}", e);
                continue;
            }
            let committed_blocks = committed_blocks.unwrap();
            for block_id_num_hash in committed_blocks {
                let res =
                    Self::persist_block(block_id_num_hash.num, &pending_blocks, storage.as_ref())
                        .await;
                if let Err(e) = res {
                    warn!("failed to persist block: {}", e);
                }
            }
        }
    }

    async fn persist_block(
        block_number: u64,
        pending_blocks: &Mutex<HashMap<u64, (StateRoot, Block)>>,
        storage: &dyn Storage,
    ) -> Result<(), String> {
        let mut pending_blocks = pending_blocks.lock().await;
        let (state_root, final_block) = pending_blocks.remove(&block_number).unwrap();
        storage.save_block(&final_block).await.unwrap();
        storage.save_state_root(final_block.header.number, state_root).await.unwrap();
        info!("Block {} persisted", block_number);
        Ok(())
    }
}
