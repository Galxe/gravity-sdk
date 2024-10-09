use crate::bootstrap::{
    init_gravity_db, init_mempool, init_network_interfaces, init_peers_and_metadata,
    start_consensus,
};
use crate::consensus_mempool_handler::{ConsensusToMempoolHandler, MempoolNotificationHandler};
use crate::network::{create_network_runtime, extract_network_configs};
use crate::utils::bimap::BiMap;
use crate::{logger, GCEIError, GTxn, GravityConsensusEngineInterface};
use aptos_config::config::NodeConfig;
use aptos_config::network_id::NetworkId;
use aptos_consensus::gravity_state_computer::ConsensusAdapterArgs;
use aptos_crypto::hash::HashValue;
use aptos_crypto::{PrivateKey, Uniform};
use aptos_event_notifications::EventNotificationSender;
use aptos_logger::info;
use aptos_mempool::MempoolClientRequest;
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use aptos_types::account_address::AccountAddress;
use aptos_types::chain_id::ChainId;
use aptos_types::mempool_status::{MempoolStatus, MempoolStatusCode};
use aptos_types::transaction::{
    GravityExtension, RawTransaction, SignedTransaction, TransactionPayload,
};
use aptos_types::vm_status::StatusCode;
use futures::StreamExt;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use std::collections::HashMap;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, RwLock};

pub struct GravityConsensusEngine {
    address: String,
    mempool_sender: mpsc::Sender<MempoolClientRequest>,
    pipeline_block_receiver: Option<
        mpsc::UnboundedReceiver<(
            HashValue,
            HashValue,
            Vec<SignedTransaction>,
            oneshot::Sender<HashValue>,
        )>,
    >,
    committed_block_ids_receiver:
        Option<mpsc::UnboundedReceiver<(Vec<[u8; 32]>, oneshot::Sender<HashValue>)>>,

    execute_result_receivers: RwLock<HashMap<HashValue, oneshot::Sender<HashValue>>>,
    persist_result_receiver: Mutex<Option<oneshot::Sender<HashValue>>>,
    runtime_vec: Vec<Runtime>,
    id_index: RwLock<BiMap>,
}

impl GravityConsensusEngine {
    async fn submit_transaction(
        &self,
        txn: SignedTransaction,
    ) -> Result<(MempoolStatus, Option<StatusCode>), GCEIError> {
        let (req_sender, callback) = oneshot::channel();
        let ret = self
            .mempool_sender
            .clone()
            .send(MempoolClientRequest::SubmitTransaction(txn, req_sender))
            .await;
        if let Err(_) = ret {
            return Err(GCEIError::ConsensusError);
        }
        let send_ret = callback.await;
        match send_ret {
            Ok(status) => match status {
                Ok(value) => Ok(value),
                Err(_) => Err(GCEIError::ConsensusError),
            },
            Err(_) => Err(GCEIError::ConsensusError),
        }
    }
}

