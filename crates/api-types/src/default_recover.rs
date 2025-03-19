use async_trait::async_trait;

use crate::{u256_define::BlockId, ExecError, ExecutionBlocks, ExternalBlock, RecoveryApi, ExecutionArgs, RecoveryError};

#[derive(Default)]
pub struct DefaultRecovery {}

#[async_trait]
impl RecoveryApi for DefaultRecovery {
    async fn send_execution_args(&self, _: ExecutionArgs) {
        ()
    }

    async fn latest_block_number(&self) -> u64 {
        0
    }

    async fn finalized_block_number(&self) -> u64 {
        0
    }
}