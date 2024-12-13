use std::sync::{Arc, Mutex};

use crate::reth_cli::RethCli;
use api_types::{
    account::{ExternalAccountAddress, ExternalChainId}, ComputeRes, ExecError, ExecTxn, ExecutionApiV2, ExternalBlock, ExternalBlockMeta, ExternalPayloadAttr, VerifiedTxn, VerifiedTxnWithAccountSeqNum
};
use async_trait::async_trait;
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use web3::types::H160;

pub struct RethCoordinator {
    reth_cli: RethCli,
    pending_buffer: Arc<Mutex<Vec<VerifiedTxnWithAccountSeqNum>>>,
}

impl RethCoordinator
{
    pub fn new(reth_cli: RethCli) -> Self {
        Self { reth_cli, pending_buffer: Arc::new(Mutex::new(Vec::new())) }
    }

    fn covert_account(&self, acc: H160) -> ExternalAccountAddress {
        let mut bytes = [0u8; 32];
        bytes[12..].copy_from_slice(acc.as_bytes());
        ExternalAccountAddress::new(bytes)
    }

    pub async fn run(&self) {
        self.reth_cli.process_pending_transactions(|txn, account_seq_num| {
            let nonce = txn.nonce.as_u64();
            let bytes = serde_json::to_vec(&txn).unwrap();
            self.pending_buffer.lock().unwrap().push(VerifiedTxnWithAccountSeqNum {
                txn: VerifiedTxn {
                    bytes,
                    sender: self.covert_account(txn.from.unwrap()),
                    sequence_number: nonce,
                    chain_id: ExternalChainId::new(0),
                },
                account_seq_num,
            });

        });
    }
}

#[async_trait]
impl ExecutionApiV2 for RethCoordinator
{
    async fn add_txn(&self, bytes: ExecTxn) -> Result<(), ExecError> {
        panic!("Reth Coordinator does not support add_txn");
    }

    async fn recv_unbroadcasted_txn(&self) -> Result<Vec<VerifiedTxn>, ExecError> {
        panic!("Reth Coordinator does not support recv_unbroadcasted_txn");
    }

    async fn check_block_txns(
        &self,
        payload_attr: ExternalPayloadAttr,
        txns: Vec<VerifiedTxn>,
    ) -> Result<bool, ExecError> {
        unimplemented!()
    }

    async fn recv_pending_txns(&self) -> Result<Vec<VerifiedTxnWithAccountSeqNum>, ExecError> {
        let mut buffer = self.pending_buffer.lock().unwrap();
        let res = buffer.drain(..).collect();
        Ok(res)
    }

    async fn send_ordered_block(
        &self,
        parent_meta: ExternalBlockMeta,
        ordered_block: ExternalBlock,
    ) -> Result<(), ExecError> {
        todo!()
    }

    async fn recv_executed_block_hash(
        &self,
        head: ExternalBlockMeta,
    ) -> Result<ComputeRes, ExecError> {
        todo!()
    }

    async fn commit_block(&self, head: ExternalBlockMeta) -> Result<(), ExecError> {
        todo!()
    }
}
