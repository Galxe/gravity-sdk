use std::iter::Peekable;

use aptos_schemadb::iterator::SchemaIterator;
use aptos_storage_interface::{db_ensure as ensure, AptosDbError, Result};
use aptos_types::{
    contract_event::ContractEvent, ledger_info::LedgerInfoWithSignatures, transaction::Version,
};

use crate::storage::schema::ledger_info::LedgerInfoSchema;

pub struct EpochEndingLedgerInfoIter<'a> {
    inner: SchemaIterator<'a, LedgerInfoSchema>,
    next_epoch: u64,
    end_epoch: u64,
}

impl<'a> EpochEndingLedgerInfoIter<'a> {
    pub(crate) fn new(
        inner: SchemaIterator<'a, LedgerInfoSchema>,
        next_epoch: u64,
        end_epoch: u64,
    ) -> Self {
        Self {
            inner,
            next_epoch,
            end_epoch,
        }
    }

    fn next_impl(&mut self) -> Result<Option<LedgerInfoWithSignatures>> {
        if self.next_epoch >= self.end_epoch {
            return Ok(None);
        }

        let ret = match self.inner.next().transpose()? {
            Some((epoch, li)) => {
                if !li.ledger_info().ends_epoch() {
                    None
                } else {
                    ensure!(
                        epoch == self.next_epoch,
                        "Epochs are not consecutive. expecting: {}, got: {}",
                        self.next_epoch,
                        epoch,
                    );
                    self.next_epoch += 1;
                    Some(li)
                }
            }
            _ => None,
        };

        Ok(ret)
    }
}

impl<'a> Iterator for EpochEndingLedgerInfoIter<'a> {
    type Item = Result<LedgerInfoWithSignatures>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_impl().transpose()
    }
}
