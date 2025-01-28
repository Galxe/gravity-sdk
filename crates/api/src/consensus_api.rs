use std::{collections::BTreeMap, sync::Arc};

use crate::{
    bootstrap::{
        init_mempool, init_network_interfaces, init_peers_and_metadata, start_consensus,
        start_node_inspection_service,
    },
    consensus_mempool_handler::{ConsensusToMempoolHandler, MempoolNotificationHandler},
    https::{https_server, HttpsServerArgs},
    logger,
    network::{create_network_runtime, extract_network_configs},
};
use api_types::{
    u256_define::BlockId, u256_define::ComputeRes, ConsensusApi, ExecutionLayer, ExternalBlock,
    ExternalBlockMeta,
};
use aptos_build_info::build_information;
use aptos_config::{config::NodeConfig, network_id::NetworkId};
use aptos_consensus::consensusdb::ConsensusDB;
use aptos_consensus::gravity_state_computer::ConsensusAdapterArgs;
use aptos_event_notifications::EventNotificationSender;
use aptos_logger::{info, warn};
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::{DbReader, DbReaderWriter};
use aptos_telemetry::service::start_telemetry_service;
use async_trait::async_trait;

use aptos_types::chain_id::ChainId;
use futures::channel::mpsc;
use tokio::runtime::Runtime;

pub struct ConsensusEngine {
    address: String,
    execution_layer: ExecutionLayer,
    runtimes: Vec<Runtime>,
}

fn fail_point_check(node_config: &NodeConfig) {
    // Ensure failpoints are configured correctly
    if fail::has_failpoints() {
        warn!("Failpoints are enabled!");

        // Set all of the failpoints
        if let Some(failpoints) = &node_config.failpoints {
            for (point, actions) in failpoints {
                fail::cfg(point, actions).unwrap_or_else(|_| {
                    panic!(
                        "Failed to set actions for failpoint! Failpoint: {:?}, Actions: {:?}",
                        point, actions
                    )
                });
            }
        }
    } else if node_config.failpoints.is_some() {
        warn!("Failpoints is set in the node config, but the binary didn't compile with this feature!");
    }
}

impl ConsensusEngine {
    pub fn get_data_from_consensus_db(node_config: &NodeConfig) -> BTreeMap<u64, ComputeRes> {
        let mut res = BTreeMap::new();
        let consensus_db =
            Arc::new(ConsensusDB::new(node_config.storage.dir(), &node_config.node_config_path));
        let ledger_infos = consensus_db.get_latest_ledger_infos();
        for ledger_info in ledger_infos {
            res.insert(
                ledger_info.ledger_info().block_number(),
                ComputeRes(*ledger_info.ledger_info().consensus_block_id()),
            );
        }
        res
    }

