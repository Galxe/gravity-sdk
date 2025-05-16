use std::sync::Arc;

use gaptos::api_types::{compute_res::ComputeRes, ExternalBlockMeta};
use async_trait::async_trait;

use gaptos::api_types::ExternalBlock;
use coex_bridge::{
    call::{self, AsyncCallImplTrait},
    get_coex_bridge, Func,
};

use crate::consensus_api::ConsensusEngine;
