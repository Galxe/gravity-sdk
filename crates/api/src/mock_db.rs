use aptos_logger::info;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
use aptos_crypto::{bls12381, hash::HashValue};
use aptos_infallible::Mutex;
use aptos_storage_interface::{AptosDbError, DbReader, DbWriter};
use aptos_types::account_address::AccountAddress;
use aptos_types::aggregate_signature::AggregateSignature;
use aptos_types::block_info::BlockInfo;
use aptos_types::contract_event::EventWithVersion;
use aptos_types::epoch_change::EpochChangeProof;
use aptos_types::epoch_state::EpochState;
use aptos_types::ledger_info::{LedgerInfo, LedgerInfoWithSignatures};
use aptos_types::on_chain_config::ValidatorSet;
use aptos_types::on_chain_config::{ConsensusAlgorithmConfig, ProposerElectionType};
use aptos_types::proof::accumulator::InMemoryTransactionAccumulator;
use aptos_types::proof::TransactionAccumulatorSummary;
use aptos_types::state_proof::StateProof;
use aptos_types::state_store::state_key::inner::StateKeyInner;
use aptos_types::validator_config::ValidatorConfig;
use aptos_types::validator_info::ValidatorInfo;
use aptos_types::validator_verifier::{ValidatorConsensusInfo, ValidatorVerifier};
use aptos_types::{
    on_chain_config::{ConfigurationResource, OnChainConsensusConfig},
    state_store::{state_key::StateKey, state_value::StateValue},
    transaction::Version,
};

pub type Result<T, E = AptosDbError> = std::result::Result<T, E>;
pub struct MockStorage {
    pub node_config_set: GravityNodeConfigSet,
    inner: Inner,
}

pub struct Inner(Arc<Mutex<HashMap<String, String>>>);

impl Default for Inner {
    fn default() -> Self {
        Self(Arc::from(Mutex::new(HashMap::default())))
    }
}

pub type GravityNodeConfigSet = BTreeMap<String, GravityNodeConfig>;

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GravityNodeConfig {
    pub consensus_private_key: Vec<u8>,
    pub consensus_public_key: String,
    pub account_address: String,
    pub network_private_key: Vec<u8>,
    pub network_public_key: String,
    pub trusted_peers_map: Vec<String>,
    pub public_ip_address: String,
    pub voting_power: u64,
}

/// Loads a config configuration file
fn load_file(path: &Path) -> GravityNodeConfigSet {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_yaml::from_str(&contents).unwrap()
}

impl MockStorage {
    pub fn new(path: &Path) -> Self {
        let config_set = load_file(path);
        Self { node_config_set: config_set, inner: Inner(Arc::new(Mutex::new(HashMap::new()))) }
    }

    pub fn mock_validators(&self) -> Vec<ValidatorInfo> {
        let mut result = vec![];
        for (i, (addr, node_config)) in self.node_config_set.iter().enumerate() {
            let private_key: bls12381::PrivateKey =
                bls12381::PrivateKey::try_from(node_config.consensus_private_key.as_slice())
                    .unwrap();
            let config = ValidatorConfig::new(
                (&private_key).into(),
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

impl DbReader for MockStorage {
    fn get_read_delegatee(&self) -> &dyn DbReader {
        self
    }

    fn get_latest_ledger_info(&self) -> Result<LedgerInfoWithSignatures> {
        let genesis = LedgerInfoWithSignatures::genesis(
            *ACCUMULATOR_PLACEHOLDER_HASH,
            ValidatorSet::new(self.mock_validators()),
        );
        println!("genesis is {:?}", genesis);
        Ok(genesis)
    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof> {
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
    ) -> Result<Option<StateValue>> {
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
                            OnChainConsensusConfig::V3 { alg, vtxn } => match alg {
                                ConsensusAlgorithmConfig::Jolteon {
                                    main,
                                    quorum_store_enabled,
                                } => {
                                    main.proposer_election_type = ProposerElectionType::FixedProposer(1);
                                    *quorum_store_enabled = false;
                                }
                                ConsensusAlgorithmConfig::DAG(_) => {}
                                ConsensusAlgorithmConfig::JolteonV2 {
                                    main,
                                    quorum_store_enabled,
                                    order_vote_enabled,
                                } => {
                                    main.proposer_election_type = ProposerElectionType::FixedProposer(1);
                                    *quorum_store_enabled = false;
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

impl DbWriter for MockStorage {}
