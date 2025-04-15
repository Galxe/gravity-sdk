// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use super::SparseMerkleTree;
use aptos_crypto::HashValue;
use aptos_types::{
    state_store::{
        combine_sharded_state_updates, state_storage_usage::StateStorageUsage,
        state_value::StateValue, ShardedStateUpdates,
    },
    transaction::Version,
};

/// This represents two state sparse merkle trees at their versions in memory with the updates
/// reflecting the difference of `current` on top of `base`.
///
/// The `base` is the state SMT that current is based on.
/// The `current` is the state SMT that results from applying updates_since_base on top of `base`.
/// `updates_since_base` tracks all those key-value pairs that's changed since `base`, useful
///  when the next checkpoint is calculated.
#[derive(Clone, Debug)]
pub struct StateDelta {
    pub base: SparseMerkleTree<StateValue>,
    pub base_version: Option<Version>,
    pub current: SparseMerkleTree<StateValue>,
    pub current_version: Option<Version>,
    pub updates_since_base: ShardedStateUpdates,
}

impl StateDelta {
    pub fn new(
        _base: SparseMerkleTree<StateValue>,
        _base_version: Option<Version>,
        _current: SparseMerkleTree<StateValue>,
        _current_version: Option<Version>,
        _updates_since_base: ShardedStateUpdates,
    ) -> Self {
        todo!()
    }

    pub fn new_empty() -> Self {
        todo!()
    }

    pub fn new_at_checkpoint(
        _root_hash: HashValue,
        _usage: StateStorageUsage,
        _checkpoint_version: Option<Version>,
    ) -> Self {
        todo!()
    }

    pub fn merge(&mut self, other: StateDelta) {
        assert!(other.follow(self));
        combine_sharded_state_updates(&mut self.updates_since_base, other.updates_since_base);

        self.current = other.current;
        self.current_version = other.current_version;
    }

    pub fn follow(&self, _other: &StateDelta) -> bool {
        todo!()
    }

    pub fn has_same_current_state(&self, _other: &StateDelta) -> bool {
        todo!()
    }

    pub fn base_root_hash(&self) -> HashValue {
        todo!()
    }

    pub fn root_hash(&self) -> HashValue {
        todo!()
    }

    pub fn next_version(&self) -> Version {
        self.current_version.map_or(0, |v| v + 1)
    }

    pub fn replace_with(&mut self, mut rhs: Self) -> Self {
        std::mem::swap(self, &mut rhs);
        rhs
    }
}

impl Default for StateDelta {
    fn default() -> Self {
        Self::new_empty()
    }
}
