use alloy_provider::{Provider, ProviderBuilder};
use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use std::{net::TcpListener, path::PathBuf, time::Duration};

use crate::{command::Executable, config::GravityConfig, output::OutputFormat};

#[derive(Debug, Parser)]
pub struct DoctorCommand {
    /// RPC URL for gravity node (overrides config)
    #[clap(long, env = "GRAVITY_RPC_URL")]
    pub rpc_url: Option<String>,

    /// Consensus server URL for DKG/consensus queries (overrides config)
    #[clap(long, env = "GRAVITY_SERVER_URL")]
    pub server_url: Option<String>,

    /// Deployment path (overrides config)
    #[clap(long, env = "GRAVITY_DEPLOY_PATH")]
    pub deploy_path: Option<String>,

    /// Skip port-conflict checks (TCP bind probes)
    #[clap(long)]
    pub skip_ports: bool,

    /// Output format
    #[clap(skip)]
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Serialize)]
struct CheckResult {
    name: String,
    #[serde(flatten)]
    status_obj: StatusPayload,
}

#[derive(Debug, Clone, Serialize)]
struct StatusPayload {
    status: Status,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

impl CheckResult {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status_obj: StatusPayload { status: Status::Ok, message: message.into(), hint: None },
        }
    }
    fn warn(name: impl Into<String>, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status_obj: StatusPayload {
                status: Status::Warn,
                message: message.into(),
                hint: Some(hint.into()),
            },
        }
    }
    fn fail(name: impl Into<String>, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status_obj: StatusPayload {
                status: Status::Fail,
                message: message.into(),
                hint: Some(hint.into()),
            },
        }
    }
    fn skip(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status_obj: StatusPayload { status: Status::Skip, message: message.into(), hint: None },
        }
    }
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    checks: Vec<CheckResult>,
    summary: Summary,
}

#[derive(Debug, Serialize)]
struct Summary {
    total: usize,
    ok: usize,
    warn: usize,
    fail: usize,
    skip: usize,
}

impl Summary {
    fn from(checks: &[CheckResult]) -> Self {
        let mut s = Self { total: checks.len(), ok: 0, warn: 0, fail: 0, skip: 0 };
        for c in checks {
            match c.status_obj.status {
                Status::Ok => s.ok += 1,
                Status::Warn => s.warn += 1,
                Status::Fail => s.fail += 1,
                Status::Skip => s.skip += 1,
            }
        }
        s
    }
}

/// Resolved inputs for doctor checks. The rpc_url/server_url/deploy_path fields
/// are populated by apply_config_defaults (CLI flag > env > profile) before
/// execute() is called; here we additionally reload the config file so the
/// config check itself can report its file path and active profile name.
struct Resolved {
    config_loaded: Result<Option<GravityConfig>, anyhow::Error>,
    rpc_url: Option<String>,
    server_url: Option<String>,
    deploy_path: Option<String>,
}

impl DoctorCommand {
    fn resolve(&self) -> Resolved {
        Resolved {
            config_loaded: GravityConfig::load(),
            rpc_url: self.rpc_url.clone(),
            server_url: self.server_url.clone(),
            deploy_path: self.deploy_path.clone(),
        }
    }
}

impl Executable for DoctorCommand {
    fn execute(self) -> Result<(), anyhow::Error> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.execute_async())
    }
}

impl DoctorCommand {
    async fn execute_async(self) -> Result<(), anyhow::Error> {
        let resolved = self.resolve();
        let mut checks = Vec::new();

        checks.push(check_config(&resolved));
        checks.push(check_rpc(&resolved).await);
        checks.push(check_server(&resolved).await);
        checks.push(check_deploy_path(&resolved));
        checks.push(check_version(&resolved).await);
        if self.skip_ports {
            checks.push(CheckResult::skip("ports", "--skip-ports set"));
        } else {
            checks.extend(check_ports(&resolved));
        }

        let summary = Summary::from(&checks);
        let report = DoctorReport { checks, summary };

        match self.output_format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            OutputFormat::Plain => {
                print_plain(&report);
            }
        }

