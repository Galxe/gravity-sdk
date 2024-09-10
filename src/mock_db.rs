use aptos_crypto::bls12381::PublicKey;
use aptos_crypto::{PrivateKey, Uniform};
use aptos_types::account_config::resources;
use lazy_static::lazy_static;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::path::Path;
use std::result;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use aptos_config::config::ConsensusConfig;
use aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
use aptos_crypto::{bls12381, hash::HashValue};
use aptos_infallible::Mutex;
use aptos_storage_interface::{AptosDbError, DbReader, DbWriter};
use aptos_types::account_address::AccountAddress;
use aptos_types::aggregate_signature::AggregateSignature;
use aptos_types::block_info::{BlockInfo, Round};
use aptos_types::contract_event::EventWithVersion;
use aptos_types::epoch_change::EpochChangeProof;
use aptos_types::epoch_state::EpochState;
use aptos_types::ledger_info::{LedgerInfo, LedgerInfoWithSignatures};
use aptos_types::network_address::NetworkAddress;
use aptos_types::on_chain_config::ValidatorSet;
use aptos_types::on_chain_config::{ConsensusAlgorithmConfig, ProposerElectionType};
use aptos_types::proof::accumulator::InMemoryTransactionAccumulator;
use aptos_types::proof::TransactionAccumulatorSummary;
use aptos_types::state_proof::StateProof;
use aptos_types::state_store::state_key::inner::StateKeyInner;
use aptos_types::timestamp::Timestamp;
use aptos_types::validator_config::ValidatorConfig;
use aptos_types::validator_info::ValidatorInfo;
use aptos_types::validator_verifier::{ValidatorConsensusInfo, ValidatorVerifier};
use aptos_types::{
    access_path::AccessPath,
    account_config::AccountResource,
    event::{EventHandle, EventKey},
    on_chain_config::{ConfigurationResource, OnChainConfig, OnChainConsensusConfig},
    state_store::{state_key::StateKey, state_value::StateValue},
    transaction::Version,
    PeerId,
};

pub type Result<T, E = AptosDbError> = std::result::Result<T, E>;
pub struct MockStorage {
    network_address: String,
    pub node_config_set: GravityNodeConfigSet,
    inner: Inner,
}

pub struct Inner(Arc<Mutex<HashMap<String, String>>>);

impl Default for Inner {
    fn default() -> Self {
        Self(Arc::from(Mutex::new(HashMap::default())))
    }
}

impl DbReader for Inner {
    fn get_state_value_by_version(
        &self,
        state_key: &StateKey,
        version: Version,
    ) -> Result<Option<StateValue>> {
        let bytes = bcs::to_bytes(&ConfigurationResource::default())?;
        Ok(Some(StateValue::new_legacy(bytes.into())))
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
}

/// Loads a config configuration file
fn load_file(path: &Path) -> GravityNodeConfigSet {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_yaml::from_str(&contents).unwrap()
}

#[cfg(test)]
mod test {
    use aptos_crypto::ed25519::Ed25519PrivateKey;
    use aptos_crypto::ed25519::Ed25519PublicKey;
    use aptos_crypto::test_utils::KeyPair;
    use aptos_crypto::{bls12381, x25519, PrivateKey};

    #[test]
    fn print_key_hex() {
        let current_dir = env!("CARGO_MANIFEST_DIR").to_string() + "/";
        let path = current_dir.clone() + "nodes_config.json";
        let node_config_set = load_file(Path::new(&path));
        node_config_set.iter().for_each(|(addr, config)| {
            let pri_k =
                bls12381::PrivateKey::try_from(config.consensus_private_key.as_slice()).unwrap();
            let pub_k = pri_k.public_key();
            println!(
                "consensus key {}: \n pub: {} pri: {:?}",
                addr,
                pub_k.to_string(),
                hex::encode(pri_k.to_bytes().as_slice()).as_str()
            );

            let pri_k =
                x25519::PrivateKey::try_from(config.network_private_key.as_slice()).unwrap();
            let pub_k = pri_k.public_key();
            println!(
                "network key {}: \n pub: {} pri: {:?}",
                addr,
                pub_k.to_string(),
                hex::encode(pri_k.to_bytes().as_slice()).as_str()
            );
        });
    }

    #[test]
    fn gen_account_private_key() {
        let current_dir = env!("CARGO_MANIFEST_DIR").to_string() + "/";
        let path = current_dir.clone() + "nodes_config.json";
        let node_config_set = load_file(Path::new(&path));
        node_config_set.iter().for_each(|(addr, config)| {
            let mut rng = thread_rng();
            let kp = KeyPair::<Ed25519PrivateKey, Ed25519PublicKey>::generate(&mut rng);
            println!(
                "{} private key {}, public key {}",
                addr,
                hex::encode(kp.private_key.to_bytes().as_slice()).as_str(),
                kp.public_key.to_string()
            )
        });
    }

