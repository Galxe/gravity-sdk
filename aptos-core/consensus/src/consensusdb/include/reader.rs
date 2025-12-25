use bytes::Bytes;
use gaptos::api_types::config_storage::{
    OnChainConfig as GravityOnChainConfig, GLOBAL_CONFIG_STORAGE,
};
use gaptos::aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
use gaptos::aptos_storage_interface::{DbReader, DbWriter};
use gaptos::aptos_types::epoch_change::EpochChangeProof;
use gaptos::aptos_types::ledger_info::LedgerInfoWithSignatures;
use gaptos::aptos_types::on_chain_config::{ConsensusAlgorithmConfig, ProposerElectionType};
use gaptos::aptos_types::on_chain_config::{OnChainConfig, ValidatorSet};
use gaptos::aptos_types::state_proof::StateProof;
use gaptos::aptos_types::state_store::state_key::inner::StateKeyInner;
use gaptos::aptos_types::{
    on_chain_config::{ConfigurationResource, OnChainConsensusConfig},
    state_store::{state_key::StateKey, state_value::StateValue},
    transaction::Version,
};
use once_cell::sync::OnceCell;

static VALIDATOR_SET: OnceCell<ValidatorSet> = OnceCell::new();

impl ConsensusDB {
    pub fn validator_set(&self) -> ValidatorSet {
        VALIDATOR_SET
            .get_or_init(|| {
                let validator_set_config = GLOBAL_CONFIG_STORAGE
                    .get()
                    .unwrap()
                    .fetch_config_bytes(GravityOnChainConfig::ValidatorSet, 0.into());
                let validator_bytes =
                    TryInto::<Bytes>::try_into(validator_set_config.unwrap()).unwrap();
                let validator_set =
                    ValidatorSet::deserialize_into_config(&validator_bytes).unwrap();
                validator_set
            })
            .clone()
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
                    self.validator_set(),
                );
                info!("genesis is {:?}", genesis);
                Ok(genesis)
            }
        }
    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof, AptosDbError> {
        let mut ledger_infos = vec![];
        if known_version == 0 {
            ledger_infos.push(LedgerInfoWithSignatures::genesis(
                *ACCUMULATOR_PLACEHOLDER_HASH,
                self.validator_set(),
            ));
        }
        ledger_infos.extend(
            self.get_range::<EpochByBlockNumberSchema>(&known_version, &u64::MAX)
                .unwrap()
                .into_iter()
                .map(|(block_number, _)| {
                    info!("get_state_proof block_number: {:?}", block_number);
                    self.get::<LedgerInfoSchema>(&block_number).unwrap().unwrap()
                }),
        );
        let ledger_info = self.get_latest_ledger_info()?;
        Ok(StateProof::new(ledger_info, EpochChangeProof::new(ledger_infos, false)))
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
                        bcs::to_bytes(&self.validator_set())?
                    } else if path.contains("consensus") {
                        let mut consensus_conf = OnChainConsensusConfig::default();
                        // todo(gravity_byteyue): currently we set quorum_store_enabled=false
                        match &mut consensus_conf {
                            OnChainConsensusConfig::V1(_) => {}
                            OnChainConsensusConfig::V2(_) => {}
                            OnChainConsensusConfig::V3 { alg, vtxn } => {}
                            OnChainConsensusConfig::V4 { alg, vtxn, window_size } => match alg {
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
                                // ConsensusAlgorithmConfig::DAG(_) => {}
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
