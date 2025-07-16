use api::consensus_api::{ConsensusEngine, ConsensusEngineArgs};
use std::sync::Arc;
pub struct AptosConsensus {
    /// The consensus engine
    consensus_engine: Arc<ConsensusEngine>,
}

impl AptosConsensus {
    pub async fn init(args: ConsensusEngineArgs) -> Arc<ConsensusEngine> {
        ConsensusEngine::init(args).await
    }
}
