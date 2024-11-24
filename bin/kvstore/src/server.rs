use std::sync::Arc;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::{
    kvstore::KvStore,
    request::{GetRequest, SetRequest},
};

pub struct Server {
    kv_store: Arc<KvStore>,
}

impl Server {
    pub fn new(kv_store: Arc<KvStore>) -> Self {
        Self { kv_store }
    }
    /// Starts the TCP server
    pub async fn start(&self, addr: &str) -> tokio::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server running on {}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let kv_store = self.kv_store.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(kv_store, stream).await {
                    eprintln!("Error handling client: {:?}", e);
                }
            });
        }
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
                    let value = parts.next().unwrap_or("").to_string();
                    let req = SetRequest {
                        key: key.as_bytes().to_vec(),
                        value: value.as_bytes().to_vec(),
                    };
                    kv_store.append_set(req).await;
                    reader.get_mut().write_all(b"OK\n").await?;
                }
                Some("GET") => {
                    let key = parts.next().unwrap_or("").to_string();
                    let req = GetRequest { key: key.as_bytes().to_vec() };
                    let value = kv_store.get_value(req).await;
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
