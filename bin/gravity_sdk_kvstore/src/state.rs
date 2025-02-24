use sha3::Digest;
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::{AccountId, AccountState, StateRoot};

#[derive(Debug)]
pub struct State {
    accounts: HashMap<String, AccountState>,
    block_number: u64,
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

        Self {
            accounts,
            block_number: 0,
        }
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
        state_update: AccountState,
    ) -> Result<(), String> {
        self.accounts.insert(
            account_id.0.clone(),
            AccountState {
                balance: state_update.balance,
                nonce: state_update.nonce,
                kv_store: state_update.kv_store,
            },
        );
        Ok(())
    }

    pub fn compute_state_root(&self) -> Result<StateRoot, String> {
        let mut accounts: Vec<_> = self.accounts.iter().collect();
        accounts.sort_by(|a, b| a.0.cmp(b.0));

        let mut hasher = sha3::Keccak256::new();
        for (address, account) in accounts {
            hasher.update(address.as_bytes());
            hasher.update(&account.nonce.to_be_bytes());
            hasher.update(&account.balance.to_be_bytes());

            let mut kv_pairs: Vec<_> = account.kv_store.iter().collect();
            kv_pairs.sort_by(|a, b| a.0.cmp(b.0));

            for (k, v) in kv_pairs {
                hasher.update(k.as_bytes());
                hasher.update(v.as_bytes());
            }
        }

        let result: [u8; 32] = hasher.finalize().into();
        Ok(StateRoot(result))
    }
}
