use aptos_storage_interface::{AptosDbError, DbReader};
use aptos_types::{
    account_address::AccountAddress,
    contract_event::EventWithVersion,
    epoch_change::EpochChangeProof,
    epoch_state::EpochState,
    proof::TransactionAccumulatorSummary,
    state_proof::StateProof,
    state_store::{state_key::StateKey, state_value::StateValue},
};

use super::ledger_db::MAX_NUM_EPOCH_ENDING_LEDGER_INFO;

impl DbReader for GravityDB {
    fn get_epoch_ending_ledger_infos(
        &self,
        start_epoch: u64,
        end_epoch: u64,
    ) -> Result<EpochChangeProof> {
        let (ledger_info_with_sigs, more) =
            Self::get_epoch_ending_ledger_infos(self, start_epoch, end_epoch)?;
        Ok(EpochChangeProof::new(ledger_info_with_sigs, more))
    }

    fn get_latest_ledger_info_option(&self) -> Result<Option<LedgerInfoWithSignatures>> {
        match self.ledger_db.metadata_db().get_latest_ledger_info_option() {
            Some(result) => Ok(Some(result)),
            None => Ok(Some(self.mock_db.get_latest_ledger_info()?)),
        }
    }

    fn get_synced_version(&self) -> Result<Option<Version>> {
        match self.ledger_db.metadata_db().get_synced_version() {
            Ok(Some(result)) => Ok(Some(result)),
            Ok(None) => self.mock_db.get_synced_version(),
            Err(e) => panic!("get_synced_version error {}", e),
        }
    }

    fn get_pre_committed_version(&self) -> Result<Option<Version>> {
        Ok(self.ledger_db.metadata_db().get_pre_committed_version())
    }

    /// Gets ledger info at specified version and ensures it's an epoch ending.
    fn get_epoch_ending_ledger_info(&self, version: u64) -> Result<LedgerInfoWithSignatures> {
        self.ledger_db
            .metadata_db()
            .get_epoch_ending_ledger_info(version)
    }

    fn get_state_proof_with_ledger_info(
        &self,
        known_version: u64,
        ledger_info_with_sigs: LedgerInfoWithSignatures,
    ) -> Result<StateProof> {
        let ledger_info = ledger_info_with_sigs.ledger_info();
        ensure!(
            known_version <= ledger_info.version(),
            "Client known_version {} larger than ledger version {}.",
            known_version,
            ledger_info.version(),
        );
        let known_epoch = self.ledger_db.metadata_db().get_epoch(known_version)?;
        let end_epoch = ledger_info.next_block_epoch();
        let epoch_change_proof = if known_epoch < end_epoch {
            let (ledger_infos_with_sigs, more) =
                self.get_epoch_ending_ledger_infos(known_epoch, end_epoch)?;
            EpochChangeProof::new(ledger_infos_with_sigs, more)
        } else {
            EpochChangeProof::new(vec![], /* more = */ false)
        };

        Ok(StateProof::new(ledger_info_with_sigs, epoch_change_proof))
    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof> {
        match self.ledger_db.metadata_db().get_latest_ledger_info() {
            Ok(ledger_info_with_sigs) => {
                return self.get_state_proof_with_ledger_info(known_version, ledger_info_with_sigs)
            },
            Err(_) => self.mock_db.get_state_proof(known_version),
        }
    }

    fn get_latest_epoch_state(&self) -> Result<EpochState> {
        let latest_ledger_info = self.ledger_db.metadata_db().get_latest_ledger_info()?;
        match latest_ledger_info.ledger_info().next_epoch_state() {
            Some(epoch_state) => Ok(epoch_state.clone()),
            None => self
                .ledger_db
                .metadata_db()
                .get_epoch_state(latest_ledger_info.ledger_info().epoch()),
        }
    }

    fn get_state_value_by_version(
        &self,
        state_key: &StateKey,
        version: Version,
    ) -> Result<Option<StateValue>> {
        self.mock_db.get_state_value_by_version(state_key, version)
    }
}

impl GravityDB {
    /// Returns ledger infos reflecting epoch bumps starting with the given epoch. If there are no
    /// more than `MAX_NUM_EPOCH_ENDING_LEDGER_INFO` results, this function returns all of them,
    /// otherwise the first `MAX_NUM_EPOCH_ENDING_LEDGER_INFO` results are returned and a flag
    /// (when true) will be used to indicate the fact that there is more.
    fn get_epoch_ending_ledger_infos(
        &self,
        start_epoch: u64,
        end_epoch: u64,
    ) -> Result<(Vec<LedgerInfoWithSignatures>, bool)> {
        self.get_epoch_ending_ledger_infos_impl(
            start_epoch,
            end_epoch,
            MAX_NUM_EPOCH_ENDING_LEDGER_INFO,
        )
    }

    fn get_epoch_ending_ledger_infos_impl(
        &self,
        start_epoch: u64,
        end_epoch: u64,
        limit: usize,
    ) -> Result<(Vec<LedgerInfoWithSignatures>, bool)> {
        ensure!(
            start_epoch <= end_epoch,
            "Bad epoch range [{}, {})",
            start_epoch,
            end_epoch,
        );
        // Note that the latest epoch can be the same with the current epoch (in most cases), or
        // current_epoch + 1 (when the latest ledger_info carries next validator set)

        let latest_epoch = self
            .ledger_db
            .metadata_db()
            .get_latest_ledger_info()?
            .ledger_info()
            .next_block_epoch();
        ensure!(
            end_epoch <= latest_epoch,
            "Unable to provide epoch change ledger info for still open epoch. asked upper bound: {}, last sealed epoch: {}",
            end_epoch,
            latest_epoch - 1,  // okay to -1 because genesis LedgerInfo has .next_block_epoch() == 1
        );

        let (paging_epoch, more) = if end_epoch - start_epoch > limit as u64 {
            (start_epoch + limit as u64, true)
        } else {
            (end_epoch, false)
        };

        let lis = self
            .ledger_db
            .metadata_db()
            .get_epoch_ending_ledger_info_iter(start_epoch, paging_epoch)?
            .collect::<Result<Vec<_>>>()?;

        ensure!(
            lis.len() == (paging_epoch - start_epoch) as usize,
            "DB corruption: missing epoch ending ledger info for epoch {}",
            lis.last()
                .map(|li| li.ledger_info().next_block_epoch() - 1)
                .unwrap_or(start_epoch),
        );
        Ok((lis, more))
    }
}
