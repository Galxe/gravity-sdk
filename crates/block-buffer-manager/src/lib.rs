use std::sync::{Arc, OnceLock};

use block_buffer_manager::BlockBufferManager;

pub mod block_buffer_manager;
static GLOBAL_BLOCK_BUFFER_MANAGER: OnceLock<Arc<BlockBufferManager>> = OnceLock::new();

pub fn get_block_buffer_manager() -> &'static Arc<BlockBufferManager> {
    GLOBAL_BLOCK_BUFFER_MANAGER.get_or_init(|| {
        BlockBufferManager::new(block_buffer_manager::BlockBufferManagerConfig::default())
    })
}

pub use block_buffer_manager::TxPool;
