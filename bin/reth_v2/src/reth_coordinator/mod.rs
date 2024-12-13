use api_types::{ComputeRes, ExecError, ExecutionApiV2, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn};
use async_trait::async_trait;

use crate::reth_cli::RethCli;

pub struct RethCoordinator {
    reth_cli: RethCli,
}

impl RethCoordinator {
    pub fn new(reth_cli: RethCli) -> Self {
        Self { reth_cli }
    }
}

#[async_trait]
impl ExecutionApiV2 for RethCoordinator {
    async fn add_txn(&self, bytes: ExecTxn) -> Result<(), ExecError> {
        panic!("Reth Coordinator does not support add_txn");
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        panic!("Reth Coordinator does not support recv_unbroadcasted_txn");
    }

    async fn check_block_txns(&self, payload_attr: ExternalPayloadAttr, txns: Vec<VerifiedTxn>) -> Result<bool, ExecError> {
        todo!()
    }

    async fn recv_pending_txns(&self) -> Result<Vec<(VerifiedTxn, u64)>, ExecError> {
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