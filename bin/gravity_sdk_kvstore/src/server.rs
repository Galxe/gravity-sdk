use api_types::{ExecTxn, ExecutionChannel};
use hex::decode;
use log::info;
use poem::{
    error::ResponseError,
    handler,
    http::StatusCode,
    listener,
    web::{Data, Json},
    EndpointExt, IntoResponse, Response, Route, Server,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    verify_signature, State, Storage, Transaction, TransactionReceipt,
    TransactionWithAccount,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionReceiptResponse {
    pub status: String,
    pub message: TransactionReceipt,
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("Failed to serialize transaction: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Failed to verify signature: {0}")]
    InvalidSignature(String),
    #[error("Transaction hash not found")]
    TransactionNotFound,
    #[error("Account not found")]
    AccountNotFound,
    #[error("Key not found")]
    KeyNotFound,
    #[error("Invalid transaction hash")]
    InvalidTransactionHash,
}

impl IntoResponse for TransactionError {
    fn into_response(self) -> Response {
        match self {
            TransactionError::SerializationError(err) => Response::builder()
                .status(StatusCode::from_u16(500).unwrap())
                .body(json!({"error": err.to_string()}).to_string()),
            TransactionError::InvalidSignature(err) => Response::builder()
                .status(StatusCode::from_u16(500).unwrap())
                .body(json!({"error": err.to_string()}).to_string()),
            TransactionError::TransactionNotFound => Response::builder()
                .status(StatusCode::from_u16(404).unwrap())
                .body(json!({"error": "Transaction not found"}).to_string()),
            TransactionError::AccountNotFound => Response::builder()
                .status(StatusCode::from_u16(404).unwrap())
                .body(json!({"error": "Account not found"}).to_string()),
            TransactionError::KeyNotFound => Response::builder()
                .status(StatusCode::from_u16(404).unwrap())
                .body(json!({"error": "Key not found"}).to_string()),
            TransactionError::InvalidTransactionHash => Response::builder()
                .status(StatusCode::from_u16(500).unwrap())
                .body(json!({"error": "Invalid transaction hash"}).to_string()),
        }
    }
}

impl ResponseError for TransactionError {
    fn status(&self) -> StatusCode {
        match self {
            TransactionError::SerializationError(_) => StatusCode::from_u16(500).unwrap(),
            TransactionError::InvalidSignature(_) => StatusCode::from_u16(500).unwrap(),
            TransactionError::TransactionNotFound => StatusCode::from_u16(404).unwrap(),
            TransactionError::AccountNotFound => StatusCode::from_u16(404).unwrap(),
            TransactionError::KeyNotFound => StatusCode::from_u16(404).unwrap(),
            TransactionError::InvalidTransactionHash => StatusCode::from_u16(500).unwrap(),
        }
    }
}

#[derive(Clone)]
struct Context {
    pub execution_channel: Arc<dyn ExecutionChannel>,
    pub state: Arc<RwLock<State>>,
    pub storage: Arc<dyn Storage>,
}

#[handler]
async fn add_txn(
    Json(transaction): Json<Transaction>,
    Data(context): Data<&Arc<Context>>,
) -> poem::Result<Json<Value>> {
    info!("add_txn: transaction: {:?}", transaction);
    let account_address =
        verify_signature(&transaction).map_err(|e| TransactionError::InvalidSignature(e))?;
    info!("add_txn: txn {:?}, address: {}", transaction, account_address);
    let txn_with_account = TransactionWithAccount { txn: transaction, address: account_address };
    match serde_json::to_string(&txn_with_account) {
        Ok(raw_txn) => {
            let txn_hash = context
                .execution_channel
                .send_user_txn(ExecTxn::RawTxn(raw_txn.into()))
                .await
                .map_err(|_| TransactionError::InvalidTransactionHash)?;
            Ok(Json(json!({ "txn_hash": txn_hash })))
        }
        Err(err) => Err(TransactionError::SerializationError(err).into()),
    }
}

fn parse_transaction_hash(hash: &str) -> Result<[u8; 32], TransactionError> {
    let bytes = decode(hash).map_err(|_| TransactionError::InvalidTransactionHash)?;
    if bytes.len() != 32 {
        return Err(TransactionError::InvalidTransactionHash);
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

#[handler]
async fn get_receipt(
    Json(transaction_hash): Json<String>,
    Data(context): Data<&Arc<Context>>,
) -> poem::Result<Json<Value>> {
    info!("get_receipt: transaction_hash: {}", transaction_hash);
    let transaction_hash = parse_transaction_hash(&transaction_hash)?;

    let receipt = context
        .storage
        .get_transaction_receipt(transaction_hash)
        .await
        .map_err(|_| TransactionError::InvalidTransactionHash)?
        .ok_or(TransactionError::TransactionNotFound)?;

    let value = serde_json::to_value(&receipt).map_err(TransactionError::SerializationError)?;
    Ok(Json(value))
}

#[handler]
async fn get_value(
    Json((account_address, key)): Json<(String, String)>,
    Data(context): Data<&Arc<Context>>,
) -> poem::Result<Json<Value>> {
    info!("get_value: account_address: {}, key: {}", account_address, key);
    // Retrieve the value from the account's key-value store
    match context.state.read().await.get_account(account_address.as_str()) {
        Some(account) => match account.kv_store.get(&key) {
            Some(value) => Ok(Json(json!(value))),
            None => Err(TransactionError::KeyNotFound.into()),
        },
        None => Err(TransactionError::AccountNotFound.into()),
    }
}

pub struct ServerApp {
    context: Arc<Context>,
}

impl ServerApp {
    pub fn new(
        execution_channel: Arc<dyn ExecutionChannel>,
        state: Arc<RwLock<State>>,
        storage: Arc<dyn Storage>,
    ) -> Self {
        Self { context: Arc::new(Context { execution_channel, state, storage }) }
    }

    pub async fn start(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let app = Route::new()
            .at("/add_txn", poem::post(add_txn.data(self.context.clone())))
            .at("/get_receipt", poem::post(get_receipt.data(self.context.clone())))
            .at("/get_value", poem::post(get_value.data(self.context.clone())));

        info!("Server running at {}", addr);
        Server::new(listener::TcpListener::bind(addr)).run(app).await?;

        Ok(())
    }
}