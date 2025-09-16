use clap::Parser;
use gaptos::{
    aptos_crypto::hash::{HashValue, ACCUMULATOR_PLACEHOLDER_HASH}, aptos_types::{
        account_address::AccountAddress, ledger_info::LedgerInfoWithSignatures, on_chain_config::ValidatorSet, validator_config::ValidatorConfig, validator_info::ValidatorInfo, waypoint::Waypoint
    }
};
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use bcs;

use crate::command::Executable;

#[derive(Debug, Deserialize)]
struct TestConfig {
    #[serde(rename = "validatorAddresses")]
    validator_addresses: Vec<String>,
    #[serde(rename = "consensusPublicKeys")]
    consensus_public_keys: Vec<String>,
    #[serde(rename = "votingPowers")]
    voting_powers: Vec<String>,
    #[serde(rename = "validatorNetworkAddresses")]
    validator_network_addresses: Vec<String>,
    #[serde(rename = "fullnodeNetworkAddresses")]
    fullnode_network_addresses: Vec<String>,
    #[serde(rename = "aptosAddresses")]
    aptos_addresses: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct GenerateWaypoint {
    /// Input JSON file path
    #[clap(long, value_parser)]
    pub input_file: PathBuf,
    
    /// Output waypoint file path
    #[clap(long, value_parser)]
    pub output_file: PathBuf,
}

impl GenerateWaypoint {
    /// Load test configuration from JSON file
    fn load_test_config(&self) -> Result<TestConfig, anyhow::Error> {
        let content = fs::read_to_string(&self.input_file)?;
        let config: TestConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Generate validator set from test configuration
    fn generate_validator_set(&self, config: &TestConfig) -> Result<ValidatorSet, anyhow::Error> {
        let mut validators = Vec::new();
        
        for i in 0..config.aptos_addresses.len() {
            // Parse account address
            let account_address = AccountAddress::from_hex_literal(&format!("0x{}", config.aptos_addresses[i]))?;

            // Parse consensus public key (BLS12-381)
            let consensus_key_bytes = hex::decode(&config.consensus_public_keys[i])?;
            let consensus_public_key = gaptos::aptos_crypto::bls12381::PublicKey::try_from(&consensus_key_bytes[..])?;

            // Parse voting power
            let voting_power: u64 = config.voting_powers[i].parse()?;
            
            // Create validator config
            let validator_config = ValidatorConfig::new(
                consensus_public_key,
                bcs::to_bytes(&vec![config.validator_network_addresses[i].clone()]).unwrap(),
                bcs::to_bytes(&vec![config.fullnode_network_addresses[i].clone()]).unwrap(),
                i as u64,
            );
            
            // Create validator info
            let validator_info = ValidatorInfo::new(account_address, voting_power, validator_config);
            validators.push(validator_info);
        }
        
        Ok(ValidatorSet::new(validators))
    }

    /// Generate a waypoint from the test configuration
    pub fn generate_waypoint(&self) -> Result<String, anyhow::Error> {
        let config = self.load_test_config()?;
        let validator_set = self.generate_validator_set(&config)?;

        
        // For now, generate a simple waypoint hash
        // In a real implementation, this would use the validator set to create a proper waypoint
        let ledger_info_with_signatures = LedgerInfoWithSignatures::genesis(*ACCUMULATOR_PLACEHOLDER_HASH, validator_set);
        let waypoint_hash = Waypoint::new_epoch_boundary(&ledger_info_with_signatures.ledger_info())?;
        let waypoint_string = format!("{}", waypoint_hash);
        
        Ok(waypoint_string)
    }
}

impl Executable for GenerateWaypoint {
    fn execute(self) -> Result<(), anyhow::Error> {
        println!("--- Generate Waypoint Start ---");
        println!("Reading input file: {:?}", self.input_file);
        
        let waypoint_string = self.generate_waypoint()?;
        println!("Generated waypoint: {}", waypoint_string);
        
        println!("--- Write Output File ---");
        fs::write(&self.output_file, &waypoint_string)?;
        println!("Waypoint written to: {:?}", self.output_file);
        println!("--- Generate Waypoint Success ---");
        
        Ok(())
    }
}
