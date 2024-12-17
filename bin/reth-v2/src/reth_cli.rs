use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Decodable2718;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::{Address, B256};
use alloy_rlp::Encodable;
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV3, ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId,
    PayloadStatusEnum,
};
use api_types::account::{ExternalAccountAddress, ExternalChainId};
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use alloy_signer::k256::sha2;
use anyhow::Context;
use api::ExecutionApi;
use api_types::{BlockBatch, BlockHashState, ExecutionBlocks, ExternalBlock, GTxn, VerifiedTxn, VerifiedTxnWithAccountSeqNum};
use jsonrpsee::core::{async_trait, Serialize};
use jsonrpsee::http_client::transport::HttpBackend;
use jsonrpsee::http_client::HttpClient;
use reth::revm::db::components::block_hash;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_db::mdbx::tx;
use reth_payload_builder::EthPayloadBuilderAttributes;
use reth_primitives::U256;
use reth_rpc_api::{EngineApiClient, EngineEthApiClient};
use reth_rpc_layer::AuthClientService;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
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

    pub async fn get_latest_block_hash(&self) -> Result<[u8; 32], String> {
        let block = self.ipc.eth().block(BlockId::Number(web3::types::BlockNumber::Latest)).await;
                
        match block {
            Ok(Some(block)) => {
                let mut bytes = [0u8; 32];
                bytes[..].copy_from_slice(block.hash.unwrap().as_bytes());
                Ok(bytes)
            }
            _ => Err("Failed to get block".to_string()),
        }
    }

    pub async fn new(ipc_url: &str, auth: AuthServerHandle) -> Self {
        let transport = web3::transports::Ipc::new(ipc_url).await.unwrap();
        let ipc = web3::Web3::new(transport);
        RethCli { ipc, auth }
    }

    pub async fn push_ordered_block(
        &self,
        ts: u64,
        parent_hash: U256,
    ) -> Result<PayloadId, String> {
        let payload_attr = Self::create_payload_attributes(parent_hash.into(), ts);
        let fcu_state = ForkchoiceState {
            head_block_hash: parent_hash.into(),
            safe_block_hash: parent_hash.into(),
            finalized_block_hash: parent_hash.into(),
        };
        let engine_api = self.auth.http_client();
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

    pub async fn process_payload_id(&self, id: PayloadId) -> Result<B256, ()> {
        let engine_api = self.auth.http_client();
        let res = <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::get_payload_v3(
            &engine_api,
            id
        ).await;
        match res {
            Ok(res) => {
                let block_hash = res.execution_payload.payload_inner.payload_inner.block_hash;
                let parent_hash = res.execution_payload.payload_inner.payload_inner.parent_hash;
                let res = <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::new_payload_v3(
                    &engine_api,
                    res.execution_payload,
                    vec![],
                    parent_hash.into(),
                ).await;
                if res.is_err() {
                    error!("Failed to process payload");
                    return Err(());
                }
                Ok(block_hash)
            },
            Err(_) => {
                error!("Failed to get payload");
                Err(())
            }
        }
    }
    
    pub async fn commit_block(&self, block_hash: U256) -> Result<(), String> {
        let engine_api = self.auth.http_client();
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

    pub async fn process_pending_transactions(&self, buffer: Arc<Mutex<Vec<VerifiedTxnWithAccountSeqNum>>>) -> Result<(), String>
    {
        let mut eth_sub =
            self.ipc.eth_subscribe().subscribe_new_pending_transactions().await.unwrap();
        while let Some(Ok(txn_hash)) = eth_sub.next().await {
            let txn = self.ipc.eth().transaction(TransactionId::Hash(txn_hash)).await;

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
                        let txn = VerifiedTxnWithAccountSeqNum {
                            txn: VerifiedTxn {
                                bytes,
                                sender: covert_account(txn.from.unwrap()),
                                sequence_number: txn.nonce.as_u64(),
                                chain_id: ExternalChainId::new(0),
                            },
                            account_seq_num: accout_nonce.as_u64(),
                        };
                        buffer.push(txn);
                    }
                    Err(e) => {
                        error!("Failed to get nonce for account {:?} with {:?}", account, e);
                    }
                }
            }
            error!("Failed to get transaction {:?}", txn_hash);
        }
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
