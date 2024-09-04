pub mod block_executor {
    use std::{
        marker::PhantomData,
        sync::{Arc, RwLock},
    };
    use anyhow::Result;

    use aptos_crypto::HashValue;
    use aptos_executor_types::{state_checkpoint_output::BlockExecutorInner, BlockExecutorTrait, ExecutorResult, StateCheckpointOutput, StateComputeResult};
    use aptos_storage_interface::DbReaderWriter;
    use aptos_types::{
        block_executor::{config::{BlockExecutorConfig, BlockExecutorConfigFromOnchain}, partitioner::ExecutableBlock}, executable::Executable, ledger_info::LedgerInfoWithSignatures, state_store::TStateView, transaction::BlockExecutableTransaction as Transaction
    };

    pub struct BlockExecutor<V> {
        pub db: DbReaderWriter,
        _p: PhantomData<V>,
        commit_ids: RwLock<Vec<HashValue>>,
    }

    impl<V> BlockExecutor<V>
    {
        pub fn new(db: DbReaderWriter) -> Self {
            Self {
                db,
                _p: PhantomData,
                commit_ids: RwLock::new(vec![HashValue::zero()]),
            }
        }
    }

    impl<V: Send + Sync> BlockExecutorTrait for BlockExecutor<V>
{
    fn committed_block_id(&self) -> HashValue {
        self.commit_ids.read().unwrap().last().cloned().unwrap_or_default()
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
        todo!()
    }

    fn ledger_update(
        &self,
        block_id: HashValue,
        parent_block_id: HashValue,
        state_checkpoint_output: StateCheckpointOutput,
    ) -> ExecutorResult<StateComputeResult> {
        todo!()
    }

    fn commit_blocks(
        &self,
        block_ids: Vec<HashValue>,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> ExecutorResult<()> {
        self.commit_ids.write().unwrap().extend(block_ids);
        Ok(())
    }

    fn finish(&self) { 

    }
}
}
