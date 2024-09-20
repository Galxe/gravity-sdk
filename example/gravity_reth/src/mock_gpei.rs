use alloy::consensus::TxEnvelope;
use alloy::primitives::B256;

pub struct MockGPEI {

}


impl MockGPEI {
    pub fn new() -> Self {
        Self {}
    }

    pub fn submit_valid_transactions(&self, transactions: Vec<TxEnvelope>) {
        println!("Submit valid transactions");
    }

    pub fn polling_order_blocks(&self) -> eyre::Result<()> {
        Ok(())
    }

    pub fn submit_compute_res(&self, compute_res: B256) {
        println!("Submit compute res");
    }

    pub fn polling_submit_blocks(&self) -> eyre::Result<()> {
        println!("Polling submit blocks");
        Ok(())
    }

    pub fn submit_max_persistence_block_id(&self) {
        println!("Submit max persistence");
    }

}