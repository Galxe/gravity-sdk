use bcs;
use clap::Parser;
use gaptos::{
    aptos_crypto::hash::ACCUMULATOR_PLACEHOLDER_HASH,
    aptos_types::{
        account_address::AccountAddress, ledger_info::LedgerInfoWithSignatures,
        on_chain_config::ValidatorSet, validator_config::ValidatorConfig,
        validator_info::ValidatorInfo, waypoint::Waypoint,
    },
};
use serde::Deserialize;
use std::{fs, path::PathBuf};

use crate::command::Executable;

/// Validator entry matching the InitialValidator struct from genesis-tool
#[derive(Debug, Deserialize)]
struct ValidatorEntry {
    operator: String,
    #[serde(rename = "consensusPubkey")]
    consensus_pubkey: String,
    #[serde(rename = "networkAddresses")]
    network_addresses: String,
    #[serde(rename = "fullnodeAddresses")]
    fullnode_addresses: String,
    #[serde(rename = "votingPower")]
    voting_power: String,
}

/// GenesisConfig matching the format used by genesis-tool
#[derive(Debug, Deserialize)]
struct GenesisConfig {
    validators: Vec<ValidatorEntry>,
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
    /// Load genesis configuration from JSON file
    fn load_genesis_config(&self) -> Result<GenesisConfig, anyhow::Error> {
        let content = fs::read_to_string(&self.input_file)?;
        let config: GenesisConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Generate validator set from genesis configuration
    fn generate_validator_set(
        &self,
        config: &GenesisConfig,
    ) -> Result<ValidatorSet, anyhow::Error> {
        let mut validators = Vec::new();

        for (i, v) in config.validators.iter().enumerate() {
            // Parse consensus public key (BLS12-381), strip 0x prefix if present
            let consensus_key_hex =
                v.consensus_pubkey.strip_prefix("0x").unwrap_or(&v.consensus_pubkey);
            let consensus_key_bytes = hex::decode(consensus_key_hex)?;
            let consensus_public_key =
                gaptos::aptos_crypto::bls12381::PublicKey::try_from(&consensus_key_bytes[..])?;

            // Derive account address from consensus pubkey using SHA3-256
            // This MUST match the derivation in genesis-tool/genesis.rs
            let account_address = {
                use tiny_keccak::{Hasher, Sha3};
                let mut hasher = Sha3::v256();
                hasher.update(&consensus_key_bytes);
                let mut output = [0u8; 32];
                hasher.finalize(&mut output);
                AccountAddress::new(output)
            };

            // Parse voting power as Wei (string like "2000000000000000000")
            // and convert to Ether by dividing by 10^18 to match gravity-reth's wei_to_ether()
            let voting_power_wei: u128 = v.voting_power.parse()?;
            let voting_power: u64 = (voting_power_wei / 1_000_000_000_000_000_000) as u64;

            // Create validator config
            let validator_config = ValidatorConfig::new(
                consensus_public_key,
                bcs::to_bytes(&vec![v.network_addresses.clone()]).unwrap(),
                bcs::to_bytes(&vec![v.fullnode_addresses.clone()]).unwrap(),
                i as u64,
            );

            // Create validator info
            let validator_info =
                ValidatorInfo::new(account_address, voting_power, validator_config, vec![]);
            validators.push(validator_info);
        }

        Ok(ValidatorSet::new(validators))
    }

    /// Generate a waypoint from the genesis configuration
    pub fn generate_waypoint(&self) -> Result<String, anyhow::Error> {
        let config = self.load_genesis_config()?;
        let validator_set = self.generate_validator_set(&config)?;

        // For now, generate a simple waypoint hash
        // In a real implementation, this would use the validator set to create a proper waypoint
        let ledger_info_with_signatures =
            LedgerInfoWithSignatures::genesis(*ACCUMULATOR_PLACEHOLDER_HASH, validator_set);
        let waypoint_hash =
            Waypoint::new_epoch_boundary(ledger_info_with_signatures.ledger_info())?;
        let waypoint_string = format!("{waypoint_hash}");

        Ok(waypoint_string)
    }
}

impl Executable for GenerateWaypoint {
    fn execute(self) -> Result<(), anyhow::Error> {
        println!("--- Generate Waypoint Start ---");
        println!("Reading input file: {:?}", self.input_file);

        let waypoint_string = self.generate_waypoint()?;
        println!("Generated waypoint: {waypoint_string}");

        println!("--- Write Output File ---");
        fs::write(&self.output_file, &waypoint_string)?;
        println!("Waypoint written to: {:?}", self.output_file);
        println!("--- Generate Waypoint Success ---");

        Ok(())
    }
}
