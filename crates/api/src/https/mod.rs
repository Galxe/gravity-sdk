pub mod heap_profiler;
mod set_failpoints;
mod tx;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_logger::info;
use axum::{
    body::Body,
    extract::Path,
    http::Request,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use heap_profiler::control_profiler;
use set_failpoints::{set_failpoint, FailpointConf};
use tx::{get_tx_by_hash, submit_tx, TxRequest};

pub struct HttpsServerArgs {
    pub address: String,
    pub cert_pem: Option<PathBuf>,
    pub key_pem: Option<PathBuf>,
}

async fn ensure_https(req: Request<Body>, next: Next) -> Response {
    if req.uri().scheme_str() != Some("https") {
        return Response::builder().status(400).body("HTTPS required".into()).unwrap();
    }
    next.run(req).await
}

pub async fn https_server(args: HttpsServerArgs) {
    rustls::crypto::ring::default_provider().install_default().unwrap();
    let submit_tx_lambda = |Json(request): Json<TxRequest>| async move {
        submit_tx(request).await
    };

    let get_tx_by_hash_lambda = |Path(request): Path<HashValue>| async move {
        get_tx_by_hash(request).await
    };

    let set_fail_point_lambda =
        |Json(request): Json<FailpointConf>| async move { set_failpoint(request).await };

    let control_profiler_lambda = |Json(request): Json<heap_profiler::ControlProfileRequest>| async move {
        control_profiler(request).await
    };

    let https_app = Router::new()
        .route("/tx/submit_tx", post(submit_tx_lambda))
        .route("/tx/get_tx_by_hash/:hash_value", get(get_tx_by_hash_lambda))
        .layer(middleware::from_fn(ensure_https));
    let http_app = Router::new()
        .route("/set_failpoint", post(set_fail_point_lambda))
        .route("/mem_prof", post(control_profiler_lambda));
    let app = Router::new().merge(https_app).merge(http_app);
    let addr: SocketAddr = args.address.parse().unwrap();
    match (args.cert_pem.clone(), args.key_pem.clone()) {
        (Some(cert_path), Some(key_path)) => {
            // configure certificate and private key used by https
            let config =
                RustlsConfig::from_pem_file(cert_path, key_path).await.unwrap_or_else(|e| {
                    panic!(
                        "error {:?}, cert {:?}, key {:?} doesn't work",
                        e, args.cert_pem, args.key_pem
                    )
                });
            info!("https server listen address {}", addr);
            axum_server::bind_rustls(addr, config)
                .serve(app.into_make_service())
                .await
                .unwrap_or_else(|e| {
                    panic!("failed to bind rustls due to {:?}", e);
                });
        }
        _ => {
            info!("http server listen address {}", addr);
            axum_server::bind(addr).serve(app.into_make_service()).await.unwrap_or_else(|e| {
                panic!("failed to bind http due to {:?}", e);
            });
        }
    }
}

#[cfg(test)]
mod test {
    use fail::fail_point;
    use rcgen::generate_simple_self_signed;
    use reqwest::ClientBuilder;
    use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

    use crate::https::tx::TxResponse;

    use super::{https_server, HttpsServerArgs};

    fn test_fail_point() -> Option<()> {
        fail_point!("unit_test_fail_point", |_| {
            println!("set test fail point");
            Some(());
        });
        None
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn work() {
        let subject_alt_names = vec!["127.0.0.1".to_string()];
        let cert = generate_simple_self_signed(subject_alt_names).unwrap();

        let cert_pem = cert.serialize_pem().unwrap();
        let key_pem = cert.serialize_private_key_pem();
        let dir = env!("CARGO_MANIFEST_DIR").to_owned();
        fs::create_dir(dir.clone() + "/src/https/test").unwrap();
        fs::write(dir.clone() + "/src/https/test/cert.pem", cert_pem).unwrap();
        fs::write(dir.clone() + "/src/https/test/key.pem", key_pem).unwrap();

        let args = HttpsServerArgs {
            address: "127.0.0.1:5425".to_owned(),
            cert_pem: Some(PathBuf::from(dir.clone() + "/src/https/test/cert.pem")),
            key_pem: Some(PathBuf::from(dir.clone() + "/src/https/test/key.pem")),
        };
        let _handler = tokio::spawn(https_server(args));
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        // read a local binary pem encoded certificate
        let pem = std::fs::read(dir.clone() + "/src/https/test/cert.pem").unwrap();
        let cert = reqwest::Certificate::from_pem(&pem).unwrap();

        let client = ClientBuilder::new()
            .add_root_certificate(cert)
            .danger_accept_invalid_hostnames(true)
            .danger_accept_invalid_certs(true)
            //.use_rustls_tls()
            .build()
            .unwrap();

        // test set_fail_point
        assert!(test_fail_point().is_none());
        let mut map = HashMap::new();
        map.insert("name", "unit_test_fail_point");
        map.insert("action", "return");
        let res =
            client.post("http://127.0.0.1:5425/set_failpoint").json(&map).send().await.unwrap();
        assert!(res.status().is_success(), "res is {:?}", res);
        assert!(test_fail_point().is_some());

        let body = client.get("https://127.0.0.1:5425/tx/get_tx_by_hash/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .send()
            .await
            .unwrap_or_else(|e| {
                panic!("failed to send due to {:?}", e)
            })
            .json::<TxResponse>()
            .await.unwrap();
        assert!(body.tx.is_empty());

        let mut map = HashMap::new();
        map.insert("tx", vec![1, 2, 3, 4]);
        let res =
            client.post("https://127.0.0.1:5425/tx/submit_tx").json(&map).send().await.unwrap();
        assert!(res.status().is_success());
    }
}
