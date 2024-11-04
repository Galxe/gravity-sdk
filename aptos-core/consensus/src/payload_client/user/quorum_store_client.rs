// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_storage::BlockStore, counters::WAIT_FOR_FULL_BLOCKS_TRIGGERED, error::QuorumStoreError,
    monitor, payload_client::user::UserPayloadClient,
};

use api_types::{BatchClient, BlockHashState, ConsensusApi};
use aptos_consensus_types::{
    common::{Payload, PayloadFilter},
    request_response::{GetPayloadCommand, GetPayloadResponse},
};
use aptos_crypto::HashValue;
use aptos_logger::info;
use aptos_types::transaction::SignedTransaction;
use fail::fail_point;
use futures::{future::BoxFuture, FutureExt, SinkExt};
use futures_channel::mpsc::SendError;
use futures_channel::{mpsc, oneshot};
use once_cell::sync::OnceCell;
use std::fmt::Debug;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::Mutex,
    time::{sleep, timeout},
};

const NO_TXN_DELAY: u64 = 30;

pub fn debug_block_hash_state(state: &BlockHashState) -> String {
    format!(
        "safe_hash: {:?}, head_hash: {:?}, finalized_hash: {:?}",
        HashValue::new(state.safe_hash),
        HashValue::new(state.head_hash),
        HashValue::new(state.finalized_hash),
    )
}

/// Client that pulls blocks from Quorum Store
#[derive(Clone)]
pub struct QuorumStoreClient {
    consensus_to_quorum_store_sender: mpsc::Sender<GetPayloadCommand>,
    /// Timeout for consensus to pull transactions from quorum store and get a response (in milliseconds)
    pull_timeout_ms: u64,
    wait_for_full_blocks_above_recent_fill_threshold: f32,
    wait_for_full_blocks_above_pending_blocks: usize,
    block_store: OnceCell<Arc<BlockStore>>,
    batch_client: Arc<BatchClient>,
    consensus_engine: OnceCell<Arc<dyn ConsensusApi>>,
    init_hash: Arc<Mutex<Option<BlockHashState>>>,
}

