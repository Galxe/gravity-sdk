use std::cell::RefCell;
use std::collections::HashMap;
use std::result;
use std::str::FromStr;
use std::sync::{Arc};
use aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH;
use aptos_crypto::{bls12381, hash::HashValue};
use aptos_infallible::Mutex;
use aptos_storage_interface::{AptosDbError, DbReader, DbWriter};
use aptos_types::contract_event::EventWithVersion;
use aptos_types::epoch_change::EpochChangeProof;
use aptos_types::ledger_info::LedgerInfoWithSignatures;
use aptos_types::network_address::NetworkAddress;
use aptos_types::on_chain_config::ValidatorSet;
use aptos_types::proof::accumulator::InMemoryTransactionAccumulator;
use aptos_types::proof::TransactionAccumulatorSummary;
use aptos_types::state_proof::StateProof;
use aptos_types::state_store::state_key::inner::StateKeyInner;
use aptos_types::validator_config::ValidatorConfig;
use aptos_types::validator_info::ValidatorInfo;
use aptos_types::account_address::AccountAddress;
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

impl MockStorage {
    pub fn new() -> Self {
        Self {
            inner: Inner::default(),
        }
    }
}

fn mock_validators() -> Vec<ValidatorInfo> {
    let mut result = vec![];
    let address  = AccountAddress::random();
    let private_key : bls12381::PrivateKey = (1..33u8).collect::<Vec<u8>>().as_slice().try_into().unwrap();
    let addr = NetworkAddress::mock();
    let config = ValidatorConfig::new((&private_key).into(), bcs::to_bytes(&vec![addr.clone()]).unwrap(), bcs::to_bytes(&vec![addr.clone()]).unwrap(), 0);
    result.push(ValidatorInfo::new(address, 10, config));
    result
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
        println!("addr is {:?}, seq string is {}", &account_string, seq_string);
        let res = seq_string.parse::<u64>().unwrap();
        seq_string.clear();
        seq_string.push_str((res + 1).to_string().as_str());
        Ok(res)

    }

    fn get_state_proof(&self, known_version: u64) -> Result<StateProof> {
        Ok(StateProof::new(
            LedgerInfoWithSignatures::genesis(*ACCUMULATOR_PLACEHOLDER_HASH, ValidatorSet::new(mock_validators())),
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
                // 1 => bcs::to_bytes(&ConfigurationResource::default())?,
                // 0 => bcs::to_bytes(&ValidatorSet::empty())?,
                // _ => panic!()
                StateKeyInner::AccessPath(p) => {
                    if p.path.contains(&('V' as u8)) {
                        println!("get mock validators");
                        bcs::to_bytes(&ValidatorSet::new(mock_validators()))?
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
