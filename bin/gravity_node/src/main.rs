use alloy_eips::BlockHashOrNumber;
use alloy_primitives::TxHash;
use consensus::mock_consensus::mock::MockConsensus;
use greth::gravity_storage;
use greth::reth;
use greth::reth::chainspec::EthereumChainSpecParser;
use greth::reth_cli_util;
use greth::reth_db;
use greth::reth_node_api;
use greth::reth_node_builder;
use greth::reth_node_ethereum;
use greth::reth_pipe_exec_layer_ext_v2;
use greth::reth_pipe_exec_layer_ext_v2::ExecutionArgs;
use greth::reth_provider;
use greth::reth_transaction_pool;
use pprof::protos::Message;
use api::check_bootstrap_config;
use consensus::aptos::AptosConsensus;
use gravity_storage::block_view_storage::BlockViewStorage;
use pprof::ProfilerGuard;
use reth::rpc::builder::auth::AuthServerHandle;
use reth_coordinator::RethCoordinator;
use reth_db::DatabaseEnv;
use reth_node_api::NodeTypesWithDBAdapter;
use reth_pipe_exec_layer_ext_v2::PipeExecLayerApi;
use reth_provider::BlockHashReader;
use reth_provider::BlockNumReader;
use reth_provider::BlockReader;
use reth_transaction_pool::TransactionPool;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::info;
mod cli;
mod consensus;
mod reth_cli;
mod reth_coordinator;

use crate::cli::Cli;
use std::collections::BTreeMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use crate::reth_cli::RethCli;
use clap::Parser;
use reth_node_builder::engine_tree_config;
use reth_node_builder::EngineNodeLauncher;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use reth_provider::providers::BlockchainProvider;

struct ConsensusArgs {
    pub engine_api: AuthServerHandle,
    pub pipeline_api: PipeExecLayerApi<BlockViewStorage<BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>>>,
    pub provider: BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
    pub tx_listener: tokio::sync::mpsc::Receiver<TxHash>,
    pub pool: reth_transaction_pool::Pool<
        reth_transaction_pool::TransactionValidationTaskExecutor<
            reth_transaction_pool::EthTransactionValidator<
                BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>,
                reth_transaction_pool::EthPooledTransaction,
            >,
        >,
        reth_transaction_pool::CoinbaseTipOrdering<reth_transaction_pool::EthPooledTransaction>,
        reth_transaction_pool::blobstore::DiskFileBlobStore,
    >,
}

