use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use alloy_primitives::private::proptest::strategy::W;
use tokio_stream::StreamExt;
use tracing::{info, error};
use web3::transports::{Http, Ipc, WebSocket};
use web3::types::TransactionId;
use web3::Web3;

pub struct RethCli {
    ipc: Web3<Ipc>,
}

impl RethCli {
    pub async fn new(ipc_url: &str) -> Self {
        let transport = web3::transports::Ipc::new(ipc_url).await.unwrap();
        let ipc = web3::Web3::new(transport);
        RethCli {
            ipc
        }
    }

    pub async fn process_pending_transactions(&self, process: impl Fn()) -> Result<(), Box<dyn Error>> {
        let mut eth_sub = self.ipc.eth_subscribe().subscribe_new_pending_transactions().await.unwrap();
        while let Some(Ok(txn_hash)) = eth_sub.next().await {
            let txn = self.ipc.eth().transaction(TransactionId::Hash(txn_hash)).await;
            info!("txn is {:?}", txn);
            process()
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