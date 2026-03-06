use anyhow::{bail, Result};
use serde::Deserialize;
use std::{
    fs,
    net::{IpAddr, ToSocketAddrs},
    path::Path,
};

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

/// Validate that a probe URL is safe to use.
///
/// Rejects non-http/https schemes and blocks requests to loopback,
/// link-local (169.254.0.0/16), and RFC 1918 private addresses to
/// prevent SSRF attacks against cloud metadata endpoints and internal services.
///
/// When the host is a DNS hostname (not an IP literal), all resolved addresses
/// are checked against the blocklist. This prevents bypasses via hostnames like
/// `metadata.google.internal` or DNS rebinding services like `169.254.169.254.nip.io`.
fn validate_probe_url(url_str: &str) -> Result<()> {
    // Manual scheme check — avoids pulling in a full URL-parser dependency
    let (scheme, rest) = url_str
        .split_once("://")
        .ok_or_else(|| anyhow::anyhow!("Probe URL is missing a scheme: '{}'", url_str))?;

    match scheme {
        "http" | "https" => {}
        s => bail!("Probe URL has disallowed scheme '{}' (must be http or https)", s),
    }

    // Extract authority (everything up to the first '/' or end of string)
    let authority = rest.split('/').next().unwrap_or(rest);
    // Strip userinfo component (e.g., "user@169.254.169.254" → "169.254.169.254")
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);
    // Strip port if present (e.g., "10.0.0.1:8080" → "10.0.0.1")
    let host = host_port.rsplit_once(':').map_or(host_port, |(h, _)| h);
    // Strip brackets from IPv6 literals (e.g., "[::1]" → "::1")
    let host = host.trim_start_matches('[').trim_end_matches(']');

    if host.is_empty() {
        bail!("Probe URL has empty host");
    }

    // Collect IPs to check: either the literal IP, or all DNS-resolved addresses
    let ips_to_check: Vec<IpAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![ip]
    } else {
        // DNS hostname — resolve and check all resulting addresses.
        // Use default port 80 for resolution (port value doesn't affect IP lookup).
        let socket_addrs: Vec<_> = (host, 80u16)
            .to_socket_addrs()
            .map(|iter| iter.collect())
            .unwrap_or_default();
        if socket_addrs.is_empty() {
            bail!(
                "Probe URL host '{}' could not be resolved — refusing unresolvable hostnames",
                host
            );
        }
        socket_addrs.into_iter().map(|sa| sa.ip()).collect()
    };

    for ip in &ips_to_check {
        if ip.is_loopback() {
            bail!("Probe URL host '{}' resolves to loopback address {}", host, ip);
        }
        if is_link_local(*ip) {
            bail!(
                "Probe URL host '{}' resolves to link-local address {} (169.254.0.0/16)",
                host,
                ip
            );
        }
        if is_rfc1918(*ip) {
            bail!(
                "Probe URL host '{}' resolves to private RFC 1918 address {}",
                host,
                ip
            );
        }
    }

    Ok(())
}

fn is_link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 169 && o[1] == 254
        }
        IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
    }
}

fn is_rfc1918(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            // 10.0.0.0/8
            o[0] == 10
            // 172.16.0.0/12
            || (o[0] == 172 && (16..=31).contains(&o[1]))
            // 192.168.0.0/16
            || (o[0] == 192 && o[1] == 168)
        }
        IpAddr::V6(_) => false,
    }
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        if let Some(probe) = &config.probe {
            validate_probe_url(&probe.url)?;
        }
        Ok(config)
    }
}
