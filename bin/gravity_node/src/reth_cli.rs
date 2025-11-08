use crate::{metrics::fetch_reth_txn_metrics, ConsensusArgs};
use alloy_consensus::{transaction::SignerRecoverable, Transaction};
use alloy_eips::{eip4895::Withdrawals, Decodable2718, Encodable2718};
use alloy_primitives::{Address, TxHash, B256, U256};
use block_buffer_manager::get_block_buffer_manager;
use dashmap::DashMap;
use core::panic;
use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId},
    compute_res::TxnStatus,
    config_storage::{ConfigStorage, OnChainConfig, OnChainConfigResType},
    u256_define::{BlockId as ExternalBlockId, TxnHash},
    ExternalBlock, VerifiedTxn, VerifiedTxnWithAccountSeqNum, GLOBAL_CRYPTO_TXN_HASHER,
};

use alloy_rpc_types_eth::TransactionRequest;
use greth::{
    gravity_storage::block_view_storage::BlockViewStorage,
    reth::rpc::builder::auth::AuthServerHandle,
    reth_db::DatabaseEnv,
    reth_node_api::NodeTypesWithDBAdapter,
    reth_node_ethereum::EthereumNode,
    reth_pipe_exec_layer_ext_v2::{ExecutionResult, OrderedBlock, PipeExecLayerApi},
    reth_primitives::TransactionSigned,
    reth_provider::{
        providers::BlockchainProvider, AccountReader, BlockNumReader, ChainSpecProvider,
    },
    reth_rpc_api::eth::{helpers::EthCall, RpcTypes},
    reth_transaction_pool::{EthPooledTransaction, TransactionPool, ValidPoolTransaction},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Instant, SystemTime},
};

use tokio::sync::Mutex;
use tracing::*;

pub(crate) type RethBlockChainProvider =
    BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>;

pub(crate) type RethTransactionPool = greth::reth_transaction_pool::Pool<
    greth::reth_transaction_pool::TransactionValidationTaskExecutor<
        greth::reth_transaction_pool::EthTransactionValidator<
            RethBlockChainProvider,
            greth::reth_transaction_pool::EthPooledTransaction,
        >,
    >,
    greth::reth_transaction_pool::CoinbaseTipOrdering<
        greth::reth_transaction_pool::EthPooledTransaction,
    >,
    greth::reth_transaction_pool::blobstore::DiskFileBlobStore,
>;

pub(crate) trait RethEthCall:
    EthCall<NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>>
{
}

impl<T> RethEthCall for T where
    T: EthCall<NetworkTypes: RpcTypes<TransactionRequest = TransactionRequest>>
{
}

pub(crate) type RethPipeExecLayerApi<EthApi> =
    PipeExecLayerApi<BlockViewStorage<RethBlockChainProvider>, EthApi>;

pub struct RethCli<EthApi: RethEthCall> {
    auth: AuthServerHandle,
    pipe_api: RethPipeExecLayerApi<EthApi>,
    chain_id: u64,
    provider: RethBlockChainProvider,
    txn_listener: Mutex<tokio::sync::mpsc::Receiver<TxHash>>,
    pool: RethTransactionPool,
    txn_cache: Arc<
        DashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>,
    >,
    txn_batch_size: usize,
}

pub fn convert_account(acc: Address) -> ExternalAccountAddress {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(acc.as_slice());
    ExternalAccountAddress::new(bytes)
}

fn calculate_txn_hash(bytes: &Vec<u8>) -> [u8; 32] {
    alloy_primitives::utils::keccak256(bytes.clone()).as_slice().try_into().unwrap()
}

impl<EthApi: RethEthCall> RethCli<EthApi> {
    pub async fn new(args: ConsensusArgs<EthApi>, txn_cache: Arc<DashMap<(ExternalAccountAddress, u64), Arc<ValidPoolTransaction<EthPooledTransaction>>>>) -> Self {
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
            txn_cache,
            txn_batch_size: 2000,
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
            for (idx, txn) in block.txns.iter().enumerate() {
                let key = (txn.sender.clone(), txn.sequence_number);
                if let Some((_, cached_txn)) = self.txn_cache.remove(&key) {
                    senders[idx] = Some(cached_txn.sender());
                    transactions[idx] = Some(cached_txn.transaction.transaction().inner().clone());
                }
            }
        }

