use anyhow::Result;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub monitoring: MonitoringConfig,
    pub alerting: AlertingConfig,
    pub probe: Option<ProbeConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProbeConfig {
    pub url: String,
    pub check_interval_seconds: u64,
    pub failure_threshold: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    pub check_interval_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MonitoringConfig {
    pub file_patterns: Vec<String>,
    pub recent_file_threshold_seconds: u64,
    pub error_pattern: String,
    pub whitelist_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AlertingConfig {
    pub feishu_webhook: Option<String>,
    pub slack_webhook: Option<String>,
    #[serde(default = "default_min_alert_interval")]
    pub min_alert_interval: u64,
}

fn default_min_alert_interval() -> u64 {
    5
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
