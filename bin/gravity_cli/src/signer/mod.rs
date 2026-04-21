//! Shared "where does the EVM signing key come from?" plumbing for
//! gravity_cli subcommands that submit on-chain transactions.
//!
//! Two sources are supported:
//!
//! 1. `--kms <resource>` — Cloud KMS (key never leaves the HSM).
//! 2. _(default)_        — interactive `rpassword` prompt on stdin.
//!
//! There is deliberately no "read the hex key from an env var" option: a
//! plaintext private key in an env var is visible in `/proc/<pid>/environ`,
//! can leak into shell history or CI logs, and offers no real security
//! advantage over the stdin prompt while adding attack surface. If you
//! need non-interactive signing, use `--kms`.
//!
//! Add to a subcommand by flattening:
//!
//! ```ignore
//! #[derive(Debug, Parser)]
//! pub struct MyCmd {
//!     // ... other flags ...
//!     #[clap(flatten)]
//!     pub signer: crate::signer::SignerArgs,
//! }
//! ```
//!
//! Then in `execute_async`:
//!
//! ```ignore
//! let resolved = self.signer.resolve().await?;
//! let provider = ProviderBuilder::new()
//!     .wallet(resolved.wallet)
//!     .connect_http(self.rpc_url.parse()?);
//! ```

use alloy_network::EthereumWallet;
use alloy_primitives::Address;
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::PrivateKeySigner;
use clap::Args;

mod kms;
pub use kms::GcpKmsSigner;

/// CLI arguments selecting the signing key source for an on-chain command.
#[derive(Debug, Clone, Args)]
pub struct SignerArgs {
    /// Sign with a key held in Google Cloud KMS instead of prompting for a
    /// plaintext private key on stdin.
    ///
    /// Format: `projects/<P>/locations/<L>/keyRings/<R>/cryptoKeys/<K>/cryptoKeyVersions/<V>`.
    /// The trailing `/cryptoKeyVersions/<V>` may be omitted; in that case
    /// version `1` is used.
    ///
    /// Authentication is via Application Default Credentials. On GCE, this
    /// means the VM's attached service account (no static credentials).
    #[clap(long, value_name = "RESOURCE")]
    pub kms: Option<String>,
}

/// Output of [`SignerArgs::resolve`]: a wallet ready for `ProviderBuilder`,
/// plus the EVM address it signs as.
pub struct ResolvedSigner {
    pub wallet: EthereumWallet,
    pub address: Address,
}

impl SignerArgs {
    /// Construct the signer described by these args.
    ///
    /// `--kms` makes a network call to KMS to fetch the public key (so the
    /// address can be derived). The default stdin path blocks on the prompt.
    pub async fn resolve(&self) -> anyhow::Result<ResolvedSigner> {
        if let Some(resource) = &self.kms {
            let resource = normalize_kms_resource(resource);
            let signer = GcpKmsSigner::new(resource).await?;
            let address = signer.address();
            Ok(ResolvedSigner { wallet: EthereumWallet::from(signer), address })
        } else {
            let raw = rpassword::prompt_password_stdout(
                "Enter private key (hex, with or without 0x prefix): ",
            )
            .map_err(|e| anyhow::anyhow!("failed to read private key: {e}"))?;
            let hex = raw.trim();
            let bytes = hex::decode(hex.trim_start_matches("0x"))
                .map_err(|e| anyhow::anyhow!("invalid private key hex: {e}"))?;
            let signing_key = SigningKey::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("invalid private key: {e}"))?;
            let signer = PrivateKeySigner::from(signing_key);
            let address = signer.address();
            Ok(ResolvedSigner { wallet: EthereumWallet::from(signer), address })
        }
    }
}

fn normalize_kms_resource(s: &str) -> String {
    let trimmed = s.trim().trim_end_matches('/');
    if trimmed.contains("/cryptoKeyVersions/") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/cryptoKeyVersions/1")
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_kms_resource;

    #[test]
    fn normalize_appends_default_version() {
        let in_ = "projects/p/locations/us-west1/keyRings/r/cryptoKeys/k";
        assert_eq!(
            normalize_kms_resource(in_),
            "projects/p/locations/us-west1/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1"
        );
    }

    #[test]
    fn normalize_keeps_explicit_version() {
        let in_ = "projects/p/locations/us-west1/keyRings/r/cryptoKeys/k/cryptoKeyVersions/3";
        assert_eq!(normalize_kms_resource(in_), in_);
    }

    #[test]
    fn normalize_strips_trailing_slash() {
        let in_ = "projects/p/locations/us-west1/keyRings/r/cryptoKeys/k/";
        assert_eq!(
            normalize_kms_resource(in_),
            "projects/p/locations/us-west1/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1"
        );
    }
}
