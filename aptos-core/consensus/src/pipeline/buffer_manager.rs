// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_storage::tracing::{observe_block, BlockStage},
    consensus_observer::{
        network_message::ConsensusObserverMessage, publisher::ConsensusPublisher,
    },
    counters::log_executor_error_occurred,
    monitor,
    network::{IncomingCommitRequest, NetworkSender},
    network_interface::ConsensusMsg,
    pipeline::{
        buffer::{Buffer, Cursor},
        buffer_item::BufferItem,
        commit_reliable_broadcast::{AckState, CommitMessage},
        execution_schedule_phase::ExecutionRequest,
        execution_wait_phase::{ExecutionResponse, ExecutionWaitRequest},
        persisting_phase::PersistingRequest,
        pipeline_phase::CountedRequest,
        signing_phase::{SigningRequest, SigningResponse},
    },
    state_replication::StateComputerCommitCallBackType,
};
use block_buffer_manager::get_block_buffer_manager;
use gaptos::aptos_bounded_executor::BoundedExecutor;
use gaptos::aptos_config::config::ConsensusObserverConfig;
use aptos_consensus_types::{
    common::{Author, Round},
    pipeline::commit_vote::CommitVote,
    pipelined_block::PipelinedBlock,
};
use gaptos::aptos_crypto::HashValue;
use aptos_executor_types::ExecutorResult;
use gaptos::aptos_logger::prelude::*;
use gaptos::aptos_network::protocols::{rpc::error::RpcError, wire::handshake::v1::ProtocolId};
use gaptos::aptos_reliable_broadcast::{DropGuard, ReliableBroadcast};
use gaptos::aptos_time_service::TimeService;
use gaptos::aptos_types::{
    account_address::AccountAddress, epoch_change::EpochChangeProof, epoch_state::EpochState,
    ledger_info::LedgerInfoWithSignatures,
};
use bytes::Bytes;
use futures::{
    channel::{
        mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
        oneshot,
    },
    future::{AbortHandle, Abortable},
    FutureExt, SinkExt, StreamExt,
};
use once_cell::sync::OnceCell;
use std::{
    collections::HashMap, f32::consts::E, sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    }
};
use tokio::time::{Duration, Instant};
use tokio_retry::strategy::ExponentialBackoff;
use gaptos::aptos_consensus::counters as counters;

pub const COMMIT_VOTE_BROADCAST_INTERVAL_MS: u64 = 1500;
pub const COMMIT_VOTE_REBROADCAST_INTERVAL_MS: u64 = 30000;
pub const LOOP_INTERVAL_MS: u64 = 1500;

#[derive(Debug, Default)]
pub struct ResetAck {}

pub enum ResetSignal {
    Stop,
    TargetRound(u64),
}

pub struct ResetRequest {
    pub tx: oneshot::Sender<ResetAck>,
    pub signal: ResetSignal,
}

pub struct OrderedBlocks {
    pub ordered_blocks: Vec<PipelinedBlock>,
    pub ordered_proof: LedgerInfoWithSignatures,
    pub callback: StateComputerCommitCallBackType,
}

impl OrderedBlocks {
    pub fn latest_round(&self) -> Round {
        self.ordered_blocks.last().expect("OrderedBlocks empty.").round()
    }
}

pub type BufferItemRootType = Cursor;
pub type Sender<T> = UnboundedSender<T>;
pub type Receiver<T> = UnboundedReceiver<T>;

pub fn create_channel<T>() -> (Sender<T>, Receiver<T>) {
    unbounded::<T>()
}

/// BufferManager handles the states of ordered blocks and
/// interacts with the execution phase, the signing phase, and
/// the persisting phase.
pub struct BufferManager {
    author: Author,

    buffer: Buffer<BufferItem>,

    // the roots point to the first *unprocessed* item.
    // None means no items ready to be processed (either all processed or no item finishes previous stage)
    execution_root: BufferItemRootType,
    execution_schedule_phase_tx: Sender<CountedRequest<ExecutionRequest>>,
    execution_schedule_phase_rx: Receiver<ExecutionWaitRequest>,
    execution_wait_phase_tx: Sender<CountedRequest<ExecutionWaitRequest>>,
    execution_wait_phase_rx: Receiver<ExecutionResponse>,

