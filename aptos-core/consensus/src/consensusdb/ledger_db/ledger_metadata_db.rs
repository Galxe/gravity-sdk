use aptos_schemadb::{SchemaBatch, DB};
use aptos_storage_interface::Result;
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use arc_swap::ArcSwap;
use crate::consensusdb::schema::ledger_info::LedgerInfoSchema;
use std::sync::Arc;

const MAX_LEDGER_INFOS: u32 = 256;

fn get_latest_ledger_info_in_db_impl(db: &DB) -> Result<Option<LedgerInfoWithSignatures>> {
    let mut iter = db.iter::<LedgerInfoSchema>()?;
    iter.seek_to_last();
    Ok(iter.next().transpose()?.map(|(_, v)| v))
}

fn get_latest_ledger_infos_in_db_impl(db: &DB) -> Result<Vec<LedgerInfoWithSignatures>> {
    let mut res: Vec<_> = vec![];
    let mut iter = db.iter::<LedgerInfoSchema>()?;
    iter.seek_to_last();
    for _ in 0..MAX_LEDGER_INFOS {
        match iter.next().transpose() {
            Ok(Some((_, v))) => res.push(v),
            _ => break,
        }
    }
    Ok(res)
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
        self.db.write_schemas(batch)
    }
}
/// LedgerInfo APIs.
impl LedgerMetadataDb {
    /// Stores the latest ledger info in memory.
    pub(crate) fn set_latest_ledger_info(&self, ledger_info_with_sigs: LedgerInfoWithSignatures) {
        self.latest_ledger_info
            .store(Arc::new(Some(ledger_info_with_sigs)));
    }
    /// Writes `ledger_info_with_sigs` to `batch`.
    pub(crate) fn put_ledger_info(
        &self,
        ledger_info_with_sigs: &LedgerInfoWithSignatures,
        batch: &SchemaBatch,
    ) -> Result<()> {
        let ledger_info = ledger_info_with_sigs.ledger_info();
        batch.put::<LedgerInfoSchema>(&ledger_info.epoch(), ledger_info_with_sigs)
    }

    pub(crate) fn get_latest_ledger_infos(&self) -> Vec<LedgerInfoWithSignatures> {
        get_latest_ledger_infos_in_db_impl(&self.db).expect("DB read failed.")
    }
}
