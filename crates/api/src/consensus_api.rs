use std::sync::Arc;

use crate::{
    bootstrap::{
        init_block_buffer_manager, init_mempool, init_peers_and_metadata,
        start_consensus, start_node_inspection_service, dkg_network_configuration,
        create_dkg_runtime, init_jwk_consensus
    },
    consensus_mempool_handler::{ConsensusToMempoolHandler, MempoolNotificationHandler},
    https::{https_server, HttpsServerArgs},
    logger,
    network::{
        consensus_network_configuration, create_network_interfaces, create_network_runtime,
        extract_network_configs, jwk_consensus_network_configuration,
        mempool_network_configuration, register_client_and_service_with_network,
        ApplicationNetworkHandle,
    },
};
use aptos_consensus::{consensusdb::ConsensusDB, gravity_state_computer::ConsensusAdapterArgs};
use block_buffer_manager::TxPool;
use build_info::build_information;
use futures::channel::mpsc;
use gaptos::{
    api_types::config_storage::{ConfigStorage, GLOBAL_CONFIG_STORAGE},
    aptos_config::{
        config::{NodeConfig, RoleType},
        network_id::NetworkId,
    },
    aptos_dkg_runtime::DKGMessage,
    aptos_event_notifications::EventNotificationSender,
    aptos_logger::{info, warn},
    aptos_network_builder::builder::NetworkBuilder,
    aptos_storage_interface::DbReaderWriter,
    aptos_telemetry::service::start_telemetry_service,
    aptos_types::chain_id::ChainId,
    aptos_validator_transaction_pool::VTxnPoolState,
};
use tokio::{runtime::Runtime, sync::Mutex};

#[cfg(unix)]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub struct ConsensusEngine {
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

pub struct ConsensusEngineArgs {
    pub node_config: NodeConfig,
    pub chain_id: u64,
    pub latest_block_number: u64,
    pub config_storage: Option<Arc<dyn ConfigStorage>>,
}

impl ConsensusEngine {
    pub async fn init(args: ConsensusEngineArgs, pool: Box<dyn TxPool>) -> Arc<Self> {
        let ConsensusEngineArgs { node_config, chain_id, latest_block_number, config_storage } =
            args;
        // Setup panic handler
        gaptos::aptos_crash_handler::setup_panic_handler();

        fail_point_check(&node_config);
        let consensus_db =
            Arc::new(ConsensusDB::new(node_config.storage.dir(), &node_config.node_config_path));
        let peers_and_metadata = init_peers_and_metadata(&node_config, &consensus_db);
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
        let db: DbReaderWriter = DbReaderWriter::from_arc(consensus_db.clone());
        let mut event_subscription_service =
            gaptos::aptos_event_notifications::EventSubscriptionService::new(Arc::new(
                gaptos::aptos_infallible::RwLock::new(db.clone()),
            ));
        // It seems stupid, refactor when debugging finished
        if config_storage.is_some() {
            match GLOBAL_CONFIG_STORAGE.set(config_storage.unwrap()) {
                Ok(_) => {}
                Err(_) => {
                    panic!("Failed to set config storage");
                }
            }
        }
        let chain_id = ChainId::from(chain_id);
        let network_configs = extract_network_configs(&node_config);

        // Create each network and register the application handles
        let mut jwk_consensus_network_handle = None;
        let mut dkg_network_handle: Option<ApplicationNetworkHandle<DKGMessage>> = None;
        let mut consensus_network_handles = vec![];
        let mut mempool_network_handles = vec![];
        for network_config in network_configs.into_iter() {
            // Create a network runtime for the config
            let runtime = create_network_runtime(&network_config);

            // Entering gives us a runtime to instantiate all the pieces of the builder
            let _enter = runtime.enter();

            let network_id = network_config.network_id;

            // Create a new network builder
            let mut network_builder = NetworkBuilder::create(
                chain_id,
                if network_id.is_vfn_network() {
                    // FIXME(nekomoto): This is a temporary solution to support block sync for
                    // validator node which is not the current epoch validator.
                    RoleType::FullNode
                } else {
                    node_config.base.role
                },
                &network_config,
                gaptos::aptos_time_service::TimeService::real(),
                Some(&mut event_subscription_service),
                peers_and_metadata.clone(),
            );

            if network_id.is_validator_network() {
                if jwk_consensus_network_handle.is_some() {
                    panic!("There can be at most one validator network!");
                } else {
                    let network_handle = register_client_and_service_with_network(
                        &mut network_builder,
                        network_id,
                        &network_config,
                        jwk_consensus_network_configuration(&node_config),
                        true,
                    );
                    jwk_consensus_network_handle = Some(network_handle);
                }
            }
     
            // Only create DKG network interface for validator nodes
            if network_id.is_validator_network() {
                dkg_network_handle = Some(register_client_and_service_with_network::<DKGMessage>(
                    &mut network_builder,
                    network_id,
                    &network_config,
                    dkg_network_configuration(&node_config),
                    false,
                ));
            }

            // Register consensus (both client and server) with the network
            let network_handle = register_client_and_service_with_network(
                &mut network_builder,
                network_id,
                &network_config,
                consensus_network_configuration(&node_config),
                true,
            );
            consensus_network_handles.push(network_handle);

            // Register mempool (both client and server) with the network
            let mempool_network_handle = register_client_and_service_with_network(
                &mut network_builder,
                network_id,
                &network_config,
                mempool_network_configuration(&node_config),
                true,
            );
            mempool_network_handles.push(mempool_network_handle);

            // Build and start the network on the runtime
            network_builder.build(runtime.handle().clone());
            network_builder.start();
            runtimes.push(runtime);
        }

        // Transform all network handles into application interfaces
        let jwk_consensus_interfaces = jwk_consensus_network_handle.map(|handle| {
            create_network_interfaces(
                vec![handle],
                jwk_consensus_network_configuration(&node_config),
                peers_and_metadata.clone(),
            )
        });
        let consensus_interfaces = create_network_interfaces(
            consensus_network_handles,
            consensus_network_configuration(&node_config),
            peers_and_metadata.clone(),
        );
        let mempool_interfaces = create_network_interfaces(
            mempool_network_handles,
            mempool_network_configuration(&node_config),
            peers_and_metadata.clone(),
        );

        let dkg_interfaces = dkg_network_handle.map(|handle| {
            create_network_interfaces(
                vec![handle],
                dkg_network_configuration(&node_config),
                peers_and_metadata.clone(),
            )
        });

        let state_sync_config = node_config.state_sync;
        // The consensus_listener would listenes the request sent by ExecutionProxy's commit
        // function And then send NotifyCommit request to mempool which is named
        // consensus_to_mempool_sender in Gravity
        let (consensus_notifier, consensus_listener) =
            gaptos::aptos_consensus_notifications::new_consensus_notifier_listener_pair(
                state_sync_config.state_sync_driver.commit_notification_timeout_ms,
            );

        // Start the node inspection service
        start_node_inspection_service(&node_config, peers_and_metadata.clone());
        let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
        let (notification_sender, notification_receiver) = mpsc::channel(1);

        let mempool_listener =
            gaptos::aptos_mempool_notifications::MempoolNotificationListener::new(
                notification_receiver,
            );
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
            pool,
        );
        runtimes.extend(mempool_runtime);