impl QuorumStoreClient {
    pub fn new(
        consensus_to_quorum_store_sender: mpsc::Sender<GetPayloadCommand>,
        pull_timeout_ms: u64,
        wait_for_full_blocks_above_recent_fill_threshold: f32,
        wait_for_full_blocks_above_pending_blocks: usize,
    ) -> Self {
        Self {
            consensus_to_quorum_store_sender,
            pull_timeout_ms,
            wait_for_full_blocks_above_recent_fill_threshold,
            wait_for_full_blocks_above_pending_blocks,
            block_store: OnceCell::new(),
            batch_client: Arc::new(BatchClient::new()),
            consensus_engine: OnceCell::new(),
            init_hash: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_consensus_to_quorum_store_sender(&self) -> mpsc::Sender<GetPayloadCommand> {
        self.consensus_to_quorum_store_sender.clone()
    }

    pub fn get_batch_client(&self) -> Arc<BatchClient> {
        self.batch_client.clone()
    }

    pub fn set_consensus_api(&self, consensus_api: Arc<dyn ConsensusApi>) {
        match self.consensus_engine.set(consensus_api) {
            Ok(_) => {}
            Err(_) => {
                panic!("failed to set consensus api to quorum store client")
            }
        }
    }

    pub fn get_block_store(&self) -> Arc<BlockStore> {
        self.block_store.get().expect("block store not set").clone()
    }

    pub fn set_init_reth_hash(
        &self,
        block_hash_state: BlockHashState,
    ) {
        let mut hash = self.init_hash.blocking_lock();
        *hash = Some(block_hash_state);
    }

    pub fn set_block_store(&self, block_store: Arc<BlockStore>) {
        match self.block_store.set(block_store) {
            Ok(_) => {}
            Err(_) => {
                panic!("failed to set block store to quorum store client")
            }
        }
    }

    fn get_safe_block_hash(&self) -> HashValue {
        self.block_store.get().expect("block store not set").get_safe_block_hash()
    }

    fn get_head_block_hash(&self) -> HashValue {
        self.block_store.get().expect("block store not set").get_head_block_hash()
    }

    fn get_finalized_block_hash(&self) -> HashValue {
        self.block_store.get().expect("block store not set").get_finalize_hash()
    }

    async fn pull_internal(
        &self,
        max_items: u64,
        max_items_after_filtering: u64,
        soft_max_items_after_filtering: u64,
        max_bytes: u64,
        max_inline_items: u64,
        max_inline_bytes: u64,
        return_non_full: bool,
        exclude_payloads: PayloadFilter,
        block_timestamp: Duration,
    ) -> anyhow::Result<Payload, QuorumStoreError> {
        let (callback, callback_rcv) = oneshot::channel();
        let req = GetPayloadCommand::GetPayloadRequest(
            max_items,
            max_items_after_filtering,
            soft_max_items_after_filtering,
            max_bytes,
            max_inline_items,
            max_inline_bytes,
            return_non_full,
            exclude_payloads.clone(),
            callback,
            block_timestamp,
        );
        // send to shared mempool
        let mut sender_capture = self.consensus_to_quorum_store_sender.clone();
        let req_closure: BoxFuture<'static, Result<(), SendError>> =
            async move { sender_capture.send(req).await }.boxed();
        let mut safe_hash = [0u8; 32];
        let mut head_hash = [0u8; 32];
        let mut finalized_hash = [0u8; 32];
        {
            let init_hash = self.init_hash.lock().await;

            if self.block_store.get().is_none() {
                info!("request payload from init hash without block store");
                finalized_hash.copy_from_slice(init_hash.unwrap().finalized_hash.as_slice());
                safe_hash.copy_from_slice(init_hash.unwrap().safe_hash.as_slice());
                head_hash.copy_from_slice(init_hash.unwrap().head_hash.as_slice());
            } else {
                let block_store = self.block_store.get().unwrap();
                // TODO(gravity_byteyue): avoid clone here
                if !block_store.get_block_tree().read().is_head_block_qc() {
                    info!("reuse payload from block store if the last block qc is not ready");
                    let block = block_store.get_block_tree().read().get_head_block().clone();
                    match block.payload() {
                        Some(payload) => {
                            return Ok(payload.clone());
                        },
                        None => {
                            panic!("block payload is empty {:?}", block);
                        }
                    }
                }
                let is_head_invalid = block_store.get_block_tree().read().is_head_block_payload_none();
                let is_safe_invalid = block_store.get_block_tree().read().is_safe_block_payload_none();
                let is_finalized_invalid = block_store.get_block_tree().read().is_finalized_block_payload_none();
                info!(
                        "request payload, head nil: {}, safe nil: {}, finalized nil: {}",
                        is_head_invalid, is_safe_invalid, is_finalized_invalid
                    );
                if is_head_invalid && is_safe_invalid && is_finalized_invalid {
                    finalized_hash.copy_from_slice(init_hash.unwrap().finalized_hash.as_slice());
                    safe_hash.copy_from_slice(init_hash.unwrap().safe_hash.as_slice());
                    head_hash.copy_from_slice(init_hash.unwrap().head_hash.as_slice());
                } else if is_finalized_invalid && !is_safe_invalid && !is_head_invalid {
                    // head block qc means the safe block qc, too
                    finalized_hash.copy_from_slice(init_hash.unwrap().finalized_hash.as_slice());
                    safe_hash.copy_from_slice(self.get_safe_block_hash().as_ref());
                    head_hash.copy_from_slice(self.get_head_block_hash().as_ref());
                } else if !is_finalized_invalid && !is_safe_invalid && !is_head_invalid {
                    finalized_hash.copy_from_slice(self.get_finalized_block_hash().as_ref());
                    safe_hash.copy_from_slice(self.get_safe_block_hash().as_ref());
                    head_hash.copy_from_slice(self.get_head_block_hash().as_ref());
                } else {
                    panic!("invalid block state with head nil: {}, safe nil: {}, finalized nil: {}", is_head_invalid, is_safe_invalid, is_finalized_invalid);
                }
            }
        }

        let block_batch = self
            .consensus_engine
            .get()
            .expect("consensus engine not set")
            .as_ref()
            .request_payload(req_closure, BlockHashState { safe_hash, head_hash, finalized_hash })
            .await
            .expect("TODO: panic message");

        // TODO(gravity_byteyue: use the following commentted code when qs is ready)
        let txns: Vec<SignedTransaction> =
            block_batch.txns.iter().map(|gtxn| gtxn.clone().into()).collect();
        Ok(Payload::DirectMempool((HashValue::new(block_batch.block_hash), txns)))

        // self.consensus_to_quorum_store_sender.clone().try_send(req).map_err(anyhow::Error::from)?;
        // wait for response
        // match monitor!(
        //     "pull_payload",
        //     timeout(Duration::from_millis(self.pull_timeout_ms), callback_rcv).await
        // ) {
        //     Err(_) => {
        //         Err(anyhow::anyhow!("[consensus] did not receive GetBlockResponse on time").into())
        //     }
        //     Ok(resp) => match resp.map_err(anyhow::Error::from)?? {
        //         GetPayloadResponse::GetPayloadResponse(payload) => Ok(payload),
        //     },
        // }
    }
}

#[async_trait::async_trait]
impl UserPayloadClient for QuorumStoreClient {
    async fn pull(
        &self,
        max_poll_time: Duration,
        max_items: u64,
        max_items_after_filtering: u64,
        soft_max_items_after_filtering: u64,
        max_bytes: u64,
        max_inline_items: u64,
        max_inline_bytes: u64,
        exclude: PayloadFilter,
        wait_callback: BoxFuture<'static, ()>,
        pending_ordering: bool,
        pending_uncommitted_blocks: usize,
        recent_max_fill_fraction: f32,
        block_timestamp: Duration,
    ) -> anyhow::Result<Payload, QuorumStoreError> {
        let return_non_full = recent_max_fill_fraction
            < self.wait_for_full_blocks_above_recent_fill_threshold
            && pending_uncommitted_blocks < self.wait_for_full_blocks_above_pending_blocks;
        let return_empty = pending_ordering && return_non_full;

        WAIT_FOR_FULL_BLOCKS_TRIGGERED.observe(if !return_non_full { 1.0 } else { 0.0 });

        fail_point!("consensus::pull_payload", |_| {
            Err(anyhow::anyhow!("Injected error in pull_payload").into())
        });
        let mut callback_wrapper = Some(wait_callback);
        // keep polling QuorumStore until there's payloads available or there's still pending payloads
        let start_time = Instant::now();

        let payload = loop {
            // Make sure we don't wait more than expected, due to thread scheduling delays/processing time consumed
            // let done = start_time.elapsed() >= max_poll_time;
            let done = true;
            let payload = self
                .pull_internal(
                    max_items,
                    max_items_after_filtering,
                    soft_max_items_after_filtering,
                    max_bytes,
                    max_inline_items,
                    max_inline_bytes,
                    return_non_full || return_empty || done,
                    exclude.clone(),
                    block_timestamp,
                )
                .await?;
            if payload.is_empty() && !return_empty && !done {
                if let Some(callback) = callback_wrapper.take() {
                    callback.await;
                }
                sleep(Duration::from_millis(NO_TXN_DELAY)).await;
                continue;
            }
            break payload;
        };
        info!(
            elapsed_time_ms = start_time.elapsed().as_millis() as u64,
            max_poll_time_ms = max_poll_time.as_millis() as u64,
            payload_len = payload.len(),
            max_items = max_items,
            max_items_after_filtering = max_items_after_filtering,
            soft_max_items_after_filtering = soft_max_items_after_filtering,
            max_bytes = max_bytes,
            max_inline_items = max_inline_items,
            max_inline_bytes = max_inline_bytes,
            pending_ordering = pending_ordering,
            return_empty = return_empty,
            return_non_full = return_non_full,
            duration_ms = start_time.elapsed().as_millis() as u64,
            "Pull payloads from QuorumStore: proposal"
        );

        Ok(payload)
    }
}
