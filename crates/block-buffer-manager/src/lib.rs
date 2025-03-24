use std::sync::{Arc, OnceLock};

use block_buffer_manager::BlockBufferManager;

pub mod block_buffer_manager;
static GLOBAL_BLOCK_BUFFER_MANAGER : OnceLock<BlockBufferManager> = OnceLock::new();

pub fn get_block_buffer_manager() -> &'static block_buffer_manager::BlockBufferManager {
    GLOBAL_BLOCK_BUFFER_MANAGER.get_or_init(|| BlockBufferManager::new())
}