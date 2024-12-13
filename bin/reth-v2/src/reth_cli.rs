use alloy_consensus::{TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::{Address, B256};
use alloy_rlp::Encodable;
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV3, ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId,
    PayloadStatusEnum,
};
use alloy_signer::k256::sha2;
use anyhow::Context;
use api::ExecutionApi;
use api_types::{BlockBatch, BlockHashState, ExecutionBlocks, GTxn};
use jsonrpsee::core::{async_trait, Serialize};
use jsonrpsee::http_client::transport::HttpBackend;
use jsonrpsee::http_client::HttpClient;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_db::mdbx::tx;
use reth_rpc_layer::AuthClientService;
use revm_primitives::ruint::aliases::U256;
use tokio_stream::StreamExt;
use web3::transports::Ipc;
use web3::types::{Transaction, TransactionId};
use web3::Web3;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tracing::info;
use tracing::log::error;

pub struct RethCli {
    ipc: Web3<Ipc>,
    auth: AuthServerHandle
}

impl RethCli {
    pub async fn new(ipc_url: &str, auth: AuthServerHandle) -> Self {
        let transport = web3::transports::Ipc::new(ipc_url).await.unwrap();
        let ipc = web3::Web3::new(transport);
        RethCli { ipc, auth }
    }

    pub async fn process_pending_transactions<T>(
        &self,
        process: T,
    ) -> Result<(), String> 
    where T :  Fn(Transaction, u64) -> ()
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
                    Ok(nonce) => {
                        process(txn, nonce.as_u64());
                        info!("Processed transaction {:?}", txn_hash);
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
