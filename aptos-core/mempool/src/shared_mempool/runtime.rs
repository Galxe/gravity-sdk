// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    core_mempool::{CoreMempool, TimelineState},
    network::{BroadcastPeerPriority, MempoolSyncMsg},
    shared_mempool::{
        coordinator::{coordinator, gc_coordinator, snapshot_job},
        types::{MempoolEventsReceiver, SharedMempool, SharedMempoolNotification},
    },
    QuorumStoreRequest,
};
use gaptos::api_types::ExecutionChannel;
use gaptos::{aptos_config::config::{NodeConfig, NodeType}, aptos_types::mempool_status::MempoolStatusCode};
use gaptos::aptos_event_notifications::{DbBackedOnChainConfig, ReconfigNotificationListener};
use gaptos::aptos_infallible::Mutex;
use gaptos::aptos_logger::{info, warn, Level};
use gaptos::aptos_mempool_notifications::MempoolNotificationListener;
use gaptos::aptos_network::application::{
    interface::{NetworkClient, NetworkServiceEvents},
    storage::PeersAndMetadata,
};

use gaptos::aptos_storage_interface::DbReader;
use gaptos::aptos_types::on_chain_config::OnChainConfigProvider;
use block_buffer_manager::get_block_buffer_manager;
use futures::channel::mpsc::{Receiver, UnboundedSender};
use std::{sync::Arc, time::Instant};
use tokio::runtime::{Handle, Runtime};

/// Bootstrap of SharedMempool.
/// Creates a separate Tokio Runtime that runs the following routines:
///   - outbound_sync_task (task that periodically broadcasts transactions to peers).
///   - inbound_network_task (task that handles inbound mempool messages and network events).
///   - gc_task (task that performs GC of all expired transactions by SystemTTL).
pub(crate) fn start_shared_mempool<ConfigProvider>(
    executor: &Handle,
    config: &NodeConfig,
    mempool: Arc<Mutex<CoreMempool>>,
    network_client: NetworkClient<MempoolSyncMsg>,
    network_service_events: NetworkServiceEvents<MempoolSyncMsg>,
    client_events: MempoolEventsReceiver,
    quorum_store_requests: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    mempool_reconfig_events: ReconfigNotificationListener<ConfigProvider>,
    db: Arc<dyn DbReader>,
    subscribers: Vec<UnboundedSender<SharedMempoolNotification>>,
    peers_and_metadata: Arc<PeersAndMetadata>,
    execution_api: Arc<dyn ExecutionChannel>,
) where
    ConfigProvider: OnChainConfigProvider,
{
    info!("try to start_shared_mempool");
    let node_type = NodeType::extract_from_config(config);
    let smp: SharedMempool<NetworkClient<MempoolSyncMsg>> =
        SharedMempool::new(
            mempool.clone(),
            config.mempool.clone(),
            network_client,
            db,
            subscribers,
            node_type,
            execution_api.clone(),
        );

    executor.spawn(coordinator(
        smp,
        executor.clone(),
        network_service_events,
        client_events,
        quorum_store_requests,
        mempool_listener,
        mempool_reconfig_events,
        config.mempool.shared_mempool_peer_update_interval_ms,
        peers_and_metadata,
    ));

    executor
        .spawn(gc_coordinator(mempool.clone(), config.mempool.system_transaction_gc_interval_ms));

    if gaptos::aptos_logger::enabled!(Level::Trace) {
        executor.spawn(snapshot_job(mempool, config.mempool.mempool_snapshot_interval_secs));
    }
}

async fn retrieve_from_execution_routine(
    mempool: Arc<Mutex<CoreMempool>>,
) {
    info!("start retrieve_from_execution_routine");
    loop {
        match get_block_buffer_manager().pop_txns(30000, 1_000_000_000).await {
            Ok(txns) => {
                let mut lock_mempool = mempool.lock();
                let start_time = Instant::now();
                let txns_len = txns.len();
                let status = lock_mempool.add_user_txns_batch(txns, true, TimelineState::NotReady, None);
                for s in status {
                    if s.code != MempoolStatusCode::Accepted {
                        panic!("invalid seq number {:?}", s);
                    }
                }
                info!("the recv_pending_txns size is {:?} take {:?} ms", txns_len, start_time.elapsed().as_millis());
            }
            Err(e) => {
                warn!("Error when recv peding txns {:?}", e);
                continue;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

pub fn bootstrap(
    config: &NodeConfig,
    db: Arc<dyn DbReader>,
    network_client: NetworkClient<MempoolSyncMsg>,
    network_service_events: NetworkServiceEvents<MempoolSyncMsg>,
    client_events: MempoolEventsReceiver,
    quorum_store_requests: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    mempool_reconfig_events: ReconfigNotificationListener<DbBackedOnChainConfig>,
    peers_and_metadata: Arc<PeersAndMetadata>,
    execution_api: Arc<dyn ExecutionChannel>,
) -> Vec<Runtime> {
    let runtime = gaptos::aptos_runtimes::spawn_named_runtime("shared-mem".into(), None);
    let retrive_runtime = gaptos::aptos_runtimes::spawn_named_runtime("retrive".into(), None);
    let mempool = Arc::new(Mutex::new(CoreMempool::new(config)));
    retrive_runtime.handle().spawn(retrieve_from_execution_routine(mempool.clone()));
    start_shared_mempool(
        runtime.handle(),
        config,
        mempool,
        network_client,
        network_service_events,
        client_events,
        quorum_store_requests,
        mempool_listener,
        mempool_reconfig_events,
        db,
        vec![],
        peers_and_metadata,
        execution_api,
    );
    vec![runtime, retrive_runtime]
}
