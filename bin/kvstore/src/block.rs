use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ops::Deref;

use api_types::{BlockBatch, GTxn};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use ethers::utils::hex;
use ethers::utils::keccak256;
use rand::{seq::IteratorRandom, Rng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha2::Digest;

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountAddress([u8; 20]);

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
struct Account {
    address: AccountAddress,
    private_key: String,
    nonce: u64,
    balance: i64,
}

impl Account {
    pub fn random() -> Self {
        let signing_key = SigningKey::random(&mut rand::thread_rng());
        let private_key_bytes = signing_key.to_bytes();
        let private_key_hex = hex::encode(&private_key_bytes);

        let public_key = signing_key.verifying_key().to_encoded_point(false);
        let public_key_bytes = &public_key.as_bytes()[1..];
        let hash = keccak256(public_key_bytes);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);

        Self {
            address: AccountAddress(address),
            private_key: private_key_hex,
            nonce: 0,
            balance: i64::MAX,
        }
    }

    fn transfer_to(&mut self, to: &Account) -> Transaction {
        let n = self.nonce;
        self.nonce += 1;
        let mut txn = Transaction {
            from_address: self.address.clone(),
            to_address: to.address.clone(),
            signature: None,
            script: None,
            nonce: n,
        };
        let bytes = serde_json::to_string(&txn).expect("Failed to serialize transaction");
        txn.signature = Some(bytes);
        txn
    }
}

#[derive(Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
struct Transaction {
    from_address: AccountAddress,
    to_address: AccountAddress,
    signature: Option<String>,
    script: Option<String>,
    nonce: u64,
}

#[derive(Clone, Hash, Serialize, Deserialize)]
struct Block {
    block_id: [u8; 32],
    txns: Vec<Transaction>,
}

impl Block {
    fn new(block_id: [u8; 32], txns: Vec<Transaction>) -> Self {
        Self { block_id, txns }
    }

    fn to_gtxns(&self) -> Vec<GTxn> {
        self.txns
            .iter()
            .map(|txn| GTxn {
                sequence_number: txn.nonce,
                max_gas_amount: todo!(),
                gas_unit_price: todo!(),
                expiration_timestamp_secs: todo!(),
                chain_id: todo!(),
                txn_bytes: serde_json::to_vec(&txn).expect("Failed to serialize txn"),
            })
            .collect()
    }

    fn calculate_hash(&self) -> [u8; 32] {
        let serialized = bincode::serialize(self).expect("Serialization failed");

        let mut hasher = Sha256::new();
        hasher.update(serialized);

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}

pub struct BlockIdTracer {
    block_id: RefCell<u64>,
}

impl BlockIdTracer {
    pub fn new() -> Self {
        BlockIdTracer { block_id: RefCell::new(0) }
    }

    pub fn next_block_id(&self) -> [u8; 32] {
        let mut block_id = self.block_id.borrow_mut();
        *block_id += 1;

        let mut result = [0u8; 32];
        result[24..].copy_from_slice(&block_id.to_be_bytes());
        result
    }
}

struct BlockStore {
    account_state: HashMap<AccountAddress, RefCell<Account>>,
    block_state: BTreeMap<[u8; 32], Block>,
    block_id_tracker: BlockIdTracer,
}

impl BlockStore {
    fn generate_rand_txn(&self) -> Transaction {
        let keys: Vec<_> = self.account_state.keys().collect();

        let mut rng = rand::thread_rng();
        let keys = keys.into_iter().choose_multiple(&mut rng, 2).into_iter().collect::<Vec<_>>();

        let from = self.account_state.get(keys.first().expect("Impossible")).unwrap();
        let to = self.account_state.get(keys.last().expect("Impossible")).unwrap();

        from.borrow_mut().transfer_to(to.borrow().deref())
    }

    pub fn generate_proposal(&self) -> BlockBatch {
        let mut rng = rand::thread_rng();
        let proposal_txn_num: i32 = rng.gen_range(100..=20000);
        let txns: Vec<Transaction> =
            (0..proposal_txn_num).map(|_| self.generate_rand_txn()).collect();
        let block = Block::new(self.block_id_tracker.next_block_id(), txns);
        BlockBatch {
            txns: block.to_gtxns(),
            block_hash: block.calculate_hash(),
        }
    }

    fn apply_gtxn(&self, gtxn: GTxn) {
        let txn: Transaction =
            serde_json::from_slice(&gtxn.txn_bytes).expect("Failed to deserialize");
        let from_account =
            self.account_state.get(&txn.from_address).expect("Failed to get account");
        let to_account = self.account_state.get(&txn.to_address).expect("Failed to get account");
        let remove_gas = gtxn.gas_unit_price;
    }

    pub fn apply_block(&self, txns: Vec<GTxn>) {}
}
