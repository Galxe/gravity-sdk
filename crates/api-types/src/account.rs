use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct ExternalAccountAddress([u8; 32]);

impl ExternalAccountAddress {
    pub fn new(address: [u8; 32]) -> Self {
        Self(address)
    }
    pub fn bytes(&self) -> [u8; 32] {
        self.0.clone()
    }
}

#[derive(Clone, Debug)]
pub struct ExternalChainId(u64);

impl ExternalChainId {
    pub fn new(id :u64) -> Self {
        Self(id)
    }

    pub fn into_u64(&self) -> u64 {
        self.0
    }
}

impl Debug for ExternalAccountAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // hex string
        write!(f, "0x{}", hex::encode(&self.0))
    }
}