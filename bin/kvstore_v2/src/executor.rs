use super::*;
use api_types::compute_res::ComputeRes;
use futures::channel::oneshot::{channel, Sender};
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{stream::FuturesUnordered, StreamExt};
use futures::{FutureExt, SinkExt};
use log::info;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PipelineExecutor {
    state: Arc<RwLock<State>>,
    block_update_accounts: HashMap<u64, Vec<AccountId>>,
    // stage one
    ordered_block_receiver: mpsc::Receiver<ExecutableBlock>,
    // stage two
    execution_queue: FuturesUnordered<BoxFuture<'static, Result<BlockExecutionResult, String>>>,
    pending_blocks: HashMap<u64, BlockExecutionPlan>,
    block_commit_senders: HashMap<u64, Sender<u64>>,
    // account occupied by block number
    account_locks: RwLock<HashMap<AccountId, VecDeque<u64>>>,
    commit_req_sender: mpsc::Sender<BlockExecutionResult>,
    // stage three:
    commit_req_receiver: mpsc::Receiver<BlockExecutionResult>,
    compute_res_senders: HashMap<u64, Sender<(ComputeRes, Sender<u64>)>>,
    pending_persisting_block: HashMap<u64, (Block, StateRoot)>,
    // stage four:
    persisting_notifiers: FuturesUnordered<futures::channel::oneshot::Receiver<u64>>,
    // stage five:
    committing_queue: FuturesUnordered<futures::channel::oneshot::Receiver<u64>>,
}

impl PipelineExecutor {
    pub fn new(
        state: Arc<RwLock<State>>,
        start_block: u64,
        ordered_block_receiver: mpsc::Receiver<ExecutableBlock>,
    ) -> Self {
        let (commit_req_sender, commit_req_receiver) = mpsc::channel(100);
        Self {
            state,
            block_update_accounts: HashMap::new(),
            ordered_block_receiver,
            execution_queue: FuturesUnordered::new(),
            pending_blocks: HashMap::new(),
            block_commit_senders: HashMap::new(),
            account_locks: RwLock::new(HashMap::new()),
            commit_req_sender,
            commit_req_receiver,
            compute_res_senders: HashMap::new(),
            pending_persisting_block: HashMap::new(),
            persisting_notifiers: FuturesUnordered::new(),
            committing_queue: FuturesUnordered::new(),
        }
    }

    pub async fn add_block(&mut self, block: ExecutableBlock) -> Result<(), String> {
        let block_number = block.block.block_number;
        self.compute_res_senders.insert(block_number, block.callbacks);
        let execution_plan = self.create_execution_plan(block.block)?;
        {
            let mut account_locks = self.account_locks.write().await;
            for tx in &execution_plan.block.transactions {
                let sender = verify_signature(tx)?;
                let sender_id = AccountId(sender);
                info!("Locking account {} for block {}", sender_id.0, block_number);
                account_locks
                    .entry(sender_id.clone())
                    .or_insert_with(VecDeque::new)
                    .push_back(block_number);
            }
        }
        self.pending_blocks.insert(block_number, execution_plan);
        self.schedule_ready_blocks().await?;
        info!("Added block {} to execution pipeline", block_number);
        Ok(())
    }

    fn create_execution_plan(&self, block: RawBlock) -> Result<BlockExecutionPlan, String> {
        let mut account_dependencies = HashMap::new();

        // TODO: implement account dependencies when enable pipeline
        for (_, tx) in block.transactions.iter().enumerate() {
            let sender = verify_signature(tx)?;
            let sender_id = AccountId(sender);
            account_dependencies.entry(sender_id).or_insert_with(HashSet::new);

            // match &tx.unsigned.kind {
            //     TransactionKind::Transfer { receiver, .. } => {
            //         let receiver_id = AccountId(receiver.clone());
            //         account_dependencies
            //             .entry(sender_id.clone())
            //             .or_insert_with(HashSet::new)
            //             .insert(receiver_id.clone());
            //         account_dependencies
            //             .entry(receiver_id)
            //             .or_insert_with(HashSet::new)
            //             .insert(sender_id);
            //     }
            //     TransactionKind::SetKV { .. } => {
            //         account_dependencies
            //             .entry(sender_id)
            //             .or_insert_with(HashSet::new);
            //     }
            // }
        }

        Ok(BlockExecutionPlan { block, account_dependencies })
    }

