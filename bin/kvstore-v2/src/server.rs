use std::sync::Arc;

use api_types::{account::ExternalAccountAddress, ExecTxn, ExecutionApiV2};
use log::warn;
use rand::Rng;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::{kv::KvStore, txn::RawTxn};

fn generate_random_address() -> ExternalAccountAddress {
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    ExternalAccountAddress::new(random_bytes)
}
pub struct Server {
    kv_store: Arc<KvStore>,
}


impl Server {
    pub fn new() -> Self {
        Self { kv_store: Arc::new(KvStore::new()), }
    }

    /// Starts the TCP server
    pub async fn start(&self, addr: &str) -> tokio::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let kv_store = self.kv_store.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(kv_store, stream).await {
                    warn!("Error handling client: {:?}", e);
                }
            });
        }
    }

    pub async fn execution_client(&self) -> Arc<dyn ExecutionApiV2> {
        self.kv_store.clone()
    }

    /// Handles a single client connection
    async fn handle_client(kv_store: Arc<KvStore>, stream: TcpStream) -> tokio::io::Result<()> {
        let mut reader = BufReader::new(stream);
        let mut buffer = String::new();

        loop {
            buffer.clear();
            let bytes_read = reader.read_line(&mut buffer).await?;
            if bytes_read == 0 {
                break; // Connection closed
            }

            let mut parts = buffer.trim().splitn(3, ' ');
            match parts.next() {
                Some("SET") => {
                    let key = parts.next().unwrap_or("").to_string();
                    let val = parts.next().unwrap_or("").to_string();
                    let raw_txn =
                        RawTxn { account: generate_random_address(), sequence_number: 1, key, val };
                    match kv_store.add_txn(ExecTxn::RawTxn(raw_txn.to_bytes())).await {
                        Ok(_) => reader.get_mut().write_all(b"OK\n").await?,
                        Err(_) => reader.get_mut().write_all(b"FAILED TO SET\n").await?,
                    }
                }
                Some("GET") => {
                    let key = parts.next().unwrap_or("").to_string();
                    let value = kv_store.get(&key).await;
                    if let Some(value) = value {
                        reader.get_mut().write_all(format!("{}\n", value).as_bytes()).await?;
                    } else {
                        reader.get_mut().write_all(b"NOT FOUND\n").await?;
                    }
                }
                Some("QUIT") => {
                    reader.get_mut().write_all(b"Goodbye!\n").await?;
                    break;
                }
                _ => {
                    reader.get_mut().write_all(b"Unknown command\n").await?;
                }
            }
        }

        Ok(())
    }
}
