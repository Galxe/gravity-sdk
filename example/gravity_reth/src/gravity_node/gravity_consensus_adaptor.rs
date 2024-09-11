use std::fmt::{Debug, Formatter};
use reth_consensus::{Consensus, ConsensusError, PostExecutionInput};
use reth_node_core::primitives::{BlockWithSenders, Header, SealedBlock, SealedHeader, U256};
use gravity_sdk::GravityConsensusEngineInterface;



#[derive(Debug, Default)]
pub(super) struct GravityConsensusAdaptor<T: GravityConsensusEngineInterface + Debug> {
    consensus_engine : T,
}

impl<T : GravityConsensusEngineInterface + Debug> Consensus for GravityConsensusAdaptor<T> {
    fn validate_header(&self, header: &SealedHeader) -> Result<(), ConsensusError> {
        println!("Gravity: validate_header : {:?}", header);
        Ok(())
    }

    fn validate_header_against_parent(&self, header: &SealedHeader, parent: &SealedHeader) -> Result<(), ConsensusError> {
        println!("Gravity: validate_header_against_parent : {:?} : {:?}", header, parent);
        Ok(())
    }

    fn validate_header_with_total_difficulty(&self, header: &Header, total_difficulty: U256) -> Result<(), ConsensusError> {
        println!("Gravity: validate_header_with_total_difficulty : {:?} : {:?}", header, total_difficulty);
        Ok(())
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock) -> Result<(), ConsensusError> {
        println!("Gravity: validate_block_pre_execution : {:?}", block);
        Ok(())
    }

    fn validate_block_post_execution(&self, block: &BlockWithSenders, input: PostExecutionInput<'_>) -> Result<(), ConsensusError> {
        println!("Gravity: validate_block_post_execution : {:?} : {:?}", block, input);
        Ok(())
    }
}