        if report.summary.fail > 0 {
            std::process::exit(1);
        }
        Ok(())
    }
}

fn print_plain(report: &DoctorReport) {
    println!("{}", "=== gravity-cli doctor ===".bold());
    println!();
    for check in &report.checks {
        let (icon, name_styled) = match check.status_obj.status {
            Status::Ok => ("[✓]".green().bold().to_string(), check.name.green().to_string()),
            Status::Warn => ("[!]".yellow().bold().to_string(), check.name.yellow().to_string()),
            Status::Fail => ("[✗]".red().bold().to_string(), check.name.red().to_string()),
            Status::Skip => ("[-]".dimmed().to_string(), check.name.dimmed().to_string()),
        };
        println!("{icon} {name_styled}: {}", check.status_obj.message);
        if let Some(hint) = &check.status_obj.hint {
            println!("    {} {hint}", "hint:".cyan());
        }
    }
    println!();
    let s = &report.summary;
    println!(
        "{} {} total  |  {} ok  |  {} warn  |  {} fail  |  {} skipped",
        "summary:".bold(),
        s.total,
        s.ok.to_string().green(),
        s.warn.to_string().yellow(),
        s.fail.to_string().red(),
        s.skip.to_string().dimmed()
    );
}

fn check_config(r: &Resolved) -> CheckResult {
    match &r.config_loaded {
        Ok(Some(cfg)) => {
            let path = GravityConfig::config_path();
            let active = &cfg.active_profile;
            if cfg.profiles.contains_key(active) {
                CheckResult::ok(
                    "config",
                    format!("loaded {} (active profile: {active})", path.display()),
                )
            } else {
                CheckResult::fail(
                    "config",
                    format!("active_profile \"{active}\" not found in profiles"),
                    "Edit ~/.gravity/config.toml to point active_profile at an existing profile, or run `gravity-cli init`",
                )
            }
        }
        Ok(None) => CheckResult::warn(
            "config",
            "no ~/.gravity/config.toml — relying on flags/env vars only",
            "Run `gravity-cli init` to create a config file",
        ),
        Err(e) => CheckResult::fail(
            "config",
            format!("failed to load config: {e}"),
            "Check ~/.gravity/config.toml syntax or run `gravity-cli init` to regenerate",
        ),
    }
}

async fn check_rpc(r: &Resolved) -> CheckResult {
    let Some(url) = r.rpc_url.as_deref() else {
        return CheckResult::skip("rpc", "no --rpc-url / GRAVITY_RPC_URL / profile rpc_url set");
    };
    let parsed = match url.parse() {
        Ok(u) => u,
        Err(e) => {
            return CheckResult::fail(
                "rpc",
                format!("invalid RPC URL \"{url}\": {e}"),
                "Use an http(s):// URL",
            );
        }
    };
    let provider = ProviderBuilder::new().connect_http(parsed);

    let chain_id_fut = tokio::time::timeout(Duration::from_secs(3), provider.get_chain_id());
    let chain_id = match chain_id_fut.await {
        Ok(Ok(id)) => id,
        Ok(Err(e)) => {
            return CheckResult::fail(
                "rpc",
                format!("{url} unreachable: {e}"),
                "Check that the node is running and the URL is correct. Try `gravity-cli localnet start` or `gravity-cli node start`",
            );
        }
        Err(_) => {
            return CheckResult::fail(
                "rpc",
                format!("{url} timed out after 3s"),
                "Node may be starting or firewalled. Verify with `curl -X POST {url}`",
            );
        }
    };

    let block =
        match tokio::time::timeout(Duration::from_secs(3), provider.get_block_number()).await {
            Ok(Ok(n)) => n,
            _ => {
                return CheckResult::warn(
                    "rpc",
                    format!("{url} reachable (chain_id={chain_id}) but eth_blockNumber failed"),
                    "Node may still be syncing",
                );
            }
        };

    if block == 0 {
        CheckResult::warn(
            "rpc",
            format!("{url} reachable (chain_id={chain_id}, block=0)"),
            "Node is up but hasn't produced blocks yet — normal if just started",
        )
    } else {
        CheckResult::ok("rpc", format!("{url} reachable (chain_id={chain_id}, block={block})"))
    }
}

