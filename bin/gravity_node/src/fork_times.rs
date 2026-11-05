//! Hardfork activation timestamps (Unix seconds) the binary hardcodes for
//! Gravity-owned chains. [`crate::chainspec`] forces them onto the genesis
//! before the chain spec is built.
//!
//! Changing a value for a shipped chain is a consensus change.

// ---------- Gravity mainnet (chain id 127001) ----------
//
// Source: gravity-mainnet-gitops/genesis.json and the coordinated upgrade
// announcements.

/// Gravity mainnet Prague activation: 2026-06-29 13:00:00 Beijing
/// (UTC+08:00), which is 2026-06-29 05:00:00 UTC. Pinned to the deployed
/// mainnet genesis `config.pragueTime`.
pub(crate) const GRAVITY_MAINNET_PRAGUE_TIME: u64 = 1_782_709_200;

/// Gravity mainnet Alpha activation: 2026-07-27 13:00:00 Beijing
/// (UTC+08:00), which is 2026-07-27 05:00:00 UTC.
pub(crate) const GRAVITY_MAINNET_ALPHA_TIME: u64 = 1_785_128_400;

/// Gravity mainnet Beta and Osaka activation: 2026-08-18 02:00:00 UTC.
///
/// Both forks share this value: Beta (Gravity EIP-7702 lockdown release +
/// block-gas last-gate) and Osaka (Ethereum hardfork) activate together.
pub(crate) const GRAVITY_MAINNET_BETA_OSAKA_TIME: u64 = 1_787_018_400;

// ---------- Longevity testnet (chain id 7771625) ----------
//
// Each value is the timestamp of the first block that crossed the fork on the
// live chain; the block numbers were read back from the public testnet RPC on
// 2026-09-30.

/// Longevity testnet Prague activation: 2026-06-08 09:04:02 UTC,
/// block 1084564 (EIP-2935 history contract deployed).
pub(crate) const LONGEVITY_TESTNET_PRAGUE_TIME: u64 = 1_780_909_442;

/// Longevity testnet Alpha activation: 2026-07-06 17:00:00 Beijing
/// (UTC+08:00), block 10787823.
pub(crate) const LONGEVITY_TESTNET_ALPHA_TIME: u64 = 1_783_328_400;

/// Longevity testnet Beta and Osaka activation: 2026-08-04 11:00:00 Beijing
/// (UTC+08:00), which is 2026-08-04 03:00:00 UTC.
///
/// Live genesis value; neither fork has a directly observable state effect.
pub(crate) const LONGEVITY_TESTNET_BETA_OSAKA_TIME: u64 = 1_785_812_400;

/// Longevity testnet Gamma activation: 2026-08-19 11:00:00 Beijing
/// (UTC+08:00), block 25666608 (NativeOracle runtime swapped).
pub(crate) const LONGEVITY_TESTNET_GAMMA_TIME: u64 = 1_787_108_400;

/// Longevity testnet TestnetOwnerFix activation: 2026-09-04 11:00:00 Beijing
/// (UTC+08:00), block 30922944 (StakePool pendingOwner set to the wrong CLI
/// address).
pub(crate) const LONGEVITY_TESTNET_OWNER_FIX_TIME: u64 = 1_788_490_800;

/// Longevity testnet TestnetOwnerFixV2 activation: 2026-09-09 18:00:00 Beijing
/// (UTC+08:00), block 32664162 (StakePool pendingOwner set to the cast-derived
/// address).
pub(crate) const LONGEVITY_TESTNET_OWNER_FIX_V2_TIME: u64 = 1_788_948_000;