#[async_trait::async_trait]
impl GravityConsensusEngineInterface for GravityConsensusEngine {
    fn init(node_config: NodeConfig) -> Self {
        let gravity_db = init_gravity_db(&node_config);
        let peers_and_metadata = init_peers_and_metadata(&node_config, &gravity_db);
        let (_remote_log_receiver, _logger_filter_update) = logger::create_logger(&node_config, Some(node_config.log_file_path.clone()));
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
                state_sync_config
                    .state_sync_driver
                    .commit_notification_timeout_ms,
            );
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
            mempool_client_sender.clone(),
            mempool_interfaces,
            mempool_client_receiver,
            consensus_to_mempool_receiver,
            mempool_listener,
            peers_and_metadata,
        );
        network_runtimes.push(mempool_runtime);
        let mut args = ConsensusAdapterArgs::new(mempool_client_sender);
        let (consensus_runtime, _, _) = start_consensus(
            &node_config,
            &mut event_subscription_service,
            consensus_network_interfaces,
            consensus_notifier,
            consensus_to_mempool_sender,
            db,
            &args,
        );
        network_runtimes.push(consensus_runtime);
        let _ = event_subscription_service.notify_initial_configs(1_u64);
        Self {
            address: node_config
                .validator_network
                .as_ref()
                .unwrap()
                .listen_address
                .to_string(),
            mempool_sender: args.mempool_sender.clone(),
            pipeline_block_receiver: args.pipeline_block_receiver.take(),
            execute_result_receivers: RwLock::new(HashMap::new()),
            committed_block_ids_receiver: args.committed_block_ids_receiver.take(),
            persist_result_receiver: Mutex::new(None),
            runtime_vec: network_runtimes,
            id_index: RwLock::new(BiMap::new()),
        }
    }

    async fn send_valid_block_transactions(
        &self,
        block_id: [u8; 32],
        txns: Vec<GTxn>,
    ) -> Result<(), GCEIError> {
        let len = txns.len();
        info!(
            "send send_valid_block_transactions, block_is {:?}, size {:?}",
            HashValue::new(block_id),
            len
        );
        for (i, txn) in txns.into_iter().enumerate() {
            let addr = AccountAddress::random();
            let raw_txn = RawTransaction::new(
                addr,
                txn.sequence_number,
                TransactionPayload::GTxnBytes(txn.txn_bytes),
                txn.max_gas_amount,
                txn.gas_unit_price,
                txn.expiration_timestamp_secs,
                ChainId::new(txn.chain_id as u8),
            );
            info!(
                "txn addr is {:?}, expiration time second {:?}",
                addr, txn.expiration_timestamp_secs
            );
            let sign_txn = SignedTransaction::new_with_gtxn(
                raw_txn,
                aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing().public_key(),
                aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap(),
                GravityExtension::new(HashValue::new(block_id), i as u32, len as u32),
            );
            let (mempool_status, _vm_status_opt) = self.submit_transaction(sign_txn).await.unwrap();
            match mempool_status.code {
                MempoolStatusCode::Accepted => {
                    info!("Submit txn success");
                }
                _ => {
                    info!("Submit txn failed {:?}", mempool_status);
                    return Err(GCEIError::ConsensusError);
                }
            }
        }
        Ok(())
    }

    async fn receive_ordered_block(&mut self) -> Result<([u8; 32], Vec<GTxn>), GCEIError> {
        info!("start to receive_ordered_block");
        let receive_result = self.pipeline_block_receiver.as_mut().unwrap().next().await;

        info!("succeed to receive_ordered_block");

        let (parent_id, block_id, txns, callback) = receive_result.unwrap();
        let return_payload_id = txns.first().unwrap().g_ext().block_id;
        info!("the txns size is {:?}, block_is {:?}, return payload id is {:?}", txns.len(), block_id, return_payload_id);
        self.id_index
            .write()
            .await
            .insert(*return_payload_id, block_id);
        let g_ext_size = txns.first().unwrap().g_ext().txn_count_in_block;
        assert_eq!(g_ext_size as usize, txns.len());
        let mut return_txns = vec![GTxn::default(); txns.len()];
        txns.iter().for_each(|txn| {
            let txn_bytes = match txn.payload() {
                TransactionPayload::GTxnBytes(bytes) => bytes,
                _ => {
                    panic!("should never consists other payload type");
                }
            };
            let gtxn = GTxn {
                sequence_number: txn.sequence_number(),
                max_gas_amount: txn.max_gas_amount(),
                gas_unit_price: txn.gas_unit_price(),
                expiration_timestamp_secs: txn.expiration_timestamp_secs(),
                chain_id: txn.chain_id().to_u8() as u64,
                txn_bytes: (*txn_bytes.clone()).to_owned(),
            };
            return_txns[txn.g_ext().txn_index_in_block as usize] = gtxn;
        });
        self.execute_result_receivers
            .write()
            .await
            .insert(return_payload_id, callback);
        info!(
            "send payload id {:?} it with block id {:?}",
            return_payload_id, block_id
        );
        info!(
            "return receive_ordered_block, the index map is {:?}",
            self.id_index.read().await
        );

        Ok((*return_payload_id, return_txns))
    }

    async fn send_compute_res(&self, id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {
        let mut index_guard = self.id_index.write().await;
        let block_id = index_guard.deref_mut().get_block_id(&id).unwrap().clone();
        info!("start to send_compute_res for payload id {:?}, block id {:?}", HashValue::new(id), block_id);
        match self
            .execute_result_receivers
            .write()
            .await
            .remove(&HashValue::new(id))
        {
            Some(callback) => Ok(callback.send(HashValue::new(res)).unwrap()),
            None => {
                panic!("return non-existent block's res");
            },
        }
    }

    async fn send_block_head(&self, block_id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {
        todo!()
    }

    async fn receive_commit_block_ids(&mut self) -> Result<Vec<[u8; 32]>, GCEIError> {
        info!("start to receive_commit_block_ids");
        let receive_result = self
            .committed_block_ids_receiver
            .as_mut()
            .unwrap()
            .next()
            .await;
        info!("succeed to receive_commit_block_ids");
        let (ids, sender) = receive_result.unwrap();
        let mut payload_ids = vec![];
        for id in ids {
            let index = self.id_index.write().await;
            payload_ids.push(index.get_payload_id(&HashValue::new(id)).unwrap().clone());
        }
        let mut locked = self.persist_result_receiver.lock().await;
        *locked = Some(sender);
        info!("the index map is {:?}", self.id_index.read().await);
        Ok(payload_ids)
    }

    async fn send_persistent_block_id(&self, id: [u8; 32]) -> Result<(), GCEIError> {
        info!("start to send_persistent_block_id");
        let mut locked = self.persist_result_receiver.lock().await;
        match locked.take() {
            Some(sender) => {
                let mut index_guard = self.id_index.write().await;
                let block_id = index_guard.deref_mut().get_block_id(&id).unwrap().clone();
                index_guard.remove(&id, &block_id);
                sender.send(block_id).expect("send persisten id error");
                Ok(())
            }
            None => panic!("send wrong persistent id"),
        }
    }

    fn is_leader(&self) -> bool {
        self.address == "/ip4/127.0.0.1/tcp/2024"
    }
}
