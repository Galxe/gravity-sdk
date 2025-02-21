use crate::metric::{RethCliMetric, METRICS};
use crate::ConsensusArgs;
use api_types::account::{ExternalAccountAddress, ExternalChainId};
use api_types::u256_define::{BlockId as ExternalBlockId, TxnHash};
use api_types::{ExecutionBlocks, ExternalBlock, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use core::panic;
use greth::reth::rpc::builder::auth::AuthServerHandle;
use greth::reth_db::DatabaseEnv;
use greth::reth_ethereum_engine_primitives::EthPayloadAttributes;
use greth::reth_node_api::NodeTypesWithDBAdapter;
use greth::reth_node_ethereum::EthereumNode;
use greth::reth_pipe_exec_layer_ext_v2::{ExecutedBlockMeta, OrderedBlock, PipeExecLayerApi};
use greth::reth_primitives::alloy_primitives::private::alloy_rlp::Decodable;
use greth::reth_primitives::alloy_primitives::private::alloy_rlp::Encodable;
use greth::reth_primitives::{
    Address, TransactionSigned, TransactionSignedEcRecovered, Withdrawals, B256,
};
use greth::reth_provider::providers::BlockchainProvider2;
use greth::reth_provider::{
    AccountReader, BlockNumReader, BlockReaderIdExt, ChainSpecProvider, DatabaseProviderFactory,
};
use greth::reth_rpc_api::EngineEthApiClient;
use greth::reth_rpc_types::BlockNumberOrTag;
use greth::reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::io::Read;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tracing::{debug, info};
use tracing::log::error;

pub struct RethCli {
    auth: AuthServerHandle,
    pipe_api: PipeExecLayerApi,
    chain_id: u64,
    provider: BlockchainProvider2<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
    txn_listener: Mutex<tokio::sync::mpsc::Receiver<greth::reth::primitives::TxHash>>,
    pool: greth::reth_transaction_pool::Pool<
        greth::reth_transaction_pool::TransactionValidationTaskExecutor<
            greth::reth_transaction_pool::EthTransactionValidator<
                BlockchainProvider2<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
                greth::reth_transaction_pool::EthPooledTransaction,
            >,
        >,
        greth::reth_transaction_pool::CoinbaseTipOrdering<
            greth::reth_transaction_pool::EthPooledTransaction,
        >,
        greth::reth_transaction_pool::blobstore::DiskFileBlobStore,
    >,
}

pub fn covert_account(acc: greth::reth_primitives::Address) -> ExternalAccountAddress {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(acc.as_slice());
    ExternalAccountAddress::new(bytes)
}

impl RethCli {
    pub async fn new(args: ConsensusArgs) -> Self {
        let chian_info = args.provider.chain_spec().chain;
        let chain_id = match chian_info.into_kind() {
            greth::reth_chainspec::ChainKind::Named(n) => n as u64,
            greth::reth_chainspec::ChainKind::Id(id) => id,
        };
        RethCli {
            auth: args.engine_api,
            pipe_api: args.pipeline_api,
            chain_id,
            provider: args.provider,
            txn_listener: Mutex::new(args.tx_listener),
            pool: args.pool,
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn create_payload_attributes(
        parent_beacon_block_root: greth::reth::primitives::B256,
        ts: u64,
    ) -> EthPayloadAttributes {
        EthPayloadAttributes {
            timestamp: ts,
            prev_randao: greth::reth::primitives::B256::ZERO,
            suggested_fee_recipient: greth::reth_primitives::Address::ZERO,
            withdrawals: Some(Vec::new()),
            parent_beacon_block_root: Some(parent_beacon_block_root),
        }
    }

    fn block_id_to_b256(block_id: ExternalBlockId) -> B256 {
        B256::new(block_id.0)
    }

    fn txn_to_signed(bytes: &mut [u8], chain_id: u64) -> (Address, TransactionSigned) {
        let txn = TransactionSignedEcRecovered::decode(&mut bytes.as_ref()).unwrap();
        (txn.signer(), txn.into_signed())
    }

    pub async fn push_ordered_block(
        &self,
        mut block: ExternalBlock,
        parent_id: B256,
    ) -> Result<(), String> {
        debug!("push ordered block {:?} with parent id {}", block, parent_id);
        let pipe_api = &self.pipe_api;
        let mut senders = vec![];
        let mut transactions = vec![];
        for (sender, txn) in
            block.txns.iter_mut().map(|txn| Self::txn_to_signed(&mut txn.bytes, self.chain_id))
        {
            senders.push(sender);
            transactions.push(txn);
        }

        let randao = match block.block_meta.randomness {
            Some(randao) => B256::from_slice(randao.0.as_ref()),
            None => B256::ZERO,
        };
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

    pub async fn recv_compute_res(&self, block_id: B256) -> Result<B256, ()> {
        debug!("recv compute res {:?}", block_id);
        let pipe_api = &self.pipe_api;
        let block_hash = pipe_api.pull_executed_block_hash(block_id).await.unwrap();
        debug!("recv compute res done");
        Ok(block_hash)
    }

    pub async fn send_committed_block_info(
        &self,
        block_id: api_types::u256_define::BlockId,
        block_hash: B256,
    ) -> Result<(), String> {
        debug!("commit block {:?} with hash {:?}", block_id, block_hash);
        let block_id = B256::from_slice(block_id.0.as_ref());
        let pipe_api = &self.pipe_api;
        pipe_api.commit_executed_block_hash(ExecutedBlockMeta { block_id, block_hash });
        debug!("commit block done");
        Ok(())
    }

    pub async fn process_pending_transactions(
        &self,
        buffer: Arc<Mutex<Vec<VerifiedTxnWithAccountSeqNum>>>,
    ) -> Result<(), String> {
        debug!("start process pending transactions");
        const BATCH_SIZE: usize = 100; // 批处理大小
        const PARALLEL_TASKS: usize = 4; // 并行任务数
        
        let start_time = std::time::Instant::now();
        let mut last_time = std::time::Instant::now();
        let mut count = 0;
        let mut total = 0;

        // 创建一个channel用于并行处理
        let (tx, mut rx) = tokio::sync::mpsc::channel::<VerifiedTxnWithAccountSeqNum>(BATCH_SIZE * 2);
        
        // 启动处理任务
        let buffer_clone = buffer.clone();
        let process_handle = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(BATCH_SIZE);
            while let Some(vtxn) = rx.recv().await {
                batch.push(vtxn);
                
                if batch.len() >= BATCH_SIZE {
                    let mut buffer_guard = buffer_clone.lock().await;
                    buffer_guard.extend(batch.drain(..));
                    drop(buffer_guard);
                }
            }
            
            // 处理剩余的交易
            if !batch.is_empty() {
                let mut buffer_guard = buffer_clone.lock().await;
                buffer_guard.extend(batch);
            }
        });

        let mut tasks = Vec::with_capacity(PARALLEL_TASKS);
        loop {
            // 收集一批交易哈希
            let mut txn_hashes = Vec::with_capacity(BATCH_SIZE);
            {
                let mut mut_txn_listener = self.txn_listener.lock().await;
                while txn_hashes.len() < BATCH_SIZE {
                    match mut_txn_listener.try_recv() {
                        Ok(Some(hash)) => txn_hashes.push(hash),
                        Ok(None) | Err(_) => break,
                    }
                }
            }
            
            if txn_hashes.is_empty() {
                break;
            }

            // 并行处理交易
            for chunk in txn_hashes.chunks(txn_hashes.len() / PARALLEL_TASKS.max(1)) {
                let chunk = chunk.to_vec();
                let tx = tx.clone();
                let pool = self.pool.clone();
                let provider = self.provider.clone();
                
                let task = tokio::spawn(async move {
                    for txn_hash in chunk {
                        METRICS.get_or_init(|| RethCliMetric::default()).reth_notify_count.increment(1);
                        
                        let txn = pool.get(&txn_hash).unwrap();
                        let sender = txn.sender();
                        let nonce = txn.nonce();
                        let txn = txn.transaction.transaction();
                        
                        let account_nonce = provider
                            .basic_account(sender)
                            .unwrap()
                            .map(|x| x.nonce)
                            .unwrap_or(txn.nonce());

                        let mut bytes = Vec::with_capacity(1024 * 4);
                        txn.encode(&mut bytes);

                        let vtxn = VerifiedTxnWithAccountSeqNum {
                            txn: VerifiedTxn {
                                bytes,
                                sender: covert_account(sender),
                                sequence_number: nonce,
                                chain_id: ExternalChainId::new(0),
                                committed_hash: TxnHash::from_bytes(txn.hash().as_slice()).into(),
                            },
                            account_seq_num: account_nonce,
                        };
                        
                        if let Err(_) = tx.send(vtxn).await {
                            break;
                        }
                    }
                });
                tasks.push(task);
            }

            // 等待所有任务完成
            for task in tasks.drain(..) {
                task.await.map_err(|e| e.to_string())?;
            }

            // 更新计数器和日志
            count += txn_hashes.len();
            if last_time.elapsed().as_secs() >= 1 {
                let elapsed = last_time.elapsed().as_secs();
                debug!(
                    "processed {} transactions in {}s with speed {}",
                    count,
                    elapsed,
                    count as f64 / elapsed as f64
                );
                total += count;
                count = 0;
                last_time = std::time::Instant::now();
            }
        }

        // 等待处理任务完成
        drop(tx);
        process_handle.await.map_err(|e| e.to_string())?;

        debug!(
            "end process pending transactions, total processed: {}, average speed: {}/s",
            total,
            total as f64 / start_time.elapsed().as_secs_f64()
        );
        Ok(())
    }

    pub async fn latest_block_number(&self) -> u64 {
        match self.provider.header_by_number_or_tag(BlockNumberOrTag::Latest).unwrap() {
            Some(header) => header.number, // The genesis block has a number of zero;
            None => 0,
        }
    }

    pub async fn finalized_block_number(&self) -> u64 {
        match self.provider.database_provider_ro().unwrap().last_block_number() {
            Ok(block_number) => {
                return block_number;
            }
            Err(e) => {
                error!("finalized_block_number error {}", e);
                return 0;
            }
        }
    }

    async fn recover_execution_blocks(&self, blocks: ExecutionBlocks) {}

    pub fn get_blocks_by_range(
        &self,
        start_block_number: u64,
        end_block_number: u64,
    ) -> ExecutionBlocks {
        let result = ExecutionBlocks {
            latest_block_hash: todo!(),
            latest_block_number: todo!(),
            blocks: vec![],
            latest_ts: todo!(),
        };
        for block_number in start_block_number..end_block_number {
            match self.provider.block_by_number_or_tag(BlockNumberOrTag::Number(block_number)) {
                Ok(block) => {
                    assert!(block.is_some());
                    let block = block.unwrap();
                    if block_number == end_block_number - 1 {
                        result.latest_block_hash = *block.hash_slow();
                        result.latest_block_number = block_number;
                        result.latest_ts = block.timestamp;
                    }
                    result.blocks.push(bincode::serialize(&block).unwrap());
                }
                Err(e) => panic!("get_blocks_by_range error {}", e),
            }
        }
        result
    }
}
