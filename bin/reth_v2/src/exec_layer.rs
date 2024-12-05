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
        println!("build_payload ExecLayer");
        let payload = ExecutionPayloadEnvelopeV3 {
            execution_payload: ExecutionPayloadV3 {
                payload_inner: ExecutionPayloadV2 {
                    payload_inner: ExecutionPayloadV1 {
                        parent_hash: Default::default(),
                        fee_recipient: Default::default(),
                        state_root: Default::default(),
                        receipts_root: Default::default(),
                        logs_bloom: Default::default(),
                        prev_randao: Default::default(),
                        block_number: 0,
                        gas_limit: 0,
                        gas_used: 0,
                        timestamp: 0,
                        extra_data: Default::default(),
                        base_fee_per_gas: Default::default(),
                        block_hash: Default::default(),
                        transactions: vec![],
                    },
                    withdrawals: vec![],
                },
                blob_gas_used: 0,
                excess_blob_gas: 0,
            },
            block_value: Default::default(),
            blobs_bundle: BlobsBundleV1 {},
            should_override_builder: false,
        };
        payload
    }

    pub async fn  run(&self) {
       println!("run ExecLayer");
        self.reth_cli.process_pending_transactions(|txn| {
            self.build_payload(vec![txn]);
        }).await.expect("\
            Error processing pending transactions");
    }
}