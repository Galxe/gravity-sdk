use crate::{metrics::{fetch_reth_txn_metrics}, ConsensusArgs};
use alloy_consensus::Transaction;
use alloy_eips::{eip4895::Withdrawals, BlockId, BlockNumberOrTag, Decodable2718, Encodable2718};
use alloy_primitives::{
    private::alloy_rlp::{Decodable, Encodable}, Address, FixedBytes, TxHash, B256
};
use gaptos::api_types::u256_define::{BlockId as ExternalBlockId, TxnHash};
use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId},
    compute_res::ComputeRes,
};
use gaptos::api_types::{
    compute_res::TxnStatus,
    GLOBAL_CRYPTO_TXN_HASHER,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use gaptos::api_types::{ExecutionBlocks, ExternalBlock, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use block_buffer_manager::get_block_buffer_manager;
use rayon::iter::IntoParallelRefMutIterator;
use core::panic;
use greth::{reth_ethereum_engine_primitives::EthPayloadAttributes, reth_node_core::primitives::account, reth_transaction_pool::{EthPooledTransaction, TransactionOrigin, ValidPoolTransaction}};
use greth::reth_node_api::NodeTypesWithDBAdapter;
use greth::reth_node_ethereum::EthereumNode;
use greth::reth_pipe_exec_layer_ext_v2::{ExecutedBlockMeta, OrderedBlock, PipeExecLayerApi};
use greth::reth_primitives::TransactionSigned;
use greth::reth_provider::providers::BlockchainProvider;
use greth::reth_provider::{
    AccountReader, BlockNumReader, BlockReaderIdExt, ChainSpecProvider, DatabaseProviderFactory,
};
use greth::reth_transaction_pool::TransactionPool;
use greth::{
    gravity_storage::block_view_storage::BlockViewStorage, reth_db::DatabaseEnv,
    reth_pipe_exec_layer_ext_v2::ExecutionResult, reth_primitives::EthPrimitives,
    reth_provider::BlockHashReader,
};
use greth::{
    reth::rpc::builder::auth::AuthServerHandle, reth_node_core::primitives::SignedTransaction,
};
use std::{collections::{HashMap, HashSet}, sync::Arc, time::{Instant, SystemTime}};
use tokio::sync::Mutex;
use tracing::*;

pub struct RethCli {
    auth: AuthServerHandle,
    pipe_api: PipeExecLayerApi<
        BlockViewStorage<
            BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
        >,
    >,
    chain_id: u64,
    provider: BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
    txn_listener: Mutex<tokio::sync::mpsc::Receiver<TxHash>>,
    pool: greth::reth_transaction_pool::Pool<
        greth::reth_transaction_pool::TransactionValidationTaskExecutor<
            greth::reth_transaction_pool::EthTransactionValidator<
                BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
                greth::reth_transaction_pool::EthPooledTransaction,
            >,
        >,
        greth::reth_transaction_pool::CoinbaseTipOrdering<
            greth::reth_transaction_pool::EthPooledTransaction,
        >,
        greth::reth_transaction_pool::blobstore::DiskFileBlobStore,
    >,
    txn_cache: Mutex<HashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>>,
    max_nonce_cache: Mutex<HashMap<Address, u64>>,
    txn_batch_size: usize,
    txn_check_interval: tokio::time::Duration,
    address_init_nonce_cache: Mutex<HashMap<Address, u64>>,
}

pub fn convert_account(acc: Address) -> ExternalAccountAddress {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(acc.as_slice());
    ExternalAccountAddress::new(bytes)
}

fn calculate_txn_hash(bytes: &Vec<u8>) -> [u8; 32] {
    alloy_primitives::utils::keccak256(bytes.clone()).as_slice().try_into().unwrap()
}

impl RethCli {
    pub async fn new(args: ConsensusArgs) -> Self {
        let chian_info = args.provider.chain_spec().chain;
        let chain_id = match chian_info.into_kind() {
            greth::reth_chainspec::ChainKind::Named(n) => n as u64,
            greth::reth_chainspec::ChainKind::Id(id) => id,
        };
        GLOBAL_CRYPTO_TXN_HASHER.get_or_init(|| Box::new(calculate_txn_hash));
        RethCli {
            auth: args.engine_api,
            pipe_api: args.pipeline_api,
            chain_id,
            provider: args.provider,
            txn_listener: Mutex::new(args.tx_listener),
            pool: args.pool,
            txn_cache: Mutex::new(HashMap::new()),
            txn_batch_size: 2000,
            txn_check_interval: std::time::Duration::from_millis(50),
            max_nonce_cache: Mutex::new(HashMap::new()),
            address_init_nonce_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn txn_to_signed(bytes: &mut [u8], chain_id: u64) -> (Address, TransactionSigned) {
        let txn = TransactionSigned::decode_2718(&mut bytes.as_ref()).unwrap();
        (txn.recover_signer().unwrap(), txn)
    }

    pub async fn push_ordered_block(
        &self,
        mut block: ExternalBlock,
        parent_id: B256,
    ) -> Result<(), String> {
        trace!("push ordered block {:?} with parent id {}", block, parent_id);
        let system_time = Instant::now();
        let pipe_api = &self.pipe_api;

                
        let mut senders = vec![None; block.txns.len()];
        let mut transactions = vec![None; block.txns.len()];

        {
            let mut cache = self.txn_cache.lock().await;
            for (idx, txn) in block.txns.iter().enumerate() {
                let key = (txn.sender.clone(), txn.sequence_number);
                if let Some(cached_txn) = cache.remove(&key) {
                    senders[idx] = Some(cached_txn.sender());
                    transactions[idx] = Some(cached_txn.transaction.transaction().tx().clone());
                }
            }
        }

        block.txns.par_iter_mut().enumerate()
            .filter(|(idx, _)| senders[*idx].is_none())
            .map(|(idx, txn)| {
                let (sender, transaction) = Self::txn_to_signed(&mut txn.bytes, self.chain_id);
                (idx, sender, transaction)
            })
            .collect::<Vec<(usize, Address, TransactionSigned)>>()
            .into_iter()
            .for_each(|(idx, sender, transaction)| {
                senders[idx] = Some(sender);
                transactions[idx] = Some(transaction);
            });

        let senders: Vec<_> = senders.into_iter().map(|x| x.unwrap()).collect();
        let transactions: Vec<_> = transactions.into_iter().map(|x| x.unwrap()).collect();

        
        let randao = match block.block_meta.randomness {
            Some(randao) => B256::from_slice(randao.0.as_ref()),
            None => B256::ZERO,
        };
        info!("push ordered block time deserialize {:?}ms", system_time.elapsed().as_millis());
        // TODO: make zero make sense
        pipe_api.push_ordered_block(OrderedBlock {
            parent_id,
            id: B256::from_slice(block.block_meta.block_id.as_bytes()),
            number: block.block_meta.block_number,
            timestamp: block.block_meta.usecs / 1000000,
            // TODO(gravity_jan): add reth coinbase
            coinbase: Address::ZERO,
            prev_randao: randao,
            withdrawals: Withdrawals::new(Vec::new()),
            transactions,
            senders,
        });
        Ok(())
    }

    pub async fn recv_compute_res(&self) -> Result<ExecutionResult, ()> {
        let pipe_api = &self.pipe_api;
        let result = pipe_api
            .pull_executed_block_hash()
            .await
            .expect("failed to recv compute res in recv_compute_res");
        debug!("recv compute res done");
        Ok(result)
    }

    pub async fn send_committed_block_info(
        &self,
        block_id: gaptos::api_types::u256_define::BlockId,
        block_hash: Option<B256>,
    ) -> Result<(), String> {
        debug!("commit block {:?} with hash {:?}", block_id, block_hash);
        let block_id = B256::from_slice(block_id.0.as_ref());
        let pipe_api = &self.pipe_api;
        pipe_api.commit_executed_block_hash(block_id, block_hash);
        debug!("commit block done");
        Ok(())
    }

    pub async fn start_mempool(&self) -> Result<(), String> {
        info!("start process pending transactions with timeout");
        let mut mut_txn_listener = self.txn_listener.lock().await;
    
        let batch_size: usize = self.txn_batch_size;
        let timeout_duration = self.txn_check_interval;
        let sleep = tokio::time::sleep(timeout_duration);
        tokio::pin!(sleep);
        let mut tx_hash_vec = Vec::with_capacity(batch_size * 100);
        loop {
            tokio::select! {
                biased;
                _size = mut_txn_listener.recv_many(&mut tx_hash_vec, batch_size * 100) => {
                    if tx_hash_vec.len() >= batch_size {
                        debug!("Hash buffer full ({} hashes), pushing transactions.", tx_hash_vec.len());
                        self.process_transaction_hashes(&mut tx_hash_vec).await?;
                        tx_hash_vec.clear();
                        sleep.as_mut().reset(tokio::time::Instant::now() + timeout_duration);
                    }
                }
                _ = &mut sleep => {
                    if !tx_hash_vec.is_empty() {
                        debug!("Timeout reached, processing {} buffered transaction hashes.", tx_hash_vec.len());
                        self.process_transaction_hashes(&mut tx_hash_vec).await?;
                        tx_hash_vec.clear();
                    }
                    sleep.as_mut().reset(tokio::time::Instant::now() + timeout_duration);
                }
            }
        }
    }


    async fn process_transaction_hashes(&self, tx_hash_vec: &mut Vec<TxHash>) -> Result<(), String> {
        let mut max_nonce_cache = self.max_nonce_cache.lock().await;
        let mut pool_txns = self.pool.get_all(tx_hash_vec.drain(..).collect());
        let mut tx_gap = HashSet::new();
        pool_txns = pool_txns.into_iter().filter(|txn| {
            let sender = txn.sender();
            let nonce = txn.nonce();
            if let Some(max_nonce) = max_nonce_cache.get_mut(&sender) {
                if nonce <= *max_nonce {
                    return false;
                }
            }
            true
        }).collect();
        pool_txns.iter().for_each(|txn| {
            let sender = txn.sender();
            let nonce = txn.nonce();
            if let Some(max_nonce) = max_nonce_cache.get_mut(&sender) {
                if nonce != *max_nonce + 1 {
                    for i in *max_nonce + 1..nonce {
                        tx_gap.insert((sender.clone(), i));
                    }
                }
                if nonce > *max_nonce {
                    *max_nonce = nonce;
                }
                if tx_gap.contains(&(sender.clone(), nonce)) {
                    tx_gap.remove(&(sender.clone(), nonce));
                }
            } else {
                max_nonce_cache.insert(sender, nonce);
            }
        });
        tx_gap.iter().for_each(|(sender, nonce)| {
            let txn = self.pool.get_pending_transactions_by_sender(sender.clone())
                .iter()
                .find(|txn| txn.nonce() == *nonce)
                .map(|txn| txn.clone());
            info!("filter sender {:?} nonce {:?} is_some {:?}", sender, nonce, txn.is_some());
            if let Some(txn) = txn {
                pool_txns.push(txn);
            }
        });
        self.process_pool_transactions(pool_txns).await?;
        Ok(())
    }

    async fn process_pool_transactions(&self, pool_txns: Vec<Arc<ValidPoolTransaction<EthPooledTransaction>>>) -> Result<(), String> {
        let mut buffer = Vec::with_capacity(pool_txns.len());
        let mut gas_limit = 0;
        for pool_txn in pool_txns {
            let txn_hash = pool_txn.hash();
            let txn_insert_time = self.pool.txn_insert_time(*txn_hash);
            if let Some(txn_insert_time) = txn_insert_time {
                fetch_reth_txn_metrics().txn_time.record((SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u64 - txn_insert_time) as f64);
            }
            let sender = pool_txn.sender();
            let nonce = pool_txn.nonce();
            let txn = pool_txn.transaction.transaction().tx();
            let account_nonce = {
                let mut init_nonce_cache = self.address_init_nonce_cache.lock().await;
                if !init_nonce_cache.contains_key(&sender) {
                    let account_nonce = self.provider.basic_account(&sender)
                        .map_err(|e| format!("Failed to get basic account for {}: {:?}", sender, e))?
                        .map(|x| x.nonce)
                        .unwrap_or(nonce);
                    init_nonce_cache.insert(sender, account_nonce);
                }
                *init_nonce_cache.get(&sender).unwrap()
            };
            gas_limit += txn.gas_limit();
            debug!("recv sender {:?} nonce {:?}, account nonce {:?} hash {:?}", sender, nonce, account_nonce, txn.hash());
            let bytes = txn.encoded_2718();
    
            let vtxn = VerifiedTxnWithAccountSeqNum {
                txn: VerifiedTxn {
                    bytes,
                    sender: convert_account(sender),
                    sequence_number: nonce,
                    chain_id: ExternalChainId::new(0),
                    committed_hash: TxnHash::from_bytes(txn.hash().as_slice()).into(),
                },
                account_seq_num: account_nonce,
            };
            
            {
                self.txn_cache.lock().await
                    .insert((vtxn.txn.sender().clone(), vtxn.txn.seq_number()), pool_txn.clone());
            }
            buffer.push(vtxn);
        }
        
        if !buffer.is_empty() {
            get_block_buffer_manager().push_txns(&mut buffer, gas_limit).await;
        }
        Ok(())
    }

    pub async fn start_execution(&self) -> Result<(), String> {
        let mut start_ordered_block = self.provider.last_block_number().unwrap() + 1;
        loop {
            // max executing block number
            let exec_blocks =
                get_block_buffer_manager().get_ordered_blocks(start_ordered_block, None).await;
            if let Err(e) = exec_blocks {
                warn!("failed to get ordered blocks: {}", e);
                continue;
            }
            let exec_blocks = exec_blocks.unwrap();
            if exec_blocks.is_empty() {
                info!("no ordered blocks");
                continue;
            }
            start_ordered_block = exec_blocks.last().unwrap().0.block_meta.block_number + 1;
            for (block, parent_id) in exec_blocks {
                info!(
                    "send reth ordered block num {:?} id {:?} with parent id {}",
                    block.block_meta.block_number, block.block_meta.block_id, parent_id
                );
                let parent_id = B256::from_slice(parent_id.as_bytes());
                self.push_ordered_block(block, parent_id).await?;
            }
        }
    }

    pub async fn start_commit_vote(&self) -> Result<(), String> {
        loop {
            let execution_result =
                self.recv_compute_res().await.expect("failed to recv compute res");
            let mut block_hash_data = [0u8; 32];
            block_hash_data.copy_from_slice(execution_result.block_hash.as_slice());
            let block_id = ExternalBlockId::from_bytes(execution_result.block_id.as_slice());
            let block_number = execution_result.block_number;
            let tx_infos = execution_result.txs_info;
            let txn_status = Arc::new(Some(
                tx_infos
                    .iter()
                    .map(|tx_info| {
                        TxnStatus {
                            txn_hash: *tx_info.tx_hash,
                            sender: convert_account( tx_info.sender).bytes(),
                            nonce: tx_info.nonce,
                            is_discarded: tx_info.is_discarded,
                        }
                    })
                    .collect(),
            ));
            get_block_buffer_manager()
                .set_compute_res(block_id, block_hash_data, block_number, txn_status)
                .await
                .expect("failed to pop ordered block ids");
        }
    }

    pub async fn start_commit(&self) -> Result<(), String> {
        let mut start_commit_num = self.provider.last_block_number().unwrap() + 1;
        loop {
            let block_ids =
                get_block_buffer_manager().get_committed_blocks(start_commit_num, None).await;
            if let Err(e) = block_ids {
                warn!("failed to get committed blocks: {}", e);
                continue;
            }
            let block_ids = block_ids.unwrap();
            if block_ids.is_empty() {
                continue;
            }
            let block_id =
                self.pipe_api.get_block_id(block_ids.last().unwrap().num).unwrap_or_else(|| {
                    panic!("commit num {} not found block id", start_commit_num);
                });
            assert_eq!(
                ExternalBlockId::from_bytes(block_id.as_slice()),
                block_ids.last().unwrap().block_id
            );
            start_commit_num = block_ids.last().unwrap().num + 1;
            for block_id_num_hash in block_ids {
                self.send_committed_block_info(
                    block_id_num_hash.block_id,
                    block_id_num_hash.hash.map(|x| B256::from_slice(x.as_slice())),
                )
                .await
                .unwrap();
            }

            let last_block_number = self.provider.last_block_number().unwrap();
            get_block_buffer_manager()
                .set_state(start_commit_num - 1, last_block_number)
                .await
                .unwrap();
        }
    }
}
