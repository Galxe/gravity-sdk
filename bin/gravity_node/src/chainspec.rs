//! Gravity chain-spec parser with per-chain hardcoded hardfork overrides.
//!
//! When the binary is the authoritative source of hardfork timestamps for a
//! given chain id, the override table is applied to the [`alloy_genesis::Genesis`]
//! value BEFORE `From<Genesis> for ChainSpec` runs in greth. This guarantees:
//!
//! - The constructed [`ChainSpec`] (and its derived `genesis_header`, `fork_id`, `fork_filter`)
//!   reflects the binary's authoritative values from the moment the `Arc<ChainSpec>` is built — no
//!   post-construction mutation, no `Arc::get_mut`, no `OnceLock` timing pain.
//! - The CL side (gaptos `ConsensusHardforks::from_genesis_extra_fields`) reads from the **same**
//!   mutated `extra_fields`, so EL and CL see one consistent view from a single source of truth.
//!
//! For named chains (`mainnet` / `sepolia` / `holesky` / `hoodi` / `dev`)
//! we delegate to upstream [`EthereumChainSpecParser`] unchanged — those
//! cached LazyLock statics are not gravity-owned chains.
//!
//! # Override semantics
//!
//! Each gated fork has one of two states:
//!
//! * `Some(ts)` → write the timestamp into the corresponding genesis field, overriding whatever the
//!   operator supplied.
//! * `None`     → remove the operator-supplied value if present; the fork will not activate.
//!
//! # Bypass risk
//!
//! Override runs inside [`GravityChainSpecParser::parse`], which means any
//! code path that constructs a [`ChainSpec`] for a gravity chain id WITHOUT
//! going through this parser silently skips the override. Concretely this
//! includes:
//!
//! - `greth::reth_chainspec::ChainSpecBuilder` and direct struct-literal construction in tests /
//!   fixtures inside greth.
//! - A future CLI command that bypasses [`crate::cli::Cli`].
//! - Anywhere in this binary that calls `From<Genesis> for ChainSpec` directly on a hand-built
//!   `Genesis` for a gravity chain id.
//!
//! Today the only production funnel for gravity mainnet
//! (`--chain $GENESIS_PATH`) goes through this parser, so the practical
//! risk is zero. The tripwire test
//! [`tests::chainspec_builder_mainnet_does_not_match_gravity_chain_id`]
//! guards the most likely accidental case.
//!
//! # Invariant: no runtime inputs
//!
//! This parser reads ONLY the CLI `--chain` string and the compile-time
//! [`HARDCODED`] table. Do NOT introduce reads from environment variables,
//! RPC, config files, or any other runtime input — the whole point is that
//! two nodes built from the same commit on the same chain id compute
//! identical fork conditions. To re-time a fork, ship a new binary; there
//! is no in-place override path by design.

use alloy_genesis::Genesis;
use greth::{
    reth::chainspec::EthereumChainSpecParser,
    reth_chainspec::ChainSpec,
    reth_cli::chainspec::{parse_genesis, ChainSpecParser},
};
use std::{fmt, sync::Arc};

/// Gravity mainnet chain id.
pub const GRAVITY_MAINNET_CHAIN_ID: u64 = 127_001;

/// Per-chain hardcoded override. Each field gates one fork.
///
/// `alpha_time` is written into `genesis.config.extra_fields["alphaTime"]`,
/// which is the same key the CL side reads. So a single
/// `alpha_time: Some(ts)` entry pins **both** EL and (future) CL Alpha to
/// the same authoritative timestamp.
struct GravityForkOverrides {
    /// Maps to `genesis.config.prague_time` (typed field on `ChainConfig`).
    prague_time: Option<u64>,
    /// Maps to `genesis.config.extra_fields["alphaTime"]`. EL reads this in
    /// `From<Genesis>`; the CL side reads the same key in
    /// `ConsensusHardforks::from_genesis_extra_fields`.
    alpha_time: Option<u64>,
}

/// Chain ids the binary owns. Listed entries are authoritative for the
/// forks they enumerate; operator-supplied values are discarded.
///
/// Adding or modifying an entry on a shipped chain id is a consensus
/// change. The PR doing so must reference the onchain governance or
/// coordination record that authorised the timestamp.
const HARDCODED: &[(u64, GravityForkOverrides)] = &[
    // Gravity mainnet. Source: gravity-mainet-gitops/genesis.json.
    (
        GRAVITY_MAINNET_CHAIN_ID,
        GravityForkOverrides {
            // Pinned to deployed mainnet genesis (config.pragueTime).
            prague_time: Some(1_782_709_200),
            // Forward-defended: no timestamp has been chosen onchain yet.
            // When governance picks one, change to Some(ts) in the release
            // that ships the activation.
            alpha_time: None,
        },
    ),
];

