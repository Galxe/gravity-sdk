use clap::Parser;
use gaptos::{
    aptos_crypto::{PrivateKey, ValidCryptoMaterial},
    aptos_keygen::KeyGen,
};
use std::{fs, path::PathBuf};

use crate::command::Executable;

use serde::Serialize;

#[derive(Debug, Serialize)]
struct ValidatorIndentity {
    account_address: String,
    account_private_key: String,
    consensus_private_key: String,
    network_private_key: String,
    consensus_public_key: String,
    network_public_key: String,
}

#[derive(Debug, Parser)]
pub struct GenerateKey {
    /// The seed used for key generation, should be a 64 character hex string and only used for
    /// testing
    ///
    /// If a predictable random seed is used, the key that is produced will be insecure and easy
    /// to reproduce.  Please do not use this unless sufficient randomness is put into the random
    /// seed.
    #[clap(long)]
    random_seed: Option<String>,
    /// Output file path
    #[clap(long, value_parser)]
    pub output_file: PathBuf,
}

impl GenerateKey {
    /// Returns a key generator with the seed if given
    pub fn key_generator(&self) -> Result<KeyGen, anyhow::Error> {
        if let Some(ref seed) = self.random_seed {
            // Strip 0x
            let seed = seed.strip_prefix("0x").unwrap_or(seed);
            let mut seed_slice = [0u8; 32];

            hex::decode_to_slice(seed, &mut seed_slice)?;
            Ok(KeyGen::from_seed(seed_slice))
        } else {
            Ok(KeyGen::from_os_rng())
        }
    }
}

// TODO(gravity_lightman): account_private_key is aptos key， not reth
impl Executable for GenerateKey {
    fn execute(self) -> Result<(), anyhow::Error> {
        println!("--- Generate Key Start ---");
        let mut key_gen = self.key_generator()?;
        let network_private_key = key_gen.generate_x25519_private_key()?;
        let consensus_private_key = key_gen.generate_bls12381_private_key();
        println!("The consensus_public_key is {:?}", consensus_private_key.public_key());

        let account_private_key = key_gen.generate_ed25519_private_key();
        let account_address = network_private_key.public_key();
        println!("The account_address is {account_address}");
        println!(
            "The last 20bit account_address is 0x{}",
            hex::encode(&account_address.as_slice()[12..])
        );
        let indentity = ValidatorIndentity {
            account_address: account_address.to_string(),
            account_private_key: hex::encode(account_private_key.to_bytes()),
            consensus_private_key: hex::encode(consensus_private_key.to_bytes()),
            network_private_key: hex::encode(network_private_key.to_bytes()),
            consensus_public_key: hex::encode(consensus_private_key.public_key().to_bytes()),
            network_public_key: hex::encode(network_private_key.public_key().to_bytes()),
        };

        println!("--- Write Output File ---");
        let yaml_string = serde_yaml::to_string(&indentity)?;
        fs::write(self.output_file, yaml_string)?;
        println!("--- Generate Key Success ---");
        Ok(())
    }
}