    signing_root: BufferItemRootType,
    signing_phase_tx: Sender<CountedRequest<SigningRequest>>,
    signing_phase_rx: Receiver<SigningResponse>,

    commit_msg_tx: Arc<NetworkSender>,
    reliable_broadcast: ReliableBroadcast<CommitMessage, ExponentialBackoff>,
    commit_proof_rb_handle: Option<DropGuard>,

    // message received from the network
    commit_msg_rx:
        Option<gaptos::aptos_channels::aptos_channel::Receiver<AccountAddress, IncomingCommitRequest>>,

    persisting_phase_tx: Sender<CountedRequest<PersistingRequest>>,
    persisting_phase_rx: Receiver<ExecutorResult<Round>>,

    block_rx: UnboundedReceiver<OrderedBlocks>,
    reset_rx: UnboundedReceiver<ResetRequest>,

    // self channel to retry execution schedule phase
    execution_schedule_retry_tx: UnboundedSender<()>,
    execution_schedule_retry_rx: UnboundedReceiver<()>,

    stop: bool,

    epoch_state: Arc<EpochState>,

    ongoing_tasks: Arc<AtomicU64>,
    // Since proposal_generator is not aware of reconfiguration any more, the suffix blocks
    // will not have the same timestamp as the reconfig block which violates the invariant
    // that block.timestamp == state.timestamp because no txn is executed in suffix blocks.
    // We change the timestamp field of the block info to maintain the invariant.
    // If the executed blocks are b1 <- b2 <- r <- b4 <- b5 with timestamp t1..t5
    // we replace t5 with t3 (from reconfiguration block) since that's the last timestamp
    // being updated on-chain.
    end_epoch_timestamp: OnceCell<u64>,
    previous_commit_time: Instant,
    reset_flag: Arc<AtomicBool>,
    bounded_executor: BoundedExecutor,
    order_vote_enabled: bool,
    highest_committed_round: Round,
    latest_round: Round,

    // Consensus publisher for downstream observers.
    consensus_observer_config: ConsensusObserverConfig,
    consensus_publisher: Option<Arc<ConsensusPublisher>>,

    commit_vote_cache: HashMap<HashValue, HashMap<HashValue, CommitVote>>,
}

