use std::sync::Arc;

use async_trait::async_trait;

use rand::Rng;

use crate::{default_recover::DefaultRecovery, u256_define::{BlockId, ComputeRes, TxnHash}, ExecError, ExecTxn, ExecutionApiV2, ExecutionLayer, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn, VerifiedTxnWithAccountSeqNum};

pub struct MockExecutionApi {}

#[async_trait]
impl ExecutionApiV2 for MockExecutionApi {
    async fn add_txn(&self, bytes: ExecTxn) -> Result<TxnHash, ExecError> {
        Ok(TxnHash::random())
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        Ok(vec![])
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
        Ok(true)
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        Ok(vec![])
    }

    async fn send_ordered_block(
        &self,
        parent_id: BlockId,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        Ok(())
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        let mut rng = rand::thread_rng();
        let random_bytes: [u8; 32] = rng.gen();
        Ok(ComputeRes::new(random_bytes))
    }

    async fn commit_block(&self, head: BlockId) -> Result<(), ExecError> {
        Ok(())
    }
}

pub fn mock_execution_layer() -> ExecutionLayer {
    ExecutionLayer {
        execution_api: Arc::new(MockExecutionApi {}),
        recovery_api: Arc::new(DefaultRecovery {}),
    }
}