use alloy_rpc_types_engine::{ExecutionPayloadEnvelopeV3, ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3, ExecutionPayloadV4};
use web3::types::Transaction;
use crate::reth_cli::RethCli;

pub struct ExecLayer {
    pub reth_cli: RethCli,
}

impl ExecLayer {
    pub fn new(reth_cli: RethCli) -> Self {
        Self { reth_cli }
    }

    pub fn build_payload(&self, txns: Vec<Transaction>) -> ExecutionPayloadEnvelopeV3 {
        todo!()
    }

    pub async fn  run(&self) {
       println!("run ExecLayer");
        self.reth_cli.process_pending_transactions(|txn| {
            self.build_payload(vec![txn]);
        }).await.expect("\
            Error processing pending transactions");
    }
}