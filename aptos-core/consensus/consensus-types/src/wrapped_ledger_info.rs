// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{quorum_cert::QuorumCert, vote_data::VoteData};
use anyhow::{ensure, Context};
use gaptos::{
    aptos_crypto::hash::CryptoHash,
    aptos_types::{
        block_info::BlockInfo, ledger_info::LedgerInfoWithSignatures,
        validator_verifier::ValidatorVerifier,
    },
};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// This struct is similar to QuorumCert, except that the verify function doesn't verify vote_data.
/// This struct is introduced to ensure backward compatibility when upgrading the consensus to use
/// order votes to execute blocks faster. When order votes are enabled, then vote_data and
/// consensus_data_hash inside signed_ledger_info are not used anywhere in the code and can be set
/// to dummy values.
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
pub struct WrappedLedgerInfo {
    /// The VoteData here is placeholder for backwards compatibility purpose and should not be used
    /// when order votes are enabled.
    vote_data: VoteData,
    /// The signed LedgerInfo of a committed block that carries the data about the certified block.
    signed_ledger_info: LedgerInfoWithSignatures,
}

impl Display for WrappedLedgerInfo {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "WrappedLedgerInfo: [{}, {}]", self.vote_data, self.signed_ledger_info)
    }
}

impl WrappedLedgerInfo {
    pub fn new(vote_data: VoteData, signed_ledger_info: LedgerInfoWithSignatures) -> Self {
        WrappedLedgerInfo { vote_data, signed_ledger_info }
    }

    #[cfg(any(test, feature = "fuzzing"))]
    pub fn dummy() -> Self {
        Self {
            vote_data: VoteData::dummy(),
            signed_ledger_info: LedgerInfoWithSignatures::new(
                gaptos::aptos_types::ledger_info::LedgerInfo::dummy(),
                gaptos::aptos_types::aggregate_signature::AggregateSignature::empty(),
            ),
        }
    }

    fn verify_consensus_data_hash(&self) -> anyhow::Result<()> {
        let vote_hash = self.vote_data.hash();
        ensure!(
            self.ledger_info().ledger_info().consensus_data_hash() == vote_hash,
            "WrappedLedgerInfo's vote data hash mismatch LedgerInfo"
        );
        Ok(())
    }

    pub fn certified_block(&self, order_vote_enabled: bool) -> anyhow::Result<&BlockInfo> {
        ensure!(
            !order_vote_enabled,
            "wrapped_ledger_info.certified_block should not be called when order votes are enabled"
        );
        self.verify_consensus_data_hash()?;
        Ok(self.vote_data.proposed())
    }

    pub fn ledger_info(&self) -> &LedgerInfoWithSignatures {
        &self.signed_ledger_info
    }

    pub fn commit_info(&self) -> &BlockInfo {
        self.ledger_info().ledger_info().commit_info()
    }

    /// If the QC commits reconfiguration and starts a new epoch
    pub fn ends_epoch(&self) -> bool {
        self.signed_ledger_info.ledger_info().ends_epoch()
    }

    pub fn verify(&self, validator: &ValidatorVerifier) -> anyhow::Result<()> {
        // Genesis's QC is implicitly agreed upon, it doesn't have real signatures.
        // If someone sends us a QC on a fake genesis, it'll fail to insert into BlockStore
        // because of the round constraint.
        //
        // VoteData is a placeholder when order votes are enabled, so it cannot determine whether
        // the signed ledger info is the unsigned genesis certificate.
        if self.ledger_info().ledger_info().round() == 0 {
            let commit_info = self.ledger_info().ledger_info().commit_info();
            ensure!(
                self.ledger_info().ledger_info().consensus_data_hash() ==
                    VoteData::new(commit_info.clone(), commit_info.clone()).hash(),
                "Genesis QC has inconsistent commit block"
            );
            ensure!(
                self.ledger_info().get_num_voters() == 0,
                "Genesis QC should not carry signatures"
            );
            return Ok(());
        }
        self.ledger_info()
            .verify_signatures(validator)
            .context("Fail to verify WrappedLedgerInfo")?;
        Ok(())
    }

    pub fn create_merged_with_executed_state(
        &self,
        executed_ledger_info: LedgerInfoWithSignatures,
    ) -> anyhow::Result<WrappedLedgerInfo> {
        let self_commit_info = self.commit_info();
        let executed_commit_info = executed_ledger_info.ledger_info().commit_info();
        ensure!(
            self_commit_info.match_ordered_only(executed_commit_info),
            "Block info from QC and executed LI need to match, {:?} and {:?}",
            self_commit_info,
            executed_commit_info
        );
        Ok(Self::new(self.vote_data.clone(), executed_ledger_info))
    }

    pub fn into_quorum_cert(self, order_vote_enabled: bool) -> anyhow::Result<QuorumCert> {
        ensure!(
            !order_vote_enabled,
            "wrapped_ledger_info.into_quorum_cert should not be called when order votes are enabled"
        );
        self.verify_consensus_data_hash()?;
        Ok(QuorumCert::new(self.vote_data.clone(), self.signed_ledger_info.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::block_test_utils::certificate_for_genesis;
    use gaptos::{
        aptos_crypto::HashValue,
        aptos_types::{
            aggregate_signature::AggregateSignature, ledger_info::LedgerInfo,
            validator_verifier::random_validator_verifier,
        },
    };

    #[test]
    fn dummy_vote_data_cannot_bypass_non_genesis_signature_verification() {
        let (_, verifier) = random_validator_verifier(1, None, false);
        let ledger_info = LedgerInfo::new(BlockInfo::random_with_epoch(1, 1), HashValue::zero());
        let wrapped = WrappedLedgerInfo::new(
            VoteData::dummy(),
            LedgerInfoWithSignatures::new(ledger_info, AggregateSignature::empty()),
        );

        assert!(wrapped.verify(&verifier).is_err());
    }

    #[test]
    fn unsigned_genesis_is_still_accepted() {
        let (_, verifier) = random_validator_verifier(1, None, false);
        assert!(certificate_for_genesis().into_wrapped_ledger_info().verify(&verifier).is_ok());
    }

    #[test]
    fn unsigned_round_zero_non_genesis_is_rejected() {
        let (_, verifier) = random_validator_verifier(1, None, false);
        let ledger_info = LedgerInfo::new(BlockInfo::random_with_epoch(1, 0), HashValue::zero());
        let wrapped = WrappedLedgerInfo::new(
            VoteData::dummy(),
            LedgerInfoWithSignatures::new(ledger_info, AggregateSignature::empty()),
        );

        assert!(wrapped.verify(&verifier).is_err());
    }
}
