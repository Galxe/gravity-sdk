use api::{check_bootstrap_config, consensus_api::ConsensusEngine, NodeConfig};
use api_types::{default_recover::DefaultRecovery, ConsensusApi, ExecutionChannel, ExecutionLayer};
use clap::Parser;
use cli::Cli;
use execution_channel::ExecutionChannelImpl;
use flexi_logger::{detailed_format, FileSpec, Logger, WriteMode};
use pipeline_blockchain::*;
use server::ServerApp;
use std::{error::Error, sync::Arc, thread};

struct TestConsensusLayer {
    consensus_engine: Arc<dyn ConsensusApi>,
}

impl TestConsensusLayer {
    fn new(node_config: NodeConfig, execution_client: Arc<dyn ExecutionChannel>) -> Self {
        let execution_layer = ExecutionLayer {
            execution_api: execution_client,
            recovery_api: Arc::new(DefaultRecovery {}),
        };
        Self { consensus_engine: ConsensusEngine::init(node_config, execution_layer, 1337) }
    }

    async fn run(self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

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
    let mut blockchain = Blockchain::new(storage.clone());

    let (ordered_block_sender, ordered_block_receiver) = futures::channel::mpsc::channel(10);
    blockchain.process_blocks_pipeline(ordered_block_receiver).await?;
    let execution_channel = Arc::new(ExecutionChannelImpl::new(ordered_block_sender));

    cli.run(move || {
        tokio::spawn(async move {
            let server = ServerApp::new(execution_channel.clone(), blockchain.state(), storage);
            let _ = thread::spawn(move || {
                let cl = TestConsensusLayer::new(gcei_config, execution_channel);
                tokio::runtime::Runtime::new().unwrap().block_on(cl.run());
            });
            server.start("127.0.0.1:9006").await.unwrap();
        })
    })
    .await;

    Ok(())
}

mod tests {
    use futures::{
        channel::{mpsc, oneshot},
        SinkExt, StreamExt,
    };
    use log::info;
    use tokio::time::interval;

    use super::*;

    #[tokio::test]
    async fn test_blockchain_processing() -> Result<(), Box<dyn Error>> {
        let _handle = Logger::try_with_str("info")
            .unwrap()
            .log_to_file(
                FileSpec::default().directory("/home/jingyue/projects/gravity-sdk/kvstore_v2/logs"),
            )
            .write_mode(WriteMode::BufferAndFlush)
            .format(detailed_format)
            .start()
            .unwrap();

        let storage = SledStorage::new("test_blockchain_db")?;
        let mut blockchain = Blockchain::new(Arc::new(storage));

        let keypair = generate_keypair();
        let address = public_key_to_address(&keypair.public_key);

        let blocks = create_test_blocks(&keypair, &address);

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

        blockchain.run().await;

        Ok(())
    }
}

fn create_test_blocks(keypair: &KeyPair, address: &str) -> Vec<Block> {
    let mut blocks = Vec::new();

    let mut nonce = 0;
    for i in 0..=2 {
        let transactions = vec![
            create_transfer_transaction(keypair, nonce, "receiver1", 100),
            create_kv_transaction(keypair, nonce + 1, "key1", "value1"),
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
    keypair: &KeyPair,
    nonce: u64,
    receiver: &str,
    amount: u64,
) -> Transaction {
    let unsigned = UnsignedTransaction {
        nonce,
        kind: TransactionKind::Transfer { receiver: receiver.to_string(), amount },
    };

    let signature = sign_transaction(&unsigned, &keypair.secret_key);

    Transaction { unsigned, signature }
}

fn create_kv_transaction(keypair: &KeyPair, nonce: u64, key: &str, value: &str) -> Transaction {
    let unsigned = UnsignedTransaction {
        nonce,
        kind: TransactionKind::SetKV { key: key.to_string(), value: value.to_string() },
    };

    let signature = sign_transaction(&unsigned, &keypair.secret_key);

    Transaction { unsigned, signature }
}
