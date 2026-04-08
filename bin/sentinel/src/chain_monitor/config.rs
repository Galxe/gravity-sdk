use crate::config::Priority;
use anyhow::Result;
use serde::Deserialize;

fn default_poll_interval() -> u64 {
    12
}

fn default_confirmation_blocks() -> u64 {
    2
}

fn default_max_consecutive_failures() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

fn default_timeout_seconds() -> u64 {
    1800
}

fn default_timeout_check_interval() -> u64 {
    60
}

fn default_drop_percentage() -> f64 {
    10.0
}

/// Top-level chain monitor configuration, added as optional field to sentinel Config.
#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ChainMonitorConfig {
    /// Ethereum L1 RPC URL (where GBridgeSender + GravityPortal live)
    pub ethereum_rpc_url: String,
    /// Gravity chain RPC URL (where GBridgeReceiver + NativeOracle live)
    pub gravity_rpc_url: String,

    /// Contract addresses
    pub gbridge_sender_address: String,
    pub gravity_portal_address: String,
    pub gbridge_receiver_address: String,
    pub native_oracle_address: String,

    /// Owner address to watch (Ownable2Step owner of bridge contracts)
    pub owner_address: String,

    /// ERC20 G token address on Ethereum (for vault balance tracking)
    pub gtoken_address: String,

    /// Polling interval for eth_getLogs (seconds)
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,

    /// Number of confirmation blocks for reorg safety on Ethereum
    #[serde(default = "default_confirmation_blocks")]
    pub confirmation_blocks: u64,

    /// Fire a connectivity alert after this many consecutive RPC failures
    #[serde(default = "default_max_consecutive_failures")]
    pub max_consecutive_failures: u32,

    /// Optional checkpoint file path for persisting block cursors across restarts
    pub checkpoint_path: Option<String>,

    // Per-rule configs
    #[serde(default)]
    pub large_withdrawal: LargeWithdrawalConfig,
    #[serde(default)]
    pub vault_balance: VaultBalanceConfig,
    #[serde(default)]
    pub bridge_timeout: BridgeTimeoutConfig,
    #[serde(default)]
    pub owner_activity: OwnerActivityConfig,
    #[serde(default)]
    pub timelock: TimelockConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LargeWithdrawalConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Threshold in wei as a decimal string (e.g., "50000000000000000000" for 50 tokens)
    #[serde(default = "default_large_withdrawal_threshold")]
    pub threshold_wei: String,
    #[serde(default)]
    pub priority: Priority,
}

fn default_large_withdrawal_threshold() -> String {
    "50000000000000000000".to_string() // 50 * 10^18
}

impl Default for LargeWithdrawalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_wei: default_large_withdrawal_threshold(),
            priority: Priority::P0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct VaultBalanceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Alert if balance drops by more than this percentage in one poll cycle
    #[serde(default = "default_drop_percentage")]
    pub drop_percentage_threshold: f64,
    /// Or absolute drop threshold in wei
    #[serde(default = "default_large_withdrawal_threshold")]
    pub drop_absolute_threshold_wei: String,
    #[serde(default)]
    pub priority: Priority,
}

impl Default for VaultBalanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            drop_percentage_threshold: default_drop_percentage(),
            drop_absolute_threshold_wei: default_large_withdrawal_threshold(),
            priority: Priority::P0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BridgeTimeoutConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Max time (seconds) between TokensLocked on Ethereum and NativeMinted on Gravity
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    /// How often to scan for timed-out nonces (seconds)
    #[serde(default = "default_timeout_check_interval")]
    pub check_interval_seconds: u64,
    #[serde(default)]
    pub priority: Priority,
}

impl Default for BridgeTimeoutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_seconds: default_timeout_seconds(),
            check_interval_seconds: default_timeout_check_interval(),
            priority: Priority::P0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct OwnerActivityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: Priority,
}

impl Default for OwnerActivityConfig {
    fn default() -> Self {
        Self { enabled: true, priority: Priority::P0 }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct TimelockConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: Priority,
    /// Expected governance/multisig address. If owner calls privileged functions
    /// from a different address, trigger a GOVERNANCE BYPASS alert.
    pub expected_governance_address: Option<String>,
}

impl Default for TimelockConfig {
    fn default() -> Self {
        Self { enabled: true, priority: Priority::P0, expected_governance_address: None }
    }
}

impl ChainMonitorConfig {
    /// Parse an address string into an alloy Address.
    pub fn parse_address(s: &str) -> Result<alloy_primitives::Address> {
        s.parse::<alloy_primitives::Address>()
            .map_err(|e| anyhow::anyhow!("Invalid address '{}': {}", s, e))
    }
}
