use gaptos::aptos_infallible::RwLock as InfallibleRwLock;
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Global map from validator_index to reth_account_address for current epoch
/// This is updated when a new epoch starts
static PROPOSER_RETH_ADDRESS_MAP: Lazy<InfallibleRwLock<HashMap<u64, Vec<u8>>>> =
    Lazy::new(|| InfallibleRwLock::new(HashMap::new()));

/// Get the reth account address for a given validator index
/// Returns None if the validator index is not found in the current epoch's validator set
pub fn get_reth_address_by_index(validator_index: u64) -> Option<Vec<u8>> {
    PROPOSER_RETH_ADDRESS_MAP.read().get(&validator_index).cloned()
}

/// Update the proposer reth address map for a new epoch
/// Maps validator_index -> reth_account_address
pub fn update_proposer_reth_index_map(
    validator_set: &gaptos::aptos_types::on_chain_config::ValidatorSet,
) {
    let mut reth_address_map = HashMap::new();
    for (index, validator) in validator_set.active_validators.iter().enumerate() {
        reth_address_map.insert(index as u64, validator.reth_account_address.clone());
    }
    *PROPOSER_RETH_ADDRESS_MAP.write() = reth_address_map;
}
