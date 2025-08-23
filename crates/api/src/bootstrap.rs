use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use crate::network::{
    build_network_interfaces, consensus_network_configuration, extract_network_ids,
    mempool_network_configuration,
};
use aptos_consensus::consensusdb::{BlockNumberSchema, BlockSchema, ConsensusDB};
use aptos_consensus::{
    gravity_state_computer::ConsensusAdapterArgs, network_interface::ConsensusMsg,
    persistent_liveness_storage::StorageWriteProxy, quorum_store::quorum_store_db::QuorumStoreDB,
};

use block_buffer_manager::{get_block_buffer_manager, TxPool};
use gaptos::{
    api_types::u256_define::BlockId,
    aptos_event_notifications::{
        DbBackedOnChainConfig, EventNotificationListener, ReconfigNotificationListener,
    },
    aptos_logger::info,
};
use gaptos::{
    aptos_channels::{aptos_channel, message_queues::QueueStyle},
    aptos_config::{
        config::{NetworkConfig, NodeConfig, Peer, PeerRole},
        network_id::NetworkId,
    },
    aptos_network::{
        protocols::network::{NetworkApplicationConfig, NetworkClientConfig, NetworkServiceConfig},
        ProtocolId,
    },
};

use aptos_mempool::{MempoolClientRequest, MempoolSyncMsg, QuorumStoreRequest};
use futures::channel::mpsc::{Receiver, Sender};
use gaptos::aptos_consensus_notifications::ConsensusNotifier;
use gaptos::aptos_crypto::{hash::GENESIS_BLOCK_ID, x25519, HashValue};
use gaptos::aptos_event_notifications::EventSubscriptionService;
use gaptos::aptos_mempool_notifications::MempoolNotificationListener;
use gaptos::aptos_network::application::{
    interface::{NetworkClient, NetworkServiceEvents},
    storage::PeersAndMetadata,
};
use gaptos::aptos_network_builder::builder::NetworkBuilder;
use gaptos::aptos_storage_interface::DbReaderWriter;
use gaptos::aptos_types::account_address::AccountAddress;
use gaptos::aptos_validator_transaction_pool::VTxnPoolState;
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

pub fn jwk_consensus_network_configuration(node_config: &NodeConfig) -> NetworkApplicationConfig {
    let direct_send_protocols: Vec<ProtocolId> =
        gaptos::aptos_jwk_consensus::network_interface::DIRECT_SEND.into();
    let rpc_protocols: Vec<ProtocolId> = gaptos::aptos_jwk_consensus::network_interface::RPC.into();

    let network_client_config =
        NetworkClientConfig::new(direct_send_protocols.clone(), rpc_protocols.clone());
    let network_service_config = NetworkServiceConfig::new(
        direct_send_protocols,
        rpc_protocols,
        aptos_channel::Config::new(node_config.jwk_consensus.max_network_channel_size)
            .queue_style(QueueStyle::FIFO),
    );
    NetworkApplicationConfig::new(network_client_config, network_service_config)
}

pub fn init_network_interfaces<T, E, J>(
    network_builder: &mut NetworkBuilder,
    network_id: NetworkId,
    network_config: &NetworkConfig,
    node_config: &NodeConfig,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> (
    ApplicationNetworkInterfaces<T>,
    ApplicationNetworkInterfaces<E>,
    ApplicationNetworkInterfaces<J>,
)
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
    E: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
    J: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
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
    let jwk_consensus_network_interfaces = build_network_interfaces::<J>(
        network_builder,
        network_id,
        &network_config,
        jwk_consensus_network_configuration(node_config),
        peers_and_metadata.clone(),
    );
    (consensus_network_interfaces, mempool_interfaces, jwk_consensus_network_interfaces)
}

/// Spawns a new thread for the node inspection service
pub fn start_node_inspection_service(
    node_config: &NodeConfig,
    peers_and_metadata: Arc<PeersAndMetadata>,
) {
    gaptos::aptos_inspection_service::start_inspection_service(
        node_config.clone(),
        None,
        peers_and_metadata,
    )
}

