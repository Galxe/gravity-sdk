use api_types::{ComputeRes, ExecError, ExecutionApiV2, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn};

pub struct RethCoordinator {
}

impl ExecutionApiV2 for RethCoordinator {
    async fn add_txn(&self, bytes: Vec<u8>) -> Result<(), ExecError> {
        todo!()
    }

    async fn check_block_txns(&self, payload_attr: ExternalPayloadAttr, txns: Vec<VerifiedTxn>) -> Result<bool, ExecError> {
        todo!()
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        todo!()
    }

    async fn send_ordered_block(&self, ordered_block: ExternalBlock) -> Result<(), ExecError> {
        todo!()
    }

    async fn recv_executed_block_hash(&self, head: ExternalBlockMeta) -> Result<ComputeRes, ExecError> {
        todo!()
    }

    async fn commit_block(&self, head: ExternalBlockMeta) -> Result<(), ExecError> {
        todo!()
    }
}