/// [`ChainSpecParser`] that wraps [`EthereumChainSpecParser`] and applies
/// the hardcoded override on the file / inline-JSON fallthrough path
/// BEFORE `From<Genesis>` runs.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct GravityChainSpecParser;

impl ChainSpecParser for GravityChainSpecParser {
    type ChainSpec = ChainSpec;
    // Inherit named-chain list from upstream so `--chain dev` etc. still
    // resolve through the cached LazyLock statics. None of these are
    // gravity-owned chain ids, so delegation is safe.
    const SUPPORTED_CHAINS: &'static [&'static str] = EthereumChainSpecParser::SUPPORTED_CHAINS;

    fn parse(s: &str) -> eyre::Result<Arc<Self::ChainSpec>> {
        // Named-chain fast path: delegate. We use `.iter().any()` rather
        // than `.contains(&s)` because the slice elements have `'static`
        // lifetime and `&s` does not.
        if Self::SUPPORTED_CHAINS.iter().any(|c| *c == s) {
            return EthereumChainSpecParser::parse(s);
        }
        // Fallthrough: file path or inline JSON. Parse, mutate, then
        // hand the mutated Genesis to `From<Genesis> for ChainSpec`.
        let mut g = parse_genesis(s)?;
        let events = apply_overrides(&mut g);
        // `tracing` is not yet initialized — `clap` calls us inside
        // `Cli::parse()`, before `main()` reaches `cli.run()` which
        // installs the subscriber. Use `eprintln!` so operators see the
        // diff in stderr (production `start.sh` redirects stderr into
        // `${LOG_DIR}/debug.log`). A silent override masks root causes
        // like a genesis regen that drops a hardfork field.
        for ev in &events {
            eprintln!("{ev}");
        }
        Ok(Arc::new(g.into()))
    }
}

/// A fork whose effective condition was changed by the override relative
/// to what the operator supplied in genesis. Emitted to stderr by
/// [`GravityChainSpecParser::parse`] so a `genesis != binary` mismatch is
/// visible at startup rather than silently papered over.
///
/// The fields are intentionally raw `Option<u64>` (genesis-supplied vs.
/// binary-forced) so the message can be grepped programmatically.
#[derive(Debug, PartialEq, Eq)]
struct OverrideEvent {
    chain_id: u64,
    fork: &'static str,
    genesis: Option<u64>,
    forced: Option<u64>,
}

impl fmt::Display for OverrideEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn opt(v: Option<u64>) -> String {
            v.map(|t| t.to_string()).unwrap_or_else(|| "<unset>".to_string())
        }
        write!(
            f,
            "WARN [GravityChainSpecParser] chainId {}: binary forces {} = {}; \
             genesis says {}. Update genesis to match if this is unexpected.",
            self.chain_id,
            self.fork,
            opt(self.forced),
            opt(self.genesis),
        )
    }
}

