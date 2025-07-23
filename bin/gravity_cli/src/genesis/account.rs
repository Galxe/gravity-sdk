use clap::Parser;

use k256::{
    ecdsa::{SigningKey, VerifyingKey},
};
use sha3::{Digest, Keccak256};
use rand_core::{RngCore, OsRng};


use std::path::PathBuf;
use tracing::info;
use std::fs;

use crate::{command::Executable, genesis::account};

use serde::Serialize;

#[derive(Debug, Serialize)]
struct Account {
    private_key: String,
    public_key: String,
    address: String,
}

#[derive(Debug, Parser)]
pub struct GenerateAccount {
    /// Output file path
    #[clap(long, value_parser)]
    pub output_file: PathBuf,
}

fn generate_eth_account() -> Result<(String, String, String), anyhow::Error> {
    // 1. Generate private key
    // SigningKey::random(&mut OsRng) uses the OS random generator to create a new private key.
    let signing_key = SigningKey::random(&mut OsRng);
    let private_key_bytes = signing_key.to_bytes();
    let private_key_hex = hex::encode(private_key_bytes);

    // 2. Derive public key from private key
    // verifying_key() derives the VerifyingKey (public key) from the SigningKey (private key).
    let verifying_key = signing_key.verifying_key();
    // to_sec1_bytes() gets the uncompressed public key bytes.
    // Note: Ethereum uses the uncompressed public key (64 bytes, without 0x04 prefix) for hashing.
    let public_key_uncompressed_bytes = verifying_key.to_sec1_bytes();

    // Remove the first byte (0x04) from the uncompressed public key
    // Ethereum address generation usually hashes the public key bytes without the 0x04 prefix.
    let public_key_for_hashing = &public_key_uncompressed_bytes[1..]; // Remove 0x04 prefix

    // 3. Derive account address from public key
    // Hash the public key using Keccak-256
    let mut hasher = Keccak256::new();
    hasher.update(public_key_for_hashing);
    let public_key_hash = hasher.finalize();

    // Take the last 20 bytes of the hash as the address
    // The address is usually the last 20 bytes (least significant bytes) of the hash, prefixed with 0x
    let address_bytes = &public_key_hash[12..]; // Take 20 bytes from the 12th byte
    let account_address = format!("0x{}", hex::encode(address_bytes));

    Ok((private_key_hex, hex::encode(public_key_for_hashing), account_address))
}

impl Executable for GenerateAccount {
    fn execute(self) -> Result<(), anyhow::Error> {
        let (private_key, public_key, address) = generate_eth_account()?;
        let account = Account {
            private_key,
            public_key,
            address,
        };
        let yaml_string = serde_yaml::to_string(&account)?;
        fs::write(self.output_file, yaml_string)?;
        Ok(())
    }
}