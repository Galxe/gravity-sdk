use gaptos::api_types::{
    account::{ExternalAccountAddress, ExternalChainId}, compute_res::ComputeRes, simple_hash::hash_to_fixed_array, u256_define::TxnHash, VerifiedTxn
};
use futures::channel::oneshot::Sender;
use hex::decode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub balance: u64,
    pub nonce: u64,
    pub kv_store: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TransactionKind {
    Transfer { receiver: String, amount: u64 },
    SetKV { key: String, value: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnsignedTransaction {
    pub nonce: u64,
    pub kind: TransactionKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub unsigned: UnsignedTransaction,
    pub signature: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionWithAccount {
    pub txn: Transaction,
    pub address: String,
}

impl From<VerifiedTxn> for TransactionWithAccount {
    fn from(value: VerifiedTxn) -> Self {
        let txn: TransactionWithAccount = serde_json::from_slice(&value.bytes()).unwrap();
        txn
    }
}

// fn convert_account(input: &str) -> Result<[u8; 32], String> {
//     let bytes = decode(input).map_err(|_| "Invalid hex string".to_string())?;
//     if bytes.len() != 32 {
//         return Err(format!("Invalid length: expected 32 bytes, got {}", bytes.len()));
//     }

//     let mut array = [0u8; 32];
//     array.copy_from_slice(&bytes);
//     Ok(array)
// }

fn convert_account(acc: &str) -> Result<[u8; 32], String> {
    let acc_bytes = hex::decode(acc).unwrap();

    if acc_bytes.len() != 20 {
        return Err(format!("Invalid length: expected 20 bytes, got {}", acc_bytes.len()));
    }

    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(&acc_bytes);

    Ok(bytes)
}

impl TransactionWithAccount {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let txn: TransactionWithAccount = serde_json::from_slice(&bytes).unwrap();
        txn
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    pub fn into_verified(self) -> VerifiedTxn {
        let bytes = self.to_bytes();
        let hash = hash_to_fixed_array(&bytes);
        VerifiedTxn::new(
            bytes,
            self.account(),
            self.txn.unsigned.nonce,
            ExternalChainId::new(0),
            TxnHash::new(hash),
        )
    }

    pub fn account(&self) -> ExternalAccountAddress {
        ExternalAccountAddress::new(convert_account(self.address.as_str()).unwrap())
        // let mut bytes = [0u8; 32];
        // bytes[12..].copy_from_slice(&string_to_u8_32(self.address.as_str()).unwrap());
        // ExternalAccountAddress::new(bytes)
    }

    pub fn sequence_number(&self) -> u64 {
        self.txn.unsigned.nonce
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub number: u64,
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub transactions_root: [u8; 32],
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBlock {
    pub block_number: u64,
    pub transactions: Vec<Transaction>,
}

pub struct ExecutableBlock {
    pub block: RawBlock,
    // send compute res as hash and receive the commit block id
    pub callbacks: Sender<(ComputeRes, Sender<u64>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone)]
pub struct BlockExecutionResult {
    pub block_number: u64,
    pub state_updates: HashMap<AccountId, AccountState>,
    pub receipts: Vec<TransactionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub transaction: Transaction,
    pub transaction_hash: [u8; 32],
    pub status: bool,
    pub gas_used: u64,
    pub logs: Vec<Log>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    pub address: String,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub nonce: u64,
    pub balance: u64,
    pub kv_store: HashMap<String, String>,
}

#[derive(Debug)]
pub struct BlockExecutionPlan {
    pub block: RawBlock,
    pub account_dependencies: HashMap<AccountId, HashSet<AccountId>>,
}

#[derive(Debug)]
pub struct TransactionWithLocation {
    pub transaction: Transaction,
    pub block_number: u64,
    pub tx_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRoot(pub [u8; 32]);

impl StateRoot {
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}
