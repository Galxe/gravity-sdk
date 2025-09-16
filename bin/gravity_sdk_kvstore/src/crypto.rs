use rand::rngs::OsRng;
use secp256k1::{
    ecdsa::{RecoverableSignature, RecoveryId},
    Message, PublicKey, Secp256k1, SecretKey,
};
use sha3::{Digest, Keccak256};

use crate::txpool::{Transaction, UnsignedTransaction};

#[derive(Debug)]
pub struct KeyPair {
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

pub fn generate_keypair() -> KeyPair {
    let secp = Secp256k1::new();
    let mut rng = OsRng::default();
    let (secret_key, public_key) = secp.generate_keypair(&mut rng);

    KeyPair { secret_key, public_key }
}

pub fn sign_transaction(tx: &UnsignedTransaction, secret_key: &SecretKey) -> String {
    let secp = Secp256k1::new();
    let message = compute_transaction_hash(tx);
    let message = Message::from_slice(&message).unwrap();

    let recoverable_signature = secp.sign_ecdsa_recoverable(&message, secret_key);
    let (recovery_id, rec_sig_bytes) = recoverable_signature.serialize_compact();

    let v = recovery_id.to_i32() as u8 + 27;

    let mut signature_bytes = Vec::with_capacity(65);
    signature_bytes.extend_from_slice(&rec_sig_bytes);
    signature_bytes.push(v);

    hex::encode(signature_bytes)
}

pub fn verify_signature(tx: &Transaction) -> Result<String, String> {
    let secp = Secp256k1::new();
    let message = compute_transaction_hash(&tx.unsigned);
    let message = Message::from_slice(&message).map_err(|e| format!("Invalid message: {}", e))?;

    let signature_bytes =
        hex::decode(&tx.signature).map_err(|e| format!("Invalid signature hex: {}", e))?;

    if signature_bytes.len() != 65 {
        return Err("Invalid signature length".to_string());
    }

    let rs_bytes = &signature_bytes[0..64];
    let signature = RecoverableSignature::from_compact(
        rs_bytes,
        RecoveryId::from_i32((signature_bytes[64] - 27) as i32).unwrap(),
    )
    .map_err(|_| "Invalid recoverable signature".to_string())?;

    let public_key = secp
        .recover_ecdsa(&message, &signature)
        .map_err(|_| "Failed to recover public key".to_string())?;

    let address = public_key_to_address(&public_key);
    Ok(address)
}

pub fn compute_transaction_hash(tx: &UnsignedTransaction) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    let encoded = bincode::serialize(tx).unwrap();
    hasher.update(&encoded);
    hasher.finalize().into()
}

pub fn public_key_to_address(public_key: &PublicKey) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(&public_key.serialize_uncompressed()[1..]);
    let result = hasher.finalize();
    hex::encode(&result[12..])
}

pub fn compute_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
    if transactions.is_empty() {
        return [0; 32];
    }

    let mut hashes: Vec<[u8; 32]> = transactions
        .iter()
        .map(|tx| {
            let mut hasher = Keccak256::new();
            let encoded = bincode::serialize(tx).unwrap();
            hasher.update(&encoded);
            hasher.finalize().into()
        })
        .collect();

    while hashes.len() > 1 {
        if hashes.len() % 2 != 0 {
            hashes.push(hashes.last().unwrap().clone());
        }

        let mut new_hashes = Vec::new();
        for chunk in hashes.chunks(2) {
            let mut hasher = Keccak256::new();
            hasher.update(&chunk[0]);
            hasher.update(&chunk[1]);
            new_hashes.push(hasher.finalize().into());
        }
        hashes = new_hashes;
    }

    hashes[0]
}
