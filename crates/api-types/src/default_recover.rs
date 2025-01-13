use async_trait::async_trait;

use crate::{u256_define::BlockId, ExecError, ExecutionBlocks, ExternalBlock, RecoveryApi, RecoveryError};

#[derive(Default)]
pub struct DefaultRecovery {}

#[async_trait]
impl RecoveryApi for DefaultRecovery {
    async fn latest_block_number(&self) -> u64 {
        0
    }

    async fn finalized_block_number(&self) -> u64 {
        0
    }

    async fn recover_ordered_block(&self, parent_id: BlockId, _: ExternalBlock) -> Result<(), ExecError> {
        Ok(())
    }

    async fn recover_execution_blocks(&self, _: ExecutionBlocks) {
        ()
    }

    async fn get_blocks_by_range(
        &self,
        _: u64,
        _: u64,
    ) -> Result<ExecutionBlocks, RecoveryError> {
        Err(RecoveryError::UnimplementError)
    }
}