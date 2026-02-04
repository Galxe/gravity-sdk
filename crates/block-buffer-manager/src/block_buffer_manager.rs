use anyhow::format_err;
use aptos_executor_types::StateComputeResult;
use gaptos::{
    api_types::{self, account::ExternalAccountAddress, u256_define::TxnHash},
    aptos_types::{epoch_state::EpochState, idl::convert_validator_set},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex, Notify,
    },
    time::Instant,
};
use tracing::{info, warn};

use gaptos::api_types::{
    compute_res::{ComputeRes, TxnStatus},
    events::contract_event::GravityEvent,
    u256_define::BlockId,
    ExternalBlock, VerifiedTxn, VerifiedTxnWithAccountSeqNum,
};

// Type alias to reduce complexity
type TxFilterFn = Box<dyn Fn((ExternalAccountAddress, u64, TxnHash)) -> bool>;

pub struct TxnItem {
    pub txns: Vec<VerifiedTxnWithAccountSeqNum>,
    pub gas_limit: u64,
    pub insert_time: SystemTime,
}

pub trait TxPool: Send + Sync + 'static {
    fn best_txns(
        &self,
        filter: Option<TxFilterFn>,
        limit: usize,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>>;

    fn get_broadcast_txns(
        &self,
        filter: Option<TxFilterFn>,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>>;

    // add external txns to the tx pool
    fn add_external_txn(&self, txns: VerifiedTxn) -> bool;

    fn remove_txns(&self, txns: Vec<VerifiedTxn>);
}

pub struct EmptyTxPool {}

impl EmptyTxPool {
    pub fn boxed() -> Box<dyn TxPool> {
        Box::new(Self {})
    }
}

impl TxPool for EmptyTxPool {
    fn best_txns(
        &self,
        _filter: Option<TxFilterFn>,
        _limit: usize,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        Box::new(vec![].into_iter())
    }

    fn get_broadcast_txns(
        &self,
        _filter: Option<TxFilterFn>,
    ) -> Box<dyn Iterator<Item = VerifiedTxn>> {
        Box::new(vec![].into_iter())
    }

    fn add_external_txn(&self, _txns: VerifiedTxn) -> bool {
        false
    }

    fn remove_txns(&self, _txns: Vec<VerifiedTxn>) {}
}

pub struct TxnBuffer {
    // (txns, gas_limit)
    txns: Mutex<Vec<TxnItem>>,
}

pub struct BlockHashRef {
    pub block_id: BlockId,
    pub num: u64,
    pub hash: Option<[u8; 32]>,
    pub persist_notifier: Option<Sender<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockKey {
    pub epoch: u64,
    pub block_number: u64,
}

impl BlockKey {
    pub fn new(epoch: u64, block_number: u64) -> Self {
        Self { epoch, block_number }
    }
}

#[derive(Debug)]
pub enum BlockState {
    Ordered {
        block: ExternalBlock,
        parent_id: BlockId,
    },
    Computed {
        id: BlockId,
        compute_result: StateComputeResult,
    },
    Committed {
        hash: Option<[u8; 32]>,
        compute_result: StateComputeResult,
        id: BlockId,
        persist_notifier: Option<Sender<()>>,
    },
    /// Historical block recovered from storage, only has block id
    Historical {
        id: BlockId,
    },
}

impl BlockState {
    pub fn get_block_id(&self) -> BlockId {
        match self {
            BlockState::Ordered { block, .. } => block.block_meta.block_id,
            BlockState::Computed { id, .. } => *id,
            BlockState::Committed { id, .. } => *id,
            BlockState::Historical { id } => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum BufferState {
    Uninitialized,
    Ready,
    EpochChange,
}

#[derive(Default)]
pub struct BlockProfile {
    pub set_ordered_block_time: Option<SystemTime>,
    pub get_ordered_blocks_time: Option<SystemTime>,
    pub set_compute_res_time: Option<SystemTime>,
    pub get_executed_res_time: Option<SystemTime>,
    pub set_commit_blocks_time: Option<SystemTime>,
    pub get_committed_blocks_time: Option<SystemTime>,
}

pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockKey, BlockState>,
    profile: HashMap<BlockKey, BlockProfile>,
    latest_commit_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
    current_epoch: u64,
    next_epoch: Option<u64>,
}

pub struct BlockBufferManagerConfig {
    pub wait_for_change_timeout: Duration,
    pub max_wait_timeout: Duration,
    pub remove_committed_blocks_interval: Duration,
    pub max_block_size: usize,
}

impl Default for BlockBufferManagerConfig {
    fn default() -> Self {
        Self {
            wait_for_change_timeout: Duration::from_millis(100),
            max_wait_timeout: Duration::from_secs(5),
            remove_committed_blocks_interval: Duration::from_secs(1),
            max_block_size: 256,
        }
    }
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    latest_epoch_change_block_number: Mutex<u64>,
    ready_notifier: Arc<Notify>,
}

impl BlockBufferManager {
    pub fn new(config: BlockBufferManagerConfig) -> Arc<Self> {
        let (sender, _recv) = tokio::sync::broadcast::channel(1024);
        let block_buffer_manager = Self {
            txn_buffer: TxnBuffer { txns: Mutex::new(Vec::new()) },
            block_state_machine: Mutex::new(BlockStateMachine {
                sender,
                blocks: HashMap::new(),
                latest_commit_block_number: 0,
                latest_finalized_block_number: 0,
                block_number_to_block_id: HashMap::new(),
                profile: HashMap::new(),
                current_epoch: 0,
                next_epoch: None,
            }),
            buffer_state: AtomicU8::new(BufferState::Uninitialized as u8),
            config,
            latest_epoch_change_block_number: Mutex::new(0),
            ready_notifier: Arc::new(Notify::new()),
        };
        let block_buffer_manager = Arc::new(block_buffer_manager);
        let clone = block_buffer_manager.clone();
        // spawn task to remove committed blocks
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(clone.config.remove_committed_blocks_interval).await;
                clone.remove_committed_blocks().await.unwrap();
            }
        });
        block_buffer_manager
    }

    async fn remove_committed_blocks(&self) -> Result<(), anyhow::Error> {
        let mut block_state_machine = self.block_state_machine.lock().await;
        if block_state_machine.blocks.len() < self.config.max_block_size {
            return Ok(());
        }
        let latest_persist_block_num = block_state_machine.latest_finalized_block_number;
        info!("remove_committed_blocks latest_persist_block_num: {:?}", latest_persist_block_num);
        block_state_machine.latest_finalized_block_number = std::cmp::max(
            block_state_machine.latest_finalized_block_number,
            latest_persist_block_num,
        );
        block_state_machine.blocks.retain(|key, _| key.block_number >= latest_persist_block_num);
        block_state_machine.profile.retain(|key, _| key.block_number >= latest_persist_block_num);
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn init(
        &self,
        latest_commit_block_number: u64,
        block_number_to_block_id_with_epoch: HashMap<u64, (u64, BlockId)>,
        initial_epoch: u64,
    ) {
        info!(
            "init block_buffer_manager with latest_commit_block_number: {:?} block_number_to_block_id count: {} initial_epoch: {}",
            latest_commit_block_number, block_number_to_block_id_with_epoch.len(), initial_epoch
        );
        let mut block_state_machine = self.block_state_machine.lock().await;
        // When init, the latest_finalized_block_number is the same as latest_commit_block_number
        block_state_machine.latest_commit_block_number = latest_commit_block_number;
        block_state_machine.latest_finalized_block_number = latest_commit_block_number;
        // Extract block_id only for block_number_to_block_id mapping
        block_state_machine.block_number_to_block_id = block_number_to_block_id_with_epoch
            .iter()
            .map(|(block_num, (_epoch, block_id))| (*block_num, *block_id))
            .collect();
        // Initialize current_epoch from the parameter
        block_state_machine.current_epoch = initial_epoch;
        if !block_number_to_block_id_with_epoch.is_empty() {
            let (commit_block_epoch, commit_block_id) =
                *block_number_to_block_id_with_epoch.get(&latest_commit_block_number).unwrap();
            block_state_machine.blocks.insert(
                BlockKey::new(commit_block_epoch, latest_commit_block_number),
                BlockState::Historical { id: commit_block_id },
            );
            *self.latest_epoch_change_block_number.lock().await = latest_commit_block_number;
        }
        self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
        // Notify all waiters that buffer is ready
        self.ready_notifier.notify_waiters();
    }

    // Helper method to wait for changes
    async fn wait_for_change(&self, timeout: Duration) -> Result<(), anyhow::Error> {
        let mut receiver = {
            let block_state_machine = self.block_state_machine.lock().await;
            block_state_machine.sender.subscribe()
        };

        tokio::select! {
            _ = receiver.recv() => Ok(()),
            _ = tokio::time::sleep(timeout) => Err(anyhow::anyhow!("Timeout waiting for change"))
        }
    }

    pub async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, anyhow::Error> {
        unimplemented!()
    }

    pub async fn push_txns(&self, txns: &mut Vec<VerifiedTxnWithAccountSeqNum>, gas_limit: u64) {
        tracing::info!("push_txns txns len: {:?}", txns.len());
        let mut pool_txns = self.txn_buffer.txns.lock().await;
        pool_txns.push(TxnItem {
            txns: std::mem::take(txns),
            gas_limit,
            insert_time: SystemTime::now(),
        });
    }

    pub fn is_ready(&self) -> bool {
        self.buffer_state.load(Ordering::SeqCst) != BufferState::Uninitialized as u8
    }

    pub fn is_epoch_change(&self) -> bool {
        self.buffer_state.load(Ordering::SeqCst) == BufferState::EpochChange as u8
    }

    pub async fn consume_epoch_change(&self) -> u64 {
        self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
        let block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.current_epoch
    }

    pub async fn latest_epoch_change_block_number(&self) -> u64 {
        *self.latest_epoch_change_block_number.lock().await
    }

    pub async fn pop_txns(
        &self,
        max_size: usize,
        gas_limit: u64,
    ) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
        let mut txn_buffer = self.txn_buffer.txns.lock().await;
        let mut total_gas_limit = 0;
        let mut count = 0;
        let total_txn = txn_buffer.iter().map(|item| item.txns.len()).sum::<usize>();
        tracing::info!("pop_txns total_txn: {:?}", total_txn);
        let split_point = txn_buffer
            .iter()
            .position(|item| {
                if total_gas_limit == 0 {
                    return false;
                }
                if total_gas_limit + item.gas_limit > gas_limit || count >= max_size {
                    return true;
                }
                total_gas_limit += item.gas_limit;
                count += 1;
                false
            })
            .unwrap_or(txn_buffer.len());
        let valid_item = txn_buffer.drain(0..split_point).collect::<Vec<_>>();
        drop(txn_buffer);
        let mut result = Vec::new();
        for mut item in valid_item {
            result.extend(std::mem::take(&mut item.txns));
        }
        Ok(result)
    }

    pub async fn set_ordered_blocks(
        &self,
        parent_id: BlockId,
        block: ExternalBlock,
    ) -> Result<(), anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        info!(
            "set_ordered_blocks {:?} num {:?} epoch {:?} parent_id {:?}",
            block.block_meta.block_id,
            block.block_meta.block_number,
            block.block_meta.epoch,
            parent_id
        );
        let mut block_state_machine = self.block_state_machine.lock().await;
        let current_epoch = block_state_machine.current_epoch;

        // Check epoch validity
        if block.block_meta.epoch < current_epoch {
            warn!(
                "set_ordered_blocks: ignoring block {} with old epoch {} (current epoch: {})",
                block.block_meta.block_number, block.block_meta.epoch, current_epoch
            );
            return Ok(());
        }

        if block.block_meta.epoch > current_epoch {
            warn!(
                "set_ordered_blocks: ignoring block {} with future epoch {} (current epoch: {})",
                block.block_meta.block_number, block.block_meta.epoch, current_epoch
            );
            return Ok(());
        }

        // At this point: block.block_meta.epoch == current_epoch
        // Check if block (epoch, number) already exists
        let block_key = BlockKey::new(block.block_meta.epoch, block.block_meta.block_number);
        if let Some(existing_state) = block_state_machine.blocks.get(&block_key) {
            let existing_block_id = existing_state.get_block_id();
            if existing_block_id == block.block_meta.block_id {
                warn!(
                    "set_ordered_blocks: block {} with epoch {} and id {:?} already exists",
                    block.block_meta.block_number,
                    block.block_meta.epoch,
                    block.block_meta.block_id
                );
            } else {
                warn!(
                    "set_ordered_blocks: block {} with epoch {} already exists with different id (existing: {:?}, new: {:?})",
                    block.block_meta.block_number, block.block_meta.epoch, existing_block_id, block.block_meta.block_id
                );
            }
            return Ok(());
        }
        let block_num = block.block_meta.block_number;
        // Try to find parent in current epoch first, then try previous epoch
        let parent_key_current =
            BlockKey::new(block.block_meta.epoch, block.block_meta.block_number - 1);
        let parent_key_prev_epoch = if block.block_meta.epoch > 0 {
            Some(BlockKey::new(block.block_meta.epoch - 1, block.block_meta.block_number - 1))
        } else {
            None
        };
        let actual_parent = block_state_machine
            .blocks
            .get(&parent_key_current)
            .or_else(|| parent_key_prev_epoch.and_then(|key| block_state_machine.blocks.get(&key)));
        let actual_parent_id = match (block_num, actual_parent) {
            (_, Some(state)) => state.get_block_id(),
            (block_number, None) => {
                info!("block number {} with no parent", block_number);
                parent_id
            }
        };
        let parent_id = if actual_parent_id == parent_id {
            parent_id
        } else {
            // TODO(gravity_alex): assert epoch
            warn!("set_ordered_blocks parent_id is not the same as actual_parent_id {:?} {:?}, might be epoch change", parent_id, actual_parent_id);
            actual_parent_id
        };

        block_state_machine
            .blocks
            .insert(block_key, BlockState::Ordered { block: block.clone(), parent_id });

        // Record time for set_ordered_blocks
        let profile =
            block_state_machine.profile.entry(block_key).or_insert_with(BlockProfile::default);
        profile.set_ordered_block_time = Some(SystemTime::now());

        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn get_ordered_blocks(
        &self,
        start_num: u64,
        max_size: Option<usize>,
        expected_epoch: u64,
    ) -> Result<Vec<(ExternalBlock, BlockId)>, anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }

        if self.is_epoch_change() {
            return Err(anyhow::anyhow!("Buffer is in epoch change"));
        }

        // Check if expected_epoch matches current_epoch early
        {
            let block_state_machine = self.block_state_machine.lock().await;
            let current_epoch = block_state_machine.current_epoch;
            if expected_epoch != current_epoch {
                warn!(
                    "get_ordered_blocks: expected_epoch {} does not match current_epoch {}",
                    expected_epoch, current_epoch
                );
                return Err(anyhow::anyhow!(
                    "Epoch mismatch: expected {expected_epoch} but current is {current_epoch}"
                ));
            }
        }

        let start = Instant::now();
        info!(
            "call get_ordered_blocks start_num: {:?} max_size: {:?} expected_epoch: {:?}",
            start_num, max_size, expected_epoch
        );
        loop {
            if start.elapsed() > self.config.max_wait_timeout {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for ordered blocks after {:?} block_number: {:?}",
                    start.elapsed(),
                    start_num
                ));
            }

            let mut block_state_machine = self.block_state_machine.lock().await;
            // get block num, block num + 1
            let mut result = Vec::new();
            let mut current_num = start_num;
            loop {
                let block_key = BlockKey::new(expected_epoch, current_num);
                match block_state_machine.blocks.get(&block_key) {
                    Some(BlockState::Ordered { block, parent_id }) => {
                        result.push((block.clone(), *parent_id));
                        // Record time for get_ordered_blocks
                        let profile = block_state_machine
                            .profile
                            .entry(block_key)
                            .or_insert_with(BlockProfile::default);
                        profile.get_ordered_blocks_time = Some(SystemTime::now());
                    }
                    Some(state) => {
                        panic!(
                            "get_ordered_blocks: found block (epoch: {expected_epoch}, num: {current_num}) in non-Ordered state: {state:?}"
                        );
                    }
                    None => {
                        // No more blocks available
                        break;
                    }
                }
                if result.len() >= max_size.unwrap_or(usize::MAX) {
                    break;
                }
                current_num += 1;
            }

            // If no blocks available, wait and retry
            if result.is_empty() {
                // Release lock before waiting
                drop(block_state_machine);
                // Wait for changes and try again
                match self.wait_for_change(self.config.wait_for_change_timeout).await {
                    Ok(_) => continue,
                    Err(_) => continue, // Timeout on the wait, retry
                }
            }

            return Ok(result);
        }
    }

    pub async fn get_executed_res(
        &self,
        block_id: BlockId,
        block_num: u64,
        epoch: u64,
    ) -> Result<StateComputeResult, anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        let start = Instant::now();
        info!("get_executed_res start {:?} num {:?}", block_id, block_num);
        loop {
            if start.elapsed() > self.config.max_wait_timeout {
                return Err(anyhow::anyhow!(
                    "get_executed_res timeout for block {:?} after {:?} block_number: {:?}",
                    block_id,
                    start.elapsed(),
                    block_num
                ));
            }

            let mut block_state_machine = self.block_state_machine.lock().await;
            let block_key = BlockKey::new(epoch, block_num);
            if let Some(block) = block_state_machine.blocks.get(&block_key) {
                match block {
                    BlockState::Computed { id, compute_result } => {
                        assert_eq!(id, &block_id);

                        // Record time for get_executed_res
                        let compute_res_clone = compute_result.clone();
                        let profile = block_state_machine
                            .profile
                            .entry(block_key)
                            .or_insert_with(BlockProfile::default);
                        profile.get_executed_res_time = Some(SystemTime::now());
                        info!(
                            "get_executed_res done with id {:?} num {:?} res {:?}",
                            block_id, block_num, compute_res_clone,
                        );
                        return Ok(compute_res_clone);
                    }
                    BlockState::Ordered { .. } => {
                        // Release lock before waiting
                        drop(block_state_machine);

                        // Wait for changes and try again
                        match self.wait_for_change(self.config.wait_for_change_timeout).await {
                            Ok(_) => continue,
                            Err(_) => continue, // Timeout on the wait, retry
                        }
                    }
                    BlockState::Committed { hash: _, compute_result, id, persist_notifier: _ } => {
                        warn!(
                            "get_executed_res done with id {:?} num {:?} res {:?}",
                            block_id, id, compute_result
                        );
                        assert_eq!(id, &block_id);
                        return Ok(compute_result.clone());
                    }
                    BlockState::Historical { id } => {
                        // Historical blocks don't have compute_result, this is an error case
                        return Err(anyhow::anyhow!(
                            "Cannot get executed result for historical block {id:?} num {block_num:?}"
                        ));
                    }
                }
            } else {
                // invariant: the missed block is removed after epoch change
                // panic!(
                //     "There is no Ordered Block but try to get executed result for block {:?}",
                //     block_id
                // )
                let latest_epoch_change_block_number =
                    *self.latest_epoch_change_block_number.lock().await;
                let msg = format!("There is no Ordered Block but try to get executed result for block {block_id:?} and block num {block_num:?}, latest epoch change block number {latest_epoch_change_block_number:?}");
                warn!("{}", msg);
                return Err(anyhow::anyhow!("{msg}"));
            }
        }
    }

    async fn calculate_new_epoch_state(
        &self,
        events: &[GravityEvent],
        block_num: u64,
        block_state_machine: &mut BlockStateMachine,
    ) -> Result<Option<EpochState>, anyhow::Error> {
        if events.is_empty() {
            return Ok(None);
        }
        let new_epoch_event = events.iter().find(|event| match event {
            GravityEvent::NewEpoch(_, _) => true,
            GravityEvent::ObservedJWKsUpdated(number, _) => {
                info!("ObservedJWKsUpdated number: {:?}", number);
                false
            }
            _ => false,
        });
        if new_epoch_event.is_none() {
            return Ok(None);
        }
        let new_epoch_event = new_epoch_event.unwrap();
        let (new_epoch, bytes) = match new_epoch_event {
            GravityEvent::NewEpoch(new_epoch, bytes) => (new_epoch, bytes),
            _ => return Err(anyhow::anyhow!("New epoch event is not NewEpoch")),
        };
        let api_validator_set = bcs::from_bytes::<
            api_types::on_chain_config::validator_set::ValidatorSet,
        >(bytes)
        .map_err(|e| format_err!("[on-chain config] Failed to deserialize into config: {e}"))?;
        let validator_set = convert_validator_set(api_validator_set)
            .map_err(|e| format_err!("[on-chain config] Failed to convert validator set: {e}"))?;
        info!(
            "block number {} get validator set from new epoch {} event {:?}",
            block_num, new_epoch, validator_set
        );
        *self.latest_epoch_change_block_number.lock().await = block_num;

        // Store the new epoch in next_epoch instead of updating current_epoch immediately
        // The current_epoch will be updated in release_inflight_blocks when the epoch change is
        // finalized
        let old_epoch = block_state_machine.current_epoch;
        block_state_machine.next_epoch = Some(*new_epoch);
        info!(
            "calculate_new_epoch_state: setting next_epoch to {} (current_epoch: {}) at block {}",
            new_epoch, old_epoch, block_num
        );

        Ok(Some(EpochState::new(*new_epoch, (&validator_set).into())))
    }

    pub async fn set_compute_res(
        &self,
        block_id: BlockId,
        block_hash: [u8; 32],
        block_num: u64,
        epoch: u64,
        txn_status: Arc<Option<Vec<TxnStatus>>>,
        events: Vec<GravityEvent>,
    ) -> Result<(), anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }

        let mut block_state_machine = self.block_state_machine.lock().await;
        let block_key = BlockKey::new(epoch, block_num);
        if let Some(BlockState::Ordered { block, parent_id: _ }) =
            block_state_machine.blocks.get(&block_key)
        {
            assert_eq!(block.block_meta.block_id, block_id);
            let txn_len = block.txns.len();
            let events_len = events.len();
            let new_epoch_state = self
                .calculate_new_epoch_state(&events, block_num, &mut block_state_machine)
                .await?;
            let compute_result = StateComputeResult::new(
                ComputeRes { data: block_hash, txn_num: txn_len as u64, txn_status, events },
                new_epoch_state,
                None,
            );
            block_state_machine
                .blocks
                .insert(block_key, BlockState::Computed { id: block_id, compute_result });

            // Record time for set_compute_res
            let profile =
                block_state_machine.profile.entry(block_key).or_insert_with(BlockProfile::default);
            profile.set_compute_res_time = Some(SystemTime::now());
            info!(
                "set_compute_res id {:?} num {:?} hash {:?} and exec time {:?}ms for {:?} txns and {:?} events",
                block_id,
                block_num,
                BlockId::from_bytes(block_hash.as_slice()),
                profile.set_compute_res_time.unwrap()
                    .duration_since(profile.get_ordered_blocks_time.unwrap())
                    .unwrap_or(Duration::ZERO)
                    .as_millis(),
                txn_len,
                events_len,
            );
            let _ = block_state_machine.sender.send(());
            return Ok(());
        }
        panic!("There is no Ordered Block but try to push compute result for block {block_id:?}")
    }

    pub async fn set_commit_blocks(
        &self,
        block_ids: Vec<BlockHashRef>,
        epoch: u64,
    ) -> Result<Vec<Receiver<()>>, anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        let mut persist_notifiers = Vec::new();
        let mut block_state_machine = self.block_state_machine.lock().await;
        for block_id_num_hash in block_ids {
            info!(
                "push_commit_blocks id {:?} num {:?}",
                block_id_num_hash.block_id, block_id_num_hash.num
            );
            let block_key = BlockKey::new(epoch, block_id_num_hash.num);
            if let Some(state) = block_state_machine.blocks.get_mut(&block_key) {
                match state {
                    BlockState::Computed { id, compute_result } => {
                        if *id == block_id_num_hash.block_id {
                            let mut persist_notifier = None;
                            if compute_result.epoch_state().is_some() {
                                info!(
                                    "push_commit_blocks num {:?} push persist_notifier",
                                    block_id_num_hash.num
                                );
                                let (tx, rx) = mpsc::channel(1);
                                persist_notifier = Some(tx);
                                persist_notifiers.push(rx);
                            }
                            *state = BlockState::Committed {
                                hash: block_id_num_hash.hash,
                                compute_result: compute_result.clone(),
                                id: block_id_num_hash.block_id,
                                persist_notifier,
                            };

                            // Record time for set_commit_blocks
                            let profile = block_state_machine
                                .profile
                                .entry(block_key)
                                .or_insert_with(BlockProfile::default);
                            profile.set_commit_blocks_time = Some(SystemTime::now());
                        } else {
                            panic!(
                                "Computed Block id and number is not equal id: {:?}={:?} num: {:?}",
                                block_id_num_hash.block_id, *id, block_id_num_hash.num
                            );
                        }
                    }
                    BlockState::Committed { hash, compute_result: _, id, persist_notifier: _ } => {
                        if *id != block_id_num_hash.block_id {
                            panic!("Commited Block id and number is not equal id: {:?}={:?} hash: {:?}={:?}", block_id_num_hash.block_id, *id, block_id_num_hash.hash, *hash);
                        }
                    }
                    BlockState::Ordered { block: _, parent_id: _ } => {
                        panic!(
                            "Set commit block meet ordered block for block id {:?} num {}",
                            block_id_num_hash.block_id, block_id_num_hash.num
                        );
                    }
                    BlockState::Historical { id } => {
                        // Historical blocks are already committed/persisted, just verify the id
                        // matches
                        if *id != block_id_num_hash.block_id {
                            panic!(
                                "Historical Block id mismatch: {:?} != {:?} num: {}",
                                block_id_num_hash.block_id, *id, block_id_num_hash.num
                            );
                        }
                    }
                }
            } else {
                panic!(
                    "There is no Block but try to push commit block for block {:?} num {}",
                    block_id_num_hash.block_id, block_id_num_hash.num
                );
            }
        }
        let _ = block_state_machine.sender.send(());
        Ok(persist_notifiers)
    }

    pub async fn get_committed_blocks(
        &self,
        start_num: u64,
        max_size: Option<usize>,
        epoch: u64,
    ) -> Result<Vec<BlockHashRef>, anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        info!("get_committed_blocks start_num: {:?} max_size: {:?}", start_num, max_size);
        let start = Instant::now();

        loop {
            if start.elapsed() > self.config.max_wait_timeout {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for committed blocks after {:?} block_number: {:?}",
                    start.elapsed(),
                    start_num
                ));
            }

            let mut block_state_machine_guard = self.block_state_machine.lock().await;
            let block_state_machine = &mut *block_state_machine_guard;
            let mut result = Vec::new();
            let mut current_num = start_num;
            loop {
                let block_key = BlockKey::new(epoch, current_num);
                match block_state_machine.blocks.get_mut(&block_key) {
                    Some(BlockState::Committed {
                        hash,
                        compute_result: _,
                        id,
                        persist_notifier,
                    }) => {
                        result.push(BlockHashRef {
                            block_id: *id,
                            num: current_num,
                            hash: *hash,
                            persist_notifier: persist_notifier.take(),
                        });

                        // Record time for get_committed_blocks
                        let profile = block_state_machine
                            .profile
                            .entry(block_key)
                            .or_insert_with(BlockProfile::default);
                        profile.get_committed_blocks_time = Some(SystemTime::now());
                    }
                    _ => {
                        break;
                    }
                }
                if result.len() >= max_size.unwrap_or(usize::MAX) {
                    break;
                }
                current_num += 1;
            }
            if result.is_empty() {
                // Release lock before waiting
                drop(block_state_machine_guard);
                // Wait for changes and try again
                match self.wait_for_change(self.config.wait_for_change_timeout).await {
                    Ok(_) => continue,
                    Err(_) => continue, // Timeout on the wait, retry
                }
            } else {
                block_state_machine.latest_finalized_block_number = std::cmp::max(
                    block_state_machine.latest_finalized_block_number,
                    result.last().unwrap().num,
                );
                return Ok(result);
            }
        }
    }

    pub async fn set_state(
        &self,
        latest_commit_block_number: u64,
        latest_finalized_block_number: u64,
    ) -> Result<(), anyhow::Error> {
        info!(
            "set latest_commit_block_number {}, latest_finalized_block_number {:?}",
            latest_commit_block_number, latest_finalized_block_number
        );
        let mut block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.latest_commit_block_number = latest_commit_block_number;
        block_state_machine.latest_finalized_block_number = latest_finalized_block_number;
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn latest_commit_block_number(&self) -> u64 {
        let block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.latest_commit_block_number
    }

    pub async fn block_number_to_block_id(&self) -> HashMap<u64, BlockId> {
        if !self.is_ready() {
            panic!("Buffer is not ready when get block_number_to_block_id");
        }
        let block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.block_number_to_block_id.clone()
    }

    pub async fn get_current_epoch(&self) -> u64 {
        // Wait for buffer to be ready using Notify
        if !self.is_ready() {
            self.ready_notifier.notified().await;
        }
        let block_state_machine = self.block_state_machine.lock().await;
        block_state_machine.current_epoch
    }

    pub async fn release_inflight_blocks(&self) {
        let mut block_state_machine = self.block_state_machine.lock().await;
        let latest_epoch_change_block_number = *self.latest_epoch_change_block_number.lock().await;
        let old_epoch = block_state_machine.current_epoch;

        // Update current_epoch from next_epoch if it exists
        if let Some(next_epoch) = block_state_machine.next_epoch.take() {
            block_state_machine.current_epoch = next_epoch;
            info!(
                "release_inflight_blocks: updating current_epoch from {} to {} at block {}",
                old_epoch, next_epoch, latest_epoch_change_block_number
            );
        }

        info!(
            "release_inflight_blocks latest_epoch_change_block_number: {:?}, current_epoch: {}",
            latest_epoch_change_block_number, block_state_machine.current_epoch
        );
        block_state_machine
            .blocks
            .retain(|key, _| key.block_number <= latest_epoch_change_block_number);
        self.buffer_state.store(BufferState::EpochChange as u8, Ordering::SeqCst);
        block_state_machine
            .profile
            .retain(|key, _| key.block_number <= latest_epoch_change_block_number);
        let _ = block_state_machine.sender.send(());
    }
}