async fn check_server(r: &Resolved) -> CheckResult {
    let Some(url) = r.server_url.as_deref() else {
        return CheckResult::skip(
            "consensus-server",
            "no --server-url / GRAVITY_SERVER_URL / profile server_url set",
        );
    };
    let trimmed = url.trim_end_matches('/');
    let base = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return CheckResult::fail(
                "consensus-server",
                format!("failed to build HTTP client: {e}"),
                "Report this as a CLI bug",
            );
        }
    };
    let endpoint = format!("{base}/dkg/status");
    match client.get(&endpoint).send().await {
        Ok(resp) if resp.status().is_success() => {
            CheckResult::ok("consensus-server", format!("{base} reachable (/dkg/status 200)"))
        }
        Ok(resp) => CheckResult::warn(
            "consensus-server",
            format!("{base} returned HTTP {}", resp.status()),
            "Consensus HTTP API may be debug-only; in release builds it is not exposed",
        ),
        Err(e) => CheckResult::fail(
            "consensus-server",
            format!("{base} unreachable: {e}"),
            "Check --server-url points at the consensus node's HTTP endpoint",
        ),
    }
}

fn check_deploy_path(r: &Resolved) -> CheckResult {
    let Some(path_str) = r.deploy_path.as_deref() else {
        return CheckResult::skip(
            "deploy-path",
            "no --deploy-path / GRAVITY_DEPLOY_PATH / profile deploy_path set",
        );
    };
    let base = PathBuf::from(path_str);
    let start_sh = base.join("script").join("start.sh");
    let stop_sh = base.join("script").join("stop.sh");

    if !base.exists() {
        return CheckResult::fail(
            "deploy-path",
            format!("{} does not exist", base.display()),
            "Run `gravity-cli localnet start` or `cluster/deploy.sh` to create a deployment",
        );
    }
    if !start_sh.exists() {
        return CheckResult::fail(
            "deploy-path",
            format!("{} missing", start_sh.display()),
            "Deployment looks incomplete — rerun deploy.sh",
        );
    }
    if !stop_sh.exists() {
        return CheckResult::warn(
            "deploy-path",
            format!("{} exists but {} is missing", start_sh.display(), stop_sh.display()),
            "`gravity-cli node stop` will not work until stop.sh is present",
        );
    }
    CheckResult::ok(
        "deploy-path",
        format!("{} has script/start.sh and script/stop.sh", base.display()),
    )
}

async fn check_version(r: &Resolved) -> CheckResult {
    let cli_version = env!("CARGO_PKG_VERSION");
    let Some(url) = r.rpc_url.as_deref() else {
        return CheckResult::skip(
            "version",
            format!("CLI version {cli_version}; no RPC to compare against"),
        );
    };
    let parsed = match url.parse() {
        Ok(u) => u,
        Err(_) => {
            return CheckResult::skip(
                "version",
                format!("CLI version {cli_version}; RPC URL unparseable, skipping node version"),
            );
        }
    };
    let provider = ProviderBuilder::new().connect_http(parsed);
    match tokio::time::timeout(Duration::from_secs(3), provider.get_client_version()).await {
        Ok(Ok(node_version)) => {
            CheckResult::ok("version", format!("CLI {cli_version}  |  node {node_version}"))
        }
        _ => CheckResult::warn(
            "version",
            format!("CLI {cli_version}; could not fetch web3_clientVersion from node"),
            "Node may not be running or may not expose the web3 RPC namespace",
        ),
    }
}

fn check_ports(r: &Resolved) -> Vec<CheckResult> {
    // Probe a few common ports. A port is "ok" if either we can bind (free) or
    // a node is listening on our own configured RPC URL (expected to be taken).
    let expected_rpc_port = r.rpc_url.as_deref().and_then(extract_port);
    let candidates: &[(u16, &str)] =
        &[(8545, "RPC (default)"), (8551, "engine authrpc"), (9101, "metrics")];

    candidates.iter().map(|(port, label)| probe_port(*port, label, expected_rpc_port)).collect()
}

