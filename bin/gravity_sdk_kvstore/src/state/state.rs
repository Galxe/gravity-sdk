use sha3::Digest;
use std::{
    collections::HashMap,
    fs::File,
    hash::{DefaultHasher, Hash, Hasher},
    io::BufReader,
};

use crate::{AccountId, AccountState, StateRoot};

#[derive(Debug)]
pub struct State {
    accounts: HashMap<String, AccountState>,
    block_number: u64,
    state_root: StateRoot,
}

impl State {
    pub fn new(genesis_path: Option<String>) -> Self {
        let accounts = if genesis_path.is_some() {
            let file = File::open(genesis_path.unwrap()).unwrap();
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap()
        } else {
            HashMap::new()
        };

        Self { accounts, block_number: 0, state_root: StateRoot::default() }
    }

    pub fn get_state_root(&self) -> &StateRoot {
        &self.state_root
    }

    pub fn get_current_block_number(&self) -> u64 {
        self.block_number
    }

    pub fn get_account(&self, address: &str) -> Option<AccountState> {
        self.accounts.get(address).cloned()
    }

    pub async fn update_account_state(
        &mut self,
        account_id: &AccountId,
        state_state: AccountState,
    ) -> Result<(), String> {
        let mut hasher = DefaultHasher::new();
        hasher.write(account_id.0.as_bytes());
        state_state.hash(&mut hasher);
        self.accounts.insert(account_id.0.clone(), state_state);
        self.state_root = self.state_root.update(hasher.finish());
        Ok(())
    }
}