    pub fn init(
        node_config: NodeConfig,
        execution_layer: ExecutionLayer,
        chain_id: u64,
    ) -> Arc<Self> {
        // Setup panic handler
        aptos_crash_handler::setup_panic_handler();

        fail_point_check(&node_config);
        let consensus_db =
            Arc::new(ConsensusDB::new(node_config.storage.dir(), &node_config.node_config_path));
        let peers_and_metadata = init_peers_and_metadata(&node_config, &consensus_db);
        info!("when init peers_and_metadata is {:?}", peers_and_metadata);
        let (remote_log_receiver, logger_filter_update) =
            logger::create_logger(&node_config, Some(node_config.log_file_path.clone()));
        let mut runtimes = vec![];
        if let Some(runtime) = start_telemetry_service(
            node_config.clone(),
            ChainId::new(chain_id),
            build_information!(),
            remote_log_receiver,
            Some(logger_filter_update),
        ) {
            runtimes.push(runtime);
        }
        let db: DbReaderWriter = DbReaderWriter::new(consensus_db.clone());
        let mut event_subscription_service =
            aptos_event_notifications::EventSubscriptionService::new(Arc::new(
                aptos_infallible::RwLock::new(db.clone()),
            ));
        let network_configs = extract_network_configs(&node_config);
        info!("Network Configs: {:?}", network_configs);

        let network_config = network_configs.get(0).unwrap();
        // Create a network runtime for the config
        let runtime = create_network_runtime(&network_config);
        // Entering gives us a runtime to instantiate all the pieces of the builder
        let _enter = runtime.enter();
        let chain_id = ChainId::from(chain_id);
        let mut network_builder = NetworkBuilder::create(
            chain_id,
            node_config.base.role,
            &network_config,
            aptos_time_service::TimeService::real(),
            Some(&mut event_subscription_service),
            peers_and_metadata.clone(),
        );
        let network_id: NetworkId = network_config.network_id;
        let (consensus_network_interfaces, mempool_interfaces) = init_network_interfaces(
            &mut network_builder,
            network_id,
            &network_config,
            &node_config,
            peers_and_metadata.clone(),
        );
        let state_sync_config = node_config.state_sync;
        // The consensus_listener would listenes the request sent by ExecutionProxy's commit function
        // And then send NotifyCommit request to mempool which is named consensus_to_mempool_sender in Gravity
        let (consensus_notifier, consensus_listener) =
            aptos_consensus_notifications::new_consensus_notifier_listener_pair(
                state_sync_config.state_sync_driver.commit_notification_timeout_ms,
            );
        // Build and start the network on the runtime
        network_builder.build(runtime.handle().clone());
        network_builder.start();
        runtimes.push(runtime);

        // Start the node inspection service
        start_node_inspection_service(&node_config, peers_and_metadata.clone());

        let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
        let (notification_sender, notification_receiver) = mpsc::channel(1);

        // Create notification senders and listeners for mempool, consensus and the storage service
        // For Gravity we only use it to notify the mempool for the committed txn gc logic
        let mempool_notifier =
            aptos_mempool_notifications::MempoolNotifier::new(notification_sender);
        let mempool_notification_handler = MempoolNotificationHandler::new(mempool_notifier);
        let mut consensus_mempool_handler =
            ConsensusToMempoolHandler::new(mempool_notification_handler, consensus_listener);
        let runtime = aptos_runtimes::spawn_named_runtime("Con2Mempool".into(), None);
        runtime.spawn(async move {
            consensus_mempool_handler.start().await;
        });
        runtimes.push(runtime);
        let mempool_listener =
            aptos_mempool_notifications::MempoolNotificationListener::new(notification_receiver);
        let (_mempool_client_sender, _mempool_client_receiver) = mpsc::channel(1);
        let mempool_runtime = init_mempool(
            &node_config,
            &db,
            &mut event_subscription_service,
            mempool_interfaces,
            _mempool_client_receiver,
            consensus_to_mempool_receiver,
            mempool_listener,
            peers_and_metadata,
            execution_layer.execution_api.clone(),
        );
        runtimes.extend(mempool_runtime);
        let mut args = ConsensusAdapterArgs::new(execution_layer.clone(), consensus_db);
        let (consensus_runtime, _, _) = start_consensus(
            &node_config,
            &mut event_subscription_service,
            consensus_network_interfaces,
            consensus_notifier,
            consensus_to_mempool_sender,
            db,
            &mut args,
        );
        runtimes.push(consensus_runtime);
        // trigger this to make epoch manager invoke new epoch
        if !node_config.https_cert_pem_path.to_str().unwrap().is_empty()
            && !node_config.https_key_pem_path.to_str().unwrap().is_empty()
        {
            let args = HttpsServerArgs {
                address: node_config.https_server_address,
                execution_api: execution_layer.execution_api.clone(),
                cert_pem: node_config.https_cert_pem_path,
                key_pem: node_config.https_key_pem_path,
            };
            let runtime = aptos_runtimes::spawn_named_runtime("Http".into(), None);
            runtime.spawn(async move { https_server(args) });
            runtimes.push(runtime);
        }
        let arc_consensus_engine = Arc::new(Self {
            address: node_config.validator_network.as_ref().unwrap().listen_address.to_string(),
            execution_layer: execution_layer.clone(),
            runtimes,
        });
        crate::coex::register_hook_func(arc_consensus_engine.clone());
        // process new round should be after init retƒh hash
        let _ = event_subscription_service.notify_initial_configs(1_u64);
        arc_consensus_engine
    }
}

#[async_trait]
impl ConsensusApi for ConsensusEngine {
    async fn send_ordered_block(&self, parent_id: [u8; 32], ordered_block: ExternalBlock) {
        info!("send_order_block {:?}", ordered_block);
        match self
            .execution_layer
            .execution_api
            .send_ordered_block(BlockId(parent_id), ordered_block)
            .await
        {
            Ok(_) => {}
            Err(_) => panic!("send_ordered_block should not fail"),
        }
    }

    async fn recv_executed_block_hash(&self, head: ExternalBlockMeta) -> ComputeRes {
        info!("recv_executed_block_hash");
        let res = match self.execution_layer.execution_api.recv_executed_block_hash(head).await {
            Ok(r) => r,
            Err(_) => panic!("send_ordered_block should not fail"),
        };
        res
    }

    async fn commit_block_hash(&self, head: [u8; 32]) {
        match self.execution_layer.execution_api.commit_block(BlockId(head)).await {
            Ok(_) => {}
            Err(_) => panic!("commit_block_hash should not fail"),
        }
    }
}
