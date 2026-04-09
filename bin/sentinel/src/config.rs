use anyhow::Result;
use serde::Deserialize;
use std::{collections::HashMap, fmt, fs, path::Path};

/// Alert priority levels. P0 is the highest (most critical).
#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Priority {
    #[serde(alias = "p0", alias = "P0")]
    #[default]
    P0,
    #[serde(alias = "p1", alias = "P1")]
    P1,
    #[serde(alias = "p2", alias = "P2")]
    P2,
}


impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::P0 => write!(f, "P0"),
            Priority::P1 => write!(f, "P1"),
            Priority::P2 => write!(f, "P2"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub monitoring: MonitoringConfig,
    pub alerting: AlertingConfig,
    /// Multiple probe endpoints, each with its own URL, interval, and threshold.
    #[serde(default)]
    pub probes: Vec<ProbeConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProbeConfig {
    pub url: String,
    pub tag: Option<String>,
    #[serde(default = "default_probe_interval")]
    pub check_interval_seconds: u64,
    #[serde(default = "default_probe_threshold")]
    pub failure_threshold: u32,
}

fn default_probe_interval() -> u64 {
    30
}

fn default_probe_threshold() -> u32 {
    3
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

/// Per-priority webhook override.
#[derive(Debug, Deserialize, Clone)]
pub struct PriorityAlertConfig {
    pub feishu_webhook: Option<String>,
    pub slack_webhook: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AlertingConfig {
    /// Priority used for errors that don't match any whitelist rules.
    /// Design Intent: Unrecognized error logs (not explicitly handled in the whitelist)
    /// are often newly introduced logs after code updates. Assigning them a configurable
    /// default priority (e.g. P2) prevents P0 alert storms that can overwhelm the ops team.
    #[serde(default)]
    pub default_priority: Priority,
    /// Default Feishu webhook (fallback when priority-specific webhook is not set)
    pub feishu_webhook: Option<String>,
    /// Default Slack webhook (fallback when priority-specific webhook is not set)
    pub slack_webhook: Option<String>,
    #[serde(default = "default_min_alert_interval")]
    pub min_alert_interval: u64,
    /// Per-priority webhook overrides. Key is the priority name (e.g. "p0", "p1", "p2").
    #[serde(default)]
    pub priorities: HashMap<Priority, PriorityAlertConfig>,
}

impl AlertingConfig {
    /// Get the effective webhooks for the given priority.
    /// Falls back to the top-level webhooks if the priority has no specific override.
    pub fn get_webhooks(&self, priority: Priority) -> (Option<&str>, Option<&str>) {
        if let Some(override_cfg) = self.priorities.get(&priority) {
            let feishu = override_cfg.feishu_webhook.as_deref().or(self.feishu_webhook.as_deref());
            let slack = override_cfg.slack_webhook.as_deref().or(self.slack_webhook.as_deref());
            (feishu, slack)
        } else {
            (self.feishu_webhook.as_deref(), self.slack_webhook.as_deref())
        }
    }

    /// Collect all unique webhook URLs across default and per-priority configs.
    pub fn all_webhooks(&self) -> Vec<(&str, &str)> {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        let mut result: Vec<(&str, &str)> = Vec::new();

        let candidates: Vec<(&str, Option<&str>)> = {
            let mut v = vec![
                ("feishu", self.feishu_webhook.as_deref()),
                ("slack", self.slack_webhook.as_deref()),
            ];
            for cfg in self.priorities.values() {
                v.push(("feishu", cfg.feishu_webhook.as_deref()));
                v.push(("slack", cfg.slack_webhook.as_deref()));
            }
            v
        };

        for (label, url) in candidates {
            if let Some(u) = url {
                if !u.is_empty() && seen.insert(u) {
                    result.push((label, u));
                }
            }
        }

        result
    }
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
