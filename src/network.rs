use std::{collections::HashMap, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use aptos_channels::{aptos_channel, message_queues::QueueStyle};
use aptos_config::{config::{NetworkConfig, NodeConfig}, network_id::NetworkId};
use aptos_crypto::{PrivateKey, Uniform};
use aptos_mempool::MempoolClientRequest;
use aptos_network::{application::{interface::{NetworkClient, NetworkServiceEvents}, storage::PeersAndMetadata}, protocols::network::{NetworkApplicationConfig, NetworkClientConfig, NetworkEvents, NetworkSender, NetworkServiceConfig}, ProtocolId};
use aptos_network_builder::builder::NetworkBuilder;
use aptos_types::{chain_id::ChainId, transaction::{RawTransaction, Script, SignedTransaction}};
use futures::{channel::oneshot, SinkExt};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

use crate::{ApplicationNetworkInterfaces, GTxn, GravityConsensusEngineInterface};
use crate::consensus_execution_adapter::ConsensusExecutionAdapter;

/// Extracts all network configs from the given node config
pub fn extract_network_configs(node_config: &NodeConfig) -> Vec<NetworkConfig> {
    let mut network_configs: Vec<NetworkConfig> = node_config.full_node_networks.to_vec();
    if let Some(network_config) = node_config.validator_network.as_ref() {
        // Ensure that mutual authentication is enabled by default!
        if !network_config.mutual_authentication {
            panic!("Validator networks must always have mutual_authentication enabled!");
        }
        network_configs.push(network_config.clone());
    }
    network_configs
}

pub fn extract_network_ids(node_config: &NodeConfig) -> Vec<NetworkId> {
    extract_network_configs(node_config)
        .into_iter()
        .map(|network_config| network_config.network_id)
        .collect()
}

/// TODO: make this configurable (e.g., for compression)
/// Returns the network application config for the consensus client and service
pub fn consensus_network_configuration(node_config: &NodeConfig) -> NetworkApplicationConfig {
    let direct_send_protocols: Vec<ProtocolId> =
        aptos_consensus::network_interface::DIRECT_SEND.into();
    let rpc_protocols: Vec<ProtocolId> = aptos_consensus::network_interface::RPC.into();

    let network_client_config =
        NetworkClientConfig::new(direct_send_protocols.clone(), rpc_protocols.clone());
    let network_service_config = NetworkServiceConfig::new(
        direct_send_protocols,
        rpc_protocols,
        aptos_channel::Config::new(node_config.consensus.max_network_channel_size)
            .queue_style(QueueStyle::FIFO)
            .counters(&aptos_consensus::counters::PENDING_CONSENSUS_NETWORK_EVENTS),
    );
    NetworkApplicationConfig::new(network_client_config, network_service_config)
}

/// Returns the network application config for the mempool client and service
pub fn mempool_network_configuration(node_config: &NodeConfig) -> NetworkApplicationConfig {
    let direct_send_protocols = vec![ProtocolId::MempoolDirectSend];
    let rpc_protocols = vec![]; // Mempool does not use RPC

    let network_client_config =
        NetworkClientConfig::new(direct_send_protocols.clone(), rpc_protocols.clone());
    let network_service_config = NetworkServiceConfig::new(
        direct_send_protocols,
        rpc_protocols,
        aptos_channel::Config::new(node_config.mempool.max_network_channel_size)
            .queue_style(QueueStyle::KLAST) // TODO: why is this not FIFO?
            .counters(&aptos_mempool::counters::PENDING_MEMPOOL_NETWORK_EVENTS),
    );
    NetworkApplicationConfig::new(network_client_config, network_service_config)
}

pub async fn mock_mempool_client_sender(mut mc_sender: aptos_mempool::MempoolClientSender) {
    let addr = aptos_types::account_address::AccountAddress::random();
    let mut seq_num = 0;
    loop {
        let txn: SignedTransaction = SignedTransaction::new(
            RawTransaction::new_script(
                addr.clone(),
                seq_num,
                Script::new(vec![], vec![], vec![]),
                0,
                0,
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60,
                ChainId::test(),
            ),
            aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing().public_key(),
            aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap(),
        );
        seq_num += 1;
        let (sender, receiver) = oneshot::channel();
        mc_sender
            .send(MempoolClientRequest::SubmitTransaction(txn, sender))
            .await;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

pub async fn mock_execution_txn_submitter(adapter: ConsensusExecutionAdapter) {
    // let addr = aptos_types::account_address::AccountAddress::random();
    let mut seq_num = 0;
    loop {
        let txn = GTxn {
            sequence_number: seq_num,
            max_gas_amount: 0,
            gas_unit_price: 0,
            expiration_timestamp_secs: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 60,
            chain_id: ChainId::test().to_u8(),
            txn_bytes: vec![],
            public_key: aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing().public_key().to_bytes(),
            signature: aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap().to_bytes(),
        };
        seq_num += 1;
        let mock_block_id: [u8; 32] = [0; 32];
        adapter.send_valid_transactions(mock_block_id, vec![txn]).await.expect("ok");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

struct ApplicationNetworkHandle<T> {
    pub network_id: NetworkId,
    pub network_sender: NetworkSender<T>,
    pub network_events: NetworkEvents<T>,
}

/// Creates an application network inteface using the given
/// handles and config.
fn create_network_interfaces<
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
>(
    network_handles: Vec<ApplicationNetworkHandle<T>>,
    network_application_config: NetworkApplicationConfig,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> ApplicationNetworkInterfaces<T> {
    // Gather the network senders and events
    let mut network_senders = HashMap::new();
    let mut network_and_events = HashMap::new();
    for network_handle in network_handles {
        let network_id = network_handle.network_id;
        network_senders.insert(network_id, network_handle.network_sender);
        network_and_events.insert(network_id, network_handle.network_events);
    }

    // Create the network client
    let network_client_config = network_application_config.network_client_config;
    let network_client = NetworkClient::new(
        network_client_config.direct_send_protocols_and_preferences,
        network_client_config.rpc_protocols_and_preferences,
        network_senders,
        peers_and_metadata,
    );

    // Create the network service events
    let network_service_events = NetworkServiceEvents::new(network_and_events);

    // Create and return the new network interfaces
    ApplicationNetworkInterfaces {
        network_client,
        network_service_events,
    }
}

/// Registers a new application client and service with the network
fn register_client_and_service_with_network<
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
>(
    network_builder: &mut NetworkBuilder,
    network_id: NetworkId,
    network_config: &NetworkConfig,
    application_config: NetworkApplicationConfig,
    allow_out_of_order_delivery: bool,
) -> ApplicationNetworkHandle<T> {
    let (network_sender, network_events) = network_builder.add_client_and_service(
        &application_config,
        network_config.max_parallel_deserialization_tasks,
        allow_out_of_order_delivery,
    );
    ApplicationNetworkHandle {
        network_id,
        network_sender,
        network_events,
    }
}

pub fn build_network_interfaces<T>(
    network_builder: &mut NetworkBuilder,
    network_id: NetworkId,
    network_config: &NetworkConfig,
    application_config: NetworkApplicationConfig,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> ApplicationNetworkInterfaces<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
    let consensus_network_handle = register_client_and_service_with_network(
        network_builder,
        network_id,
        &network_config,
        application_config.clone(),
        true,
    );
    create_network_interfaces(
        vec![consensus_network_handle],
        application_config,
        peers_and_metadata.clone(),
    )
}

/// Creates a network runtime for the given network config
pub fn create_network_runtime(network_config: &NetworkConfig) -> Runtime {
    let network_id = network_config.network_id;

    // Create the runtime
    let thread_name = format!(
        "network-{}",
        network_id.as_str().chars().take(3).collect::<String>()
    );
    aptos_runtimes::spawn_named_runtime(thread_name, network_config.runtime_threads)
}