fn probe_port(port: u16, label: &str, expected_rpc_port: Option<u16>) -> CheckResult {
    let name = format!("port-{port}");
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => CheckResult::ok(name, format!("{port} ({label}) is free")),
        Err(_) => {
            if Some(port) == expected_rpc_port {
                CheckResult::ok(name, format!("{port} ({label}) bound — this is our RPC endpoint"))
            } else {
                CheckResult::warn(
                    name,
                    format!("{port} ({label}) is already bound"),
                    "Another process may conflict if you start a node on default ports",
                )
            }
        }
    }
}

fn extract_port(url: &str) -> Option<u16> {
    url.parse::<reqwest::Url>().ok().and_then(|u| u.port_or_known_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_port_handles_http_and_default() {
        assert_eq!(extract_port("http://localhost:8545"), Some(8545));
        assert_eq!(extract_port("http://localhost"), Some(80));
        assert_eq!(extract_port("https://example.com"), Some(443));
        assert_eq!(extract_port("not-a-url"), None);
    }

    #[test]
    fn summary_counts_correctly() {
        let checks = vec![
            CheckResult::ok("a", "m"),
            CheckResult::warn("b", "m", "h"),
            CheckResult::fail("c", "m", "h"),
            CheckResult::fail("d", "m", "h"),
            CheckResult::skip("e", "m"),
        ];
        let s = Summary::from(&checks);
        assert_eq!(s.total, 5);
        assert_eq!(s.ok, 1);
        assert_eq!(s.warn, 1);
        assert_eq!(s.fail, 2);
        assert_eq!(s.skip, 1);
    }

    #[test]
    fn check_deploy_path_missing_dir() {
        let r = Resolved {
            config_loaded: Ok(None),
            rpc_url: None,
            server_url: None,
            deploy_path: Some("/tmp/gravity-doctor-test-does-not-exist-xyz".to_string()),
        };
        let res = check_deploy_path(&r);
        assert_eq!(res.status_obj.status, Status::Fail);
    }

    #[test]
    fn check_deploy_path_skipped_when_none() {
        let r = Resolved {
            config_loaded: Ok(None),
            rpc_url: None,
            server_url: None,
            deploy_path: None,
        };
        let res = check_deploy_path(&r);
        assert_eq!(res.status_obj.status, Status::Skip);
    }

    #[test]
    fn check_config_fail_when_active_profile_missing() {
        // Build a GravityConfig with an active_profile that isn't in profiles.
        let mut cfg =
            GravityConfig { active_profile: "ghost".to_string(), profiles: Default::default() };
        cfg.profiles.insert("other".to_string(), Default::default());
        let r = Resolved {
            config_loaded: Ok(Some(cfg)),
            rpc_url: None,
            server_url: None,
            deploy_path: None,
        };
        let res = check_config(&r);
        assert_eq!(res.status_obj.status, Status::Fail);
    }

    #[test]
    fn deploy_path_ok_when_both_scripts_present() {
        let tmp = tempdir_with_scripts();
        let r = Resolved {
            config_loaded: Ok(None),
            rpc_url: None,
            server_url: None,
            deploy_path: Some(tmp.path().to_string_lossy().to_string()),
        };
        let res = check_deploy_path(&r);
        assert_eq!(res.status_obj.status, Status::Ok, "message was: {}", res.status_obj.message);
        // tempdir drops on scope exit
        drop(tmp);
    }

    // Minimal tempdir replacement — avoids adding a new dev-dep for one test.
    struct TempDir {
        path: PathBuf,
    }
    impl TempDir {
        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    fn tempdir_with_scripts() -> TempDir {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!("gravity-doctor-{pid}-{nanos}"));
        let script_dir = base.join("script");
        std::fs::create_dir_all(&script_dir).unwrap();
        std::fs::write(script_dir.join("start.sh"), "#!/bin/bash\n").unwrap();
        std::fs::write(script_dir.join("stop.sh"), "#!/bin/bash\n").unwrap();
        TempDir { path: base }
    }
}
