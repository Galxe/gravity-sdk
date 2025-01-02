use api::{consensus_api::ConsensusEngine, NodeConfig};
use api_types::default_recover::DefaultRecovery;
use api_types::{ConsensusApi, ExecutionApiV2, ExecutionLayer};
use std::sync::Arc;
pub struct AptosConsensus {
    /// The execution client for interacting with the execution layer
    execution_client: Arc<dyn ExecutionApiV2>,
    /// The consensus engine
    consensus_engine: Arc<dyn ConsensusApi>,
}

impl AptosConsensus {
    pub fn init(node_config: NodeConfig, execution_client: Arc<dyn ExecutionApiV2>) {
        let execution_layer = ExecutionLayer {
            execution_api: execution_client.clone(),
            recovery_api: Arc::new(DefaultRecovery::default()),
        };

        let consensus_engine = ConsensusEngine::init(
            node_config,
            execution_layer,
            1337, // Chain ID
        );
    }
}
