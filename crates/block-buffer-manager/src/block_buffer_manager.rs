use log::{info, warn};
use std::{
    cell::OnceCell,
    collections::HashMap,
    ops::Deref,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, SystemTime},
};
use tokio::{sync::Mutex, time::Instant};

use gaptos::api_types::{
    compute_res::{self, ComputeRes, TxnStatus}, u256_define::BlockId, ExternalBlock, VerifiedTxn,
    VerifiedTxnWithAccountSeqNum,
};
use itertools::Itertools;

pub struct TxnItem {
    pub txns: Vec<VerifiedTxnWithAccountSeqNum>,
    pub gas_limit: u64,
}

pub struct TxnBuffer {
    // (txns, gas_limit)
    txns: Mutex<Vec<TxnItem>>,
}

pub struct BlockHashRef {
    pub block_id: BlockId,
    pub num: u64,
    pub hash: Option<[u8; 32]>,
}

pub enum BlockState {
    Ordered { block: ExternalBlock, parent_id: BlockId },
    Computed { id: BlockId, compute_res: ComputeRes },
    Committed { hash: Option<[u8; 32]>, compute_res: ComputeRes, id: BlockId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum BufferState {
    Uninitialized,
    Ready,
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
    blocks: HashMap<u64, BlockState>,
    profile: HashMap<u64, BlockProfile>,
    latest_commit_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
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
            }),
            buffer_state: AtomicU8::new(BufferState::Uninitialized as u8),
            config,
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
        block_state_machine.blocks.retain(|num, _| *num > latest_persist_block_num);
        block_state_machine.profile.retain(|num, _| *num > latest_persist_block_num);
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn init(
        &self,
        latest_commit_block_number: u64,
        block_number_to_block_id: HashMap<u64, BlockId>,
    ) {
        info!("init block_buffer_manager with latest_commit_block_number: {:?} block_number_to_block_id: {:?}", latest_commit_block_number, block_number_to_block_id);
        let mut block_state_machine = self.block_state_machine.lock().await;
        // When init, the latest_finalized_block_number is the same as latest_commit_block_number
        block_state_machine.latest_commit_block_number = latest_commit_block_number;
        block_state_machine.latest_finalized_block_number = latest_commit_block_number;
        block_state_machine.block_number_to_block_id = block_number_to_block_id;
        self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
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
        let mut pool_txns = self.txn_buffer.txns.lock().await;
        pool_txns.push(TxnItem { txns: std::mem::take(txns), gas_limit });
    }

    pub fn is_ready(&self) -> bool {
        self.buffer_state.load(Ordering::SeqCst) == BufferState::Ready as u8
    }

    pub async fn pop_txns(
        &self,
        max_size: usize,
        gas_limit: u64,
    ) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, anyhow::Error> {
        let mut txn_buffer = self.txn_buffer.txns.lock().await;
        let mut total_gas_limit = 0;
        let mut count = 0;
        let split_point = txn_buffer.iter()
            .position(|item| {
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
            "set_ordered_blocks {:?} num {:?}",
            block.block_meta.block_id, block.block_meta.block_number
        );
        let mut block_state_machine = self.block_state_machine.lock().await;
        if block_state_machine.blocks.contains_key(&block.block_meta.block_number) {
            log::warn!(
                "set_ordered_blocks block {:?} block num {} already exists",
                block.block_meta.block_id,
                block.block_meta.block_number
            );
            return Ok(());
        }
        let block_num = block.block_meta.block_number;
        block_state_machine
            .blocks
            .insert(block_num, BlockState::Ordered { block: block.clone(), parent_id });
        
        // Record time for set_ordered_blocks
        let profile = block_state_machine.profile.entry(block_num).or_insert_with(BlockProfile::default);
        profile.set_ordered_block_time = Some(SystemTime::now());
        
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn get_ordered_blocks(
        &self,
        start_num: u64,
        max_size: Option<usize>,
    ) -> Result<Vec<(ExternalBlock, BlockId)>, anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        let start = Instant::now();
        info!("call get_ordered_blocks start_num: {:?} max_size: {:?}", start_num, max_size);
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
            while let Some(block) = block_state_machine.blocks.get(&current_num) {
                match block {
                    BlockState::Ordered { block, parent_id } => {
                        result.push((block.clone(), *parent_id));
                        // Record time for get_ordered_blocks
                        let profile = block_state_machine.profile.entry(current_num).or_insert_with(BlockProfile::default);
                        profile.get_ordered_blocks_time = Some(SystemTime::now());
                    }
                    _ => {
                        panic!("There is no Ordered Block but try to get ordered blocks for block {:?}", current_num);
                    }
                }
                if result.len() >= max_size.unwrap_or(usize::MAX) {
                    break;
                }
                current_num += 1;
            }

            if result.is_empty() {
                // Release lock before waiting
                drop(block_state_machine);
                // Wait for changes and try again
                match self.wait_for_change(self.config.wait_for_change_timeout).await {
                    Ok(_) => continue,
                    Err(_) => continue, // Timeout on the wait, retry
                }
            } else {
                return Ok(result);
            }
        }
    }

