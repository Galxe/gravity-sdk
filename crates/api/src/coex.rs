use std::sync::Arc;

use api_types::{compute_res::ComputeRes, ConsensusApi, ExternalBlockMeta};
use async_trait::async_trait;

use api_types::ExternalBlock;
use coex_bridge::{
    call::{self, AsyncCallImplTrait},
    get_coex_bridge, Func,
};

use crate::consensus_api::ConsensusEngine;

pub struct SendOrderedBlocksCall {
    consensus_engine: Arc<ConsensusEngine>,
}

#[async_trait]
impl AsyncCallImplTrait for SendOrderedBlocksCall {
    type Input = ([u8; 32], ExternalBlock);
    type Output = ();
    async fn call(&self, input: Self::Input) -> Result<Self::Output, ()> {
        Ok(self.consensus_engine.send_ordered_block(input.0, input.1).await)
    }
}

pub struct RecvExecutedBlockHashCall {
    consensus_engine: Arc<ConsensusEngine>,
}

#[async_trait]
impl AsyncCallImplTrait for RecvExecutedBlockHashCall {
    type Input = ExternalBlockMeta;
    type Output = ComputeRes;
    async fn call(&self, input: Self::Input) -> Result<Self::Output, ()> {
        Ok(self.consensus_engine.recv_executed_block_hash(input).await)
    }
}

pub struct CommitBlockHashCall {
    consensus_engine: Arc<ConsensusEngine>,
}

#[async_trait]
impl AsyncCallImplTrait for CommitBlockHashCall {
    type Input = [u8; 32];
    type Output = ();
    async fn call(&self, input: Self::Input) -> Result<Self::Output, ()> {
        Ok(self.consensus_engine.commit_block_hash(input).await)
    }
}

pub fn register_hook_func(consensus_engine: Arc<ConsensusEngine>) {
    let coex_bridge = get_coex_bridge();
    coex_bridge.register(
        "send_ordered_block".to_string(),
        Func::SendOrderedBlocks(Arc::new(call::AsyncCall::new(Box::new(SendOrderedBlocksCall {
            consensus_engine: consensus_engine.clone(),
        })))),
    );
    coex_bridge.register(
        "recv_executed_block_hash".to_string(),
        Func::RecvExecutedBlockHash(Arc::new(call::AsyncCall::new(Box::new(
            RecvExecutedBlockHashCall { consensus_engine: consensus_engine.clone() },
        )))),
    );
    coex_bridge.register(
        "commit_block_hash".to_string(),
        Func::CommittedBlockHash(Arc::new(call::AsyncCall::new(Box::new(CommitBlockHashCall {
            consensus_engine,
        })))),
    );
}
