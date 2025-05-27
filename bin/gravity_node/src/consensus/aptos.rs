use api::{consensus_api::ConsensusEngine, NodeConfig};
use std::sync::Arc;
pub struct AptosConsensus {
    /// The consensus engine
    consensus_engine: Arc<ConsensusEngine>,
}

impl AptosConsensus {
    pub async fn init(node_config: NodeConfig, chain_id: u64, latest_block_number: u64) -> Arc<ConsensusEngine> {
        let consensus_engine = ConsensusEngine::init(
            node_config,
            chain_id, // Chain ID
            latest_block_number
        ).await;
        consensus_engine
    }
}