        block
            .txns
            .par_iter_mut()
            .enumerate()
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

        let (randao, randomness) = match block.block_meta.randomness {
            Some(randao) => (B256::from_slice(randao.0.as_ref()), U256::from_be_slice(randao.0.as_ref())),
            None => (B256::ZERO, U256::from(0)),
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
            epoch: block.block_meta.epoch,
            proposer: block
                .block_meta
                .proposer
                .map(|x| x.bytes().into()),
            extra_data: block.extra_data,
            randomness,
            enable_randomness: block.enable_randomness,
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

    pub async fn wait_for_block_persistence(&self, block_number: u64) -> Result<(), String> {
        debug!("wait for block persistence {:?}", block_number);
        let pipe_api = &self.pipe_api;
        pipe_api.wait_for_block_persistence(block_number).await;
        debug!("wait for block persistence done");
        Ok(())
    }

    pub async fn start_execution(&self) -> Result<(), String> {
        let mut start_ordered_block = self.provider.last_block_number().unwrap() + 1;
        loop {
            // max executing block number
            let exec_blocks =
                get_block_buffer_manager().get_ordered_blocks(start_ordered_block, None).await;
            if let Err(e) = exec_blocks {
                let from = start_ordered_block;
                if e.to_string().contains("Buffer is in epoch change") {
                    get_block_buffer_manager().consume_epoch_change();
                    start_ordered_block = self.provider.last_block_number().unwrap() + 1;
                    warn!("Buffer is in epoch change, reset start_ordered_block from {} to {}", from, start_ordered_block);
                } else {
                    warn!("failed to get ordered blocks: {}", e);
                }
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
                    .map(|tx_info| TxnStatus {
                        txn_hash: *tx_info.tx_hash,
                        sender: convert_account(tx_info.sender).bytes(),
                        nonce: tx_info.nonce,
                        is_discarded: tx_info.is_discarded,
                    })
                    .collect(),
            ));
            let events = execution_result.gravity_events;
            get_block_buffer_manager()
                .set_compute_res(block_id, block_hash_data, block_number, txn_status, events)
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
            let mut persist_notifiers = Vec::new();
            for block_id_num_hash in block_ids {
                self.send_committed_block_info(
                    block_id_num_hash.block_id,
                    block_id_num_hash.hash.map(|x| B256::from_slice(x.as_slice())),
                )
                .await
                .unwrap();
                if let Some(persist_notifier) = block_id_num_hash.persist_notifier {
                    persist_notifiers.push((block_id_num_hash.num, persist_notifier));
                }
            }

            let last_block_number = self.provider.last_block_number().unwrap();
            get_block_buffer_manager()
                .set_state(start_commit_num - 1, last_block_number)
                .await
                .unwrap();
            for (block_number, persist_notifier) in persist_notifiers {
                info!("wait_for_block_persistence num {:?} send persist_notifier", block_number);
                self.wait_for_block_persistence(block_number).await.unwrap();
                let _ = persist_notifier.send(());
            }
        }
    }
}
pub struct RethCliConfigStorage<EthApi: RethEthCall> {
    reth_cli: Arc<RethCli<EthApi>>,
}

impl<EthApi: RethEthCall> RethCliConfigStorage<EthApi> {
    pub fn new(reth_cli: Arc<RethCli<EthApi>>) -> Self {
        Self { reth_cli }
    }
}

impl<EthApi: RethEthCall> ConfigStorage for RethCliConfigStorage<EthApi> {
    fn fetch_config_bytes(
        &self,
        config_name: OnChainConfig,
        block_number: u64,
    ) -> Option<OnChainConfigResType> {
        self.reth_cli.pipe_api.fetch_config_bytes(config_name, block_number)
    }
}
