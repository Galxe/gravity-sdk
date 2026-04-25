//! Push raw bytes (an `IdentityBlob` YAML serialization) directly to a GCP
//! Secret Manager resource over HTTPS, without ever materializing them on
//! disk.
//!
//! This is the upload-side counterpart to `gaptos::aptos_config::config::
//! gcp_secret::fetch_secret` (read-side, on the runtime VM). Auth strategy
//! mirrors that module: `GCP_ACCESS_TOKEN` env var first, then GCE metadata
//! server. Operator credentials with `secretmanager.admin` (or finer-grained
//! `secrets.create` + `versions.add`) are required.
//!
//! Used by `genesis generate-key --secret <resource>` so a freshly
//! generated keypair can flow keypair → process RAM → TLS → Secret Manager
//! without an intermediate file.

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use std::time::Duration;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
const SECRET_MANAGER_HOST: &str = "https://secretmanager.googleapis.com";
const TOKEN_ENV: &str = "GCP_ACCESS_TOKEN";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

struct SecretRef {
    project: String,
    name: String,
}

fn parse_resource(resource: &str) -> anyhow::Result<SecretRef> {
    let s = resource.trim().trim_end_matches('/');
    // addVersion is per-secret; ignore any /versions/<V> suffix.
    let s = match s.find("/versions/") {
        Some(idx) => &s[..idx],
        None => s,
    };
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 4 || parts[0] != "projects" || parts[2] != "secrets" {
        bail!("expected projects/<P>/secrets/<S>[/versions/<V>], got '{resource}'");
    }
    Ok(SecretRef { project: parts[1].to_string(), name: parts[3].to_string() })
}

fn access_token() -> anyhow::Result<String> {
    if let Ok(t) = std::env::var(TOKEN_ENV) {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    metadata_token()
}

fn metadata_token() -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("build metadata client")?;
    let resp = client
        .get(METADATA_TOKEN_URL)
        .header("Metadata-Flavor", "Google")
        .send()
        .map_err(|e| {
            anyhow!(
                "GCE metadata server unreachable ({e}) — set {TOKEN_ENV} for non-GCE \
                 environments, e.g. `export {TOKEN_ENV}=$(gcloud auth print-access-token)`"
            )
        })?
        .error_for_status()
        .context("metadata server returned non-2xx")?;
    let body: MetadataToken = resp.json().context("decode metadata token response")?;
    Ok(body.access_token)
}

#[derive(Deserialize)]
struct MetadataToken {
    access_token: String,
}

#[derive(Deserialize)]
struct AddVersionResponse {
    name: String,
}

/// Push `payload` as a new version of the given secret. If the secret
/// container does not yet exist, creates it (automatic replication) and
/// retries. Returns the resulting full version resource path
/// (`projects/<P>/secrets/<S>/versions/<N>`).
pub fn push_secret(resource: &str, payload: &[u8]) -> anyhow::Result<String> {
    let r = parse_resource(resource)?;
    let token = access_token()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("build reqwest client")?;
    let body = serde_json::json!({
        "payload": { "data": base64::encode(payload) }
    });

    match add_version(&client, &token, &r, &body)? {
        AddVersionOutcome::Ok(name) => Ok(name),
        AddVersionOutcome::SecretMissing => {
            create_secret(&client, &token, &r)?;
            match add_version(&client, &token, &r, &body)? {
                AddVersionOutcome::Ok(name) => Ok(name),
                AddVersionOutcome::SecretMissing => bail!(
                    "addVersion still reports secret missing after create — IAM \
                     propagation lag? Retry in a few seconds."
                ),
            }
        }
    }
}

enum AddVersionOutcome {
    Ok(String),
    SecretMissing,
}

fn add_version(
    client: &reqwest::blocking::Client,
    token: &str,
    r: &SecretRef,
    body: &serde_json::Value,
) -> anyhow::Result<AddVersionOutcome> {
    let url =
        format!("{SECRET_MANAGER_HOST}/v1/projects/{}/secrets/{}:addVersion", r.project, r.name);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(body)
        .send()
        .with_context(|| format!("POST {url}"))?;
    let status = resp.status();
    if status.is_success() {
        let v: AddVersionResponse = resp.json().context("decode addVersion response")?;
        return Ok(AddVersionOutcome::Ok(v.name));
    }
    if status.as_u16() == 404 {
        return Ok(AddVersionOutcome::SecretMissing);
    }
    let text = resp.text().unwrap_or_default();
    bail!("addVersion failed ({status}) for projects/{}/secrets/{}: {text}", r.project, r.name);
}

fn create_secret(
    client: &reqwest::blocking::Client,
    token: &str,
    r: &SecretRef,
) -> anyhow::Result<()> {
    let url =
        format!("{SECRET_MANAGER_HOST}/v1/projects/{}/secrets?secretId={}", r.project, r.name);
    let body = serde_json::json!({ "replication": { "automatic": {} } });
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .with_context(|| format!("POST {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().unwrap_or_default();
        bail!(
            "create secret failed ({status}) for projects/{}/secrets/{}: {text}",
            r.project,
            r.name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_resource;

    #[test]
    fn parse_full_resource() {
        let r = parse_resource("projects/p/secrets/s/versions/3").unwrap();
        assert_eq!(r.project, "p");
        assert_eq!(r.name, "s");
    }

    #[test]
    fn parse_short_resource() {
        let r = parse_resource("projects/p/secrets/s").unwrap();
        assert_eq!(r.project, "p");
        assert_eq!(r.name, "s");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_resource("not-a-resource").is_err());
        assert!(parse_resource("projects/p/somethingelse/s").is_err());
    }
}
