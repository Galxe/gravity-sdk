// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Tasks that are executed by coordinators (short-lived compared to coordinators)
use super::types::MempoolMessageId;
use crate::{
    core_mempool::{CoreMempool, TimelineState},
    counters,
    logging::{LogEntry, LogEvent, LogSchema},
    network::{BroadcastError, BroadcastPeerPriority, MempoolSyncMsg},
    shared_mempool::{
        types::{
            SharedMempool,
            SubmissionStatusBundle,
        },
        use_case_history::UseCaseHistory,
    },
    thread_pool::IO_POOL,
    QuorumStoreRequest, QuorumStoreResponse,
};
use api_types::VerifiedTxn;
use gaptos::aptos_config::network_id::PeerNetworkId;
use aptos_consensus_types::common::RejectedTransactionSummary;
use gaptos::aptos_crypto::HashValue;
use gaptos::aptos_infallible::{Mutex, RwLock};
use gaptos::aptos_logger::prelude::*;
use gaptos::aptos_mempool_notifications::CommittedTransaction;
use gaptos::aptos_metrics_core::HistogramTimer;
use gaptos::aptos_network::application::interface::NetworkClientInterface;
use gaptos::aptos_types::{
    mempool_status::MempoolStatusCode,
    on_chain_config::{OnChainConfigPayload, OnChainConfigProvider, OnChainConsensusConfig},
    transaction::SignedTransaction,
};
// use aptos_vm_validator::vm_validator::{get_account_sequence_number, TransactionValidation};
use futures::channel::oneshot;
use rayon::prelude::*;
use std::{
    cmp,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::runtime::Handle;

// ============================== //
//  broadcast_coordinator tasks  //
// ============================== //

/// Attempts broadcast to `peer` and schedules the next broadcast.
pub(crate) async fn execute_broadcast<NetworkClient>(
    peer: PeerNetworkId,
    backoff: bool,
    smp: &mut SharedMempool<NetworkClient>,
    executor: Handle,
    transactions: Vec<VerifiedTxn>,
) where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{
    let network_interface = &smp.network_interface.clone();
    // If there's no connection, don't bother to broadcast
    if network_interface.sync_states_exists(&peer) {
        if let Err(err) = network_interface.execute_broadcast(peer, backoff, smp, transactions).await {
            match err {
                BroadcastError::NetworkError(peer, error) => warn!(LogSchema::event_log(
                    LogEntry::BroadcastTransaction,
                    LogEvent::NetworkSendFail
                )
                .peer(&peer)
                .error(&error)),
                BroadcastError::NoTransactions(_) | BroadcastError::PeerNotPrioritized(_, _) => {
                    sample!(SampleRate::Duration(Duration::from_secs(60)), trace!("{:?}", err));
                }
                _ => {
                    sample!(SampleRate::Duration(Duration::from_secs(60)), debug!("{:?}", err));
                }
            }
        }
    } else {
        // Drop the scheduled broadcast, we're not connected anymore
        return;
    }
    let schedule_backoff = network_interface.is_backoff_mode(&peer);

    let interval_ms = if schedule_backoff {
        smp.config.shared_mempool_backoff_interval_ms
    } else {
        smp.config.shared_mempool_tick_interval_ms
    };
}

/// Processes get transaction by hash request by client.
pub(crate) async fn process_client_get_transaction<NetworkClient>(
    smp: SharedMempool<NetworkClient>,
    hash: HashValue,
    callback: oneshot::Sender<Option<SignedTransaction>>,
    timer: HistogramTimer,
) where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{
    timer.stop_and_record();
    let _timer = counters::process_get_txn_latency_timer_client();
    let txn = smp.mempool.lock().get_by_hash(hash);

    if callback.send(txn).is_err() {
        warn!(LogSchema::event_log(LogEntry::GetTransaction, LogEvent::CallbackFail));
        counters::CLIENT_CALLBACK_FAIL.inc();
    }
}

/// Processes transactions from other nodes.
pub(crate) async fn process_transaction_broadcast<NetworkClient>(
    smp: SharedMempool<NetworkClient>,
    // The sender of the transactions can send the time at which the transactions were inserted
    // in the sender's mempool. The sender can also send the priority of this node for the sender
    // of the transactions.
    transactions: Vec<(VerifiedTxn, Option<u64>, Option<BroadcastPeerPriority>)>,
    timeline_state: TimelineState,
    peer: PeerNetworkId,
    timer: HistogramTimer,
) where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{
    timer.stop_and_record();
    let _timer = counters::process_txn_submit_latency_timer(peer.network_id());
    process_incoming_transactions(&smp, transactions, timeline_state, false);

    // let ack_response = gen_ack_response(message_id, results, &peer);

    // // Respond to the peer with an ack. Note: ack response messages should be
    // // small enough that they always fit within the maximum network message
    // // size, so there's no need to check them here.
    // if let Err(e) = smp.network_interface.send_message_to_peer(peer, ack_response) {
    //     counters::network_send_fail_inc(counters::ACK_TXNS);
    //     warn!(LogSchema::event_log(LogEntry::BroadcastACK, LogEvent::NetworkSendFail)
    //         .peer(&peer)
    //         .error(&e.into()));
    //     return;
    // }
    // notify_subscribers(SharedMempoolNotification::ACK, &smp.subscribers);
}

/// If `MempoolIsFull` on any of the transactions, provide backpressure to the downstream peer.
fn gen_ack_response(
    message_id: MempoolMessageId,
    results: Vec<SubmissionStatusBundle>,
    peer: &PeerNetworkId,
) -> MempoolSyncMsg {
    let mut backoff_and_retry = false;
    for (_, (mempool_status, _)) in results.into_iter() {
        if mempool_status.code == MempoolStatusCode::MempoolIsFull {
            backoff_and_retry = true;
            break;
        }
    }

    update_ack_counter(peer, counters::SENT_LABEL, backoff_and_retry, backoff_and_retry);
    MempoolSyncMsg::BroadcastTransactionsResponse {
        message_id,
        retry: backoff_and_retry,
        backoff: backoff_and_retry,
    }
}

pub(crate) fn update_ack_counter(
    peer: &PeerNetworkId,
    direction_label: &str,
    retry: bool,
    backoff: bool,
) {
    if retry {
        counters::shared_mempool_ack_inc(
            peer.network_id(),
            direction_label,
            counters::RETRY_BROADCAST_LABEL,
        );
    }
    if backoff {
        counters::shared_mempool_ack_inc(
            peer.network_id(),
            direction_label,
            counters::BACKPRESSURE_BROADCAST_LABEL,
        );
    }
}

/// Submits a list of SignedTransaction to the local mempool
/// and returns a vector containing [SubmissionStatusBundle].
pub(crate) fn process_incoming_transactions<NetworkClient>(
    smp: &SharedMempool<NetworkClient>,
    transactions: Vec<(VerifiedTxn, Option<u64>, Option<BroadcastPeerPriority>)>,
    timeline_state: TimelineState,
    client_submitted: bool,
)
where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{

    let start_storage_read = Instant::now();
    // let state_view = smp
    //     .db
    //     .latest_state_checkpoint_view()
    //     .expect("Failed to get latest state checkpoint view.");

    // Track latency: fetching seq number
    let seq_numbers = IO_POOL.install(|| {
        transactions
            .par_iter()
            .map(|(t, _, _)| {
                // TODO(gravity_lightman): Figure out how to get the account view
                // get_account_sequence_number(&state_view, t.sender()).map_err(|e| {
                //     error!(LogSchema::new(LogEntry::DBError).error(&e));
                //     counters::DB_ERROR.inc();
                //     e
                // })
                Ok(1).map_err(|e| {
                    error!(LogSchema::new(LogEntry::DBError).error(&e));
                    counters::DB_ERROR.inc();
                    e
                })
            })
            .collect::<Vec<_>>()
    });
    // Track latency for storage read fetching sequence number
    let storage_read_latency = start_storage_read.elapsed();
    counters::PROCESS_TXN_BREAKDOWN_LATENCY
        .with_label_values(&[counters::FETCH_SEQ_NUM_LABEL])
        .observe(storage_read_latency.as_secs_f64() / transactions.len() as f64);

    let transactions: Vec<_> = transactions
        .into_iter()
        .enumerate()
        .filter_map(|(idx, (t, ready_time_at_sender, priority))| {
            Some((t, 1, ready_time_at_sender, priority))
        })
        .collect();

    validate_and_add_transactions(
        transactions,
        smp,
        timeline_state,
        client_submitted,
    );
    // notify_subscribers(SharedMempoolNotification::NewTransactions, &smp.subscribers);
}

/// Perfoms VM validation on the transactions and inserts those that passes
/// validation into the mempool.
#[cfg(not(feature = "consensus-only-perf-test"))]
fn validate_and_add_transactions<NetworkClient>(
    transactions: Vec<(VerifiedTxn, u64, Option<u64>, Option<BroadcastPeerPriority>)>,
    smp: &SharedMempool<NetworkClient>,
    timeline_state: TimelineState,
    client_submitted: bool,
) where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{
    let mut mempool = smp.mempool.lock();
    for (idx, (transaction, sequence_info, ready_time_at_sender, priority)) in
        transactions.into_iter().enumerate()
    {
        let mempool_status = mempool.send_user_txn(
            transaction.into(),
            sequence_info,
            timeline_state,
            client_submitted,
            ready_time_at_sender,
            priority.clone(),
        );
    }
}

/// In consensus-only mode, insert transactions into the mempool directly
/// without any VM validation.
///
/// We want to populate transactions as fast as and
/// as much as possible into the mempool, and the VM validator would interfere with
/// this because validation has some overhead and the validator bounds the number of
/// outstanding sequence numbers.
#[cfg(feature = "consensus-only-perf-test")]
fn validate_and_add_transactions<NetworkClient>(
    transactions: Vec<(SignedTransaction, u64, Option<u64>)>,
    smp: &SharedMempool<NetworkClient>,
    timeline_state: TimelineState,
    statuses: &mut Vec<(
        SignedTransaction,
        (MempoolStatus, Option<StatusCode>, Option<BroadcastPeerPriority>),
    )>,
    client_submitted: bool,
) where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{
    use super::priority;

    let mut mempool = smp.mempool.lock();
    for (transaction, sequence_info, ready_time_at_sender, priority) in transactions.into_iter() {
        let mempool_status = mempool.send_user_txn(
            transaction.clone(),
            0,
            sequence_info,
            timeline_state,
            client_submitted,
            read_time_at_sender,
            priority,
        );
        statuses.push((transaction, (mempool_status, None)));
    }
}

fn log_txn_process_results(results: &[SubmissionStatusBundle], sender: Option<PeerNetworkId>) {
    let network = match sender {
        Some(peer) => peer.network_id().to_string(),
        None => counters::CLIENT_LABEL.to_string(),
    };
    for (txn, (mempool_status, maybe_vm_status)) in results.iter() {
        if let Some(vm_status) = maybe_vm_status {
            trace!(
                SecurityEvent::InvalidTransactionMempool,
                failed_transaction = txn,
                vm_status = vm_status,
                sender = sender,
            );
            counters::shared_mempool_transactions_processed_inc(
                counters::VM_VALIDATION_LABEL,
                &network,
            );
            continue;
        }
        match mempool_status.code {
            MempoolStatusCode::Accepted => counters::shared_mempool_transactions_processed_inc(
                counters::SUCCESS_LABEL,
                &network,
            ),
            _ => counters::shared_mempool_transactions_processed_inc(
                &mempool_status.code.to_string(),
                &network,
            ),
        }
    }
}

// ================================= //
// intra-node communication handlers //
// ================================= //

/// Only applies to Validators. Either provides transactions to consensus [`GetBlockRequest`] or
/// handles rejecting transactions [`RejectNotification`]
pub(crate) fn process_quorum_store_request<NetworkClient>(
    smp: &SharedMempool<NetworkClient>,
    req: QuorumStoreRequest,
) where
    NetworkClient: NetworkClientInterface<MempoolSyncMsg>,
{
    // Start latency timer
    let start_time = Instant::now();

    let (resp, callback, counter_label) = match req {
        QuorumStoreRequest::GetBatchRequest(
            max_txns,
            max_bytes,
            return_non_full,
            exclude_transactions,
            callback,
        ) => {
            let txns;
            {
                let lock_timer = counters::mempool_service_start_latency_timer(
                    counters::GET_BLOCK_LOCK_LABEL,
                    counters::REQUEST_SUCCESS_LABEL,
                );
                let mempool = smp.mempool.lock();
                lock_timer.observe_duration();

                let max_txns = cmp::max(max_txns, 1);
                let _get_batch_timer = counters::mempool_service_start_latency_timer(
                    counters::GET_BLOCK_GET_BATCH_LABEL,
                    counters::REQUEST_SUCCESS_LABEL,
                );
                txns =
                    mempool.get_batch(max_txns, max_bytes, return_non_full, exclude_transactions);
            }

            // mempool_service_transactions is logged inside get_batch

            (QuorumStoreResponse::GetBatchResponse(txns), callback, counters::GET_BLOCK_LABEL)
        }
        QuorumStoreRequest::RejectNotification(transactions, callback) => {
            counters::mempool_service_transactions(
                counters::COMMIT_CONSENSUS_LABEL,
                transactions.len(),
            );
            process_rejected_transactions(&smp.mempool, transactions);
            (QuorumStoreResponse::CommitResponse(), callback, counters::COMMIT_CONSENSUS_LABEL)
        }
    };
    // Send back to callback
    let result = if callback.send(Ok(resp)).is_err() {
        error!(LogSchema::event_log(LogEntry::QuorumStore, LogEvent::CallbackFail));
        counters::REQUEST_FAIL_LABEL
    } else {
        counters::REQUEST_SUCCESS_LABEL
    };
    let latency = start_time.elapsed();
    counters::mempool_service_latency(counter_label, result, latency);
}

/// Remove transactions that are committed (or rejected) so that we can stop broadcasting them.
pub(crate) fn process_committed_transactions(
    mempool: &Mutex<CoreMempool>,
    use_case_history: &Mutex<UseCaseHistory>,
    transactions: Vec<CommittedTransaction>,
    block_timestamp_usecs: u64,
) {
    let mut pool = mempool.lock();
    let block_timestamp = Duration::from_micros(block_timestamp_usecs);

    let tracking_usecases = {
        let mut history = use_case_history.lock();
        history.update_usecases(&transactions);
        history.compute_tracking_set()
    };

    for transaction in transactions {
        pool.log_commit_transaction(
            &transaction.sender,
            transaction.sequence_number,
            tracking_usecases
                .get(&transaction.use_case)
                .map(|name| (transaction.use_case.clone(), name)),
            block_timestamp,
        );
        pool.commit_transaction(&transaction.sender, transaction.sequence_number);
    }
}

pub(crate) fn process_rejected_transactions(
    mempool: &Mutex<CoreMempool>,
    transactions: Vec<RejectedTransactionSummary>,
) {
    let mut pool = mempool.lock();

    for transaction in transactions {
        pool.reject_transaction(
            &transaction.sender,
            transaction.sequence_number,
            &transaction.hash,
            &transaction.reason,
        );
    }
}

/// Processes on-chain reconfiguration notifications.  Restarts validator with the new info.
pub(crate) async fn process_config_update<P>(
    config_update: OnChainConfigPayload<P>,
    broadcast_within_validator_network: Arc<RwLock<bool>>,
) where
    // V: TransactionValidation,
    P: OnChainConfigProvider,
{
    info!(LogSchema::event_log(LogEntry::ReconfigUpdate, LogEvent::Process));

    let consensus_config: anyhow::Result<OnChainConsensusConfig> = config_update.get();
    match consensus_config {
        Ok(consensus_config) => {
            *broadcast_within_validator_network.write() =
                !consensus_config.quorum_store_enabled() && !consensus_config.is_dag_enabled()
        }
        Err(e) => {
            error!(
                "Failed to read on-chain consensus config, keeping value broadcast_within_validator_network={}: {}",
                *broadcast_within_validator_network.read(),
                e
            );
        }
    }
}
