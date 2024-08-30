// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    namespaced::NAMESPACE_SEPARATOR, CryptoStorage, Error, GetResponse, KVStorage,
    PublicKeyResponse,
};
use aptos_crypto::{
    ed25519::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature},
    hash::CryptoHash,
};
use aptos_infallible::RwLock;
use aptos_time_service::{TimeService, TimeServiceTrait};
use aptos_vault_client::Client;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

const TRANSIT_NAMESPACE_SEPARATOR: &str = "__";

/// VaultStorage utilizes Vault for maintaining encrypted, authenticated data. This
/// version currently matches the behavior of OnDiskStorage and InMemoryStorage. In the future,
/// Vault will be able to create keys, sign messages, and handle permissions across different
/// services. The specific vault service leveraged herein is called KV (Key Value) Secrets Engine -
/// Version 2 (<https://www.vaultproject.io/api/secret/kv/kv-v2.html>). So while Secure Storage
/// calls pointers to data keys, Vault has actually a secret that contains multiple key value
/// pairs.
pub struct VaultStorage {
    client: Client,
    time_service: TimeService,
    renew_ttl_secs: Option<u32>,
    next_renewal: AtomicU64,
    use_cas: bool,
    secret_versions: RwLock<HashMap<String, u32>>,
}

impl VaultStorage {
    pub fn new(
        host: String,
        token: String,
        certificate: Option<String>,
        renew_ttl_secs: Option<u32>,
        use_cas: bool,
        connection_timeout_ms: Option<u64>,
        response_timeout_ms: Option<u64>,
    ) -> Self {
        todo!()
    }

    // Made into an accessor so we can get auto-renewal
    fn client(&self) -> &Client {
        todo!()
    }

    #[cfg(any(test, feature = "testing"))]
    fn reset_kv(&self, path: &str) -> Result<(), Error> {
        todo!()
    }

    #[cfg(any(test, feature = "testing"))]
    fn reset_crypto(&self) -> Result<(), Error> {
        todo!()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn revoke_token_self(&self) -> Result<(), Error> {
        todo!()
    }

    fn key_version(&self, name: &str, version: &Ed25519PublicKey) -> Result<u32, Error> {
        todo!()
    }

    fn crypto_name(&self, name: &str) -> String {
        name.replace(NAMESPACE_SEPARATOR, TRANSIT_NAMESPACE_SEPARATOR)
    }

    fn unnamespaced<'a>(&self, name: &'a str) -> &'a str {
        name.rsplit_once(NAMESPACE_SEPARATOR)
            .map(|(_, key)| key)
            .unwrap_or(name)
    }
}

impl KVStorage for VaultStorage {
    fn available(&self) -> Result<(), Error> {
        todo!()
    }

    fn get<T: DeserializeOwned>(&self, key: &str) -> Result<GetResponse<T>, Error> {
        todo!()
    }

    fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), Error> {
        let secret = key;
        todo!()
    }

    #[cfg(any(test, feature = "testing"))]
    fn reset_and_clear(&mut self) -> Result<(), Error> {
        self.secret_versions.write().clear();
        self.reset_kv("")?;
        self.reset_crypto()?;
        Ok(())
    }
}

impl CryptoStorage for VaultStorage {
    fn create_key(&mut self, name: &str) -> Result<Ed25519PublicKey, Error> {
        todo!()
    }

    fn export_private_key(&self, name: &str) -> Result<Ed25519PrivateKey, Error> {
        todo!()
    }

    fn export_private_key_for_version(
        &self,
        name: &str,
        version: Ed25519PublicKey,
    ) -> Result<Ed25519PrivateKey, Error> {
        todo!()
    }

    fn import_private_key(&mut self, name: &str, key: Ed25519PrivateKey) -> Result<(), Error> {
        todo!()
    }

    fn get_public_key(&self, name: &str) -> Result<PublicKeyResponse, Error> {
        todo!()
    }

    fn get_public_key_previous_version(&self, name: &str) -> Result<Ed25519PublicKey, Error> {
        todo!()
    }

    fn rotate_key(&mut self, name: &str) -> Result<Ed25519PublicKey, Error> {
        todo!()
    }

    fn sign<T: CryptoHash + Serialize>(
        &self,
        name: &str,
        message: &T,
    ) -> Result<Ed25519Signature, Error> {
        todo!()
    }

    fn sign_using_version<T: CryptoHash + Serialize>(
        &self,
        name: &str,
        version: Ed25519PublicKey,
        message: &T,
    ) -> Result<Ed25519Signature, Error> {
        todo!()
    }
}

#[cfg(test)]
pub mod policy {
    use super::*;
    use crate::{Capability, Identity, Policy};
    use aptos_vault_client as vault;

    const APTOS_DEFAULT: &str = "aptos_default";

    /// VaultStorage utilizes Vault for maintaining encrypted, authenticated data. This
    /// version currently matches the behavior of OnDiskStorage and InMemoryStorage. In the future,
    /// Vault will be able to create keys, sign messages, and handle permissions across different
    /// services. The specific vault service leveraged herein is called KV (Key Value) Secrets Engine -
    /// Version 2 (https://www.vaultproject.io/api/secret/kv/kv-v2.html). So while Secure Storage
    /// calls pointers to data keys, Vault has actually a secret that contains multiple key value
    /// pairs.
    pub struct VaultPolicy {
        vault: VaultStorage,
        namespace: Option<String>,
    }

