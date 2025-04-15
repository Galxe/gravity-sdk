// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    contract_event::ContractEvent,
    state_store::state_key::StateKey,
    transaction::{BlockExecutableTransaction, Transaction},
    write_set::WriteOp,
};
use aptos_crypto::{hash::CryptoHash, HashValue};
use move_core_types::{account_address::AccountAddress, language_storage::StructTag};
// use move_vm_types::delayed_values::delayed_field_id::DelayedFieldID;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{ExtractUniqueIndex, ExtractWidth};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SignatureVerifiedTransaction {
    Valid(Transaction),
    Invalid(Transaction),
}

impl SignatureVerifiedTransaction {
    pub fn into_inner(self) -> Transaction {
        match self {
            SignatureVerifiedTransaction::Valid(txn) => txn,
            SignatureVerifiedTransaction::Invalid(txn) => txn,
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            SignatureVerifiedTransaction::Valid(_) => true,
            SignatureVerifiedTransaction::Invalid(_) => false,
        }
    }

    pub fn sender(&self) -> Option<AccountAddress> {
        match self {
            SignatureVerifiedTransaction::Valid(txn) => match txn {
                Transaction::UserTransaction(txn) => Some(txn.sender()),
                _ => None,
            },
            SignatureVerifiedTransaction::Invalid(_) => None,
        }
    }

    pub fn hash(&self) -> HashValue {
        match self {
            SignatureVerifiedTransaction::Valid(txn) => txn.hash(),
            SignatureVerifiedTransaction::Invalid(txn) => txn.hash(),
        }
    }

    pub fn expect_valid(&self) -> &Transaction {
        match self {
            SignatureVerifiedTransaction::Valid(txn) => txn,
            SignatureVerifiedTransaction::Invalid(_) => panic!("Expected valid transaction"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct GravityId(u64);

impl ExtractWidth for GravityId {
    fn extract_width(&self) -> u32 {
        todo!()
    }
}

impl ExtractUniqueIndex for GravityId {
    fn extract_unique_index(&self) -> u32 {
        todo!()
    }
}

impl From<u64> for GravityId {
    fn from(value: u64) -> Self {
        todo!()
    }
}

impl From<(u32, u32)> for GravityId {
    fn from(value: (u32, u32)) -> Self {
        todo!()
    }
}

impl BlockExecutableTransaction for SignatureVerifiedTransaction {
    type Event = ContractEvent;
    type Identifier = GravityId;
    type Key = StateKey;
    type Tag = StructTag;
    type Value = WriteOp;

    fn user_txn_bytes_len(&self) -> usize {
        match self {
            SignatureVerifiedTransaction::Valid(Transaction::UserTransaction(txn)) => {
                txn.txn_bytes_len()
            },
            _ => 0,
        }
    }
}



// impl From<(u32, u32)> for u64 {
//     fn from(value: (u32, u32)) -> Self {
//         todo!()
//     }
// }

impl From<Transaction> for SignatureVerifiedTransaction {
    fn from(txn: Transaction) -> Self {
        match txn {
            Transaction::UserTransaction(txn) => match txn.verify_signature() {
                Ok(_) => SignatureVerifiedTransaction::Valid(Transaction::UserTransaction(txn)),
                Err(_) => SignatureVerifiedTransaction::Invalid(Transaction::UserTransaction(txn)),
            },
            _ => SignatureVerifiedTransaction::Valid(txn),
        }
    }
}

pub fn into_signature_verified_block(txns: Vec<Transaction>) -> Vec<SignatureVerifiedTransaction> {
    txns.into_iter().map(|t| t.into()).collect()
}

pub trait TransactionProvider: Debug {
    fn get_transaction(&self) -> Option<&Transaction>;
}

impl TransactionProvider for SignatureVerifiedTransaction {
    fn get_transaction(&self) -> Option<&Transaction> {
        if self.is_valid() {
            Some(self.expect_valid())
        } else {
            None
        }
    }
}

impl TransactionProvider for Transaction {
    fn get_transaction(&self) -> Option<&Transaction> {
        Some(self)
    }
}
