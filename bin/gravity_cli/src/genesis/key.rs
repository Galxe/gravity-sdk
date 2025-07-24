use clap::Parser;
use gaptos::{
    api_types::u256_define::AccountAddress,
    aptos_crypto::{ed25519::{self, Ed25519PublicKey}, ValidCryptoMaterial},
    aptos_keygen::KeyGen,
    aptos_types::transaction::authenticator::AuthenticationKey,
};
use std::path::PathBuf;
use tracing::info;
use std::fs;

use crate::command::Executable;

use serde::Serialize;

#[derive(Debug, Serialize)]
struct ValidatorIndentity {
    account_address: String,
    account_private_key: String,
    consensus_private_key: String,
    network_private_key: String,
}

#[derive(Debug, Parser)]
pub struct GenerateKey {
    /// The seed used for key generation, should be a 64 character hex string and only used for testing
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
        info!("--- Generate Key Start ---");
        let mut key_gen = self.key_generator()?;
        let network_private_key = key_gen.generate_x25519_private_key()?;
        info!("The network_private_key is {:?}", network_private_key);
        let consensus_private_key = key_gen.generate_bls12381_private_key();
        info!("The consensus_private_key is {:?}", consensus_private_key);

        let account_private_key = key_gen.generate_ed25519_private_key();
        info!("The account_private_key is {:?}", account_private_key);
        let account_address =
            account_address_from_public_key(&ed25519::Ed25519PublicKey::from(&account_private_key));
        info!("The account_address is {:?}", account_address);
        let indentity = ValidatorIndentity {
            account_address: account_address.to_string(),
            account_private_key: hex::encode(account_private_key.to_bytes()),
            consensus_private_key: hex::encode(consensus_private_key.to_bytes()),
            network_private_key: hex::encode(network_private_key.to_bytes()),
        };

        info!("--- Write Output File ---");
        let yaml_string = serde_yaml::to_string(&indentity)?;
        fs::write(self.output_file, yaml_string)?;
        info!("--- Generate Key Success ---");
        Ok(())
    }
}

pub fn account_address_from_public_key(public_key: &Ed25519PublicKey) -> AccountAddress {
    let auth_key = AuthenticationKey::ed25519(public_key);
    account_address_from_auth_key(&auth_key)
}

pub fn account_address_from_auth_key(auth_key: &AuthenticationKey) -> AccountAddress {
    AccountAddress::new(*auth_key.account_address())
}
