use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use crate::network::{build_network_interfaces, consensus_network_configuration, extract_network_ids, mempool_network_configuration};
use gaptos::api_types::u256_define::BlockId;
use block_buffer_manager::get_block_buffer_manager;
use gaptos::aptos_config::{
    config::{NetworkConfig, NodeConfig, Peer, PeerRole},
    network_id::NetworkId,
};
use aptos_consensus::consensusdb::{BlockNumberSchema, BlockSchema, ConsensusDB};
use aptos_consensus::{
    gravity_state_computer::ConsensusAdapterArgs, network_interface::ConsensusMsg,
    persistent_liveness_storage::StorageWriteProxy, quorum_store::quorum_store_db::QuorumStoreDB,
};

use gaptos::aptos_consensus_notifications::ConsensusNotifier;
use gaptos::aptos_crypto::{hash::GENESIS_BLOCK_ID, x25519, HashValue};
use gaptos::aptos_event_notifications::EventSubscriptionService;
use aptos_mempool::{MempoolClientRequest, MempoolSyncMsg, QuorumStoreRequest};
use gaptos::aptos_mempool_notifications::MempoolNotificationListener;
use gaptos::aptos_network::application::{
    interface::{NetworkClient, NetworkServiceEvents},
    storage::PeersAndMetadata,
};
use gaptos::aptos_network_builder::builder::NetworkBuilder;
use gaptos::aptos_storage_interface::DbReaderWriter;
use gaptos::aptos_types::account_address::AccountAddress;
use gaptos::aptos_validator_transaction_pool::VTxnPoolState;
use futures::channel::mpsc::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio::{runtime::Runtime, sync::Mutex};

const RECENT_BLOCKS_RANGE: u64 = 256;

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

/// Spawns a new thread for the node inspection service
pub fn start_node_inspection_service(
    node_config: &NodeConfig,
    peers_and_metadata: Arc<PeersAndMetadata>,
) {
    gaptos::aptos_inspection_service::start_inspection_service(node_config.clone(), None, peers_and_metadata)
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
        consensus_network_interfaces.network_service_events,
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
    _mempool_client_receiver: Receiver<MempoolClientRequest>,
    consensus_to_mempool_receiver: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> Vec<Runtime> {
    let mempool_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Mempool must subscribe to reconfigurations");
    aptos_mempool::bootstrap(
        &node_config,
        Arc::clone(&db.reader),
        mempool_interfaces.network_client,
        mempool_interfaces.network_service_events,
        _mempool_client_receiver,
        consensus_to_mempool_receiver,
        mempool_listener,
        mempool_reconfig_subscription,
        peers_and_metadata,
    )
}

pub fn init_peers_and_metadata(
    node_config: &NodeConfig,
    consensus_db: &Arc<ConsensusDB>,
) -> Arc<PeersAndMetadata> {
    let listen_address = node_config.validator_network.as_ref().unwrap().listen_address.to_string();
    let gravity_node_config = consensus_db
        .node_config_set
        .get(&listen_address)
        .expect(&format!("addr {:?} has no config", listen_address));
    let network_ids = extract_network_ids(node_config);
    let peers_and_metadata = PeersAndMetadata::new(&network_ids);
    let mut peer_set = HashMap::new();
    for trusted_peer in &gravity_node_config.trusted_peers_map {
        let trusted_peer_config = consensus_db
            .node_config_set
            .get(trusted_peer)
            .or_else(|| {
                consensus_db
                    .node_config_set
                    .iter()
                    .map(|(_, config)| config)
                    .find(|config| config.public_ip_address == *trusted_peer)
            })
            .expect(&format!("NodeConfig for {:?} not found", trusted_peer));
        let mut set = HashSet::new();
        let public_key = x25519::PublicKey::try_from(
            hex::decode(trusted_peer_config.network_public_key.as_bytes()).unwrap().as_slice(),
        ).unwrap();
        set.insert(public_key);
        let trust_peer = Peer::new(vec![trusted_peer.parse().unwrap()], set, PeerRole::Validator);
        peer_set.insert(
            AccountAddress::try_from(trusted_peer_config.account_address.clone()).unwrap(),
            trust_peer,
        );
    }
    let _ = peers_and_metadata.set_trusted_peers(&NetworkId::Validator, peer_set);
    peers_and_metadata
}

pub async fn init_block_buffer_manager(
    consensus_db: &Arc<ConsensusDB>,
    latest_block_number: u64,
) {
    let start_block_number = if latest_block_number > RECENT_BLOCKS_RANGE {
        latest_block_number - RECENT_BLOCKS_RANGE
    } else {
        0
    };

    let mut block_number_to_block_id = HashMap::new();
    consensus_db
        .get_all::<BlockNumberSchema>()
        .unwrap()
        .into_iter()
        .filter(|(_, block_number)| block_number >= &start_block_number)
        .for_each(|((epoch, block_id), block_number)| {
            if !block_number_to_block_id.contains_key(&block_number) {
                block_number_to_block_id
                    .insert(block_number, (epoch, BlockId::from_bytes(block_id.as_slice())));
            } else {
                let (cur_epoch, _) = block_number_to_block_id.get(&block_number).unwrap();
                if *cur_epoch < epoch {
                    block_number_to_block_id
                        .insert(block_number, (epoch, BlockId::from_bytes(block_id.as_slice())));
                }
            }
        });
    let mut block_number_to_block_id: HashMap<_, _> = block_number_to_block_id
        .into_iter()
        .map(|(block_number, (_, block_id))| (block_number, block_id))
        .collect();
    if start_block_number == 0 {
        block_number_to_block_id.insert(0u64, BlockId::from_bytes(GENESIS_BLOCK_ID.as_slice()));
    }
    get_block_buffer_manager().init(latest_block_number, block_number_to_block_id).await;
}
