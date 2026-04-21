//! Google Cloud KMS-backed implementation of [`alloy_signer::Signer`] and
//! [`alloy_network::TxSigner`] for secp256k1 EVM signatures.
//!
//! ## Lifecycle
//!
//! [`GcpKmsSigner::new`] does one network call to Cloud KMS at construction
//! time to fetch the key version's public key. From the public key it derives
//! the EVM address (`keccak256(uncompressed_pub[1..])[12..]`). After that,
//! `address()` and `chain_id()` are cheap accessors.
//!
//! Each `sign_hash` call does:
//!
//! 1. KMS `AsymmetricSign` on the 32-byte digest.
//! 2. Parse the DER `(r, s)` returned by KMS.
//! 3. Normalize `s` to the EIP-2 low-S half so verifying code that rejects high-S signatures (which
//!    Ethereum does) accepts the result.
//! 4. Try `v ∈ {0, 1}` for the recovery id; pick the one that recovers to the same public key we
//!    cached at construction.
//!
//! ## Authentication
//!
//! Authentication uses Application Default Credentials. On a GCE VM with a
//! bound service account, this picks up the metadata-server token with no
//! extra config. Locally it honours `GOOGLE_APPLICATION_CREDENTIALS`.

use alloy_consensus::SignableTransaction;
use alloy_network::TxSigner;
use alloy_primitives::{Address, ChainId, Signature, B256, U256};
use alloy_signer::{
    k256::ecdsa::{RecoveryId, Signature as K256Signature, VerifyingKey},
    Result as SignerResult, Signer,
};
use async_trait::async_trait;
use google_cloud_kms::{
    client::{Client, ClientConfig},
    grpc::kms::v1::{
        digest::Digest as DigestVariant, AsymmetricSignRequest, Digest, GetPublicKeyRequest,
    },
};
use sha3::{Digest as _, Keccak256};

/// EVM signer that delegates raw signing to a Cloud KMS asymmetric key.
///
/// Construct with [`GcpKmsSigner::new`]. The signer is `Clone` only via the
/// underlying [`Client`], which is itself cheap to clone (an `Arc` internally).
#[derive(Clone)]
pub struct GcpKmsSigner {
    client: Client,
    key_resource: String,
    address: Address,
    verifying_key: VerifyingKey,
    chain_id: Option<ChainId>,
}

impl std::fmt::Debug for GcpKmsSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcpKmsSigner")
            .field("key_resource", &self.key_resource)
            .field("address", &self.address)
            .field("chain_id", &self.chain_id)
            .finish()
    }
}

impl GcpKmsSigner {
    /// Connect to Cloud KMS, fetch the key's public key, and cache the
    /// derived EVM address.
    ///
    /// `key_resource` must be the full key-version resource path, e.g.
    /// `projects/<P>/locations/<L>/keyRings/<R>/cryptoKeys/<K>/cryptoKeyVersions/1`.
    /// (`SignerArgs::resolve` normalizes the path before getting here.)
    pub async fn new(key_resource: String) -> anyhow::Result<Self> {
        let config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| anyhow::anyhow!("KMS auth setup failed: {e}"))?;
        let client = Client::new(config)
            .await
            .map_err(|e| anyhow::anyhow!("KMS client connect failed: {e}"))?;

        let resp = client
            .get_public_key(GetPublicKeyRequest { name: key_resource.clone() }, None)
            .await
            .map_err(|e| anyhow::anyhow!("KMS GetPublicKey failed: {e}"))?;

        let verifying_key = parse_secp256k1_pem(&resp.pem)
            .map_err(|e| anyhow::anyhow!("KMS pubkey parse failed: {e}"))?;
        let address = address_from_verifying_key(&verifying_key);

        Ok(Self { client, key_resource, address, verifying_key, chain_id: None })
    }

    /// The EVM address derived from the KMS key's public key at construction.
    pub fn address(&self) -> Address {
        self.address
    }
}

