use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct GravityConfig {
    pub active_profile: String,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProfileConfig {
    pub rpc_url: Option<String>,
    pub server_url: Option<String>,
    pub deploy_path: Option<String>,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u128>,
}

impl GravityConfig {
    /// Returns the default config directory: ~/.gravity/
    pub fn config_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".gravity")
    }

    /// Returns the default config file path: ~/.gravity/config.toml
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load config from ~/.gravity/config.toml. Returns Ok(None) if file doesn't exist.
    pub fn load() -> Result<Option<Self>, anyhow::Error> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read config file {}: {e}", path.display()))?;
        let config: GravityConfig = toml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse config file {}: {e}", path.display()))?;
        Ok(Some(config))
    }

    /// Get the active profile config, optionally overridden by a CLI flag.
    pub fn active_profile(&self, override_name: Option<&str>) -> Option<&ProfileConfig> {
        let name = override_name.unwrap_or(&self.active_profile);
        self.profiles.get(name)
    }
}

/// Resolve a required string parameter: CLI flag > config value.
/// clap with `env` feature already handles CLI > env var, so `cli_value` reflects both.
pub fn resolve_required(
    cli_value: Option<String>,
    config_value: Option<&String>,
    field_name: &str,
    env_name: &str,
) -> Result<String, anyhow::Error> {
    cli_value
        .or_else(|| config_value.cloned())
        .ok_or_else(|| {
            anyhow!(
                "--{field_name} is required. Set it via CLI flag, {env_name} env var, or ~/.gravity/config.toml"
            )
        })
}

/// Resolve an optional parameter with a default: CLI flag > config value > default.
pub fn resolve_with_default<T: Clone>(
    cli_value: Option<T>,
    config_value: Option<&T>,
    default: T,
) -> T {
    cli_value.or_else(|| config_value.cloned()).unwrap_or(default)
}