/// Apply the hardcoded schedule for `g.config.chain_id` (if listed) in
/// place. Returns a list of forks whose condition was actually changed by
/// the override (i.e. genesis disagreed with the binary). Caller decides
/// how to surface these — `parse()` writes them to stderr. No-op (and
/// empty events vec) if the chain id is not in [`HARDCODED`].
fn apply_overrides(g: &mut Genesis) -> Vec<OverrideEvent> {
    let mut events = Vec::new();
    let Some((_, ov)) = HARDCODED.iter().find(|(id, _)| *id == g.config.chain_id) else {
        return events;
    };
    let chain_id = g.config.chain_id;

    // Prague: typed `ChainConfig` field. `Some` overwrites, `None` clears.
    if g.config.prague_time != ov.prague_time {
        events.push(OverrideEvent {
            chain_id,
            fork: "prague_time",
            genesis: g.config.prague_time,
            forced: ov.prague_time,
        });
        g.config.prague_time = ov.prague_time;
    }

    // Alpha: `extra_fields` map. `OtherFields` derefs to
    // `BTreeMap<String, serde_json::Value>`, so we can `insert` / `remove`
    // directly.
    let genesis_alpha = g.config.extra_fields.get("alphaTime").and_then(|v| v.as_u64());
    if genesis_alpha != ov.alpha_time {
        events.push(OverrideEvent {
            chain_id,
            fork: "alphaTime",
            genesis: genesis_alpha,
            forced: ov.alpha_time,
        });
        match ov.alpha_time {
            Some(t) => {
                g.config.extra_fields.insert("alphaTime".to_string(), serde_json::Value::from(t));
            }
            None => {
                g.config.extra_fields.remove("alphaTime");
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use greth::reth_chainspec::{
        ChainSpecBuilder, EthereumHardfork, EthereumHardforks, ForkCondition, Hardforks,
    };

    fn genesis_json(chain_id: u64, prague_time: Option<u64>, alpha_time: Option<u64>) -> String {
        let prague = match prague_time {
            Some(t) => format!(r#","pragueTime":{t}"#),
            None => String::new(),
        };
        let alpha = match alpha_time {
            Some(t) => format!(r#","alphaTime":{t}"#),
            None => String::new(),
        };
        format!(
            r#"{{"config":{{"chainId":{chain_id},"homesteadBlock":0,"eip150Block":0,"eip155Block":0,"eip158Block":0,"byzantiumBlock":0,"constantinopleBlock":0,"petersburgBlock":0,"istanbulBlock":0,"berlinBlock":0,"londonBlock":0,"terminalTotalDifficulty":0,"terminalTotalDifficultyPassed":true,"shanghaiTime":0,"cancunTime":0{prague}{alpha}}},"nonce":"0x0","timestamp":"0x0","extraData":"0x","gasLimit":"0x4c4b40","difficulty":"0x0","mixHash":"0x0000000000000000000000000000000000000000000000000000000000000000","coinbase":"0x0000000000000000000000000000000000000000","alloc":{{}},"number":"0x0","gasUsed":"0x0","parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000"}}"#
        )
    }

    fn build_via_parser(
        chain_id: u64,
        prague_time: Option<u64>,
        alpha_time: Option<u64>,
    ) -> Arc<ChainSpec> {
        let s = genesis_json(chain_id, prague_time, alpha_time);
        GravityChainSpecParser::parse(&s).expect("parse should succeed")
    }

    fn table_prague_timestamp() -> u64 {
        HARDCODED
            .iter()
            .find(|(id, _)| *id == GRAVITY_MAINNET_CHAIN_ID)
            .and_then(|(_, ov)| ov.prague_time)
            .expect("mainnet must have a Some(ts) Prague entry")
    }

    // ---------- Trait wiring ----------

    /// All named chains still resolve via the inherited delegation.
    #[test]
    fn parse_known_chains_delegates_to_upstream() {
        for &chain in GravityChainSpecParser::SUPPORTED_CHAINS {
            assert!(
                GravityChainSpecParser::parse(chain).is_ok(),
                "Failed to parse named chain `{chain}`",
            );
        }
    }

    // ---------- Ethereum-side: Prague override ----------

    #[test]
    fn mainnet_overrides_prague_when_genesis_supplies_a_different_value() {
        let table_ts = table_prague_timestamp();
        let cs = build_via_parser(GRAVITY_MAINNET_CHAIN_ID, Some(100), None);
        assert_eq!(
            cs.hardforks.fork(EthereumHardfork::Prague),
            ForkCondition::Timestamp(table_ts),
            "binary value must win over genesis",
        );
        assert!(!cs.is_prague_active_at_timestamp(100));
        assert!(!cs.is_prague_active_at_timestamp(table_ts - 1));
        assert!(cs.is_prague_active_at_timestamp(table_ts));
    }

    #[test]
    fn mainnet_overrides_prague_when_genesis_omits_it() {
        let table_ts = table_prague_timestamp();
        let cs = build_via_parser(GRAVITY_MAINNET_CHAIN_ID, None, None);
        assert_eq!(cs.hardforks.fork(EthereumHardfork::Prague), ForkCondition::Timestamp(table_ts),);
        assert!(!cs.is_prague_active_at_timestamp(table_ts - 1));
        assert!(cs.is_prague_active_at_timestamp(table_ts));
    }

    #[test]
    fn non_mainnet_chain_id_passes_prague_through() {
        // Gravity testnet chain id — not in the table, so genesis wins.
        let cs = build_via_parser(7_771_625, Some(100), None);
        assert_eq!(cs.hardforks.fork(EthereumHardfork::Prague), ForkCondition::Timestamp(100),);
        assert!(cs.is_prague_active_at_timestamp(100));
        assert!(!cs.is_prague_active_at_timestamp(99));
    }

    #[test]
    fn mainnet_does_not_touch_shanghai_or_cancun() {
        let cs = build_via_parser(GRAVITY_MAINNET_CHAIN_ID, None, None);
        assert!(cs.is_shanghai_active_at_timestamp(0));
        assert!(cs.is_cancun_active_at_timestamp(0));
    }

    // ---------- Gravity-side (extra_fields): Alpha forward-defence ----------

    #[test]
    fn mainnet_forces_alpha_removed_when_genesis_supplies_alpha_time() {
        // After the parser runs, alphaTime must be removed from the
        // preserved Genesis. Both EL `From<Genesis>` and CL
        // `from_genesis_extra_fields` read from this same `extra_fields`
        // map, so a single removal forward-defends both layers.
        let cs = build_via_parser(GRAVITY_MAINNET_CHAIN_ID, None, Some(100));
        assert!(
            cs.genesis().config.extra_fields.get("alphaTime").is_none(),
            "alphaTime must be removed by override",
        );
    }

    #[test]
    fn mainnet_alpha_remains_removed_when_genesis_omits_it() {
        let cs = build_via_parser(GRAVITY_MAINNET_CHAIN_ID, None, None);
        assert!(cs.genesis().config.extra_fields.get("alphaTime").is_none());
    }

    #[test]
    fn non_mainnet_chain_id_passes_alpha_through() {
        // Testnet: not in the table, so the operator's alphaTime survives
        // in extra_fields, and the CL side will pick it up at startup.
        let cs = build_via_parser(7_771_625, None, Some(100));
        assert_eq!(
            cs.genesis().config.extra_fields.get("alphaTime").and_then(|v| v.as_u64()),
            Some(100),
        );
    }

    // ---------- Unified-view invariant ----------

    /// D-1 explicitly DOES mutate `chain_spec.genesis` (unlike #369's
    /// EL-local property). This is intentional: it gives EL and CL one
    /// consistent view. Asserting both directions makes the contract
    /// explicit.
    #[test]
    fn post_override_genesis_reflects_the_table() {
        let cs = build_via_parser(GRAVITY_MAINNET_CHAIN_ID, Some(100), Some(50));
        assert_eq!(
            cs.genesis().config.prague_time,
            Some(table_prague_timestamp()),
            "preserved Genesis must reflect the table (Prague)",
        );
        assert!(
            cs.genesis().config.extra_fields.get("alphaTime").is_none(),
            "preserved Genesis must reflect the table (alphaTime removed)",
        );
        assert_eq!(
            cs.hardforks.fork(EthereumHardfork::Prague),
            ForkCondition::Timestamp(table_prague_timestamp()),
        );
    }

    // ---------- Idempotency ----------

    #[test]
    fn parse_is_idempotent_on_mainnet() {
        let s = genesis_json(GRAVITY_MAINNET_CHAIN_ID, Some(100), Some(50));
        let a = GravityChainSpecParser::parse(&s).expect("parse a");
        let b = GravityChainSpecParser::parse(&s).expect("parse b");
        assert_eq!(a.hardforks, b.hardforks);
        assert_eq!(a.genesis_hash(), b.genesis_hash());
    }

    // ---------- Bypass tripwire ----------

    /// `ChainSpecBuilder` bypasses `GravityChainSpecParser`. Today this is
    /// safe because upstream `MAINNET` has chain id 1, not 127001. If a
    /// future change ever makes this assertion false, the bypass becomes a
    /// real bug and this test fails loud.
    #[test]
    fn chainspec_builder_mainnet_does_not_match_gravity_chain_id() {
        assert_ne!(ChainSpecBuilder::mainnet().build().chain.id(), GRAVITY_MAINNET_CHAIN_ID,);
    }

    // ---------- Governance pin ----------

    /// Consensus-affecting pin: this is the gravity-mainnet Prague
    /// activation timestamp, sourced from the deployed mainnet genesis
    /// (`gravity-mainet-gitops/genesis.json` → `config.pragueTime`).
    /// Changing this value requires onchain governance / network-wide
    /// coordination — this test exists specifically to catch a casual edit.
    #[test]
    fn gravity_mainnet_prague_timestamp_pinned_to_governance_value() {
        assert_eq!(table_prague_timestamp(), 1_782_709_200);
    }

    // ---------- Mismatch-warning event emission ----------

    /// Parse the JSON helper directly to a `Genesis` (without running the
    /// override) so tests can call `apply_overrides` themselves and assert
    /// on the returned events.
    fn parse_genesis_only(
        chain_id: u64,
        prague_time: Option<u64>,
        alpha_time: Option<u64>,
    ) -> Genesis {
        let s = genesis_json(chain_id, prague_time, alpha_time);
        serde_json::from_str(&s).expect("valid genesis json")
    }

    #[test]
    fn apply_overrides_emits_event_when_genesis_prague_differs() {
        let mut g = parse_genesis_only(GRAVITY_MAINNET_CHAIN_ID, Some(100), None);
        let events = apply_overrides(&mut g);
        // Prague: genesis Some(100) vs forced Some(table_ts) → event.
        // Alpha:  genesis None      vs forced None           → no event.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].fork, "prague_time");
        assert_eq!(events[0].genesis, Some(100));
        assert_eq!(events[0].forced, Some(table_prague_timestamp()));
    }

    #[test]
    fn apply_overrides_emits_event_when_genesis_omits_prague() {
        let mut g = parse_genesis_only(GRAVITY_MAINNET_CHAIN_ID, None, None);
        let events = apply_overrides(&mut g);
        // Prague: genesis None vs forced Some(table_ts) → event.
        // Alpha:  genesis None vs forced None           → no event.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].fork, "prague_time");
        assert_eq!(events[0].genesis, None);
        assert_eq!(events[0].forced, Some(table_prague_timestamp()));
    }

    #[test]
    fn apply_overrides_emits_event_when_genesis_supplies_alpha() {
        let mut g =
            parse_genesis_only(GRAVITY_MAINNET_CHAIN_ID, Some(table_prague_timestamp()), Some(100));
        let events = apply_overrides(&mut g);
        // Prague: genesis Some(table_ts) vs forced Some(table_ts) → no event.
        // Alpha:  genesis Some(100)      vs forced None           → event.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].fork, "alphaTime");
        assert_eq!(events[0].genesis, Some(100));
        assert_eq!(events[0].forced, None);
    }

    /// Both forks disagree → both events fire, in declared table order
    /// (prague_time first, then alphaTime).
    #[test]
    fn apply_overrides_emits_both_events_when_both_disagree() {
        let mut g = parse_genesis_only(GRAVITY_MAINNET_CHAIN_ID, Some(42), Some(99));
        let events = apply_overrides(&mut g);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].fork, "prague_time");
        assert_eq!(events[1].fork, "alphaTime");
    }

    /// Genesis already matches the binary on every gated fork → silent.
    #[test]
    fn apply_overrides_is_silent_when_genesis_already_matches_table() {
        let mut g =
            parse_genesis_only(GRAVITY_MAINNET_CHAIN_ID, Some(table_prague_timestamp()), None);
        let events = apply_overrides(&mut g);
        assert!(events.is_empty(), "no events when genesis agrees with the table");
    }

    /// Chain id not in `HARDCODED` → entirely no-op, no events even if
    /// genesis sets values that would mismatch the gravity-mainnet table.
    #[test]
    fn apply_overrides_is_silent_for_chain_id_not_in_table() {
        let mut g = parse_genesis_only(7_771_625, Some(100), Some(50));
        let events = apply_overrides(&mut g);
        assert!(events.is_empty(), "no events for chain id outside HARDCODED");
    }

    /// The stderr line is grep-friendly: stable prefix, raw values, and
    /// `<unset>` token for `None`. Tested so future formatting changes
    /// have to consciously update operator-facing strings.
    #[test]
    fn override_event_display_is_grep_friendly() {
        let ev = OverrideEvent {
            chain_id: GRAVITY_MAINNET_CHAIN_ID,
            fork: "prague_time",
            genesis: None,
            forced: Some(1_782_709_200),
        };
        let s = format!("{ev}");
        assert!(s.starts_with("WARN [GravityChainSpecParser]"), "got: {s}");
        assert!(s.contains("chainId 127001"), "got: {s}");
        assert!(s.contains("prague_time"), "got: {s}");
        assert!(s.contains("1782709200"), "got: {s}");
        assert!(s.contains("<unset>"), "got: {s}");
    }
}