impl BufferManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        author: Author,
        execution_schedule_phase_tx: Sender<CountedRequest<ExecutionRequest>>,
        execution_schedule_phase_rx: Receiver<ExecutionWaitRequest>,
        execution_wait_phase_tx: Sender<CountedRequest<ExecutionWaitRequest>>,
        execution_wait_phase_rx: Receiver<ExecutionResponse>,
        signing_phase_tx: Sender<CountedRequest<SigningRequest>>,
        signing_phase_rx: Receiver<SigningResponse>,
        commit_msg_tx: Arc<NetworkSender>,
        commit_msg_rx: gaptos::aptos_channels::aptos_channel::Receiver<
            AccountAddress,
            IncomingCommitRequest,
        >,
        persisting_phase_tx: Sender<CountedRequest<PersistingRequest>>,
        persisting_phase_rx: Receiver<ExecutorResult<Round>>,
        block_rx: UnboundedReceiver<OrderedBlocks>,
        reset_rx: UnboundedReceiver<ResetRequest>,
        epoch_state: Arc<EpochState>,
        ongoing_tasks: Arc<AtomicU64>,
        reset_flag: Arc<AtomicBool>,
        executor: BoundedExecutor,
        order_vote_enabled: bool,
        highest_committed_round: Round,
        consensus_observer_config: ConsensusObserverConfig,
        consensus_publisher: Option<Arc<ConsensusPublisher>>,
    ) -> Self {
        let buffer = Buffer::<BufferItem>::new();

        let rb_backoff_policy =
            ExponentialBackoff::from_millis(2).factor(50).max_delay(Duration::from_secs(5));

        let (tx, rx) = unbounded();

        Self {
            author,

            buffer,

            execution_root: None,
            execution_schedule_phase_tx,
            execution_schedule_phase_rx,
            execution_wait_phase_tx,
            execution_wait_phase_rx,

            signing_root: None,
            signing_phase_tx,
            signing_phase_rx,

            reliable_broadcast: ReliableBroadcast::new(
                author,
                epoch_state.verifier.get_ordered_account_addresses(),
                commit_msg_tx.clone(),
                rb_backoff_policy,
                TimeService::real(),
                Duration::from_millis(COMMIT_VOTE_BROADCAST_INTERVAL_MS),
                executor.clone(),
            ),
            commit_proof_rb_handle: None,
            commit_msg_tx,
            commit_msg_rx: Some(commit_msg_rx),

            persisting_phase_tx,
            persisting_phase_rx,

            block_rx,
            reset_rx,

            execution_schedule_retry_tx: tx,
            execution_schedule_retry_rx: rx,

            stop: false,

            epoch_state,
            ongoing_tasks,
            end_epoch_timestamp: OnceCell::new(),
            previous_commit_time: Instant::now(),
            reset_flag,
            bounded_executor: executor,
            order_vote_enabled,
            highest_committed_round,
            latest_round: highest_committed_round,

            consensus_observer_config,
            consensus_publisher,
            commit_vote_cache: HashMap::new(),
        }
    }

    fn do_reliable_broadcast(&self, message: CommitMessage) -> Option<DropGuard> {
        // If consensus observer is enabled, we don't need to broadcast
        if self.consensus_observer_config.observer_enabled {
            return None;
        }

        // Otherwise, broadcast the message and return the drop guard
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let task = self.reliable_broadcast.broadcast(
            message,
            AckState::new(self.epoch_state.verifier.get_ordered_account_addresses_iter()),
        );
        tokio::spawn(Abortable::new(task, abort_registration));
        Some(DropGuard::new(abort_handle))
    }

    fn create_new_request<Request>(&self, req: Request) -> CountedRequest<Request> {
        CountedRequest::new(req, self.ongoing_tasks.clone())
    }

    fn spawn_retry_request<T: Send + 'static>(
        mut sender: Sender<T>,
        request: T,
        duration: Duration,
    ) {
        counters::BUFFER_MANAGER_RETRY_COUNT.inc();
        spawn_named!("retry request", async move {
            tokio::time::sleep(duration).await;
            sender.send(request).await.expect("Failed to send retry request");
        });
    }

    /// process incoming ordered blocks
    /// push them into the buffer and update the roots if they are none.
    async fn process_ordered_blocks(&mut self, ordered_blocks: OrderedBlocks) {
        let OrderedBlocks { ordered_blocks, ordered_proof, callback } = ordered_blocks;

        info!(
            "Receive ordered block {}, the queue size is {}",
            ordered_proof.commit_info(),
            self.buffer.len() + 1,
        );

        let request = self.create_new_request(ExecutionRequest {
            ordered_blocks: ordered_blocks.clone(),
            lifetime_guard: self.create_new_request(()),
        });
        if let Some(consensus_publisher) = &self.consensus_publisher {
            let message = ConsensusObserverMessage::new_ordered_block_message(
                ordered_blocks.clone().into_iter().map(Arc::new).collect(),
                ordered_proof.clone(),
            );
            consensus_publisher.publish_message(message).await;
        }
        self.execution_schedule_phase_tx
            .send(request)
            .await
            .expect("Failed to send execution schedule request");

        let item = BufferItem::new_ordered(ordered_blocks, ordered_proof, callback);
        self.buffer.push_back(item);
    }

    /// Set the execution root to the first not executed item (Ordered) and send execution request
    /// Set to None if not exist
    /// Return Some(block_id) if the block needs to be scheduled for retry
    fn advance_execution_root(&mut self) -> Option<HashValue> {
        let cursor = self.execution_root;
        self.execution_root =
            self.buffer.find_elem_from(cursor.or_else(|| *self.buffer.head_cursor()), |item| {
                item.is_ordered()
            });
        if self.execution_root.is_some() && cursor == self.execution_root {
            // Schedule retry.
            self.execution_root
        } else {
            info!("Advance execution root from {:?} to {:?}", cursor, self.execution_root);
            // Otherwise do nothing, because the execution wait phase is driven by the response of
            // the execution schedule phase, which is in turn fed as soon as the ordered blocks
            // come in.
            None
        }
    }

    /// Set the signing root to the first not signed item (Executed) and send execution request
    /// Set to None if not exist
    async fn advance_signing_root(&mut self) {
        let cursor = self.signing_root;
        self.signing_root =
            self.buffer.find_elem_from(cursor.or_else(|| *self.buffer.head_cursor()), |item| {
                item.is_executed()
            });
        info!("Advance signing root from {:?} to {:?}", cursor, self.signing_root);
        if self.signing_root.is_some() {
            let item = self.buffer.get(&self.signing_root);
            let executed_item = item.unwrap_executed_ref();
            let request = self.create_new_request(SigningRequest {
                ordered_ledger_info: executed_item.ordered_proof.clone(),
                commit_ledger_info: executed_item.partial_commit_proof.ledger_info().clone(),
            });
            if cursor == self.signing_root {
                let sender = self.signing_phase_tx.clone();
                Self::spawn_retry_request(sender, request, Duration::from_millis(100));
            } else {
                self.signing_phase_tx.send(request).await.expect("Failed to send signing request");
            }
        }
    }

    /// Pop the prefix of buffer items until (including) target_block_id
    /// Send persist request.
    async fn advance_head(&mut self, target_block_id: HashValue) {
        let mut blocks_to_persist: Vec<Arc<PipelinedBlock>> = vec![];

        while let Some(item) = self.buffer.pop_front() {
            blocks_to_persist.extend(
                item.get_blocks()
                    .iter()
                    .map(|eb| Arc::new(eb.clone()))
                    .collect::<Vec<Arc<PipelinedBlock>>>(),
            );
            if self.signing_root == Some(item.block_id()) {
                self.signing_root = None;
            }
            if self.execution_root == Some(item.block_id()) {
                self.execution_root = None;
            }
            if item.block_id() == target_block_id {
                let aggregated_item = item.unwrap_aggregated();
                let block = aggregated_item
                    .executed_blocks
                    .last()
                    .expect("executed_blocks should be not empty")
                    .block();
                observe_block(block.timestamp_usecs(), BlockStage::COMMIT_CERTIFIED);
                // As all the validators broadcast commit votes directly to all other validators,
                // the proposer do not have to broadcast commit decision again.
                let commit_proof = aggregated_item.commit_proof.clone();
                if commit_proof.ledger_info().ends_epoch() {
                    // the epoch ends, reset to avoid executing more blocks, execute after
                    // this persisting request will result in BlockNotFound
                    self.reset().await;
                }
                if let Some(consensus_publisher) = &self.consensus_publisher {
                    let message =
                        ConsensusObserverMessage::new_commit_decision_message(commit_proof.clone());
                    consensus_publisher.publish_message(message).await;
                }
                counters::SEND_TO_PERSISTING_BLOCK_COUNTER.inc_by(blocks_to_persist.len().try_into().unwrap());
                self.persisting_phase_tx
                    .send(self.create_new_request(PersistingRequest {
                        blocks: blocks_to_persist,
                        commit_ledger_info: aggregated_item.commit_proof,
                        // we use the last callback
                        // this is okay because the callback function (from BlockStore::commit)
                        // takes in the actual blocks and ledger info from the state computer
                        // the encoded values are references to the block_tree, storage, and a commit root
                        // the block_tree and storage are the same for all the callbacks in the current epoch
                        // the commit root is used in logging only.
                        callback: aggregated_item.callback,
                    }))
                    .await
                    .expect("Failed to send persist request");
                // this needs to be done after creating the persisting request to avoid it being lost
                // TODO(gravity_alex): Move into persisting phase like aptos
                if commit_proof.ledger_info().ends_epoch() {
                    self.commit_msg_tx
                        .send_epoch_change(EpochChangeProof::new(vec![commit_proof], false))
                        .await;
                }
                info!("Advance head to {:?}", self.buffer.head_cursor());
                self.previous_commit_time = Instant::now();
                return;
            }
        }
        unreachable!("Aggregated item not found in the list");
    }

    /// Reset any request in buffer manager, this is important to avoid race condition with state sync.
    /// Internal requests are managed with ongoing_tasks.
    /// Incoming ordered blocks are pulled, it should only have existing blocks but no new blocks until reset finishes.
    async fn reset(&mut self) {
        self.buffer = Buffer::new();
        self.execution_root = None;
        self.signing_root = None;
        self.previous_commit_time = Instant::now();
        self.commit_proof_rb_handle.take();
        // purge the incoming blocks queue
        while let Ok(Some(_)) = self.block_rx.try_next() {}
        // Wait for ongoing tasks to finish before sending back ack.
        get_block_buffer_manager().release_inflight_blocks().await;
        while self.ongoing_tasks.load(Ordering::SeqCst) > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// It pops everything in the buffer and if reconfig flag is set, it stops the main loop
    async fn process_reset_request(&mut self, request: ResetRequest) {
        let ResetRequest { tx, signal } = request;
        info!("Receive reset");
        self.reset_flag.store(true, Ordering::SeqCst);

        match signal {
            ResetSignal::Stop => self.stop = true,
            ResetSignal::TargetRound(round) => {
                self.highest_committed_round = round;
                self.latest_round = round;
            }
        }

        self.reset().await;
        let _ = tx.send(ResetAck::default());
        self.reset_flag.store(false, Ordering::SeqCst);
        info!("Reset finishes");
    }

    async fn process_execution_schedule_response(&mut self, response: ExecutionWaitRequest) {
        // pass through to the execution wait phase
        let request = self.create_new_request(response);
        self.execution_wait_phase_tx
            .send(request)
            .await
            .expect("Failed to send execution wait request.");
    }

    async fn retry_schedule_phase(&mut self) {
        let mut cursor = self.execution_root;
        let mut count = 0;
        while cursor.is_some() {
            let ordered_blocks = self.buffer.get(&cursor).get_blocks().clone();
            let request = self.create_new_request(ExecutionRequest {
                ordered_blocks,
                lifetime_guard: self.create_new_request(()),
            });
            count += 1;
            self.execution_schedule_phase_tx
                .send(request)
                .await
                .expect("Failed to send execution schedule request.");
            cursor = self.buffer.get_next(&cursor);
        }
        info!("Reschedule {} execution requests from {:?}", count, self.execution_root);
    }

    /// If the response is successful, advance the item to Executed, otherwise panic (TODO fix).
    #[allow(clippy::unwrap_used)]
    async fn process_execution_response(&mut self, response: ExecutionResponse) {
        let ExecutionResponse { block_id, inner } = response;
        // find the corresponding item, may not exist if a reset or aggregated happened
        let current_cursor = self.buffer.find_elem_by_key(self.execution_root, block_id);
        if current_cursor.is_none() {
            return;
        }

        let executed_blocks = match inner {
            Ok(result) => result,
            Err(e) => {
                log_executor_error_occurred(
                    e,
                    &counters::BUFFER_MANAGER_RECEIVED_EXECUTOR_ERROR_COUNT,
                    block_id,
                );
                return;
            }
        };
        info!("Receive executed response {}", executed_blocks.last().unwrap().block_info());
        let current_item = self.buffer.get(&current_cursor);

        if current_item.block_id() != block_id {
            error!(
                block_id = block_id,
                expected_block_id = current_item.block_id(),
                "Received result for unexpected block id. Ignoring."
            );
            return;
        }

        // Handle reconfiguration timestamp reconciliation.
        // end epoch timestamp is set to the first block that causes the reconfiguration.
        // once it's set, any subsequent block commit info will be set to this timestamp.
        if self.end_epoch_timestamp.get().is_none() {
            let maybe_reconfig_timestamp = executed_blocks
                .iter()
                .find(|b| b.block_info().has_reconfiguration())
                .map(|b| b.timestamp_usecs());
            if let Some(timestamp) = maybe_reconfig_timestamp {
                debug!("Reconfig happens, set epoch end timestamp to {}", timestamp);
                self.end_epoch_timestamp
                    .set(timestamp)
                    .expect("epoch end timestamp should only be set once");
            }
        }

        let item = self.buffer.take(&current_cursor);
        let new_item = item.advance_to_executed_or_aggregated(
            executed_blocks,
            &self.epoch_state.verifier,
            self.end_epoch_timestamp.get().cloned(),
            self.order_vote_enabled,
        );
        let aggregated = new_item.is_aggregated();
        self.buffer.set(&current_cursor, new_item);
        if aggregated {
            self.advance_head(block_id).await;
        }
    }

    /// If the signing response is successful, advance the item to Signed and broadcast commit votes.
    async fn process_signing_response(&mut self, response: SigningResponse) {
        let SigningResponse { signature_result, commit_ledger_info } = response;
        let signature = match signature_result {
            Ok(sig) => sig,
            Err(e) => {
                error!("Signing failed {:?}", e);
                return;
            }
        };
        info!("Receive signing response {}", commit_ledger_info.commit_info());
        // find the corresponding item, may not exist if a reset or aggregated happened
        let current_cursor =
            self.buffer.find_elem_by_key(self.signing_root, commit_ledger_info.commit_info().id());
        if current_cursor.is_some() {
            let item = self.buffer.take(&current_cursor);
            // it is possible that we already signed this buffer item (double check after the final integration)
            if item.is_executed() {
                // we have found the buffer item
                let mut signed_item = item.advance_to_signed(self.author, signature);
                let signed_item_mut = signed_item.unwrap_signed_mut();
                let commit_vote = signed_item_mut.commit_vote.clone();
                let commit_vote = CommitMessage::Vote(commit_vote);
                signed_item_mut.rb_handle =
                    self.do_reliable_broadcast(commit_vote).map(|handle| (Instant::now(), handle));
                self.buffer.set(&current_cursor, signed_item);
            } else {
                self.buffer.set(&current_cursor, item);
            }
        }
    }

    fn add_signature_if_matched_from_cache(
        &mut self,
        item: &mut BufferItem,
        target_block_id: &HashValue,
    ) {
        if let Some(cache) = self.commit_vote_cache.get(target_block_id) {
            cache.iter().for_each(|(_, vote)| {
                match item.add_signature_if_matched(vote.clone()) {
                    Ok(_) => {}
                    Err(e) => {
                        error!(commit_info = ?item.commit_info(), target_block_id = ?target_block_id, vote = ?vote, "Failed to add commit vote from cache");
                    }
                }
            });
            self.commit_vote_cache.remove(target_block_id);
        }
    }

    /// process the commit vote messages
    /// it scans the whole buffer for a matching blockinfo
    /// if found, try advancing the item to be aggregated
    fn process_commit_message(&mut self, commit_msg: IncomingCommitRequest) -> Option<HashValue> {
        let IncomingCommitRequest { req, protocol, response_sender } = commit_msg;
        match req {
            CommitMessage::Vote(vote) => {
                // find the corresponding item
                let author = vote.author();
                let commit_info = vote.commit_info().clone();
                info!("Receive commit vote {} from {}", commit_info, author);
                if commit_info.round() >= self.latest_round {
                    if !self.commit_vote_cache.contains_key(&commit_info.id()) {
                        self.commit_vote_cache.insert(commit_info.id(), HashMap::new());
                    }
                    self.commit_vote_cache
                        .get_mut(&commit_info.id())
                        .unwrap()
                        .insert(HashValue::new(*author), vote.clone());
                }
                let target_block_id = vote.commit_info().id();
                let current_cursor =
                    self.buffer.find_elem_by_key(*self.buffer.head_cursor(), target_block_id);
                if current_cursor.is_some() {
                    let mut item = self.buffer.take(&current_cursor);
                    self.add_signature_if_matched_from_cache(&mut item, &target_block_id);
                    let new_item = match item.add_signature_if_matched(vote) {
                        Ok(()) => {
                            let response =
                                ConsensusMsg::CommitMessage(Box::new(CommitMessage::Ack(())));
                            if let Ok(bytes) = protocol.to_bytes(&response) {
                                let _ = response_sender.send(Ok(bytes.into()));
                            }
                            item.try_advance_to_aggregated(&self.epoch_state.verifier)
                        }
                        Err(e) => {
                            error!(
                                error = ?e,
                                author = author,
                                commit_info = commit_info,
                                "Failed to add commit vote",
                            );
                            reply_nack(protocol, response_sender);
                            item
                        }
                    };
                    self.buffer.set(&current_cursor, new_item);
                    if self.buffer.get(&current_cursor).is_aggregated() {
                        return Some(target_block_id);
                    } else {
                        return None;
                    }
                }
            }
            CommitMessage::Decision(commit_proof) => {
                let target_block_id = commit_proof.ledger_info().commit_info().id();
                let cursor =
                    self.buffer.find_elem_by_key(*self.buffer.head_cursor(), target_block_id);
                info!("Receive commit decision {}", commit_proof.ledger_info().commit_info());
                if cursor.is_some() {
                    let item = self.buffer.take(&cursor);
                    let new_item = item.try_advance_to_aggregated_with_ledger_info(
                        commit_proof.ledger_info().clone(),
                    );
                    let aggregated = new_item.is_aggregated();
                    self.buffer.set(&cursor, new_item);
                    if aggregated {
                        let response =
                            ConsensusMsg::CommitMessage(Box::new(CommitMessage::Ack(())));
                        if let Ok(bytes) = protocol.to_bytes(&response) {
                            let _ = response_sender.send(Ok(bytes.into()));
                        }
                        return Some(target_block_id);
                    }
                }
                reply_nack(protocol, response_sender); // TODO: send_commit_proof() doesn't care about the response and this should be direct send not RPC
            }
            CommitMessage::Ack(_) => {
                // It should be filtered out by verify, so we log errors here
                error!("Unexpected ack message");
            }
            CommitMessage::Nack => {
                error!("Unexpected NACK message");
            }
        }
        None
    }

    /// this function retries all the items until the signing root
    /// note that there might be other signed items after the signing root
    async fn rebroadcast_commit_votes_if_needed(&mut self) {
        if self.previous_commit_time.elapsed()
            < Duration::from_millis(COMMIT_VOTE_BROADCAST_INTERVAL_MS)
        {
            return;
        }
        let mut cursor = *self.buffer.head_cursor();
        let mut count = 0;
        while cursor.is_some() {
            {
                let mut item = self.buffer.take(&cursor);
                if !item.is_signed() {
                    self.buffer.set(&cursor, item);
                    break;
                }
                let signed_item = item.unwrap_signed_mut();
                let re_broadcast = match &signed_item.rb_handle {
                    None => true,
                    // Since we don't persist the votes, nodes that crashed would lose the votes even after send ack,
                    // We'll try to re-initiate the broadcast after 30s.
                    Some((start_time, _)) => {
                        start_time.elapsed()
                            >= Duration::from_millis(COMMIT_VOTE_REBROADCAST_INTERVAL_MS)
                    }
                };
                if re_broadcast {
                    let commit_vote = CommitMessage::Vote(signed_item.commit_vote.clone());
                    signed_item.rb_handle = self
                        .do_reliable_broadcast(commit_vote)
                        .map(|handle| (Instant::now(), handle));
                    count += 1;
                }
                self.buffer.set(&cursor, item);
            }
            cursor = self.buffer.get_next(&cursor);
        }
        if count > 0 {
            info!("Start reliable broadcast {} commit votes", count);
        }
    }

    fn update_buffer_manager_metrics(&self) {
        let mut cursor = *self.buffer.head_cursor();
        let mut pending_ordered = 0;
        let mut pending_executed = 0;
        let mut pending_signed = 0;
        let mut pending_aggregated = 0;

        while cursor.is_some() {
            match self.buffer.get(&cursor) {
                BufferItem::Ordered(_) => {
                    pending_ordered += 1;
                }
                BufferItem::Executed(_) => {
                    pending_executed += 1;
                }
                BufferItem::Signed(_) => {
                    pending_signed += 1;
                }
                BufferItem::Aggregated(_) => {
                    pending_aggregated += 1;
                }
            }
            cursor = self.buffer.get_next(&cursor);
        }

        counters::NUM_BLOCKS_IN_PIPELINE
            .with_label_values(&["ordered"])
            .set(pending_ordered as i64);
        counters::NUM_BLOCKS_IN_PIPELINE
            .with_label_values(&["executed"])
            .set(pending_executed as i64);
        counters::NUM_BLOCKS_IN_PIPELINE.with_label_values(&["signed"]).set(pending_signed as i64);
        counters::NUM_BLOCKS_IN_PIPELINE
            .with_label_values(&["aggregated"])
            .set(pending_aggregated as i64);
    }

    fn need_backpressure(&self) -> bool {
        const MAX_BACKLOG: Round = 20;
        self.highest_committed_round + MAX_BACKLOG < self.latest_round
    }

    pub async fn start(mut self) {
        info!("Buffer manager starts.");
        let (verified_commit_msg_tx, mut verified_commit_msg_rx) = create_channel();
        let mut interval = tokio::time::interval(Duration::from_millis(LOOP_INTERVAL_MS));
        let mut commit_msg_rx = self.commit_msg_rx.take().expect("commit msg rx must exist");
        let epoch_state = self.epoch_state.clone();
        let bounded_executor = self.bounded_executor.clone();
        spawn_named!("buffer manager verification", async move {
            while let Some(commit_msg) = commit_msg_rx.next().await {
                let tx = verified_commit_msg_tx.clone();
                let epoch_state_clone = epoch_state.clone();
                bounded_executor
                    .spawn(async move {
                        match commit_msg.req.verify(&epoch_state_clone.verifier) {
                            Ok(_) => {
                                let _ = tx.unbounded_send(commit_msg);
                            }
                            Err(e) => warn!("Invalid commit message: {}", e),
                        }
                    })
                    .await;
            }
        });
        while !self.stop {
            // advancing the root will trigger sending requests to the pipeline
            counters::EXECUTED_BLOCK_COUNTER
                .set((self.latest_round as f64 - self.highest_committed_round as f64));
            ::tokio::select! {
                Some(blocks) = self.block_rx.next(), if !self.need_backpressure() => {
                    self.latest_round = blocks.latest_round();
                    counters::CREATED_EXECUTED_BLOCK_COUNTER.set(self.latest_round as f64);
                    monitor!("buffer_manager_process_ordered", {
                    self.process_ordered_blocks(blocks).await;
                    if self.execution_root.is_none() {
                        self.advance_execution_root();
                    }});
                },
                Some(reset_event) = self.reset_rx.next() => {
                    monitor!("buffer_manager_process_reset",
                    self.process_reset_request(reset_event).await);
                },
                Some(response) = self.execution_schedule_phase_rx.next() => {
                    monitor!("buffer_manager_process_execution_schedule_response", {
                    self.process_execution_schedule_response(response).await;
                })},
                Some(response) = self.execution_wait_phase_rx.next() => {
                    monitor!("buffer_manager_process_execution_wait_response", {
                    let response_block_id = response.block_id;
                    self.process_execution_response(response).await;
                    if let Some(block_id) = self.advance_execution_root() {
                        // if the response is for the current execution root, retry the schedule phase
                        if response_block_id == block_id {
                            let mut tx = self.execution_schedule_retry_tx.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                // buffer manager can be dropped at the point of sending retry
                                let _ = tx.send(()).await;
                            });
                        }
                    }
                    if self.signing_root.is_none() {
                        self.advance_signing_root().await;
                    }});
                },
                _ = self.execution_schedule_retry_rx.next() => {
                    monitor!("buffer_manager_process_execution_schedule_retry",
                    self.retry_schedule_phase().await);
                },
                Some(response) = self.signing_phase_rx.next() => {
                    monitor!("buffer_manager_process_signing_response", {
                    self.process_signing_response(response).await;
                    self.advance_signing_root().await
                    })
                },
                Some(Ok(round)) = self.persisting_phase_rx.next() => {
                    // see where `need_backpressure()` is called.
                    self.highest_committed_round = round;
                    counters::FINALIZED_EXECUTED_BLOCK_COUNTER.set(self.highest_committed_round as f64);
                },
                Some(rpc_request) = verified_commit_msg_rx.next() => {
                    monitor!("buffer_manager_process_commit_message",
                    if let Some(aggregated_block_id) = self.process_commit_message(rpc_request) {
                        self.advance_head(aggregated_block_id).await;
                        if self.execution_root.is_none() {
                            self.advance_execution_root();
                        }
                        if self.signing_root.is_none() {
                            self.advance_signing_root().await;
                        }
                    });
                }
                _ = interval.tick().fuse() => {
                    monitor!("buffer_manager_process_interval_tick", {
                    self.update_buffer_manager_metrics();
                    self.rebroadcast_commit_votes_if_needed().await
                    });
                },
                // no else branch here because interval.tick will always be available
            }
        }
        info!("Buffer manager stops.");
    }
}

fn reply_nack(protocol: ProtocolId, response_sender: oneshot::Sender<Result<Bytes, RpcError>>) {
    let response = ConsensusMsg::CommitMessage(Box::new(CommitMessage::Nack));
    if let Ok(bytes) = protocol.to_bytes(&response) {
        let _ = response_sender.send(Ok(bytes.into()));
    }
}
