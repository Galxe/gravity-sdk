use std::{collections::HashSet, sync::Arc, time::Instant};

use aptos_crypto::HashValue;
use aptos_infallible::Mutex;
use aptos_types::validator_txn::ValidatorTransaction;

#[derive(Clone)]
pub struct VTxnPoolState {
    inner: Arc<Mutex<PoolStateInner>>,
}

impl Default for VTxnPoolState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PoolStateInner::default())),
        }
    }
}

#[derive(Default)]
pub struct PoolStateInner {
}

impl PoolStateInner {
    pub fn pull(
        &mut self,
        deadline: Instant,
        mut max_items: u64,
        mut max_bytes: u64,
        filter: TransactionFilter,
    ) -> Vec<ValidatorTransaction> {
        vec![]
    }
}

pub enum TransactionFilter {
    PendingTxnHashSet(HashSet<HashValue>),
}

impl VTxnPoolState {
    pub fn pull(
        &self,
        deadline: Instant,
        max_items: u64,
        max_bytes: u64,
        filter: TransactionFilter,
    ) -> Vec<ValidatorTransaction> {
        self.inner
            .lock()
            .pull(deadline, max_items, max_bytes, filter)
    }
}