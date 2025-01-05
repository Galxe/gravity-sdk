use alloy_consensus::{TxEip1559, TxEip2930, TxLegacy};
use alloy_primitives::{Address, B256};
use api_types::account::{ExternalAccountAddress, ExternalChainId};
use api_types::u256_define::{BlockId as ExternalBlockId, TxnHash};
use api_types::{simple_hash, ExternalBlock, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use jsonrpsee::core::Serialize;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_ethereum_engine_primitives::EthPayloadAttributes;
use reth_pipe_exec_layer_ext_v2::{ExecutedBlockMeta, OrderedBlock, PipeExecLayerApi};
use reth_primitives::{Bytes, Signature, TransactionSigned, TxKind, Withdrawals, U256};
use reth_rpc_api::EngineEthApiClient;
use reth_rpc_types::{AccessList, AccessListItem};
use std::io::Read;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use tracing::info;
use tracing::log::error;
use web3::transports::Ipc;
use web3::types::{BlockId, Transaction, TransactionId, H160};
use web3::Web3;

pub struct RethCli {
    ipc: Web3<Ipc>,
    auth: AuthServerHandle,
    pipe_api: Mutex<PipeExecLayerApi>,
    chain_id: u64,
}

pub fn covert_account(acc: H160) -> ExternalAccountAddress {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(acc.as_bytes());
    ExternalAccountAddress::new(bytes)
}

impl RethCli {
    fn create_payload_attributes(parent_beacon_block_root: B256, ts: u64) -> EthPayloadAttributes {
        EthPayloadAttributes {
            timestamp: ts,
            prev_randao: B256::ZERO,
            suggested_fee_recipient: Address::ZERO,
            withdrawals: Some(Vec::new()),
            parent_beacon_block_root: Some(parent_beacon_block_root),
        }
    }

    pub async fn get_latest_block_hash(&self) -> Result<B256, String> {
        let block = self.ipc.eth().block(BlockId::Number(web3::types::BlockNumber::Latest)).await;

        match block {
            Ok(Some(block)) => {
                let hash = B256::from_slice(block.hash.unwrap().as_bytes());
                Ok(hash)
            }
            _ => Err("Failed to get block".to_string()),
        }
    }

    pub async fn new(ipc_url: &str, auth: AuthServerHandle, pipe_api: PipeExecLayerApi) -> Self {
        let transport = web3::transports::Ipc::new(ipc_url).await.unwrap();
        let ipc = web3::Web3::new(transport);
        let chain_id = ipc.eth().chain_id().await.unwrap();
        RethCli { ipc, auth, pipe_api: Mutex::new(pipe_api), chain_id: chain_id.as_u64() }
    }

    fn block_id_to_b256(block_id: ExternalBlockId) -> B256 {
        B256::new(block_id.0)
    }

    fn construct_sig(txn: &Transaction) -> Signature {
        let r = txn.r.unwrap();
        let s = txn.s.unwrap();
        let v = txn.v.unwrap();
        let odd_y_parity = match v.as_u64() {
            27 => false,
            28 => true,
            v => v % 2 == 1, // EIP-155 情况
        };
        let mut bytes = [0u8; 32];
        r.to_big_endian(&mut bytes);
        let r = U256::from_be_bytes(bytes);
        let mut bytes = [0u8; 32];
        s.to_big_endian(&mut bytes);
        let s = U256::from_be_bytes(bytes);
        Signature { r, s, odd_y_parity }
    }

    fn to_tx_kind(address: Option<H160>) -> TxKind {
        match address {
            Some(address) => TxKind::Call(Address::new(address.0)),
            None => TxKind::Create,
        }
    }

    fn convert_accest_list(access_list: Option<Vec<web3::types::AccessListItem>>) -> AccessList {
        AccessList(
            access_list
                .unwrap_or_default()
                .iter()
                .map(|x| AccessListItem {
                    address: Address::from_slice(x.address.as_bytes()),
                    storage_keys: x
                        .storage_keys
                        .iter()
                        .map(|x| B256::from_slice(x.as_bytes()))
                        .collect(),
                })
                .collect(),
        )
    }

    fn convert_to_reth_transaction(
        tx: web3::types::Transaction,
        chain_id: u64,
    ) -> reth::primitives::Transaction {
        match tx.transaction_type.map(|t| t.as_u64()) {
            Some(0) => reth::primitives::Transaction::Legacy(TxLegacy {
                chain_id: Some(chain_id),
                nonce: tx.nonce.as_u64(),
                gas_price: tx.gas_price.unwrap().as_u128(),
                gas_limit: tx.gas.as_u128(),
                to: Self::to_tx_kind(tx.to),
                value: U256::from(tx.value.as_u128()),
                input: Bytes::copy_from_slice(tx.input.0.as_slice()),
            }),
            Some(1) => reth::primitives::Transaction::Eip2930(TxEip2930 {
                chain_id,
                nonce: tx.nonce.as_u64(),
                gas_price: tx.gas_price.unwrap().as_u128(),
                gas_limit: tx.gas.as_u128(),
                to: Self::to_tx_kind(tx.to),
                value: U256::from(tx.value.as_u128()),
                access_list: Self::convert_accest_list(tx.access_list),
                input: Bytes::copy_from_slice(tx.input.0.as_slice()),
            }),
            Some(2) => reth::primitives::Transaction::Eip1559(TxEip1559 {
                chain_id,
                nonce: tx.nonce.as_u64(),
                gas_limit: tx.gas.as_u128(),
                to: Self::to_tx_kind(tx.to),
                value: U256::from(tx.value.as_u128()),
                access_list: Self::convert_accest_list(tx.access_list),
                input: Bytes::copy_from_slice(tx.input.0.as_slice()),
                max_fee_per_gas: tx.max_fee_per_gas.unwrap().as_u128(),
                max_priority_fee_per_gas: tx.max_priority_fee_per_gas.unwrap().as_u128(),
            }),
            _ => panic!("Unknown transaction type {:?}", tx.transaction_type),
        }
    }

    fn txn_to_signed(bytes: &[u8], chain_id: u64) -> (Address, TransactionSigned) {
        let txn = serde_json::from_slice::<Transaction>(bytes).unwrap();
        let address = txn.from.unwrap();
        let address = Address::new(address.0);
        let hash = txn.hash;
        let hash = B256::from_slice(hash.as_bytes());
        let signature = Self::construct_sig(&txn);
        let transaction = Self::convert_to_reth_transaction(txn, chain_id);
        info!("txn to signed {:?}", transaction);
        info!("address {:?}", address);
        (address, TransactionSigned { hash, signature, transaction })
    }

    pub async fn push_ordered_block(
        &self,
        block: ExternalBlock,
        parent_id: B256,
    ) -> Result<(), String> {
        info!("push ordered block {:?} with parent id {}", block, parent_id);
        let pipe_api = self.pipe_api.lock().await;
        let mut senders = vec![];
        let mut transactions = vec![];
        for (sender, txn) in
            block.txns.iter().map(|txn| Self::txn_to_signed(&txn.bytes, self.chain_id))
        {
            senders.push(sender);
            transactions.push(txn);
        }
        // TODO: make zero make sense
        pipe_api.push_ordered_block(OrderedBlock {
            parent_id,
            id: B256::from_slice(block.block_meta.block_id.as_bytes()),
            number: block.block_meta.block_number,
            timestamp: block.block_meta.usecs / 1000000,
            // TODO(gravity_jan): add reth coinbase
            coinbase: Address::ZERO,
            // TODO(gravity_jan): add aptos random
            prev_randao: B256::ZERO,
            withdrawals: Withdrawals::new(Vec::new()),
            transactions,
            senders,
        });
        Ok(())
    }

    pub async fn recv_compute_res(&self, block_id: B256) -> Result<B256, ()> {
        let pipe_api = self.pipe_api.lock().await;
        let block_hash = pipe_api.pull_executed_block_hash(block_id).await.unwrap();
        Ok(block_hash)
    }

    pub async fn commit_block(
        &self,
        block_id: api_types::u256_define::BlockId,
        block_hash: B256,
    ) -> Result<(), String> {
        let block_id = B256::from_slice(block_id.0.as_ref());
        let pipe_api = self.pipe_api.lock().await;
        pipe_api.commit_executed_block_hash(ExecutedBlockMeta { block_id, block_hash });
        Ok(())
    }

    pub async fn process_pending_transactions(
        &self,
        buffer: Arc<Mutex<Vec<VerifiedTxnWithAccountSeqNum>>>,
    ) -> Result<(), String> {
        let mut eth_sub =
            self.ipc.eth_subscribe().subscribe_new_pending_transactions().await.unwrap();
        info!("start process pending transactions");
        while let Some(Ok(txn_hash)) = eth_sub.next().await {
            info!("get txn hash {:?}", txn_hash);
            let txn = self.ipc.eth().transaction(TransactionId::Hash(txn_hash)).await;
            info!("get txn {:?}", txn);
            if let Ok(Some(txn)) = txn {
                let account = match txn.from {
                    Some(account) => account,
                    None => {
                        error!("Transaction has no from account");
                        continue;
                    }
                };
                let accout_nonce = self.ipc.eth().transaction_count(account, None).await;

                match accout_nonce {
                    Ok(accout_nonce) => {
                        let mut buffer = buffer.lock().await;
                        let bytes = serde_json::to_vec(&txn).unwrap();
                        let committed_hash = TxnHash::new(simple_hash::hash_to_fixed_array(&bytes));
                        let vtxn = VerifiedTxnWithAccountSeqNum {
                            txn: VerifiedTxn {
                                bytes,
                                sender: covert_account(txn.from.unwrap()),
                                sequence_number: txn.nonce.as_u64(),
                                chain_id: ExternalChainId::new(0),
                                committed_hash: committed_hash.into(),
                            },
                            account_seq_num: accout_nonce.as_u64(),
                        };
                        println!(
                            "push txn nonce: {} acc_nonce: {}",
                            txn.nonce, vtxn.account_seq_num
                        );
                        buffer.push(vtxn);
                    }
                    Err(e) => {
                        error!("Failed to get nonce for account {:?} with {:?}", account, e);
                    }
                }
            } else {
                error!("Failed to get transaction {:?} {:?}", txn_hash, txn);
            }
        }
        info!("end process pending transactions");
        Ok(())
    }
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: Vec<String>,
    id: u64,
}

#[derive(serde::Deserialize)]
struct JsonRpcResponse<T> {
    result: T,
}
