use alloy_consensus::{TxEip1559, TxEip2930, TxLegacy};
use alloy_primitives::{Address, B256};
use alloy_rpc_types_engine::ForkchoiceState;
use api_types::account::{ExternalAccountAddress, ExternalChainId};
use api_types::BlockId as ExternalBlockId;
use api_types::{ExternalBlock, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use jsonrpsee::core::Serialize;
use jsonrpsee::http_client::transport::HttpBackend;
use jsonrpsee::http_client::HttpClient;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_payload_builder::PayloadId;
use reth_pipe_exec_layer_ext::{OrderedBlock, PipeExecLayerApi};
use reth_primitives::{Bytes, Signature, TransactionSigned, TxKind, U256};
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use reth_rpc_layer::AuthClientService;
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
        parent_hash: B256,
    ) -> Result<PayloadId, String> {
        let pipe_api = self.pipe_api.lock().await;
        let payload_attr = Self::create_payload_attributes(parent_hash.into(), block.block_meta.ts);
        let fcu_state = ForkchoiceState {
            head_block_hash: parent_hash.into(),
            safe_block_hash: parent_hash.into(),
            finalized_block_hash: parent_hash.into(),
        };
        let engine_api = self.auth.http_client();
        let mut senders = vec![];
        let mut transactions = vec![];
        for (sender, txn) in
            block.txns.iter().map(|txn| Self::txn_to_signed(&txn.bytes, self.chain_id))
        {
            senders.push(sender);
            transactions.push(txn);
        }
        pipe_api.push_ordered_block(OrderedBlock {
            block_id: Self::block_id_to_b256(block.block_meta.block_id),
            parent_hash: parent_hash.into(),
            transactions,
            senders,
        });
        let res = <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v3(
            &engine_api,
            fcu_state, 
            Some(payload_attr)
        ).await;
        match res {
            Ok(res) => {
                let payload_id = res.payload_id;
                match payload_id {
                    Some(payload_id) => Ok(payload_id),
                    None => Err("Payload id not found".to_string()),
                }
            }
            Err(_) => Err("Failed to push ordered block".to_string()),
        }
    }

    pub async fn process_payload_id(
        &self,
        block_id: B256,
        payload_id: PayloadId,
    ) -> Result<B256, ()> {
        let mut pipe_api = self.pipe_api.lock().await;
        let block_hash = pipe_api.pull_executed_block_hash(payload_id, block_id).await.unwrap();
        Ok(block_hash)
    }

    pub async fn commit_block(
        &self,
        parent_beacon_block_root: B256,
        payload_id: PayloadId,
        block_hash: B256,
    ) -> Result<(), String> {
        let mut pipe_api = self.pipe_api.lock().await;
        pipe_api.ready_to_get_payload(payload_id).await.unwrap();
        let engine_api = self.auth.http_client();
        let payload = <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<
            EthEngineTypes,
        >>::get_payload_v3(&engine_api, payload_id)
        .await
        .unwrap();
        assert_eq!(payload.execution_payload.payload_inner.payload_inner.block_hash, block_hash);
        pipe_api.ready_to_new_payload(block_hash.into()).await.unwrap();
        let res = <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::new_payload_v3(
            &engine_api,
            payload.execution_payload,
            vec![],
            parent_beacon_block_root,
        ).await;
        info!("payload status {:?}", res);
        let res = <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v3(
            &engine_api,
            ForkchoiceState {
                head_block_hash: block_hash.into(),
                safe_block_hash: block_hash.into(),
                finalized_block_hash: block_hash.into(),
            },
            None
        ).await;
        match res {
            Ok(_) => Ok(()),
            Err(_) => Err("Failed to commit block".to_string()),
        }
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
                        let vtxn = VerifiedTxnWithAccountSeqNum {
                            txn: VerifiedTxn {
                                bytes,
                                sender: covert_account(txn.from.unwrap()),
                                sequence_number: txn.nonce.as_u64(),
                                chain_id: ExternalChainId::new(0),
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
