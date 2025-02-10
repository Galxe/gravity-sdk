use api::{consensus_api::ConsensusEngine, NodeConfig};
use api_types::u256_define::ComputeRes;
use api_types::{ConsensusApi, ExecutionApiV2, ExecutionLayer};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::reth_coordinator::RethCoordinator;
pub struct AptosConsensus {
    /// The execution client for interacting with the execution layer
    execution_client: Arc<dyn ExecutionApiV2>,
    /// The consensus engine
    consensus_engine: Arc<dyn ConsensusApi>,
}

impl AptosConsensus {
    pub fn get_data_from_consensus_db(node_config: &NodeConfig) -> BTreeMap<u64, ComputeRes> {
        ConsensusEngine::get_data_from_consensus_db(node_config)
    }

    pub fn init(node_config: NodeConfig, execution_client: Arc<RethCoordinator>) {
        let execution_layer = ExecutionLayer {
            execution_api: execution_client.clone(),
            recovery_api: execution_client.clone(),
        };

        let consensus_engine = ConsensusEngine::init(
            node_config,
            execution_layer,
            1337, // Chain ID
        );
    }
}
