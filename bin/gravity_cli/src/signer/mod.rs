//! Shared "where does the EVM signing key come from?" plumbing for
//! gravity_cli subcommands that submit on-chain transactions.
//!
//! Three sources are supported, mutually exclusive:
//!
//! 1. `--kms <resource>`        — Cloud KMS (key never leaves the HSM).
//! 2. `--private-key-env <VAR>` — read the hex private key from an env var.
//! 3. _(default)_               — interactive `rpassword` prompt on stdin.
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
    /// Sign with a key held in Google Cloud KMS instead of a local private key.
    ///
    /// Format: `projects/<P>/locations/<L>/keyRings/<R>/cryptoKeys/<K>/cryptoKeyVersions/<V>`.
    /// The trailing `/cryptoKeyVersions/<V>` may be omitted; in that case
    /// version `1` is used.
    ///
    /// Authentication is via Application Default Credentials. On GCE, this
    /// means the VM's attached service account (no static credentials).
    #[clap(long, value_name = "RESOURCE", conflicts_with = "private_key_env")]
    pub kms: Option<String>,

    /// Read the hex private key (with or without `0x` prefix) from this
    /// environment variable instead of prompting on stdin.
    ///
    /// Intended for CI / automation where the env var is injected by a
    /// secrets manager. Prefer `--kms` for interactive / production use:
    /// a plaintext key in an env var is visible to anyone with access to
    /// `/proc/<pid>/environ`, and inline invocations (`GRAV_KEY=… cli …`)
    /// may leak into shell history.
    #[clap(long, value_name = "ENV_VAR")]
    pub private_key_env: Option<String>,
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
            let hex = read_private_key_hex(self.private_key_env.as_deref())?;
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

fn read_private_key_hex(env_var: Option<&str>) -> anyhow::Result<String> {
    let raw = match env_var {
        Some(name) => std::env::var(name).map_err(|_| {
            anyhow::anyhow!("env var `{name}` is not set (referenced by --private-key-env)")
        })?,
        None => rpassword::prompt_password_stdout(
            "Enter private key (hex, with or without 0x prefix): ",
        )
        .map_err(|e| anyhow::anyhow!("failed to read private key: {e}"))?,
    };
    Ok(raw.trim().to_string())
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
