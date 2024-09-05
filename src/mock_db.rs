use std::cell::RefCell;
use std::collections::HashMap;
use std::result;
use std::str::FromStr;
use std::sync::Arc;
use aptos_crypto::bls12381::PublicKey;
use aptos_crypto::{PrivateKey, Uniform};
use lazy_static::lazy_static;

use rand::{thread_rng, Rng};

use aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
use aptos_crypto::{bls12381, hash::HashValue};
use aptos_infallible::Mutex;
use aptos_storage_interface::{AptosDbError, DbReader, DbWriter};
use aptos_types::account_address::AccountAddress;
use aptos_types::contract_event::EventWithVersion;
use aptos_types::epoch_change::EpochChangeProof;
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use aptos_types::network_address::{self, NetworkAddress};
use aptos_types::on_chain_config::ValidatorSet;
use aptos_types::proof::accumulator::InMemoryTransactionAccumulator;
use aptos_types::proof::TransactionAccumulatorSummary;
use aptos_types::state_proof::StateProof;
use aptos_types::state_store::state_key::inner::StateKeyInner;
use aptos_types::validator_config::ValidatorConfig;
use aptos_types::validator_info::ValidatorInfo;
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

lazy_static! {
    static ref VALIDATOR_MAP: HashMap<String, Vec<String>> = {
        let mut validator_map: HashMap<String, Vec<String>> = HashMap::new();
        validator_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            vec!["/ip4/127.0.0.1/tcp/1234".to_string()],
        );
        validator_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            vec!["/ip4/127.0.0.1/tcp/6180".to_string()],
        );
        validator_map
    };
    static ref CONSENSUS_PRI_MAP: HashMap<String, Vec<u8>> = {
        let mut consensus_pri_map: HashMap<String, Vec<u8>> = HashMap::new();
        consensus_pri_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            vec![
                0x8, 0xf4, 0xf1, 0x19, 0x54, 0x7f, 0x55, 0xb8, 0x6f, 0x87, 0x0, 0x96, 0x33, 0x23,
                0x58, 0x5a, 0xfc, 0x91, 0xe3, 0x46, 0xf5, 0x5d, 0x7a, 0x4b, 0xcf, 0xd9, 0x28, 0xf3,
                0x38, 0x68, 0x34, 0xb8,
            ],
        );
        consensus_pri_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            vec![
                0x10, 0x85, 0x37, 0xf2, 0x61, 0xb7, 0x6a, 0xfe, 0x51, 0x16, 0xee, 0x10, 0xbc, 0xce,
                0xb9, 0xcd, 0x11, 0x79, 0xf1, 0xaa, 0xfa, 0xc3, 0x90, 0x5, 0xd4, 0x34, 0x3a, 0x2f,
                0x5b, 0xdd, 0x29, 0x8e,
            ],
        );
        consensus_pri_map
    };
    static ref CONSENSUS_PUB_MAP: HashMap<String, String> = {
        let mut consensus_pub_map: HashMap<String, String> = HashMap::new();
        consensus_pub_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            "851d41932d866f5fabed6673898e15473e6a0adcf5033d2c93816c6b115c85ad3451e0bac61d570d5ed9f23e1e7f77c4".to_string(),
        );
        consensus_pub_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            "958a9e1e0aef70cc5d99f22403c5cf9de35f8d818553f499b6f29a975aa3b70a95fcb45281f04070e516ad7acd9c7c99".to_string(),
        );
        consensus_pub_map
    };
    pub static ref ACCOUNT_ADDRESS_MAP: HashMap<String, AccountAddress> = {
        let mut account_address_map: HashMap<String, AccountAddress> = HashMap::new();
        account_address_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            AccountAddress::from_str(
                "7682dbbb2efe6a3e005b7fa13872e99f21165874b20cb9c8c877f8a0a0c5b779",
            )
            .unwrap(),
        );
        account_address_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            AccountAddress::from_str(
                "2d86b40a1d692c0749a0a0426e2021ee24e2430da0f5bb9c2ae6c586bf3e0a0f",
            )
            .unwrap(),
        );
        account_address_map
    };
    pub static ref NETWORK_MAP: HashMap<String, Vec<u8>> = {
        let mut network_map: HashMap<String, Vec<u8>> = HashMap::new();
        network_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            vec![216, 118, 84, 212, 149, 198, 180, 2, 202, 156, 156, 4, 36, 147, 105, 53, 109, 67, 55, 132, 99, 179, 89, 203, 171, 83, 142, 101, 166, 170, 9, 118],
        );
        network_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            vec![32, 9, 18, 160, 136, 89, 128, 20, 184, 140, 209, 251, 145, 219, 223, 45, 241, 139, 53, 33, 132, 167, 39, 192, 118, 240, 124, 253, 20, 95, 162, 103],
        );
        network_map
    };
    pub static ref NETWORK_PUB_MAP: HashMap<String, String> = {
        let mut network_pub_map: HashMap<String, String> = HashMap::new();
        network_pub_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            "7682dbbb2efe6a3e005b7fa13872e99f21165874b20cb9c8c877f8a0a0c5b779".to_string(),
        );
        network_pub_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            "2d86b40a1d692c0749a0a0426e2021ee24e2430da0f5bb9c2ae6c586bf3e0a0f".to_string(),
        );
        network_pub_map
    };

    pub static ref TRUSTED_PEERS_MAP: HashMap<String, Vec<String>> = {
        let mut trusted_peers_map: HashMap<String, Vec<String>> = HashMap::new();
        trusted_peers_map.insert(
            "/ip4/127.0.0.1/tcp/6180".to_string(),
            vec!["/ip4/127.0.0.1/tcp/2024".to_string()],
        );
        trusted_peers_map.insert(
            "/ip4/127.0.0.1/tcp/2024".to_string(),
            vec!["/ip4/127.0.0.1/tcp/6180".to_string()],
        );
        trusted_peers_map
    };
}

