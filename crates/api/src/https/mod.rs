mod tx;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use api_types::ExecutionApi;
use aptos_crypto::HashValue;
use aptos_logger::info;
use axum::{
    extract::{Json, Path},
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use tx::{get_tx_by_hash, submit_tx, TxRequest};

pub struct HttpsServerArgs {
    pub address: String,
    pub execution_api: Option<Arc<dyn ExecutionApi>>,
    pub cert_pem: PathBuf,
    pub key_pem: PathBuf,
}

pub async fn https_server(args: HttpsServerArgs) {
    rustls::crypto::ring::default_provider().install_default().unwrap();
    let execution_api_clone = args.execution_api.clone();
    let submit_tx_lambda = |Json(request): Json<TxRequest>| async move { 
        submit_tx(request, execution_api_clone).await 
    };

    let execution_api_clone = args.execution_api.clone();
    let get_tx_by_hash_lambda =
        |Path(request): Path<HashValue>| async move { get_tx_by_hash(request, execution_api_clone).await };

    let app = Router::new()
        .route("/tx/submit_tx", post(submit_tx_lambda))
        .route("/tx/get_tx_by_hash/:hash_value", get(get_tx_by_hash_lambda));
    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(args.cert_pem, args.key_pem)
        .await
        .unwrap();
    let addr: SocketAddr = args.address.parse().unwrap();
    info!("https server listen address {}", addr);
    axum_server::bind_rustls(addr, config).serve(app.into_make_service()).await.unwrap();
}

mod test {
    use rcgen::generate_simple_self_signed;
    use reqwest::ClientBuilder;
    use std::{collections::HashMap, fs, path::PathBuf, thread::sleep};

    use crate::https::tx::TxResponse;

    use super::{https_server, HttpsServerArgs};

    #[tokio::test]
    async fn work() {
        let subject_alt_names = vec!["127.0.0.1".to_string()];
        let cert = generate_simple_self_signed(subject_alt_names).unwrap();

        // 获取 PEM 格式的证书和私钥
        let cert_pem = cert.serialize_pem().unwrap();
        let key_pem = cert.serialize_private_key_pem();
        fs::create_dir("./src/https/test");
        // 保存到文件
        fs::write("./src/https/test/cert.pem", cert_pem);
        fs::write("./src/https/test/key.pem", key_pem);

        let args = HttpsServerArgs {
            address: "127.0.0.1:5425".to_owned(),
            execution_api: None,
            cert_pem: PathBuf::from(env!("CARGO_MANIFEST_DIR").to_owned() + "/src/https/test/cert.pem"),
            key_pem: PathBuf::from(env!("CARGO_MANIFEST_DIR").to_owned() + "/src/https/test/key.pem"),
        };
        tokio::spawn(https_server(args));
        sleep(std::time::Duration::from_secs(1));
        // read a local binary pem encoded certificate
        let pem = std::fs::read("./src/https/test/cert.pem").unwrap();
        let cert = reqwest::Certificate::from_pem(&pem).unwrap();

        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build().unwrap();

        let body = client.get("https://127.0.0.1:5425/tx/get_tx_by_hash/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .send()
            .await.unwrap()
            .json::<TxResponse>()
            .await.unwrap();
        assert!(body.tx.is_empty());

        let mut map = HashMap::new();
        map.insert("tx", vec![1, 2, 3, 4]);
        let res = client.post("https://127.0.0.1:5425/tx/submit_tx")
            .json(&map)
            .send()
            .await.unwrap();
        assert!(res.status().is_success());
    }
}