        // Create shared VTxn pool first
        let vtxn_pool = VTxnPoolState::default();

        // Create DKG runtime with shared VTxn pool
        let dkg_runtime = create_dkg_runtime(
            &mut node_config.clone(), 
            &mut event_subscription_service, 
            dkg_interfaces,
            &vtxn_pool
        );
        if let Some(dkg_runtime) = dkg_runtime {
            runtimes.push(dkg_runtime);
        }
        if let Some(jwk_consensus_interfaces) = jwk_consensus_interfaces {
            let jwk_consensus_runtime = init_jwk_consensus(
                &node_config,
                &mut event_subscription_service,
                jwk_consensus_interfaces,
                &vtxn_pool,
            );
            runtimes.push(jwk_consensus_runtime);
        }
        init_block_buffer_manager(&consensus_db, latest_block_number).await;
        let mut args = ConsensusAdapterArgs::new(consensus_db);
        let (consensus_runtime, _, _) = start_consensus(
            &node_config,
            &mut event_subscription_service,
            consensus_interfaces,
            consensus_notifier,
            consensus_to_mempool_sender,
            db,
            &mut args,
            vtxn_pool,
        );
        runtimes.push(consensus_runtime);
        // Create notification senders and listeners for mempool, consensus and the storage service
        // For Gravity we only use it to notify the mempool for the committed txn gc logic
        let mempool_notifier =
            gaptos::aptos_mempool_notifications::MempoolNotifier::new(notification_sender);
        let mempool_notification_handler = MempoolNotificationHandler::new(mempool_notifier);
        let event_subscription_service = Arc::new(Mutex::new(event_subscription_service));
        let mut consensus_mempool_handler = ConsensusToMempoolHandler::new(
            mempool_notification_handler,
            consensus_listener,
            event_subscription_service.clone(),
        );
        let runtime = gaptos::aptos_runtimes::spawn_named_runtime("Con2Mempool".into(), None);
        runtime.spawn(async move {
            consensus_mempool_handler.start().await;
        });
        runtimes.push(runtime);
        // trigger this to make epoch manager invoke new epoch
        let args = HttpsServerArgs {
            address: node_config.https_server_address,
            cert_pem: node_config
                .https_cert_pem_path
                .clone()
                .to_str()
                .filter(|s| !s.is_empty())
                .map(|_| node_config.https_cert_pem_path),
            key_pem: node_config
                .https_key_pem_path
                .clone()
                .to_str()
                .filter(|s| !s.is_empty())
                .map(|_| node_config.https_key_pem_path),
        };
        let runtime = gaptos::aptos_runtimes::spawn_named_runtime("Http".into(), None);
        runtime.spawn(async move { https_server(args) });
        runtimes.push(runtime);
        let arc_consensus_engine = Arc::new(Self { runtimes });
        // process new round should be after init retƒh hash
        info!("pass latest_block_number: {:?} to event_subscription_service", latest_block_number);
        let _ = event_subscription_service.lock().await.notify_initial_configs(latest_block_number);
        arc_consensus_engine
    }
}
