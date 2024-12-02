use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct ExternalAccountAddress([u8; 32]);

#[derive(Clone, Debug)]
pub struct ExternalChainId(u64);

impl ExternalChainId {
    pub fn new(id :u64) -> Self {
        Self(id)
    }
}

impl Debug for ExternalAccountAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // hex string
        write!(f, "0x{}", hex::encode(&self.0))
    }
}