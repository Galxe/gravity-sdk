use std::sync::Arc;

use gaptos::api_types::{compute_res::ComputeRes, ExternalBlockMeta};
use async_trait::async_trait;

use gaptos::api_types::ExternalBlock;
use crate::consensus_api::ConsensusEngine;
