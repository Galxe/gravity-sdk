use anyhow::{anyhow, Context};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Default faucet key — the standard anvil/hardhat test account #0. Matches
/// `cluster/faucet.sh`. Users can override via `--from-key` or the
/// `GRAVITY_LOCALNET_FAUCET_KEY` env var.
pub const DEFAULT_ANVIL_FAUCET_KEY: &str =
    "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

/// Resolve the cluster scripts directory.
///
/// Precedence:
/// 1. Explicit `--cluster-dir` / `GRAVITY_CLUSTER_DIR`
/// 2. Walk up from CWD looking for `cluster/start.sh` (up to 5 parents)
pub fn resolve_cluster_dir(explicit: Option<&str>) -> Result<PathBuf, anyhow::Error> {
    if let Some(dir) = explicit {
        let p = PathBuf::from(dir);
        if !p.join("start.sh").exists() {
            return Err(anyhow!(
                "cluster-dir {} does not contain start.sh",
                p.display()
            ));
        }
        return Ok(p);
    }

    let cwd = std::env::current_dir().context("reading current directory")?;
    let mut current: &Path = &cwd;
    for _ in 0..6 {
        let candidate = current.join("cluster").join("start.sh");
        if candidate.exists() {
            return Ok(current.join("cluster"));
        }
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }
    Err(anyhow!(
        "Could not locate `cluster/start.sh` from {} (walked up 6 levels). \
         Pass --cluster-dir or set GRAVITY_CLUSTER_DIR",
        cwd.display()
    ))
}

/// Resolve the cluster config file path. Defaults to `<cluster_dir>/cluster.toml`.
pub fn resolve_config(cluster_dir: &Path, explicit: Option<&str>) -> PathBuf {
    match explicit {
        Some(p) => PathBuf::from(p),
        None => cluster_dir.join("cluster.toml"),
    }
}

#[derive(Debug, Deserialize)]
pub struct ClusterToml {
    pub cluster: ClusterSection,
    #[serde(default)]
    pub nodes: Vec<NodeEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ClusterSection {
    pub base_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct NodeEntry {
    pub host: String,
    pub rpc_port: u16,
}

/// Parse `cluster.toml` for the fields the CLI needs (base_dir, first node's
/// host/rpc_port). Other TOML keys are ignored.
pub fn load_cluster_toml(path: &Path) -> Result<ClusterToml, anyhow::Error> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading cluster config {}", path.display()))?;
    let parsed: ClusterToml = toml::from_str(&content)
        .with_context(|| format!("parsing cluster config {}", path.display()))?;
    Ok(parsed)
}

/// Derive the default RPC URL for a cluster (first node's host + port).
pub fn derive_rpc_url(cfg: &ClusterToml) -> Result<String, anyhow::Error> {
    let first = cfg.nodes.first().ok_or_else(|| {
        anyhow!("cluster.toml has no [[nodes]] entries — cannot derive RPC URL")
    })?;
    Ok(format!("http://{}:{}", first.host, first.rpc_port))
}
