use aptos_consensus_types::vote_msg::VoteMsg;
use aptos_crypto::HashValue;
use aptos_schemadb::{SchemaBatch, DB};
use aptos_storage_interface::{AptosDbError, Result};
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use arc_swap::{access::Access, ArcSwap};
use crate::consensusdb::schema::ledger_info::{self, LedgerInfoSchema};
use std::sync::Arc;
use aptos_logger::info;

const MAX_LEDGER_INFOS: u32 = 256;

fn get_latest_ledger_info_in_db_impl(db: &DB) -> Result<Option<LedgerInfoWithSignatures>> {
    match db.iter::<LedgerInfoSchema>() {
        Err(e) => {
            if let AptosDbError::Other(msg) = &e {
                if msg.contains("column family name: ledger_info") {
                    return Ok(None);
                }
            }
            Err(e)
        }
        Ok(mut iter) => {
            iter.seek_to_last();
            Ok(iter.next().transpose()?.map(|(_, v)| v))
        }
    }
}

fn get_latest_ledger_infos_in_db_impl(db: &DB) -> Result<Vec<LedgerInfoWithSignatures>> {
    match db.rev_iter::<LedgerInfoSchema>() {
        Err(e) => {
            if let AptosDbError::Other(msg) = &e {
                if msg.contains("column family name: ledger_info") {
                    return Ok(vec![]);
                }
            }
            Err(e)
        }
        Ok(mut iter) => {
            let mut res: Vec<_> = vec![];
            iter.seek_to_last();
            for _ in 0..MAX_LEDGER_INFOS {
                match iter.next().transpose() {
                    Ok(Some((_, v))) => res.push(v),
                    _ => break,
                }
            }
            Ok(res)
        }
    }
}

#[derive(Debug)]
pub(crate) struct LedgerMetadataDb {
    db: Arc<DB>,
    /// We almost always need the latest ledger info and signatures to serve read requests, so we
    /// cache it in memory in order to avoid reading DB and deserializing the object frequently. It
    /// should be updated every time new ledger info and signatures are persisted.
    latest_ledger_info: ArcSwap<Option<LedgerInfoWithSignatures>>,
}
impl LedgerMetadataDb {
    pub(super) fn new(db: Arc<DB>) -> Self {
        let latest_ledger_info = get_latest_ledger_info_in_db_impl(&db).expect("DB read failed.");
        let latest_ledger_info = ArcSwap::from(Arc::new(latest_ledger_info));
        Self {
            db,
            latest_ledger_info,
        }
    }
    pub(super) fn db(&self) -> &DB {
        &self.db
    }
    pub(super) fn db_arc(&self) -> Arc<DB> {
        Arc::clone(&self.db)
    }
    pub(crate) fn write_schemas(&self, batch: SchemaBatch) -> Result<()> {
        match self.db.write_schemas(batch) {
            Ok(()) => Ok(()),
            Err(e) => panic!("{}", e),
        }
    }
}
/// LedgerInfo APIs.
impl LedgerMetadataDb {
    /// Stores the latest ledger info in memory.
    pub(crate) fn set_latest_ledger_info(&self, ledger_info_with_sigs: LedgerInfoWithSignatures) {
        self.latest_ledger_info
            .store(Arc::new(Some(ledger_info_with_sigs)));
    }

    pub(crate) fn get_latest_ledger_info(&self) -> Option<LedgerInfoWithSignatures> {
        let latest_ledger_info = self.latest_ledger_info.load();
        latest_ledger_info.as_ref().clone()
    }

    /// Writes `ledger_info_with_sigs` to `batch`.
    pub(crate) fn put_ledger_info(
        &self,
        ledger_info_with_sigs: &LedgerInfoWithSignatures,
        batch: &SchemaBatch,
    ) -> Result<()> {
        let ledger_info = ledger_info_with_sigs.ledger_info();
        batch.put::<LedgerInfoSchema>(&ledger_info.block_number(), ledger_info_with_sigs)
    }

    pub(crate) fn get_latest_ledger_infos(&self) -> Vec<LedgerInfoWithSignatures> {
        get_latest_ledger_infos_in_db_impl(&self.db).expect("DB read failed.")
    }

    pub(crate) fn get_block_hash(&self, block_number: u64) -> Option<HashValue> {
        match self.db.get::<LedgerInfoSchema>(&block_number) {
            Ok(Some(ledger_info)) => Some(ledger_info.ledger_info().block_hash()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::consensusdb::schema::LEDGER_INFO_CF_NAME;

    use super::*;
    use aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
    use aptos_types::on_chain_config::ValidatorSet;
    use rocksdb::Options;

    fn init_db() -> Arc<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        Arc::new(DB::open(
            "/tmp/node1/data/consensus_db",
            "ledger_meta_db_test",
            vec![LEDGER_INFO_CF_NAME],
            &opts,
        ).unwrap())
    }

    #[test]
    fn test_write_read() {
        // {
        //     let ledger_metadata_db = LedgerMetadataDb::new(init_db());
        //     for i in 0..100 {
        //         let batch = SchemaBatch::default();
        //         let mut ledger_info_with_sigs = LedgerInfoWithSignatures::genesis(
        //                     *ACCUMULATOR_PLACEHOLDER_HASH,
        //                     ValidatorSet::empty(),
        //         );
        //         ledger_info_with_sigs.set_block_number(i);
        //         ledger_metadata_db.put_ledger_info(&ledger_info_with_sigs, &batch);
        //         ledger_metadata_db.write_schemas(batch);
        //     }
        // }

        {
            let ledger_metadata_db = LedgerMetadataDb::new(init_db());
            let res = ledger_metadata_db.get_latest_ledger_infos();
            println!("res len {} ", res.len());
            println!("latest_block_number {}", ledger_metadata_db.get_latest_ledger_info().unwrap().ledger_info().block_number());
        }
    }
}
