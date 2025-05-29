use api::{check_bootstrap_config, consensus_api::{ConsensusEngine, ConsensusEngineArgs}, NodeConfig};
use clap::Parser;
use cli::Cli;
use execution_channel::ExecutionChannelImpl;
use flexi_logger::{detailed_format, FileSpec, Logger, WriteMode};
use gravity_sdk_kvstore::*;
use secp256k1::SecretKey;
use server::ServerApp;
use std::{error::Error, sync::Arc, thread};

struct TestConsensusLayer {
    node_config: NodeConfig,
}

impl TestConsensusLayer {
    fn new(node_config: NodeConfig) -> Self {
        Self { node_config }
    }

    async fn run(self) {
        let _consensus_engine = ConsensusEngine::init(ConsensusEngineArgs {
            node_config: self.node_config,
            chain_id: 1337,
            latest_block_number: 0,
        }).await;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

/// **Note:** This code serves as a minimum viable implementation for demonstrating how to build a DApp using `gravity-sdk`.
/// It does not include account balance validation, comprehensive error handling, or robust runtime fault tolerance.
/// Current limitations and future tasks include:
/// 1. Block Synchronization: Block synchronization is not yet implemented.
/// A basic Recover API implementation is required for block synchronization functionality.
///
/// 2. State Persistence: The server does not load persisted state data on restart,
/// leading to state resets after each restart.
///
/// 3. Execution Pipeline: Although the execution layer pipeline is designed with
/// five stages, it currently executes blocks serially instead of in a pipelined manner.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let log_dir = cli.log_dir.clone();
    let _handle = Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(log_dir))
        .write_mode(WriteMode::BufferAndFlush)
        .format(detailed_format)
        .start()
        .unwrap();
    let storage = Arc::new(SledStorage::new("blockchain_db")?);
    let genesis_path = cli.genesis_path.clone();
    let mut blockchain = Blockchain::new(storage.clone(), genesis_path);

    let (ordered_block_sender, ordered_block_receiver) = futures::channel::mpsc::channel(10);
    blockchain.process_blocks_pipeline(ordered_block_receiver).await?;
    let listen_url = cli.listen_url.clone();

    cli.run(move || {
        tokio::spawn(async move {
            let server = ServerApp::new(blockchain.state(), storage);
            let _ = thread::spawn(move || {
                let cl = TestConsensusLayer::new(gcei_config);
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });
            server.start(listen_url.as_str()).await.unwrap();
        })
    })
    .await;

    Ok(())
}

mod tests {
    use std::str::FromStr;

    use futures::{
        channel::{mpsc, oneshot},
        SinkExt, StreamExt,
    };
    use hex::FromHex;
    use log::info;
    use secp256k1::{KeyPair, PublicKey, SecretKey};
    use tokio::time::interval;

    use super::*;

    #[tokio::test]
    async fn test_blockchain_processing() -> Result<(), Box<dyn Error>> {
        let storage = SledStorage::new("test_blockchain_db")?;
        let path = None;
        let mut blockchain = Blockchain::new(Arc::new(storage), path);

        let keypair = generate_keypair();
        let secret_key_bytes = keypair.secret_key.secret_bytes();
        let secret_key_hex = hex::encode(secret_key_bytes);
        let address = public_key_to_address(&keypair.public_key);
        info!("secret key {:?}, address {:?}", secret_key_hex, address);

        let blocks = create_test_blocks(&keypair.secret_key, &address);

        let (ordered_block_sender, ordered_block_receiver) = futures::channel::mpsc::channel(10);
        blockchain.process_blocks_pipeline(ordered_block_receiver).await?;
        let mut compute_res_receivers = vec![];

        for (idx, block) in blocks.iter().enumerate() {
            // TODO(): maybe we should send parent block id to get parent block number to read the state root
            let raw_block =
                RawBlock { block_number: idx as u64 + 1, transactions: block.transactions.clone() };
            let (compute_res_sender, compute_res_receiver) = futures::channel::oneshot::channel();
            let executable_block =
                ExecutableBlock { block: raw_block, callbacks: compute_res_sender };
            ordered_block_sender.clone().send(executable_block).await?;
            compute_res_receivers.push(compute_res_receiver);
        }

        for (idx, compute_res) in compute_res_receivers.into_iter().enumerate() {
            let (compute_res, sender) = compute_res.await?;
            info!("Block {} compute res: {:?}", idx + 1, compute_res);
            sender.send(idx as u64 + 1).unwrap();
        }

        info!("the stats is {:?}", blockchain.state());

        blockchain.run().await;

        Ok(())
    }
}

fn create_test_blocks(secret_key: &SecretKey, address: &str) -> Vec<Block> {
    let mut blocks = Vec::new();

    let mut nonce = 0;
    for i in 0..=2 {
        let transactions = vec![
            create_transfer_transaction(secret_key, nonce, "receiver1", 100),
            create_kv_transaction(secret_key, nonce + 1, "key1", "value1"),
        ];
        nonce += 2;

        blocks.push(Block {
            header: BlockHeader {
                number: i + 1,
                parent_hash: [0; 32],
                state_root: [0; 32],
                transactions_root: compute_merkle_root(&transactions),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            transactions,
        });
    }

    blocks
}

fn create_transfer_transaction(
    secret_key: &SecretKey,
    nonce: u64,
    receiver: &str,
    amount: u64,
) -> Transaction {
    let unsigned = UnsignedTransaction {
        nonce,
        kind: TransactionKind::Transfer { receiver: receiver.to_string(), amount },
    };

    let signature = sign_transaction(&unsigned, &secret_key);

    Transaction { unsigned, signature }
}

fn create_kv_transaction(
    secret_key: &SecretKey,
    nonce: u64,
    key: &str,
    value: &str,
) -> Transaction {
    let unsigned = UnsignedTransaction {
        nonce,
        kind: TransactionKind::SetKV { key: key.to_string(), value: value.to_string() },
    };

    let signature = sign_transaction(&unsigned, &secret_key);

    Transaction { unsigned, signature }
}
