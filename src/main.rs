mod mock_db;
mod network;
mod services;
use std::{
    borrow::Borrow, collections::{HashMap, HashSet}, env, path::PathBuf, sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }, thread
};

use aptos_config::{
    config::{
        DiscoveryMethod, FileDiscovery, Identity, InitialSafetyRulesConfig, NetworkConfig, NodeConfig, OnDiskStorageConfig, Peer, PeerRole, SecureBackend, WaypointConfig
    },
    network_id::{NetworkId, PeerNetworkId},
};
use aptos_crypto::x25519;
use aptos_event_notifications::EventNotificationSender;
use aptos_infallible::RwLock;
use aptos_mempool::MempoolSyncMsg;
use aptos_network::{
    application::{
        interface::{NetworkClient, NetworkClientInterface, NetworkServiceEvents},
        storage::PeersAndMetadata,
    },
    protocols::direct_send::Message,
    transport::ConnectionMetadata,
    ProtocolId,
};
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use aptos_types::chain_id::ChainId;
use aptos_validator_transaction_pool::VTxnPoolState;

use futures::{channel::mpsc, StreamExt};
use mock_db::{ACCOUNT_ADDRESS_MAP, NETWORK_MAP, NETWORK_PUB_MAP, TRUSTED_PEERS_MAP};
use network::{
    build_network_interfaces, consensus_network_configuration, create_network_runtime,
    extract_network_configs, extract_network_ids, mempool_network_configuration,
};
use tokio::time::sleep;

pub struct ApplicationNetworkInterfaces<T> {
    pub network_client: NetworkClient<T>,
    pub network_service_events: NetworkServiceEvents<T>,
}

pub fn create_peers_and_metadata(node_config: &NodeConfig) -> Arc<PeersAndMetadata> {
    let network_ids = extract_network_ids(node_config);
    PeersAndMetadata::new(&network_ids)
}

use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "template_config")]
    config_path: String,
    #[clap(long, default_value = "data")]
    data_path: String,
    #[clap(long, default_value = "127.0.0.1")]
    ip: String,
    #[clap(long, default_value = "6180")]
    port: String,
    #[clap(long, default_value = "")]
    discovery_path: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let current_dir = env!("CARGO_MANIFEST_DIR").to_string();
    let config_path = current_dir.clone() + "/" + &cli.config_path;
    let data_path = current_dir.clone() + "/" + &cli.data_path;
    let listen_address = format!("/ip4/{}/tcp/{}", &cli.ip, &cli.port);

    let mut node_config = aptos_config::config::NodeConfig::default();
    node_config.validator_network = Some(NetworkConfig::network_with_id(NetworkId::Validator));
    node_config.validator_network.as_mut().unwrap().identity = Identity::from_config(
        x25519::PrivateKey::try_from(mock_db::NETWORK_MAP.get(&listen_address).unwrap().as_slice()).unwrap(),
        *mock_db::ACCOUNT_ADDRESS_MAP.get(&listen_address).unwrap(),
    );
    node_config
        .validator_network
        .as_mut()
        .unwrap()
        .reset_listen_address_and_prepare_identity(listen_address.parse().unwrap());
    node_config
        .consensus
        .safety_rules
        .initial_safety_rules_config = InitialSafetyRulesConfig::FromFile {
        identity_blob_path: PathBuf::from(config_path.clone() + "/validator-identity.yaml"),
        waypoint: WaypointConfig::FromFile(PathBuf::from(config_path.clone() + "/waypoint.txt")),
    };
    if !cli.discovery_path.is_empty() {
        node_config
            .validator_network
            .as_mut()
            .unwrap()
            .discovery_method = DiscoveryMethod::File(FileDiscovery {
            path: (current_dir.clone() + "/" + &cli.discovery_path).into(),
            interval_secs: 360,
        })
    }
    let secure_backend_path = PathBuf::from(data_path.clone());
    let mut on_disk_storage_config = OnDiskStorageConfig::default();
    on_disk_storage_config.set_data_dir(secure_backend_path);
    node_config.consensus.safety_rules.backend =
        SecureBackend::OnDiskStorage(on_disk_storage_config);
    let backend = &node_config.consensus.safety_rules.backend;
    let chain_id = ChainId::test();
    let db: DbReaderWriter = DbReaderWriter::new(mock_db::MockStorage::new(listen_address.clone()));
    let peers_and_metadata = create_peers_and_metadata(&node_config);
    let mut peer_set = HashMap::new();
    for trusted_peer in TRUSTED_PEERS_MAP.get(&listen_address).unwrap() {
        let mut set = HashSet::new();
        let trusted_peer_private_key = x25519::PrivateKey::try_from(NETWORK_MAP.get(trusted_peer).unwrap().as_slice()).unwrap();
        set.insert(x25519::PublicKey::try_from(&trusted_peer_private_key).unwrap());
        let trust_peer = Peer::new(vec![trusted_peer.parse().unwrap()],
        set, PeerRole::Validator);
        peer_set.insert(*ACCOUNT_ADDRESS_MAP.get(trusted_peer).unwrap(), trust_peer);
    }
    peers_and_metadata.set_trusted_peers(&NetworkId::Validator, peer_set);
    let mut event_subscription_service =
        aptos_event_notifications::EventSubscriptionService::new(Arc::new(RwLock::new(db.clone())));
    let network_configs = extract_network_configs(&node_config);

    node_config.storage.dir = PathBuf::from(data_path.clone());
    let network_config = network_configs.get(0).unwrap();
    let mut network_builder = NetworkBuilder::create(
        chain_id,
        node_config.base.role,
        &network_config,
        aptos_time_service::TimeService::real(),
        Some(&mut event_subscription_service),
        peers_and_metadata.clone(),
    );
    let network_id: NetworkId = network_config.network_id;
    let consensus_network_interfaces = build_network_interfaces(
        &mut network_builder,
        network_id,
        &network_config,
        consensus_network_configuration(&node_config),
        peers_and_metadata.clone(),
    );
    let mempool_interfaces = build_network_interfaces(
        &mut network_builder,
        network_id,
        &network_config,
        mempool_network_configuration(&node_config),
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
    tokio::spawn(async move {
        network::mock_mempool_client_sender(mempool_client_sender).await;
    });
    let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
    let (notification_sender, notification_receiver) = mpsc::channel(1);

    let mempool_notifier = aptos_mempool_notifications::MempoolNotifier::new(notification_sender);
    let mempool_listener =
        aptos_mempool_notifications::MempoolNotificationListener::new(notification_receiver);

    let mempool_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Mempool must subscribe to reconfigurations");
    let mempool = aptos_mempool::bootstrap(
        &node_config,
        Arc::clone(&db.reader),
        mempool_interfaces.network_client,
        mempool_interfaces.network_service_events,
        mempool_client_receiver,
        consensus_to_mempool_receiver,
        mempool_listener,
        mempool_reconfig_subscription,
        peers_and_metadata,
    );
    let consensus_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Consensus must subscribe to reconfigurations");
    let vtxn_pool = VTxnPoolState::default();
    let consensus = aptos_consensus::consensus_provider::start_consensus(
        &node_config,
        consensus_network_interfaces.network_client,
        consensus_network_interfaces.network_service_events, // 这个network_service_events会在coordinator的那个(network_id, event) = events.select_next_some()上用到
        Arc::new(consensus_notifier),
        consensus_to_mempool_sender,
        db.clone(),
        consensus_reconfig_subscription,
        vtxn_pool,
        None,
    );
    let _ = event_subscription_service.notify_initial_configs(1_u64);
    loop {
        thread::park();
    }
}
