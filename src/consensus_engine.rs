use crate::bootstrap::{
    init_mempool, init_network_interfaces, init_peers_and_metadata, start_consensus,
};
use crate::network::{create_network_runtime, extract_network_configs};
use crate::storage::db::GravityDB;
use crate::{GCEIError, GTxn, GravityConsensusEngineInterface};
use aptos_config::config::NodeConfig;
use aptos_config::network_id::NetworkId;
use aptos_consensus::gravity_state_computer::ConsensusAdapterArgs;
use aptos_crypto::hash::HashValue;
use aptos_crypto::{PrivateKey, Uniform};
use aptos_event_notifications::EventNotificationSender;
use aptos_mempool::MempoolClientRequest;
use aptos_network_builder::builder::NetworkBuilder;
use aptos_storage_interface::DbReaderWriter;
use aptos_types::account_address::AccountAddress;
use aptos_types::chain_id::ChainId;
use aptos_types::mempool_status::{MempoolStatus, MempoolStatusCode};
use aptos_types::transaction::authenticator::TransactionAuthenticator;
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
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub struct GravityConsensusEngine {
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
        Option<mpsc::UnboundedReceiver<(Vec<[u8; 32]>, oneshot::Sender<()>)>>,

    execute_result_receivers: RwLock<HashMap<HashValue, oneshot::Sender<HashValue>>>,
    persist_result_receiver: Mutex<Option<oneshot::Sender<()>>>,
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
    fn init(node_config: NodeConfig, gravity_db: GravityDB) -> Self {
        let peers_and_metadata = init_peers_and_metadata(&node_config, &gravity_db);
        let db: DbReaderWriter = DbReaderWriter::new(gravity_db);
        let mut event_subscription_service =
            aptos_event_notifications::EventSubscriptionService::new(Arc::new(
                aptos_infallible::RwLock::new(db.clone()),
            ));
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

        let _mempool_notifier =
            aptos_mempool_notifications::MempoolNotifier::new(notification_sender);
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
        let mut args = ConsensusAdapterArgs::new(mempool_client_sender);
        start_consensus(
            &node_config,
            &mut event_subscription_service,
            consensus_network_interfaces,
            consensus_notifier,
            consensus_to_mempool_sender,
            db,
            &args,
        );
        let _ = event_subscription_service.notify_initial_configs(1_u64);
        Self {
            mempool_sender: args.mempool_sender.clone(),
            pipeline_block_receiver: args.pipeline_block_receiver.take(),
            execute_result_receivers: RwLock::new(HashMap::new()),
            committed_block_ids_receiver: args.committed_block_ids_receiver.take(),
            persist_result_receiver: Mutex::new(None),
        }
    }

    async fn send_valid_block_transactions(
        &self,
        block_id: [u8; 32],
        txns: Vec<GTxn>,
    ) -> Result<(), GCEIError> {
        println!("send send_valid_block_transactions");
        let len = txns.len();
        for (i, txn) in txns.into_iter().enumerate() {
            let raw_txn = RawTransaction::new(
                AccountAddress::random(),
                txn.sequence_number,
                TransactionPayload::GTxnBytes(txn.txn_bytes),
                txn.max_gas_amount,
                txn.gas_unit_price,
                txn.expiration_timestamp_secs,
                ChainId::new(txn.chain_id as u8),
            );
            let sign_txn = SignedTransaction::new_with_gtxn(
                raw_txn,
                aptos_crypto::ed25519::Ed25519PrivateKey::generate_for_testing().public_key(),
                aptos_crypto::ed25519::Ed25519Signature::try_from(&[1u8; 64][..]).unwrap(),
                GravityExtension::new(HashValue::new(block_id), i as u32, len as u32),
            );
            let (mempool_status, _vm_status_opt) = self.submit_transaction(sign_txn).await.unwrap();
            match mempool_status.code {
                MempoolStatusCode::Accepted => {}
                _ => {
                    return Err(GCEIError::ConsensusError);
                }
            }
        }
        Ok(())
    }

    async fn receive_ordered_block(&mut self) -> Result<([u8; 32], Vec<GTxn>), GCEIError> {
        println!("start to receive_ordered_block");
        let receive_result = self.pipeline_block_receiver.as_mut().unwrap().next().await;

        println!("succeed to receive_ordered_block");

        let (parent_id, block_id, txns, callback) = receive_result.unwrap();
        let gtxns = txns
            .iter()
            .map(|txn| {
                let txn_bytes = match txn.payload() {
                    TransactionPayload::GTxnBytes(bytes) => bytes,
                    _ => {
                        todo!()
                    }
                };
                // let (pkey, sig) = match txn.authenticator() {
                //     TransactionAuthenticator::Ed25519 {
                //         public_key,
                //         signature,
                //     } => (public_key, signature),
                //     _ => {
                //         todo!()
                //     }
                // };
                GTxn {
                    sequence_number: txn.sequence_number(),
                    max_gas_amount: txn.max_gas_amount(),
                    gas_unit_price: txn.gas_unit_price(),
                    expiration_timestamp_secs: txn.expiration_timestamp_secs(),
                    chain_id: txn.chain_id().to_u8() as u64,
                    txn_bytes: (*txn_bytes.clone()).to_owned(),
                }
            })
            .collect();
        self.execute_result_receivers
            .write()
            .await
            .insert(parent_id, callback);

        Ok((*parent_id, gtxns))
    }

    async fn send_compute_res(&self, id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {
        println!("start to send_compute_res");
        match self
            .execute_result_receivers
            .write()
            .await
            .remove(&HashValue::new(id))
        {
            Some(callback) => Ok(callback.send(HashValue::new(res)).unwrap()),
            None => todo!(),
        }
    }

    async fn send_block_head(&self, block_id: [u8; 32], res: [u8; 32]) -> Result<(), GCEIError> {
        todo!()
    }

    async fn receive_commit_block_ids(&mut self) -> Result<Vec<[u8; 32]>, GCEIError> {
        println!("start to receive_commit_block_ids");
        let receive_result = self
            .committed_block_ids_receiver
            .as_mut()
            .unwrap()
            .next()
            .await;
        let (ids, sender) = receive_result.unwrap();
        let mut locked = self.persist_result_receiver.lock().await;
        *locked = Some(sender);
        Ok(ids)
    }

    async fn send_persistent_block_id(&self, id: [u8; 32]) -> Result<(), GCEIError> {
        println!("start to send_persistent_block_id");
        let mut locked = self.persist_result_receiver.lock().await;
        match locked.take() {
            Some(sender) => {
                sender.send(());
                Ok(())
            }
            None => todo!(),
        }
    }
}