fn run_reth(
    tx: mpsc::Sender<(ConsensusArgs, u64)>,
    cli: Cli<EthereumChainSpecParser>,
    execution_args_rx: oneshot::Receiver<ExecutionArgs>,
) {
    reth_cli_util::sigsegv_handler::install();

    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = {
        cli.run(|builder, _| {
            let tx = tx.clone();
            async move {
                let handle = builder
                    .with_types_and_provider::<EthereumNode, BlockchainProvider<_>>()
                    .with_components(EthereumNode::components())
                    .with_add_ons(EthereumAddOns::default())
                    .launch_with_fn(|builder| {
                        let launcher = EngineNodeLauncher::new(
                            builder.task_executor().clone(),
                            builder.config().datadir(),
                            engine_tree_config::TreeConfig::default(),
                        );
                        builder.launch_with(launcher)
                    })
                    .await?;
                let chain_spec = handle.node.chain_spec();
                let pending_listener: tokio::sync::mpsc::Receiver<TxHash> =
                    handle.node.pool.pending_transactions_listener();
                let engine_cli = handle.node.auth_server_handle().clone();
                let provider: BlockchainProvider<
                    reth_node_api::NodeTypesWithDBAdapter<EthereumNode, Arc<reth_db::DatabaseEnv>>,
                > = handle.node.provider;
                let latest_block_number = provider.last_block_number().unwrap();
                info!("The latest_block_number is {}", latest_block_number);
                let latest_block_hash = provider.block_hash(latest_block_number).unwrap().unwrap();
                let latest_block = provider
                    .block(BlockHashOrNumber::Number(latest_block_number))
                    .unwrap()
                    .unwrap();
                let pool: reth_transaction_pool::Pool<
                    reth_transaction_pool::TransactionValidationTaskExecutor<
                        reth_transaction_pool::EthTransactionValidator<
                            BlockchainProvider<
                                NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>,
                            >,
                            reth_transaction_pool::EthPooledTransaction,
                        >,
                    >,
                    reth_transaction_pool::CoinbaseTipOrdering<
                        reth_transaction_pool::EthPooledTransaction,
                    >,
                    reth_transaction_pool::blobstore::DiskFileBlobStore,
                > = handle.node.pool;

                let storage = BlockViewStorage::new(
                    provider.clone(),
                    latest_block.number,
                    latest_block_hash,
                    BTreeMap::new(),
                );
                let pipeline_api_v2: PipeExecLayerApi<BlockViewStorage<BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>>> = reth_pipe_exec_layer_ext_v2::new_pipe_exec_layer_api(
                    chain_spec,
                    storage,
                    latest_block.header,
                    latest_block_hash,
                    execution_args_rx,
                );
                let args = ConsensusArgs {
                    engine_api: engine_cli,
                    pipeline_api: pipeline_api_v2,
                    provider,
                    tx_listener: pending_listener,
                    pool,
                };
                tx.send((args, latest_block_number)).await.ok();
                handle.node_exit_future.await
            }
        })
    } {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}


struct ProfilingState {
    guard: Option<ProfilerGuard<'static>>,
    profile_count: usize,
}

fn setup_pprof_profiler() -> Arc<Mutex<ProfilingState>> {
    let profiling_state = Arc::new(Mutex::new(ProfilingState {
        guard: None,
        profile_count: 0,
    }));
    
    let profiling_state_clone = profiling_state.clone();
    
    thread::spawn(move || {
        let config = 99;
        
        let start = Instant::now();
        let max_duration = Duration::from_secs(60 * 30); // 最多运行30min
        
        while start.elapsed() < max_duration {
            let profile_duration = Duration::from_secs(3 * 60);
            
            {
                let mut state = profiling_state_clone.lock().unwrap();
                state.guard = Some(ProfilerGuard::new(config).unwrap());
                println!("Started profiling session #{}", state.profile_count + 1);
            }
            
            thread::sleep(profile_duration);
            // 
            {
                let mut state = profiling_state_clone.lock().unwrap();
                if let Some(guard) = state.guard.take() {
                    if let Ok(report) = guard.report().build() {
                        let timestamp = std::time::SystemTime::now();
                        let count = state.profile_count;
                        
                        let now = std::time::SystemTime::now();
                        let formatted_time = {
                            let elapsed = now.duration_since(std::time::UNIX_EPOCH).unwrap();
                            let secs = elapsed.as_secs();
                            let time = time::OffsetDateTime::from_unix_timestamp(secs as i64).unwrap();
                            format!("{:02}-{:02}-{:02}-{:02}", 
                                time.day(), time.hour(), time.minute(), time.second())
                        };
                        let flamegraph_path = format!("profile_{}_flame_{}.svg", count, formatted_time);
                        if let Ok(file) = File::create(&flamegraph_path) {
                            if let Err(e) = report.flamegraph(file) {
                                eprintln!("Failed to write flamegraph: {}", e);
                            } else {
                                println!("Wrote flamegraph to {}", flamegraph_path);
                            }
                        }
                        

                        let proto_path = format!("profile_{}_proto_{:?}.pb", count, timestamp);
                        if let Ok(mut file) = File::create(&proto_path) {
                            if let Ok(profile) = report.pprof() {
                                let mut content = Vec::new();
                                if profile.write_to_vec(&mut content).is_ok() {
                                    if std::io::Write::write_all(&mut file, &content).is_ok() {
                                        println!("Wrote protobuf to {}", proto_path);
                                    }
                                }
                            }
                        }
                        state.profile_count += 1;
                    }
                }
            }
            
            thread::sleep(Duration::from_secs(5));
        }
    });
    
    profiling_state
}


fn main() {
    let _profiling_state = if std::env::var("ENABLE_PPROF").is_ok() {
        Some(setup_pprof_profiler())
    } else {
        None
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let cli = Cli::parse();
    let gcei_config = check_bootstrap_config(cli.gravity_node_config.node_config_path.clone());
    let (execution_args_tx, execution_args_rx) = oneshot::channel();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Some((args, latest_block_number)) = rx.recv().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                let client = RethCli::new(args).await;
                let chain_id = client.chain_id();
                let coordinator =
                    Arc::new(RethCoordinator::new(client, latest_block_number, execution_args_tx));
                if std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap() {
                    let mock = MockConsensus::new().await;
                    tokio::spawn(async move {
                        mock.run().await;
                    });
                } else {
                    AptosConsensus::init(gcei_config, chain_id, latest_block_number).await;
                }
                coordinator.send_execution_args().await;
                coordinator.run().await;
                tokio::signal::ctrl_c().await.unwrap();
            }
        });
    });
    run_reth(tx, cli, execution_args_rx);
}
