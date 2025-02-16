use async_trait::async_trait;

use crate::{u256_define::BlockId, ExecError, ExecutionBlocks, ExternalBlock, RecoveryApi, ExecutionArgs, RecoveryError};

#[derive(Default)]
pub struct DefaultRecovery {}

#[async_trait]
impl RecoveryApi for DefaultRecovery {
    async fn register_execution_args(&self, args: ExecutionArgs) {
        ()
    }

    async fn latest_block_number(&self) -> u64 {
        0
    }

    async fn finalized_block_number(&self) -> u64 {
        0
    }

    async fn recover_ordered_block(&self, parent_id: BlockId, _: ExternalBlock) -> Result<(), ExecError> {
        Ok(())
    }
}