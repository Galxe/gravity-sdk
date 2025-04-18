use gaptos::aptos_crypto::bls12381;
use gaptos::aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
use gaptos::aptos_storage_interface::{DbReader, DbWriter};
use gaptos::aptos_types::account_address::AccountAddress;
use gaptos::aptos_types::aggregate_signature::AggregateSignature;
use gaptos::aptos_types::block_info::BlockInfo;
use gaptos::aptos_types::epoch_change::EpochChangeProof;
use gaptos::aptos_types::epoch_state::EpochState;
use gaptos::aptos_types::ledger_info::{LedgerInfo, LedgerInfoWithSignatures};
use gaptos::aptos_types::on_chain_config::ValidatorSet;
use gaptos::aptos_types::on_chain_config::{ConsensusAlgorithmConfig, ProposerElectionType};
use gaptos::aptos_types::state_proof::StateProof;
use gaptos::aptos_types::state_store::state_key::inner::StateKeyInner;
use gaptos::aptos_types::validator_config::ValidatorConfig;
use gaptos::aptos_types::validator_info::ValidatorInfo;
use gaptos::aptos_types::validator_verifier::{ValidatorConsensusInfo, ValidatorVerifier};
use gaptos::aptos_types::{
    on_chain_config::{ConfigurationResource, OnChainConsensusConfig},
    state_store::{state_key::StateKey, state_value::StateValue},
    transaction::Version,
};

impl ConsensusDB {
    pub fn mock_validators(&self) -> Vec<ValidatorInfo> {
        let mut result = vec![];
        for (i, (addr, node_config)) in self.node_config_set.iter().enumerate() {
            // let x = hex::decode(node_config.consensus_public_key.as_bytes()).unwrap();
            let public_key = bls12381::PublicKey::try_from(
                hex::decode(node_config.consensus_public_key.as_bytes()).unwrap().as_slice()
            )
            .unwrap();
            let config = ValidatorConfig::new(
                public_key,
                bcs::to_bytes(&vec![addr.clone()]).unwrap(),
                bcs::to_bytes(&vec![addr.clone()]).unwrap(),
                i as u64,
            );
            result.push(ValidatorInfo::new(
                AccountAddress::try_from(node_config.account_address.clone()).unwrap(),
                node_config.voting_power,
                config,
            ));
        }
        result
    }
}

// TODO(gravity_byteyue): this is a temporary solution to enable quorum store
// We should get the value from the storage instead of using env variable
fn enable_quorum_store() -> bool {
    std::env::var("ENABLE_QUORUM_STORE").map(|s| s.parse().unwrap()).unwrap_or(true)
}

fn fixed_proposer() -> bool {
    std::env::var("FIXED_PROPOSER").map(|s| s.parse().unwrap()).unwrap_or(true)
}

impl DbReader for ConsensusDB {
    fn get_read_delegatee(&self) -> &dyn DbReader {
        self
    }

    fn get_latest_ledger_info(&self) -> Result<LedgerInfoWithSignatures, AptosDbError> {
        match self.ledger_db.metadata_db().get_latest_ledger_info() {
            Some(ledger_info) => Ok(ledger_info),
            None => {
                let genesis = LedgerInfoWithSignatures::genesis(
                    *ACCUMULATOR_PLACEHOLDER_HASH,
                    ValidatorSet::new(self.mock_validators()),
                );
                info!("genesis is {:?}", genesis);
                Ok(genesis)
            }
        }
    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof, AptosDbError> {
        let infos = self
            .mock_validators()
            .iter()
            .map(|v| {
                ValidatorConsensusInfo::new(
                    v.account_address,
                    v.consensus_public_key().clone(),
                    v.consensus_voting_power(),
                )
            })
            .collect();
        let verifier = ValidatorVerifier::new(infos);
        let epoch_state = EpochState::new(1, verifier);
        let block_info =
            BlockInfo::new(1, 0, HashValue::zero(), HashValue::zero(), 0, 0, Some(epoch_state));
        let ledger_info = LedgerInfo::new(block_info, HashValue::zero());
        Ok(StateProof::new(
            LedgerInfoWithSignatures::genesis(
                *ACCUMULATOR_PLACEHOLDER_HASH,
                ValidatorSet::new(self.mock_validators()),
            ),
            EpochChangeProof::new(
                vec![LedgerInfoWithSignatures::new(ledger_info, AggregateSignature::empty())],
                false,
            ),
        ))
    }

    fn get_state_value_by_version(
        &self,
        state_key: &StateKey,
        version: Version,
    ) -> Result<Option<StateValue>, AptosDbError> {
        let key = state_key.inner();
        let bytes = {
            match key {
                StateKeyInner::AccessPath(p) => {
                    let path = p.to_string();
                    if path.contains("Validator") {
                        bcs::to_bytes(&ValidatorSet::new(self.mock_validators()))?
                    } else if path.contains("consensus") {
                        let mut consensus_conf = OnChainConsensusConfig::default();
                        // todo(gravity_byteyue): currently we set quorum_store_enabled=false
                        match &mut consensus_conf {
                            OnChainConsensusConfig::V1(_) => {}
                            OnChainConsensusConfig::V2(_) => {}
                            OnChainConsensusConfig::V3 {alg, vtxn } => {}
                            OnChainConsensusConfig::V4 {alg,vtxn, window_size } => match alg {
                                ConsensusAlgorithmConfig::Jolteon {
                                    main,
                                    quorum_store_enabled,
                                } => {
                                    main.proposer_election_type = match fixed_proposer() {
                                        true => {
                                            info!("proposer_election_type use fixed proposer");
                                            ProposerElectionType::FixedProposer(1)
                                        }
                                        false => {
                                            info!("proposer_election_type use rotating proposer");
                                            ProposerElectionType::RotatingProposer(1)
                                        }
                                    };
                                    *quorum_store_enabled = enable_quorum_store();
                                }
                                ConsensusAlgorithmConfig::DAG(_) => {}
                                ConsensusAlgorithmConfig::JolteonV2 {
                                    main,
                                    quorum_store_enabled,
                                    order_vote_enabled,
                                } => {
                                    main.proposer_election_type = match fixed_proposer() {
                                        true => {
                                            info!("proposer_election_type use fixed proposer");
                                            ProposerElectionType::FixedProposer(1)
                                        }
                                        false => {
                                            info!("proposer_election_type use rotating proposer");
                                            ProposerElectionType::RotatingProposer(1)
                                        }
                                    };
                                    *quorum_store_enabled = enable_quorum_store();
                                    *order_vote_enabled = false;
                                }
                            },
                        }
                        bcs::to_bytes(&bcs::to_bytes(&consensus_conf)?)?
                    } else {
                        let mut resources = ConfigurationResource::default();
                        resources.epoch = 1;
                        bcs::to_bytes(&resources)?
                    }
                }
                StateKeyInner::TableItem { .. } => panic!(),
                StateKeyInner::Raw(_) => panic!(),
            }
        };
        Ok(Some(StateValue::new_legacy(bytes.into())))
    }
}
