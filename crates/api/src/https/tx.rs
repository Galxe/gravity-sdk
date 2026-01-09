use axum::{http::StatusCode, response::Json as JsonResponse};
use gaptos::{aptos_crypto::HashValue, aptos_logger::info};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TxRequest {
    tx: Vec<u8>,
    //    Public key and signature to authenticate
    //    authenticator: (),
}

#[derive(Serialize, Deserialize)]
pub struct SubmitResponse {
    hash: [u8; 32],
    //    Public key and signature to authenticate
    //    authenticator: (),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TxResponse {
    pub tx: Vec<u8>,
    // tx status
}

// example:
// curl -X POST -H "Content-Type:application/json" -d '{"tx": [1, 2, 3, 4]}' https://127.0.0.1:1024/tx/submit_tx
pub async fn submit_tx(_request: TxRequest) -> Result<JsonResponse<SubmitResponse>, StatusCode> {
    todo!()
}

// example:
// curl https://127.0.0.1:1024/tx/get_tx_by_hash/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pub async fn get_tx_by_hash(request: HashValue) -> Result<JsonResponse<TxResponse>, StatusCode> {
    info!("get transaction by hash {}", request);
    Ok(JsonResponse(TxResponse { tx: vec![] }))
}