#[async_trait]
impl Signer for GcpKmsSigner {
    async fn sign_hash(&self, hash: &B256) -> SignerResult<Signature> {
        let resp = self
            .client
            .asymmetric_sign(
                AsymmetricSignRequest {
                    name: self.key_resource.clone(),
                    digest: Some(Digest {
                        digest: Some(DigestVariant::Sha256(hash.as_slice().to_vec())),
                    }),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| alloy_signer::Error::other(format!("KMS AsymmetricSign failed: {e}")))?;

        let mut sig = K256Signature::from_der(&resp.signature)
            .map_err(|e| alloy_signer::Error::other(format!("DER parse failed: {e}")))?;

        // EIP-2 low-S form. Ethereum rejects high-S signatures.
        if let Some(normalized) = sig.normalize_s() {
            sig = normalized;
        }

        let parity = recover_parity(hash.as_slice(), &sig, &self.verifying_key).map_err(|e| {
            alloy_signer::Error::other(format!("recovery id resolution failed: {e}"))
        })?;

        let r = U256::from_be_slice(&sig.r().to_bytes());
        let s = U256::from_be_slice(&sig.s().to_bytes());
        Ok(Signature::new(r, s, parity))
    }

    fn address(&self) -> Address {
        self.address
    }

    fn chain_id(&self) -> Option<ChainId> {
        self.chain_id
    }

    fn set_chain_id(&mut self, chain_id: Option<ChainId>) {
        self.chain_id = chain_id;
    }
}

#[async_trait]
impl TxSigner<Signature> for GcpKmsSigner {
    fn address(&self) -> Address {
        self.address
    }

    async fn sign_transaction(
        &self,
        tx: &mut dyn SignableTransaction<Signature>,
    ) -> SignerResult<Signature> {
        let hash = tx.signature_hash();
        self.sign_hash(&hash).await
    }
}

/// Parse a PEM-encoded SubjectPublicKeyInfo holding a secp256k1 point into
/// a `VerifyingKey`. KMS returns this exact format from `GetPublicKey`.
fn parse_secp256k1_pem(pem: &str) -> Result<VerifyingKey, anyhow::Error> {
    use alloy_signer::k256::pkcs8::DecodePublicKey;
    VerifyingKey::from_public_key_pem(pem).map_err(|e| anyhow::anyhow!("PEM/SPKI decode: {e}"))
}

/// EVM address: `keccak256(uncompressed_pubkey_without_0x04_prefix)[12..]`.
fn address_from_verifying_key(vk: &VerifyingKey) -> Address {
    let point = vk.to_encoded_point(/* compress = */ false);
    let bytes = point.as_bytes();
    debug_assert_eq!(bytes[0], 0x04, "expected uncompressed sec1 encoding");
    let hash = Keccak256::digest(&bytes[1..]);
    Address::from_slice(&hash[12..])
}

/// Try `v=0` and `v=1`; pick the one whose recovered key matches `expected`.
/// Returns `true` for the y-odd parity (Ethereum's V=28 / yParity=1) and
/// `false` for y-even (V=27 / yParity=0).
fn recover_parity(
    digest: &[u8],
    sig: &K256Signature,
    expected: &VerifyingKey,
) -> Result<bool, anyhow::Error> {
    for v in 0u8..=1 {
        let rid =
            RecoveryId::try_from(v).map_err(|e| anyhow::anyhow!("invalid recovery id {v}: {e}"))?;
        if let Ok(recovered) = VerifyingKey::recover_from_prehash(digest, sig, rid) {
            if recovered == *expected {
                return Ok(v == 1);
            }
        }
    }
    Err(anyhow::anyhow!("no recovery id matched the expected public key"))
}

#[cfg(test)]
mod tests {
    use super::address_from_verifying_key;
    use alloy_primitives::address;
    use alloy_signer::k256::ecdsa::{SigningKey, VerifyingKey};

    /// Confirm address derivation matches the well-known Ethereum test vector
    /// for the all-ones private key.
    #[test]
    fn address_derivation_matches_known_vector() {
        // Anvil account #0 — privkey 0xac09…ff80 → 0xf39F…2266
        let priv_hex = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let bytes = hex::decode(priv_hex).unwrap();
        let sk = SigningKey::from_slice(&bytes).unwrap();
        let vk = VerifyingKey::from(&sk);
        assert_eq!(
            address_from_verifying_key(&vk),
            address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        );
    }
}