impl MockStorage {
    pub fn new(network_address: String) -> Self {
        Self {
            network_address: network_address,
            inner: Inner(Arc::new(Mutex::new(HashMap::new()))),
        }
    }

    fn mock_validators(&self) -> Vec<ValidatorInfo> {
        let mut result = vec![];
        let address = ACCOUNT_ADDRESS_MAP.get(&self.network_address).unwrap();
        let private_key: bls12381::PrivateKey =
            bls12381::PrivateKey::try_from(CONSENSUS_PRI_MAP.get(&self.network_address).unwrap().as_slice())
                .unwrap();
        let addr: NetworkAddress = VALIDATOR_MAP.get(&self.network_address).unwrap()[0]
            .parse()
            .unwrap();
        let config = ValidatorConfig::new(
            (&private_key).into(),
            bcs::to_bytes(&vec![addr.clone()]).unwrap(),
            bcs::to_bytes(&vec![addr.clone()]).unwrap(),
            0,
        );
        result.push(ValidatorInfo::new(address.clone(), 10, config));
        result
    }
}

impl DbReader for MockStorage {
    fn get_read_delegatee(&self) -> &dyn DbReader {
        &self.inner
    }

    fn get_latest_ledger_info(&self) -> Result<LedgerInfoWithSignatures> {
        Ok(LedgerInfoWithSignatures::genesis(
            *ACCUMULATOR_PLACEHOLDER_HASH,
            ValidatorSet::empty(),
        ))
    }

    fn get_sequence_num(&self, addr: AccountAddress) -> anyhow::Result<u64> {
        let account_string = addr.to_standard_string();
        let mut guard = self.inner.0.lock();
        let seq_string = guard.entry(account_string.clone()).or_insert("0".to_string());
        let res = seq_string.parse::<u64>().unwrap();
        seq_string.clear();
        seq_string.push_str((res + 1).to_string().as_str());
        Ok(res)

    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof> {
        Ok(StateProof::new(
            LedgerInfoWithSignatures::genesis(
                *ACCUMULATOR_PLACEHOLDER_HASH,
                ValidatorSet::new(self.mock_validators()),
            ),
            EpochChangeProof::new(vec![], false),
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
                    if p.path.contains(&('V' as u8)) {
                        bcs::to_bytes(&ValidatorSet::new(self.mock_validators()))?
                    } else {
                        bcs::to_bytes(&ConfigurationResource::default())?
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