    pub async fn get_executed_res(
        &self,
        block_id: BlockId,
        block_num: u64,
    ) -> Result<ComputeRes, anyhow::Error> {
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
            if let Some(block) = block_state_machine.blocks.get(&block_num) {
                match block {
                    BlockState::Computed { id, compute_res } => {
                        
                        assert_eq!(id, &block_id);
                        
                        // Record time for get_executed_res
                        let compute_res_clone = compute_res.clone();
                        let profile = block_state_machine.profile.entry(block_num).or_insert_with(BlockProfile::default);
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
                    BlockState::Committed { hash: _, compute_res, id } => {
                        log::warn!(
                            "get_executed_res done with id {:?} num {:?} res {:?}",
                            block_id,
                            id,
                            compute_res
                        );
                        assert_eq!(id, &block_id);
                        return Ok(compute_res.clone());
                    }
                }
            } else {
                panic!(
                    "There is no Ordered Block but try to get executed result for block {:?}",
                    block_id
                )
            }
        }
    }

    pub async fn set_compute_res(
        &self,
        block_id: BlockId,
        block_hash: [u8; 32],
        block_num: u64,
        txn_status: Arc<Option<Vec<TxnStatus>>>,
    ) -> Result<(), anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        
        let mut block_state_machine = self.block_state_machine.lock().await;
        if let Some(BlockState::Ordered { block, parent_id: _ }) =
            block_state_machine.blocks.get(&block_num)
        {
            assert_eq!(block.block_meta.block_id, block_id);
            let txn_len = block.txns.len();
            block_state_machine.blocks.insert(
                block_num,
                BlockState::Computed {
                    id: block_id,
                    compute_res: ComputeRes { data: block_hash, txn_num: txn_len as u64, txn_status, events: vec![] },
                },
            );
            
            // Record time for set_compute_res
            let profile = block_state_machine.profile.entry(block_num).or_insert_with(BlockProfile::default);
            profile.set_compute_res_time = Some(SystemTime::now());
            info!(
                "set_compute_res id {:?} num {:?} hash {:?} and exec time {:?}ms for {:?} txns",
                block_id,
                block_num,
                BlockId::from_bytes(block_hash.as_slice()),
                profile.set_compute_res_time.unwrap()
                    .duration_since(profile.get_ordered_blocks_time.unwrap())
                    .unwrap_or(Duration::ZERO)
                    .as_millis(),
                txn_len
            );
            let _ = block_state_machine.sender.send(());
            return Ok(());
        }
        panic!("There is no Ordered Block but try to push compute result for block {:?}", block_id)
    }

    pub async fn set_commit_blocks(
        &self,
        block_ids: Vec<BlockHashRef>,
    ) -> Result<(), anyhow::Error> {
        if !self.is_ready() {
            panic!("Buffer is not ready");
        }
        let mut block_state_machine = self.block_state_machine.lock().await;
        for block_id_num_hash in block_ids {
            info!(
                "push_commit_blocks id {:?} num {:?}",
                block_id_num_hash.block_id, block_id_num_hash.num
            );
            if let Some(state) = block_state_machine.blocks.get_mut(&block_id_num_hash.num) {
                match state {
                    BlockState::Computed { id, compute_res } => {
                        if *id == block_id_num_hash.block_id {
                            *state = BlockState::Committed {
                                hash: block_id_num_hash.hash,
                                compute_res: compute_res.clone(),
                                id: block_id_num_hash.block_id,
                            };
                            
                            // Record time for set_commit_blocks
                            let profile = block_state_machine.profile.entry(block_id_num_hash.num).or_insert_with(BlockProfile::default);
                            profile.set_commit_blocks_time = Some(SystemTime::now());
                        } else {
                            panic!(
                                "Computed Block id and number is not equal id: {:?}={:?} num: {:?}",
                                block_id_num_hash.block_id, *id, block_id_num_hash.num
                            );
                        }
                    }
                    BlockState::Committed { hash, compute_res: _, id } => {
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
                }
            } else {
                panic!(
                    "There is no Block but try to push commit block for block {:?} num {}",
                    block_id_num_hash.block_id, block_id_num_hash.num
                );
            }
        }
        let _ = block_state_machine.sender.send(());
        Ok(())
    }

    pub async fn get_committed_blocks(
        &self,
        start_num: u64,
        max_size: Option<usize>,
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

            let mut block_state_machine = self.block_state_machine.lock().await;
            let mut result = Vec::new();
            let mut current_num = start_num;
            while let Some(block) = block_state_machine.blocks.get(&current_num) {
                match block {
                    BlockState::Committed { hash, compute_res: _, id } => {
                        result.push(BlockHashRef { block_id: *id, num: current_num, hash: *hash });
                        
                        // Record time for get_committed_blocks
                        let profile = block_state_machine.profile.entry(current_num).or_insert_with(BlockProfile::default);
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
                drop(block_state_machine);
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
}
