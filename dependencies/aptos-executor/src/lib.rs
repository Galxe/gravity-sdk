mod mock_block_tree;

pub mod block_executor {
    use std::{
        marker::PhantomData,
        sync::{Arc, RwLock},
    };
    use anyhow::{Ok, Result};

    use aptos_crypto::HashValue;
    use aptos_executor_types::{state_checkpoint_output::{self, BlockExecutorInner}, BlockExecutorTrait, ExecutorResult, StateCheckpointOutput, StateComputeResult};
    use aptos_storage_interface::DbReaderWriter;
    use aptos_types::{
        block_executor::{config::{BlockExecutorConfig, BlockExecutorConfigFromOnchain}, partitioner::ExecutableBlock}, executable::Executable, ledger_info::LedgerInfoWithSignatures, state_store::TStateView, transaction::BlockExecutableTransaction as Transaction
    };

    use crate::mock_block_tree::MockBlockTree;

    pub struct BlockExecutor {
        pub db: DbReaderWriter,
        block_tree: RwLock<MockBlockTree>,
    }

    impl BlockExecutor
    {
        pub fn new(db: DbReaderWriter) -> Self {
            Self {
                db,
                block_tree: RwLock::new(MockBlockTree::new()),
            }
        }
    }

    impl BlockExecutorTrait for BlockExecutor
{
    fn committed_block_id(&self) -> HashValue {
        self.block_tree.read().unwrap().commited_blocks.last().cloned().unwrap_or_default()
    }

    fn reset(&self) -> Result<()> {
        Ok(())
    }

    fn execute_and_state_checkpoint(
        &self,
        block: ExecutableBlock,
        parent_block_id: HashValue,
        onchain_config: BlockExecutorConfigFromOnchain,
    ) -> ExecutorResult<StateCheckpointOutput> {
        let state_checkpoint_output = StateCheckpointOutput::default();
        ExecutorResult::Ok(state_checkpoint_output)
    }

    fn ledger_update(
        &self,
        block_id: HashValue,
        parent_block_id: HashValue,
        state_checkpoint_output: StateCheckpointOutput,
    ) -> ExecutorResult<StateComputeResult> {
        let res = StateComputeResult::new_dummy_with_root_hash(block_id);
        ExecutorResult::Ok(res)
    }

    fn commit_blocks(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        self.block_tree.write().unwrap().commited_blocks.extend(block_ids);
        ExecutorResult::Ok(())
    }

    fn finish(&self) { 

    }
}
}
