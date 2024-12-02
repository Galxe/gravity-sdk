use std::fmt::Debug;

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct ExternalAccountAddress([u8; 32]);

#[derive(Clone, Debug)]
pub struct ExternalChainId(u64);

impl Debug for ExternalAccountAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // hex string
        write!(f, "0x{}", hex::encode(&self.0))
    }
}