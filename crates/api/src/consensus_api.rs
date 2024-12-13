use std::{future::IntoFuture, hash::Hash, sync::Arc, time::Duration};

use crate::{
    bootstrap::{
        init_mempool, init_network_interfaces, init_peers_and_metadata,
        start_consensus, start_node_inspection_service,
    }, consensus_mempool_handler::{ConsensusToMempoolHandler, MempoolNotificationHandler}, https::{https_server, HttpsServerArgs}, logger, network::{create_network_runtime, extract_network_configs}
};
use api_types::{BlockBatch, BlockHashState, ComputeRes, ConsensusApi, ExecutionApi, ExecutionApiV2, ExternalBlock, ExternalBlockMeta, GTxn};
use aptos_config::{config::NodeConfig, network_id::NetworkId};
use aptos_consensus::gravity_state_computer::ConsensusAdapterArgs;
use aptos_consensus::consensusdb::ConsensusDB;
use aptos_event_notifications::EventNotificationSender;
use aptos_logger::info;
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use async_trait::async_trait;
use futures::channel::mpsc::SendError;

use aptos_crypto::{HashValue, PrivateKey, Uniform};
use aptos_types::{
    chain_id::ChainId,
    transaction::{GravityExtension, RawTransaction, SignedTransaction, TransactionPayload},
};
use futures::channel::mpsc;
use futures::future::BoxFuture;
use lazy_static::lazy_static;
use tokio::runtime::Runtime;
use coex_bridge::CoExBridge;

pub struct ConsensusEngine {
    address: String,
    execution_api: Arc<dyn ExecutionApiV2>,
    runtime_vec: Vec<Runtime>,
}



impl ConsensusEngine {
    pub fn init(
        node_config: NodeConfig,
        execution_api: Arc<dyn ExecutionApiV2>,
        block_hash_state: BlockHashState,
        chain_id: u64,
    ) -> Arc<Self> {
        let consensus_db =
            Arc::new(ConsensusDB::new(node_config.storage.dir(), &node_config.node_config_path));
        let peers_and_metadata = init_peers_and_metadata(&node_config, &consensus_db);
        let (_remote_log_receiver, _logger_filter_update) =
            logger::create_logger(&node_config, Some(node_config.log_file_path.clone()));
        let db: DbReaderWriter = DbReaderWriter::new(consensus_db.clone());
        let mut event_subscription_service =
            aptos_event_notifications::EventSubscriptionService::new(Arc::new(
                aptos_infallible::RwLock::new(db.clone()),
            ));
        let network_configs = extract_network_configs(&node_config);

        let network_config = network_configs.get(0).unwrap();
        let mut network_runtimes = vec![];
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
        network_runtimes.push(runtime);

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
        network_runtimes.push(runtime);
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
            execution_api.clone(),
        );
        network_runtimes.extend(mempool_runtime);
        let mut args = ConsensusAdapterArgs::new(execution_api.clone(), consensus_db);
        let (consensus_runtime, _, _, execution_proxy) = start_consensus(
            &node_config,
            &mut event_subscription_service,
            consensus_network_interfaces,
            consensus_notifier,
            consensus_to_mempool_sender,
            db,
            &mut args,
        );
        network_runtimes.push(consensus_runtime);
        // trigger this to make epoch manager invoke new epoch
        let arc_self = Arc::new(Self {
            address: node_config.validator_network.as_ref().unwrap().listen_address.to_string(),
            execution_api: execution_api.clone(),
            runtime_vec: network_runtimes,
        });
        // process new round should be after init reth hash
        let _ = event_subscription_service.notify_initial_configs(1_u64);

        execution_proxy.set_consensus_engine(arc_self.clone());
        if !node_config.https_cert_pem_path.to_str().unwrap().is_empty()
                && !node_config.https_key_pem_path.to_str().unwrap().is_empty() {
            let args = HttpsServerArgs {
                address: node_config.https_server_address,
                execution_api: Some(execution_api.clone()),
                cert_pem: node_config.https_cert_pem_path,
                key_pem: node_config.https_key_pem_path,
            };
            tokio::spawn(https_server(args));
        }
        arc_self
    }
}

#[async_trait]
impl ConsensusApi for ConsensusEngine {
    async fn send_ordered_block(&self, ordered_block: ExternalBlock) {
        info!("send_order_block {:?}", ordered_block);
        match self.execution_api.send_ordered_block(ordered_block).await {
            Ok(_) => {
            },
            Err(_) => panic!("send_ordered_block should not fail"),
        }
    }

    async fn recv_executed_block_hash(&self, head: ExternalBlockMeta) -> ComputeRes {
        info!("recv_executed_block_hash");
        let res = match self.execution_api.recv_executed_block_hash(head).await {
            Ok(r) => r,
            Err(_) => panic!("send_ordered_block should not fail"),
        };
        res
    }

    async fn commit_block_hash(&self, head: ExternalBlockMeta) {
        match self.execution_api.commit_block(head).await {
            Ok(_) => {

            },
            Err(_) => panic!("commit_block_hash should not fail"),
        }
    }
}