    async fn schedule_ready_blocks(&mut self) -> Result<(), String> {
        let mut ready_blocks = Vec::new();

        for (block_number, plan) in &self.pending_blocks {
            info!("the account lock is  {:?}", self.account_locks);

            let account_dependencies = &plan.account_dependencies;
            let account_locks = self.account_locks.read().await;

            if account_dependencies.is_empty() {
                info!("Block {} has no dependencies, ready to execute", block_number);
                ready_blocks.push(*block_number);
            } else {
                let mut all_accounts_ready = true;
                for (account, _) in account_dependencies {
                    if let Some(locked_by) = account_locks.get(account) {
                        if locked_by.front().unwrap() < block_number {
                            all_accounts_ready = false;
                            break;
                        }
                    } else {
                    }
                }

                if all_accounts_ready {
                    info!("Block {} is ready to execute", block_number);
                    ready_blocks.push(*block_number);
                }
            }
        }

        for block_number in ready_blocks {
            if let Some(plan) = self.pending_blocks.remove(&block_number) {
                let state = self.state.clone();
                info!("Scheduling block {} for execution", block_number);
                self.execution_queue.push(Self::execute_block(plan, state).boxed());
            }
        }

        Ok(())
    }

    async fn execute_block(
        plan: BlockExecutionPlan,
        state: Arc<RwLock<State>>,
    ) -> Result<BlockExecutionResult, String> {
        let mut execution_result = BlockExecutionResult {
            block_number: plan.block.block_number,
            state_updates: HashMap::new(),
            receipts: Vec::new(),
        };

        let mut pending_transactions = plan.block.transactions;
        info!(
            "Executing block {} with txns {:?}",
            execution_result.block_number, pending_transactions
        );

        for tx in pending_transactions.iter_mut() {
            let state = state.read().await;
            let receipt =
                Self::execute_transaction(tx, &mut execution_result.state_updates, &state)?;

            execution_result.receipts.push(receipt);
        }

        Ok(execution_result)
    }

