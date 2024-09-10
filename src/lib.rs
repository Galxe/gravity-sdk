mod mock_db;
mod network;

use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::Arc, thread};

use aptos_config::{config::{NodeConfig, Peer, PeerRole}, network_id::NetworkId};
use aptos_crypto::x25519;
use aptos_event_notifications::EventNotificationSender;
use aptos_infallible::RwLock;
use aptos_network::application::{interface::{NetworkClient, NetworkServiceEvents}, storage::PeersAndMetadata};
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use aptos_types::{account_address::AccountAddress, chain_id::ChainId};
use aptos_validator_transaction_pool::VTxnPoolState;
use clap::Parser;
use futures::channel::mpsc;
use network::{build_network_interfaces, consensus_network_configuration, create_network_runtime, extract_network_configs, extract_network_ids, mempool_network_configuration};

pub struct ApplicationNetworkInterfaces<T> {
    pub network_client: NetworkClient<T>,
    pub network_service_events: NetworkServiceEvents<T>,
}
/// GCEI: Gravity Consensus Engine Interface
///
/// This trait defines the interface for a consensus process engine.
/// It outlines the key operations that any consensus engine should implement
/// to participate in the blockchain consensus process.
pub trait GravityConsensusEngineInterface {
    /// Initialize the consensus engine.
    ///
    /// This function should be called when the consensus engine starts up.
    /// It may include tasks such as:
    /// - Setting up initial state
    /// - Connecting to the network
    /// - Loading configuration
    fn init();

    /// Receive and process valid transactions.
    ///
    /// This function is responsible for:
    /// - Accepting incoming transactions from the network or mempool
    /// - Validating the transactions
    /// - Adding valid transactions to the local transaction pool
    fn submit_valid_transactions();

    /// Poll for ordered blocks.
    ///
    /// This function should:
    /// - Check for new blocks that have been ordered by the consensus mechanism
    /// - Retrieve the ordered blocks
    /// - Prepare them for processing
    ///
    /// TODO(gravity_xiejian): use txn id rather than total txn in block
    /// Returns: Option<Block> - The next ordered block, if available
    fn polling_ordered_block();

    /// Submit computation results.
    ///
    /// After processing a block, this function should:
    /// - Package the results of any computations or state changes
    /// - Submit these results back to the consensus mechanism
    ///
    /// Parameters:
    /// - `result`: The computation result to be submitted
    fn submit_compute_res();

    /// Submit Block head.
    ///
    /// After processing a block, this function should:
    /// - Package the block head
    /// - Submit these results back to the consensus mechanism
    ///
    /// Parameters:
    /// - `result`: The computation result to be submitted
    fn submit_block_head();

    /// Commit batch finalized block IDs.
    ///
    /// This function is called when a block is finalized. It should:
    /// - Mark the specified blocks as finalized in the local state
    /// - Trigger any necessary callbacks or events related to block finalization
    ///
    /// Parameters:
    /// - `block_ids`: A vector of block IDs that have been finalized
    fn polling_commit_block_ids();

    /// Return the commit ids, the consensus can delete these transactions after submitting.
    fn submit_commit_block_ids();
}

/// Runs an Gravity validator or fullnode
#[derive(Clone, Debug, Parser)]
#[clap(name = "Gravity Node", author, version)]
pub struct GravityNodeArgs {
    #[clap(short = 'f', long)]
    /// Path to node configuration file (or template for local test mode).
    node_config_path: Option<PathBuf>,
    #[clap(long)]
    mockdb_config_path: Option<PathBuf>,
}

impl GravityNodeArgs {
    pub fn run(self) {
        // Get the config file path
        let config_path = self.node_config_path.expect("Config is required to launch node");
        if !config_path.exists() {
            panic!(
                "The node config file could not be found! Ensure the given path is correct: {:?}",
                config_path.display()
            )
        }

        // A config file exists, attempt to parse the config
        let config = NodeConfig::load_from_path(config_path.clone()).unwrap_or_else(|error| {
            panic!(
                "Failed to load the node config file! Given file path: {:?}. Error: {:?}",
                config_path.display(),
                error
            )
        });

        // Start the node
        start(config, self.mockdb_config_path).expect("Node should start correctly");
    }
}

pub fn create_peers_and_metadata(node_config: &NodeConfig) -> Arc<PeersAndMetadata> {
    let network_ids = extract_network_ids(node_config);
    PeersAndMetadata::new(&network_ids)
}

// Start an Gravity node
pub fn start(
    node_config: NodeConfig,
    mockdb_config_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let listen_address = node_config.validator_network.as_ref().unwrap().listen_address.to_string();
    let db = mock_db::MockStorage::new(listen_address.clone(), mockdb_config_path.unwrap().as_path());
    let gravity_node_config = db.node_config_set.get(&listen_address).unwrap();
    let peers_and_metadata = create_peers_and_metadata(&node_config);
    let mut peer_set = HashMap::new();
    for trusted_peer in &gravity_node_config.trusted_peers_map {
        let trusted_peer_config = db.node_config_set.get(trusted_peer).unwrap();
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
    let db: DbReaderWriter = DbReaderWriter::new(db);
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

    // tokio::spawn(async move {
    //     network::mock_mempool_client_sender(mempool_client_sender.clone()).await;
    // });
    (0..5).for_each(|_| {
        let s = mempool_client_sender.clone();
        tokio::spawn(async move {
            network::mock_mempool_client_sender(s).await;
        });
    });
    let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
    let (notification_sender, notification_receiver) = mpsc::channel(1);

    let _mempool_notifier = aptos_mempool_notifications::MempoolNotifier::new(notification_sender);
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
    let _consensus = aptos_consensus::consensus_provider::start_consensus(
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