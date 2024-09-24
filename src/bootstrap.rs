use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Arc,
    thread,
};

use aptos_config::{
    config::{NetworkConfig, NodeConfig, Peer, PeerRole, RocksdbConfigs, StorageDirPaths},
    network_id::NetworkId,
};
use aptos_consensus::{
    gravity_state_computer::ConsensusAdapterArgs, network_interface::ConsensusMsg,
    persistent_liveness_storage::StorageWriteProxy, quorum_store::quorum_store_db::QuorumStoreDB,
};
use aptos_consensus_notifications::ConsensusNotifier;
use aptos_crypto::x25519;
use aptos_event_notifications::{EventNotificationSender, EventSubscriptionService};
use aptos_infallible::RwLock;
use aptos_mempool::{MempoolClientRequest, MempoolSyncMsg, QuorumStoreRequest};
use aptos_mempool_notifications::MempoolNotificationListener;
use aptos_network::application::{
    interface::{NetworkClient, NetworkServiceEvents},
    storage::PeersAndMetadata,
};
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use aptos_types::{account_address::AccountAddress, chain_id::ChainId};
use aptos_validator_transaction_pool::VTxnPoolState;
use futures::channel::mpsc::{self, Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

use crate::{
    consensus_engine::GravityConsensusEngine,
    mock_db::GravityNodeConfig,
    network::{
        self, build_network_interfaces, consensus_network_configuration, create_network_runtime,
        extract_network_configs, extract_network_ids, mempool_network_configuration,
    },
    storage::{self, db::GravityDB},
};
pub struct ApplicationNetworkInterfaces<T> {
    pub network_client: NetworkClient<T>,
    pub network_service_events: NetworkServiceEvents<T>,
}

pub fn check_bootstrap_config(node_config_path: Option<PathBuf>) -> NodeConfig {
    // Get the config file path
    let config_path = node_config_path.expect("Config is required to launch node");
    if !config_path.exists() {
        panic!(
            "The node config file could not be found! Ensure the given path is correct: {:?}",
            config_path.display()
        )
    }

    // A config file exists, attempt to parse the config
    NodeConfig::load_from_path(config_path.clone()).unwrap_or_else(|error| {
        panic!(
            "Failed to load the node config file! Given file path: {:?}. Error: {:?}",
            config_path.display(),
            error
        )
    })
}

// Start an Gravity node
pub fn start(node_config: NodeConfig, mockdb_config_path: Option<PathBuf>) -> anyhow::Result<()> {
    let listen_address = node_config
        .validator_network
        .as_ref()
        .unwrap()
        .listen_address
        .to_string();
    let db_paths = node_config.storage.dir();
    let gravity_db = init_gravity_db(listen_address.clone(), db_paths, mockdb_config_path);
    let gravity_node_config = gravity_db
        .mock_db
        .node_config_set
        .get(&listen_address)
        .unwrap();
    let peers_and_metadata =
        init_peers_and_metadata(&node_config, &gravity_db, &gravity_node_config);
    let db: DbReaderWriter = DbReaderWriter::new(gravity_db);
    let mut event_subscription_service =
        aptos_event_notifications::EventSubscriptionService::new(Arc::new(RwLock::new(db.clone())));
    let network_configs = extract_network_configs(&node_config);

    let network_config = network_configs.get(0).unwrap();
    let chain_id = ChainId::test();
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
    let (consensus_notifier, consensus_listener) =
        aptos_consensus_notifications::new_consensus_notifier_listener_pair(
            state_sync_config
                .state_sync_driver
                .commit_notification_timeout_ms,
        );
    let mut network_runtimes = vec![];
    // Create a network runtime for the config
    let runtime = create_network_runtime(&network_config);
    // Build and start the network on the runtime
    network_builder.build(runtime.handle().clone());
    network_builder.start();
    network_runtimes.push(runtime);

    // TODO(Gravity_byteyue): delete the following comment
    // 这里要看aptos代码的setup_environment_and_start_node函数下的start_mempool_runtime_and_get_consensus_sender的逻辑，不然这里channel好像对不上都
    // start consensus确实是用consensus_to_mempool_receiver，但是在setup_environment_and_start_node才有Receiver<MempoolClientRequest>
    // setup_environment_and_start_node 调用了 bootstrap_api_and_indexer ，在其中构造了 mempool_client_sender 和 mempool_client_receiver, 然后 bootstrap_api_and_indexer
    // 返回了 receiver, 接下来 setup_environment_and_start_node 把 receiver 传递给 start_mempool_runtime_and_get_consensus_sender ,
    // 在其中构造了 consensus_to_mempool_sender 和 consensus_to_mempool_receiver
    // 并返回了sender
    let (mempool_client_sender, mempool_client_receiver) = mpsc::channel(1);

    let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
    let (notification_sender, notification_receiver) = mpsc::channel(1);

    let _mempool_notifier = aptos_mempool_notifications::MempoolNotifier::new(notification_sender);
    let mempool_listener =
        aptos_mempool_notifications::MempoolNotificationListener::new(notification_receiver);

    init_mempool(
        &node_config,
        &db,
        &mut event_subscription_service,
        mempool_client_sender.clone(),
        mempool_interfaces,
        mempool_client_receiver,
        consensus_to_mempool_receiver,
        mempool_listener,
        peers_and_metadata,
    );
    let mut arg = ConsensusAdapterArgs::new(mempool_client_sender);
    let adapter = GravityConsensusEngine::new(&mut arg);
    start_consensus(
        &node_config,
        &mut event_subscription_service,
        consensus_network_interfaces,
        consensus_notifier,
        consensus_to_mempool_sender,
        db,
        arg,
    );
    let _ = event_subscription_service.notify_initial_configs(1_u64);
    tokio::spawn(async move {
        network::mock_execution_txn_submitter(adapter).await;
    });
    loop {
        thread::park();
    }
}

pub fn init_network_interfaces<T, E>(
    network_builder: &mut NetworkBuilder,
    network_id: NetworkId,
    network_config: &NetworkConfig,
    node_config: &NodeConfig,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> (
    ApplicationNetworkInterfaces<T>,
    ApplicationNetworkInterfaces<E>,
)
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
    E: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
    let consensus_network_interfaces = build_network_interfaces::<T>(
        network_builder,
        network_id,
        &network_config,
        consensus_network_configuration(node_config),
        peers_and_metadata.clone(),
    );
    let mempool_interfaces = build_network_interfaces::<E>(
        network_builder,
        network_id,
        &network_config,
        mempool_network_configuration(node_config),
        peers_and_metadata.clone(),
    );
    (consensus_network_interfaces, mempool_interfaces)
}

pub fn start_consensus(
    node_config: &NodeConfig,
    event_subscription_service: &mut EventSubscriptionService,
    consensus_network_interfaces: ApplicationNetworkInterfaces<ConsensusMsg>,
    consensus_notifier: ConsensusNotifier,
    consensus_to_mempool_sender: Sender<QuorumStoreRequest>,
    db: DbReaderWriter,
    arg: ConsensusAdapterArgs,
) -> (Runtime, Arc<StorageWriteProxy>, Arc<QuorumStoreDB>) {
    let consensus_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Consensus must subscribe to reconfigurations");
    let vtxn_pool = VTxnPoolState::default();
    aptos_consensus::consensus_provider::start_consensus(
        &node_config,
        consensus_network_interfaces.network_client,
        consensus_network_interfaces.network_service_events, // 这个network_service_events会在coordinator的那个(network_id, event) = events.select_next_some()上用到
        Arc::new(consensus_notifier),
        consensus_to_mempool_sender,
        db.clone(),
        consensus_reconfig_subscription,
        vtxn_pool,
        None,
        arg,
    )
}

pub fn init_mempool(
    node_config: &NodeConfig,
    db: &DbReaderWriter,
    event_subscription_service: &mut EventSubscriptionService,
    mempool_client_sender: Sender<MempoolClientRequest>,
    mempool_interfaces: ApplicationNetworkInterfaces<MempoolSyncMsg>,
    mempool_client_receiver: Receiver<MempoolClientRequest>,
    consensus_to_mempool_receiver: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> Runtime {
    (0..5).for_each(|_| {
        let s = mempool_client_sender.clone();
        tokio::spawn(async move {
            network::mock_mempool_client_sender(s).await;
        });
    });

    let mempool_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Mempool must subscribe to reconfigurations");
    aptos_mempool::bootstrap(
        &node_config,
        Arc::clone(&db.reader),
        mempool_interfaces.network_client,
        mempool_interfaces.network_service_events,
        mempool_client_receiver,
        consensus_to_mempool_receiver,
        mempool_listener,
        mempool_reconfig_subscription,
        peers_and_metadata,
    )
}

pub fn init_peers_and_metadata(
    node_config: &NodeConfig,
    gravity_db: &GravityDB,
    gravity_node_config: &GravityNodeConfig,
) -> Arc<PeersAndMetadata> {
    let network_ids = extract_network_ids(node_config);
    let peers_and_metadata = PeersAndMetadata::new(&network_ids);
    let mut peer_set = HashMap::new();
    for trusted_peer in &gravity_node_config.trusted_peers_map {
        let trusted_peer_config = gravity_db
            .mock_db
            .node_config_set
            .get(trusted_peer)
            .unwrap();
        let mut set = HashSet::new();
        let trusted_peer_private_key =
            x25519::PrivateKey::try_from(trusted_peer_config.network_private_key.as_slice())
                .unwrap();
        set.insert(x25519::PublicKey::try_from(&trusted_peer_private_key).unwrap());
        let trust_peer = Peer::new(
            vec![trusted_peer.parse().unwrap()],
            set,
            PeerRole::Validator,
        );
        peer_set.insert(
            AccountAddress::try_from(trusted_peer_config.account_address.clone()).unwrap(),
            trust_peer,
        );
    }
    let _ = peers_and_metadata.set_trusted_peers(&NetworkId::Validator, peer_set);
    peers_and_metadata
}

pub fn init_gravity_db(
    listen_address: String,
    mut db_paths: PathBuf,
    mockdb_config_path: Option<PathBuf>,
) -> GravityDB {
    db_paths.push("gravity_db");
    let _ = fs::create_dir(&db_paths);
    let db_paths = StorageDirPaths::from_path(db_paths);
    storage::db::GravityDB::open(
        &db_paths,
        RocksdbConfigs::default(),
        listen_address,
        mockdb_config_path.unwrap().as_path(),
    )
    .unwrap()
}