    use aptos_crypto::{Uniform, ValidCryptoMaterial};
    use aptos_types::account_address::from_identity_public_key;
    use rand::thread_rng;
    use std::os::unix::thread;
    use std::path::Path;

    use super::load_file;

    #[test]
    fn println_consensus_pri_key() {
        for _ in 0..2 {
            let mut rng = thread_rng();
            let private_key = bls12381::PrivateKey::generate(&mut rng);
            println!(
                "consensus private key {:?}, public key {}",
                private_key.to_bytes(),
                private_key.public_key().to_string()
            );
        }
    }

    #[test]
    fn println_network_pri_key() {
        for _ in 0..2 {
            let mut rng = thread_rng();
            let private_key = x25519::PrivateKey::generate(&mut rng);
            println!(
                "network private key {:?}, public key {}",
                private_key.to_bytes(),
                private_key.public_key().to_string()
            );
        }
    }
}

impl MockStorage {
    pub fn new(network_address: String, path: &Path) -> Self {
        let config_set = load_file(path);
        Self {
            network_address: network_address,
            node_config_set: config_set,
            inner: Inner(Arc::new(Mutex::new(HashMap::new()))),
        }
    }

    fn mock_validators(&self) -> Vec<ValidatorInfo> {
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
                10,
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

    // todo(gravity_byteyue): 应该是commit的时候才增加这个sequence number
    fn get_sequence_num(&self, addr: AccountAddress) -> anyhow::Result<u64> {
        let account_string = addr.to_standard_string();
        let mut guard = self.inner.0.lock();
        let seq_string = guard
            .entry(account_string.clone())
            .or_insert("0".to_string());
        println!(
            "addr is {:?}, seq string is {}",
            &account_string, seq_string
        );
        let res = seq_string.parse::<u64>().unwrap();
        seq_string.clear();
        seq_string.push_str((res + 1).to_string().as_str());
        Ok(res)
    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof> {
        let now = SystemTime::now().elapsed().unwrap().as_secs();
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
        let block_info = BlockInfo::new(
            1,
            0,
            HashValue::zero(),
            HashValue::zero(),
            0,
            0,
            Some(epoch_state),
        );
        let ledger_info = LedgerInfo::new(block_info, HashValue::zero());
        Ok(StateProof::new(
            LedgerInfoWithSignatures::genesis(
                *ACCUMULATOR_PLACEHOLDER_HASH,
                ValidatorSet::new(self.mock_validators()),
            ),
            // 传递不能为空
            EpochChangeProof::new(
                vec![LedgerInfoWithSignatures::new(
                    ledger_info,
                    AggregateSignature::empty(),
                )],
                false,
            ),
        ))
    }

    fn get_accumulator_summary(
        &self,
        ledger_version: Version,
    ) -> Result<TransactionAccumulatorSummary> {
        let accumulator = InMemoryTransactionAccumulator::default();
        let accumulator = accumulator.append(&[HashValue::random()]);
        Ok(TransactionAccumulatorSummary::new(accumulator)?)
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
                        // todo(gravity_byteyue): 这里让quorum_store_enabled=false, 保证不会走quorumstorebuilder
                        match &mut consensus_conf {
                            OnChainConsensusConfig::V1(_) => {}
                            OnChainConsensusConfig::V2(_) => {}
                            OnChainConsensusConfig::V3 { alg, vtxn } => match alg {
                                ConsensusAlgorithmConfig::Jolteon {
                                    main,
                                    quorum_store_enabled,
                                } => {
                                    main.proposer_election_type =
                                        ProposerElectionType::FixedProposer(1);
                                    *quorum_store_enabled = false;
                                }
                                ConsensusAlgorithmConfig::DAG(_) => {}
                                ConsensusAlgorithmConfig::JolteonV2 {
                                    main,
                                    quorum_store_enabled,
                                    order_vote_enabled,
                                } => {
                                    main.proposer_election_type =
                                        ProposerElectionType::FixedProposer(1);
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

    fn get_latest_state_checkpoint_version(&self) -> Result<Option<Version>> {
        Ok(Some(0))
    }

    fn get_latest_block_events(&self, num_events: usize) -> Result<Vec<EventWithVersion>> {
        Ok(vec![])
    }
}

impl DbWriter for MockStorage {}
