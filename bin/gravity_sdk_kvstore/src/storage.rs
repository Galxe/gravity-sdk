use async_trait::async_trait;
use sled::{transaction::TransactionError, Db};
use std::{collections::HashMap, path::Path};

use crate::{AccountId, AccountState, Block, StateRoot, TransactionReceipt};

#[async_trait]
pub trait Storage: Send + Sync + 'static {
    async fn save_block(&self, block: &Block) -> Result<(), String>;
    async fn get_block(&self, number: u64) -> Result<Option<Block>, String>;
    async fn save_transaction_receipts(
        &self,
        receipts: Vec<TransactionReceipt>,
    ) -> Result<(), String>;
    async fn get_transaction_receipt(
        &self,
        transaction_hash: [u8; 32],
    ) -> Result<Option<TransactionReceipt>, String>;
    async fn save_state_root(&self, block_number: u64, root: StateRoot) -> Result<(), String>;
    async fn get_state_root(&self, block_number: u64) -> Result<Option<StateRoot>, String>;
    async fn save_account_state(
        &self,
        account_id: &AccountId,
        state: &AccountState,
    ) -> Result<(), String>;
    async fn get_account_state(
        &self,
        account_id: &AccountId,
    ) -> Result<Option<AccountState>, String>;
}

#[derive(Clone)]
pub struct SledStorage {
    db: Db,
}

impl SledStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let db = sled::open(path).map_err(|e| format!("Failed to open database: {}", e))?;
        Ok(Self { db })
    }

    fn block_key(number: u64) -> Vec<u8> {
        format!("block:{}", number).into_bytes()
    }

    fn state_root_key(number: u64) -> Vec<u8> {
        format!("state_root:{}", number).into_bytes()
    }

    fn account_key(account_id: &AccountId) -> Vec<u8> {
        format!("account:{}", account_id.0).into_bytes()
    }
}

#[async_trait]
impl Storage for SledStorage {
    async fn save_block(&self, block: &Block) -> Result<(), String> {
        let encoded =
            bincode::serialize(block).map_err(|e| format!("Failed to serialize block: {}", e))?;

        self.db
            .insert(Self::block_key(block.header.number), encoded)
            .map_err(|e| format!("Failed to save block: {}", e))?;

        self.db.flush().map_err(|e| format!("Failed to flush database: {}", e))?;

        Ok(())
    }

    async fn get_block(&self, number: u64) -> Result<Option<Block>, String> {
        match self.db.get(Self::block_key(number)) {
            Ok(Some(data)) => {
                let block = bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to deserialize block: {}", e))?;
                Ok(Some(block))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Failed to get block: {}", e)),
        }
    }

    async fn save_transaction_receipts(
        &self,
        receipts: Vec<TransactionReceipt>,
    ) -> Result<(), String> {
        let encodes = receipts
            .iter()
            .map(|receipt| {
                let encoded =
                    bincode::serialize(receipt).expect("Failed to serialize transaction receipt");
                (receipt.transaction_hash.clone(), encoded)
            })
            .collect::<HashMap<_, _>>();

        self.db
            .transaction(|tx_db| {
                for (tx_hash, receipt) in &encodes {
                    tx_db.insert(tx_hash, receipt.clone())?;
                }
                Ok(())
            })
            .map_err(|e: TransactionError| format!("Failed to save transaction receipts"))?;

        Ok(())
    }

    async fn get_transaction_receipt(
        &self,
        transaction_hash: [u8; 32],
    ) -> Result<Option<TransactionReceipt>, String> {
        match self.db.get(transaction_hash) {
            Ok(Some(data)) => {
                let receipt = bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to deserialize block: {}", e))?;
                Ok(receipt)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Failed to get block: {}", e)),
        }
    }

    async fn save_state_root(&self, block_number: u64, root: StateRoot) -> Result<(), String> {
        let encoded = bincode::serialize(&root)
            .map_err(|e| format!("Failed to serialize state root: {}", e))?;

        self.db
            .insert(Self::state_root_key(block_number), encoded)
            .map_err(|e| format!("Failed to save state root: {}", e))?;

        self.db.flush().map_err(|e| format!("Failed to flush database: {}", e))?;

        Ok(())
    }

    async fn get_state_root(&self, block_number: u64) -> Result<Option<StateRoot>, String> {
        match self.db.get(Self::state_root_key(block_number)) {
            Ok(Some(data)) => {
                let root = bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to deserialize state root: {}", e))?;
                Ok(Some(root))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Failed to get state root: {}", e)),
        }
    }

    async fn save_account_state(
        &self,
        account_id: &AccountId,
        state: &AccountState,
    ) -> Result<(), String> {
        let encoded = bincode::serialize(state)
            .map_err(|e| format!("Failed to serialize account state: {}", e))?;

        self.db
            .insert(Self::account_key(account_id), encoded)
            .map_err(|e| format!("Failed to save account state: {}", e))?;

        self.db.flush().map_err(|e| format!("Failed to flush database: {}", e))?;

        Ok(())
    }

    async fn get_account_state(
        &self,
        account_id: &AccountId,
    ) -> Result<Option<AccountState>, String> {
        match self.db.get(Self::account_key(account_id)) {
            Ok(Some(data)) => {
                let state = bincode::deserialize(&data)
                    .map_err(|e| format!("Failed to deserialize account state: {}", e))?;
                Ok(Some(state))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Failed to get account state: {}", e)),
        }
    }
}
