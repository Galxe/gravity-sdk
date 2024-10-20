use std::sync::Arc;

use crate::{ 
    bootstrap::{
        init_gravity_db, init_mempool, init_network_interfaces, init_peers_and_metadata,
        start_consensus,
    },
    consensus_mempool_handler::{ConsensusToMempoolHandler, MempoolNotificationHandler},
    logger,
    network::{create_network_runtime, extract_network_configs},
};
use api_types::{ConsensusApi, ExecutionApi, GTxn};
use aptos_config::{config::NodeConfig, network_id::NetworkId};
use aptos_consensus::{
    gravity_state_computer::ConsensusAdapterArgs,
};
use api_types::BatchClient;
use aptos_event_notifications::EventNotificationSender;
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use async_trait::async_trait;
use futures::{
    channel::mpsc::SendError,
};

use aptos_crypto::{HashValue, PrivateKey, Uniform};
use aptos_types::{
    chain_id::ChainId,
    transaction::{GravityExtension, RawTransaction, SignedTransaction, TransactionPayload},
};
use futures::channel::mpsc;
use futures::future::BoxFuture;
use tokio::runtime::Runtime;

pub struct ConsensusEngine {
    address: String,
    execution_api: Arc<dyn ExecutionApi>,
    batch_client: Arc<BatchClient>,
    runtime_vec: Vec<Runtime>,
}


impl ConsensusEngine {
    pub fn init(node_config: NodeConfig, execution_api: Arc<dyn ExecutionApi>) -> Arc<Self> {
        let gravity_db = init_gravity_db(&node_config);
        let peers_and_metadata = init_peers_and_metadata(&node_config, &gravity_db);
        let (_remote_log_receiver, _logger_filter_update) =
            logger::create_logger(&node_config, Some(node_config.log_file_path.clone()));
        let db: DbReaderWriter = DbReaderWriter::new(gravity_db);
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

        let (mempool_client_sender, mempool_client_receiver) = mpsc::channel(1);

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

        let mempool_runtime = init_mempool(
            &node_config,
            &db,
            &mut event_subscription_service,
            mempool_interfaces,
            mempool_client_receiver,
            consensus_to_mempool_receiver,
            mempool_listener,
            peers_and_metadata,
        );
        network_runtimes.push(mempool_runtime);
        let mut args = ConsensusAdapterArgs::new(mempool_client_sender);
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
        let quorum_store_client =
            args.quorum_store_client.as_mut().unwrap();
        let _ = event_subscription_service.notify_initial_configs(1_u64);
        let arc_self = Arc::new(Self {
            address: node_config.validator_network.as_ref().unwrap().listen_address.to_string(),
            execution_api,
            batch_client: quorum_store_client.get_batch_client(),
            runtime_vec: network_runtimes,
        });
        quorum_store_client.set_consensus_api(arc_self.clone());
        execution_proxy.set_consensus_engine(arc_self.clone());
        arc_self
    }
}

#[async_trait]
impl ConsensusApi for ConsensusEngine {
    async fn request_payload(
        &self,
        closure: BoxFuture<Result<(), SendError>>,
        safe_block_hash: [u8; 32],
        head_block_hash: [u8; 32],
    ) -> Result<(), SendError>
    {
        let txns = self.execution_api.request_transactions(safe_block_hash, head_block_hash).await;
        // let len = txns.len();
        // let signed_txns: Vec<SignedTransaction> = txns
        //     .into_iter()
        //     .enumerate()
        //     .map(|(i, txn)| {
        //         let addr = aptos_types::account_address::AccountAddress::random();
        //         let raw_txn = RawTransaction::new(
        //             addr,
        //             txn.sequence_number,
        //             TransactionPayload::GTxnBytes(txn.txn_bytes),
        //             txn.max_gas_amount,
        //             txn.gas_unit_price,
        //             txn.expiration_timestamp_secs,
        //             ChainId::new(txn.chain_id as u8),
        //         );
        //         SignedTransaction::new_with_gtxn(
        //             raw_txn,
        //             aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing().public_key(),
        //             aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap(),
        //             GravityExtension::new(HashValue::new(safe_block_hash), i as u32, len as u32),
        //         )
        //     })
        //     .collect();
        self.batch_client.submit(txns);
        closure.await
        // self.consensus_to_quorum_store_tx.clone().send(request).await;
        // payload是通过callback发回去的 这里似乎不需要返回payload
    }

    async fn send_order_block(&self, txns: Vec<GTxn>) {
        // let mut return_txns = vec![GTxn::default(); txns.len()];
        // txns.iter().for_each(|txn| {
        //     let txn_bytes = match txn.payload() {
        //         TransactionPayload::GTxnBytes(bytes) => bytes,
        //         _ => {
        //             panic!("should never consists other payload type");
        //         }
        //     };
        //     let gtxn = GTxn {
        //         sequence_number: txn.sequence_number(),
        //         max_gas_amount: txn.max_gas_amount(),
        //         gas_unit_price: txn.gas_unit_price(),
        //         expiration_timestamp_secs: txn.expiration_timestamp_secs(),
        //         chain_id: txn.chain_id().to_u8() as u64,
        //         txn_bytes: (*txn_bytes.clone()).to_owned(),
        //     };
        //     return_txns[txn.g_ext().txn_index_in_block as usize] = gtxn;
        // });
        self.execution_api.send_ordered_block(txns).await
    }

    async fn recv_executed_block_hash(&self) -> [u8; 32] {
        self.execution_api.recv_executed_block_hash().await
    }

    async fn commit_block_hash(&self, block_ids: Vec<[u8; 32]>) {
        self.execution_api.commit_block_hash(block_ids).await
    }
}