    impl VaultPolicy {
        pub fn new(vault: VaultStorage, namespace: Option<String>) -> Self {
            Self { vault, namespace }
        }

        // Made into an accessor so we can get auto-renewal
        fn client(&self) -> &Client {
            self.vault.client()
        }

        fn reset_policies(&self) -> Result<(), Error> {
            todo!();
            Ok(())
        }

        /// Creates a token but uses the namespace for policies
        pub fn create_token(&self, mut policies: Vec<&str>) -> Result<String, Error> {
            todo!()
        }

        /// Create a new policy in Vault, see the explanation for Policy for how the data is
        /// structured. Vault does not distingush a create and update. An update must first read the
        /// existing policy, amend the contents,  and then be applied via this API.
        pub fn set_policy(
            &self,
            policy_name: &str,
            engine: &VaultEngine,
            key: &str,
            capabilities: &[Capability],
        ) -> Result<(), Error> {
            todo!()
        }

        pub fn set_policies(
            &self,
            name: &str,
            engine: &VaultEngine,
            policy: &Policy,
        ) -> Result<(), Error> {
            for perm in &policy.permissions {
                match &perm.id {
                    Identity::User(id) => self.set_policy(id, engine, name, &perm.capabilities)?,
                    Identity::Anyone => {
                        self.set_policy(APTOS_DEFAULT, engine, name, &perm.capabilities)?
                    },
                    Identity::NoOne => (),
                };
            }
            Ok(())
        }

        fn crypto_name(&self, name: &str) -> String {
            self.name(name, &VaultEngine::Transit)
        }

        fn secret_name(&self, name: &str) -> String {
            self.name(name, &VaultEngine::KVSecrets)
        }

        fn name(&self, name: &str, engine: &VaultEngine) -> String {
            if let Some(namespace) = &self.namespace {
                format!("{}{}{}", namespace, engine.ns_seperator(), name)
            } else {
                name.into()
            }
        }
    }

    impl KVStorage for VaultPolicy {
        fn available(&self) -> Result<(), Error> {
            self.vault.available()
        }

        fn get<T: DeserializeOwned>(&self, key: &str) -> Result<GetResponse<T>, Error> {
            let secret = self.secret_name(key);
            self.vault.get(&secret)
        }

        fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), Error> {
            let secret = self.secret_name(key);
            self.vault.set(&secret, value)
        }

        fn reset_and_clear(&mut self) -> Result<(), Error> {
            self.vault.reset_and_clear()?;
            self.reset_policies()
        }
    }

    impl CryptoStorage for VaultPolicy {
        fn create_key(&mut self, name: &str) -> Result<Ed25519PublicKey, Error> {
            let ns_name = self.crypto_name(name);
            self.vault.create_key(&ns_name)
        }

        fn export_private_key(&self, name: &str) -> Result<Ed25519PrivateKey, Error> {
            let name = self.crypto_name(name);
            self.vault.export_private_key(&name)
        }

        fn export_private_key_for_version(
            &self,
            name: &str,
            version: Ed25519PublicKey,
        ) -> Result<Ed25519PrivateKey, Error> {
            let name = self.crypto_name(name);
            self.vault.export_private_key_for_version(&name, version)
        }

        fn import_private_key(&mut self, name: &str, key: Ed25519PrivateKey) -> Result<(), Error> {
            let ns_name = self.crypto_name(name);
            self.vault.import_private_key(&ns_name, key)
        }

        fn get_public_key(&self, name: &str) -> Result<PublicKeyResponse, Error> {
            let name = self.crypto_name(name);
            self.vault.get_public_key(&name)
        }

        fn get_public_key_previous_version(&self, name: &str) -> Result<Ed25519PublicKey, Error> {
            let name = self.crypto_name(name);
            self.vault.get_public_key_previous_version(&name)
        }

        fn rotate_key(&mut self, name: &str) -> Result<Ed25519PublicKey, Error> {
            let ns_name = self.crypto_name(name);
            self.vault.rotate_key(&ns_name)
        }

        fn sign<T: CryptoHash + Serialize>(
            &self,
            name: &str,
            message: &T,
        ) -> Result<Ed25519Signature, Error> {
            let name = self.crypto_name(name);
            self.vault.sign(&name, message)
        }

        fn sign_using_version<T: CryptoHash + Serialize>(
            &self,
            name: &str,
            version: Ed25519PublicKey,
            message: &T,
        ) -> Result<Ed25519Signature, Error> {
            let name = self.crypto_name(name);
            self.vault.sign_using_version(&name, version, message)
        }
    }

    pub enum VaultEngine {
        KVSecrets,
        Transit,
    }

    impl VaultEngine {
        fn to_policy_path(&self) -> &str {
            match self {
                VaultEngine::KVSecrets => "secret/data",
                VaultEngine::Transit => "transit/keys",
            }
        }

        fn ns_seperator(&self) -> &str {
            match self {
                VaultEngine::KVSecrets => NAMESPACE_SEPARATOR,
                VaultEngine::Transit => TRANSIT_NAMESPACE_SEPARATOR,
            }
        }
    }
}
