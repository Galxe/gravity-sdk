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
use aptos_event_notifications::EventSubscriptionService;
use aptos_mempool::{MempoolClientRequest, MempoolSyncMsg, QuorumStoreRequest};
use aptos_mempool_notifications::MempoolNotificationListener;
use aptos_network::application::{
    interface::{NetworkClient, NetworkServiceEvents},
    storage::PeersAndMetadata,
};
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use aptos_types::account_address::AccountAddress;
use aptos_validator_transaction_pool::VTxnPoolState;
use futures::channel::mpsc::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio::{runtime::Runtime, sync::Mutex};

use crate::{
    consensus_engine::GravityConsensusEngine,
    network::{
        self, build_network_interfaces, consensus_network_configuration, extract_network_ids,
        mempool_network_configuration,
    },
    storage::{self, db::GravityDB},
    GravityConsensusEngineInterface,
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
pub fn start(node_config: NodeConfig) -> anyhow::Result<()> {
    let test_mode = node_config.test_mode;
    let adapter = GravityConsensusEngine::init(node_config);
    let submitter_mutex = adapter.clone();
    if test_mode {
        tokio::spawn(async move {
            network::mock_execution_txn_submitter(submitter_mutex).await;
        });
        tokio::spawn(async move {
            network::mock_execution_receive_block(adapter).await;
        });
    }
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
) -> (ApplicationNetworkInterfaces<T>, ApplicationNetworkInterfaces<E>)
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
    arg: &mut ConsensusAdapterArgs,
) -> (Runtime, Arc<StorageWriteProxy>, Arc<QuorumStoreDB>) {
    let consensus_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Consensus must subscribe to reconfigurations");
    let vtxn_pool = VTxnPoolState::default();
    // TODO(gravity_byteyue: return quorum store client also)
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
    mempool_interfaces: ApplicationNetworkInterfaces<MempoolSyncMsg>,
    mempool_client_receiver: Receiver<MempoolClientRequest>,
    consensus_to_mempool_receiver: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> Runtime {
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
) -> Arc<PeersAndMetadata> {
    let listen_address = node_config.validator_network.as_ref().unwrap().listen_address.to_string();
    let gravity_node_config = gravity_db.mock_db.node_config_set.get(&listen_address).unwrap();
    let network_ids = extract_network_ids(node_config);
    let peers_and_metadata = PeersAndMetadata::new(&network_ids);
    let mut peer_set = HashMap::new();
    for trusted_peer in &gravity_node_config.trusted_peers_map {
        let trusted_peer_config = gravity_db.mock_db.node_config_set.get(trusted_peer).unwrap();
        let mut set = HashSet::new();
        let trusted_peer_private_key =
            x25519::PrivateKey::try_from(trusted_peer_config.network_private_key.as_slice())
                .unwrap();
        set.insert(x25519::PublicKey::from(&trusted_peer_private_key));
        let trust_peer = Peer::new(vec![trusted_peer.parse().unwrap()], set, PeerRole::Validator);
        peer_set.insert(
            AccountAddress::try_from(trusted_peer_config.account_address.clone()).unwrap(),
            trust_peer,
        );
    }
    let _ = peers_and_metadata.set_trusted_peers(&NetworkId::Validator, peer_set);
    peers_and_metadata
}

pub fn init_gravity_db(node_config: &NodeConfig) -> GravityDB {
    let listen_address = node_config.validator_network.as_ref().unwrap().listen_address.to_string();
    let mut db_paths = node_config.storage.dir();
    db_paths.push("gravity_db");
    let _ = fs::create_dir(&db_paths);
    let db_paths = StorageDirPaths::from_path(db_paths);
    storage::db::GravityDB::open(
        &db_paths,
        RocksdbConfigs::default(),
        listen_address,
        &node_config.mock_db_path.as_path(),
    )
    .unwrap()
}