pub fn start_consensus(
    node_config: &NodeConfig,
    event_subscription_service: &mut EventSubscriptionService,
    consensus_network_interfaces: ApplicationNetworkInterfaces<ConsensusMsg>,
    consensus_notifier: ConsensusNotifier,
    consensus_to_mempool_sender: Sender<QuorumStoreRequest>,
    db: DbReaderWriter,
    arg: &mut ConsensusAdapterArgs,
    vtxn_pool: VTxnPoolState,
) -> (Runtime, Arc<StorageWriteProxy>, Arc<QuorumStoreDB>) {
    let consensus_reconfig_subscription = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("Consensus must subscribe to reconfigurations");
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

pub fn start_jwk_consensus_runtime(
    node_config: &NodeConfig,
    jwk_consensus_subscriptions: Option<(
        ReconfigNotificationListener<DbBackedOnChainConfig>,
        EventNotificationListener,
    )>,
    jwk_consensus_network_interfaces: Option<
        ApplicationNetworkInterfaces<gaptos::aptos_jwk_consensus::types::JWKConsensusMsg>,
    >,
) -> (Runtime, VTxnPoolState) {
    let vtxn_pool = VTxnPoolState::default();
    let jwk_consensus_runtime = match jwk_consensus_network_interfaces {
        Some(interfaces) => {
            let ApplicationNetworkInterfaces { network_client, network_service_events } =
                interfaces;
            let (reconfig_events, onchain_jwk_updated_events) = jwk_consensus_subscriptions.expect(
                "JWK consensus needs to listen to NewEpochEvents and OnChainJWKMapUpdated events.",
            );
            let my_addr = node_config.validator_network.as_ref().unwrap().peer_id();
            let jwk_consensus_runtime = gaptos::aptos_jwk_consensus::start_jwk_consensus_runtime(
                my_addr,
                &node_config.consensus.safety_rules,
                network_client,
                network_service_events,
                reconfig_events,
                onchain_jwk_updated_events,
                vtxn_pool.clone(),
            );
            Some(jwk_consensus_runtime)
        }
        _ => None,
    };
    (jwk_consensus_runtime.expect("JWK consensus runtime must be started"), vtxn_pool)
}

pub fn init_jwk_consensus(
    node_config: &NodeConfig,
    event_subscription_service: &mut EventSubscriptionService,
    jwk_consensus_network_interfaces: ApplicationNetworkInterfaces<
        gaptos::aptos_jwk_consensus::types::JWKConsensusMsg,
    >,
) -> (Runtime, VTxnPoolState) {
    // TODO(gravity): only valdiator should subscribe the reconf events
    let reconfig_events = event_subscription_service
        .subscribe_to_reconfigurations()
        .expect("JWK consensus must subscribe to reconfigurations");
    let jwk_updated_events = event_subscription_service
        .subscribe_to_events(vec![], vec!["0x1::jwks::ObservedJWKsUpdated".to_string()])
        .expect("JWK consensus must subscribe to DKG events");
    start_jwk_consensus_runtime(
        node_config,
        Some((reconfig_events, jwk_updated_events)),
        Some(jwk_consensus_network_interfaces),
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
    pool: Box<dyn TxPool>,
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
        pool,
    )
}

pub fn init_peers_and_metadata(
    node_config: &NodeConfig,
    consensus_db: &Arc<ConsensusDB>,
) -> Arc<PeersAndMetadata> {
    let network_ids = extract_network_ids(node_config);
    let peers_and_metadata = PeersAndMetadata::new(&network_ids);
    peers_and_metadata
}

pub async fn init_block_buffer_manager(consensus_db: &Arc<ConsensusDB>, latest_block_number: u64) {
    let start_block_number = if latest_block_number > RECENT_BLOCKS_RANGE {
        latest_block_number - RECENT_BLOCKS_RANGE
    } else {
        0
    };

    let max_epoch = consensus_db.get_max_epoch();

    let mut block_number_to_block_id = HashMap::new();
    for epoch_i in (1..=max_epoch).rev() {
        let mut has_large = false;
        let start_key = (epoch_i, HashValue::zero());
        let end_key = (epoch_i, HashValue::new([u8::MAX; HashValue::LENGTH]));
        consensus_db
            .get_range_with_filter::<BlockNumberSchema, _>(&start_key, &end_key, |(_, block_number)| *block_number >= start_block_number && *block_number <= latest_block_number)
            .unwrap()
            .into_iter()
            .for_each(|((epoch, block_id), block_number)| {
                has_large = true;
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
        if !has_large {
            break;
        }
    }
    info!("init_block_buffer_manager get_max_epoch {}", max_epoch);

    let mut block_number_to_block_id: HashMap<_, _> = block_number_to_block_id
        .into_iter()
        .map(|(block_number, (_, block_id))| (block_number, block_id))
        .collect();
    if start_block_number == 0 {
        block_number_to_block_id.insert(0u64, BlockId::from_bytes(GENESIS_BLOCK_ID.as_slice()));
    }
    get_block_buffer_manager().init(latest_block_number, block_number_to_block_id).await;
}