    fn execute_transaction(
        tx: &Transaction,
        state_updates: &mut HashMap<AccountId, AccountState>,
        state: &State,
    ) -> Result<TransactionReceipt, String> {
        let sender = verify_signature(tx)?;
        let sender_id = AccountId(sender.clone());
        info!(
            "Executing transaction from {} to {:?}, state is {:?}",
            sender, tx.unsigned.kind, state
        );

        let mut sender_state = state_updates
            .get(&sender_id)
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
                "Invalid nonce, tx nonce {}, state nonce {}",
                tx.unsigned.nonce, sender_state.nonce
            ));
        }
        sender_state.nonce += 1;

        match &tx.unsigned.kind {
            TransactionKind::Transfer { receiver, amount } => {
                if sender_state.balance < *amount {
                    return Err(format!("Insufficient balance"));
                }

                let receiver_id = AccountId(receiver.clone());
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

                state_updates.insert(sender_id, sender_state);
                state_updates.insert(receiver_id, receiver_state);
            }
            TransactionKind::SetKV { key, value } => {
                sender_state.kv_store.insert(key.clone(), value.clone());
                state_updates.insert(sender_id, sender_state);
            }
        }

        Ok(TransactionReceipt {
            transaction: tx.clone(),
            transaction_hash: compute_transaction_hash(&tx.unsigned),
            status: true,
            gas_used: 21000, // to simplify, we used one fiexd gas num
            logs: Vec::new(),
        })
    }

    pub async fn process_execution_results(
        &mut self,
        execution_result: Result<BlockExecutionResult, String>,
    ) -> Result<(), String> {
        info!("Processing execution result {:?}", execution_result);
        let execution_result = execution_result?;
        let block_number = execution_result.block_number;
        let update_accounts = execution_result.state_updates.keys().cloned().collect::<Vec<_>>();

        self.block_update_accounts.insert(block_number, update_accounts);

        info!("send commit request for block {}", block_number);
        let (block_commit_sender, block_commit_receiver) = oneshot::channel();
        self.block_commit_senders.insert(block_number, block_commit_sender);
        self.commit_req_sender
            .send(execution_result.clone())
            .await
            .map_err(|e| format!("Failed to send commit result: {}", e))?;

        self.committing_queue.push(block_commit_receiver);

        Ok(())
    }

    async fn release_account_locks(&mut self, block_number: u64, accounts: Vec<AccountId>) {
        let mut account_locks = self.account_locks.write().await;
        for account in accounts {
            if let Some(locked_by) = account_locks.get_mut(&account) {
                assert_eq!(locked_by.front().unwrap(), &block_number);
                locked_by.retain(|v| *v != block_number);
            }
        }
    }

    fn create_block_from_result(result: &BlockExecutionResult) -> Result<Block, String> {
        Ok(Block {
            header: BlockHeader {
                number: result.block_number,
                parent_hash: [0; 32], // todo(): read from storage
                state_root: [0; 32],
                transactions_root: compute_merkle_root(
                    &result.receipts.iter().map(|r| r.transaction.clone()).collect::<Vec<_>>(),
                ),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            transactions: result.receipts.iter().map(|r| r.transaction.clone()).collect(),
        })
    }

    async fn process_commit_request(
        &mut self,
        state: Arc<RwLock<State>>,
        result: BlockExecutionResult,
    ) -> Result<(), String> {
        info!("Committing block {}", result.block_number);
        let block_number = result.block_number;
        let block = Self::create_block_from_result(&result)?;
        let mut state_guard = state.write().await;
        for (account_id, state_update) in result.state_updates {
            state_guard.update_account_state(&account_id, state_update).await?;
        }

        let state_root = state_guard.compute_state_root()?;

        let compute_res = ComputeRes { data: state_root.0, txn_num: result.receipts.len() as u64 };
        let send_computes_res_sender =
            self.compute_res_senders.remove(&result.block_number).unwrap();

        let (persist_sender, persist_receiver) = channel();
        info!("send compute res for block {:?}, computeres {:?}", block_number, compute_res);
        send_computes_res_sender.send((compute_res, persist_sender)).unwrap();

        let mut final_block = block;
        final_block.header.state_root = state_root.0;
        self.pending_persisting_block.insert(result.block_number, (final_block, state_root));

        self.persisting_notifiers.push(persist_receiver);

        info!("Committed block {}", block_number);
        Ok(())
    }

    async fn persist_block(
        &mut self,
        block_number: u64,
        storage: Arc<dyn Storage>,
    ) -> Result<(), String> {
        let (final_block, state_root) =
            self.pending_persisting_block.remove(&block_number).unwrap();
        let storage = storage.clone();
        storage.save_block(&final_block).await.unwrap();
        storage.save_state_root(final_block.header.number, state_root).await.unwrap();
        let _ = self.block_commit_senders.remove(&block_number).unwrap().send(block_number);

        info!("Block {} persisted", block_number);
        Ok(())
    }

    pub async fn start(mut self, storage: Arc<dyn Storage>) {
        loop {
            tokio::select! {
                // Stage one: Get ordered block from the consensus layer and trigger the execution process
                block = self.ordered_block_receiver.select_next_some() => {
                    self.add_block(block).await.unwrap();
                },
                // Stage two: Get the execution result and send it to consensus layer
                Some(execution_result) = self.execution_queue.next() => {
                    self.process_execution_results(execution_result).await.unwrap();
                },
                // Stage three: Get the execution result consensus result from consensus layer
                execution_result = self.commit_req_receiver.select_next_some() => {
                    self.process_commit_request(self.state.clone(), execution_result).await.unwrap();
                },
                // Stage four: persite all the data
                Some(commit_block_number_result) = self.persisting_notifiers.next() => {
                    let block_number = commit_block_number_result.unwrap();
                    self.persist_block(block_number, storage.clone()).await.unwrap();
                },
                // Stage five: release the resource acquired and schedule for next block
                Some(block_number_ret) = self.committing_queue.next() => {
                    let block_number = block_number_ret.unwrap();
                    let update_accounts = self.block_update_accounts.remove(&block_number).unwrap();
                    self.release_account_locks(block_number, update_accounts).await;
                    info!("Processing execution result {:?} try commit release", block_number);
                    self.schedule_ready_blocks().await.unwrap();
                    info!("Block committed");
                },
            }
        }
    }
}
