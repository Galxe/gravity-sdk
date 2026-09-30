# feat(chainspec): hardcode Longevity testnet hardfork timestamps

## 1. Background and Current State

- Problem/outcome: Longevity testnet (chain id `7771625`) takes every hardfork
  timestamp from each node's own genesis file. The repository copies have
  already drifted from the chain: `genesis/testnet/genesis.json` (and the
  mono-grav cluster snapshot) lack `testnetOwnerFixV2Time`, which was set on
  live nodes through a rolling `upgrade.json`. A node started from the
  repository genesis would never schedule TestnetOwnerFixV2 and would diverge
  when replaying block `32664162`. Release `v1.10.1` (the next testnet upgrade,
  also a recommended mainnet upgrade) should make the binary the authoritative
  source of testnet fork times, as it already is for mainnet.
- Current implementation:
  - `bin/gravity_node/src/chainspec.rs` wraps `EthereumChainSpecParser`. For a
    chain id listed in the `HARDCODED` table, `apply_overrides` rewrites the
    parsed `Genesis` before `From<Genesis> for ChainSpec` runs, and prints one
    stderr `WARN [GravityChainSpecParser]` line per field that genesis
    disagreed with. `Some(ts)` forces a value; `None` removes it.
  - The table has one entry (mainnet `127001`) and four fields:
    `prague_time`, `osaka_time`, `alphaTime`, `betaTime`.
  - greth `main` (`5235476`) builds Gravity hardforks from five genesis
    `extra_fields` keys: `gammaTime`, `alphaTime`, `betaTime`,
    `testnetOwnerFixTime`, `testnetOwnerFixV2Time`
    (`crates/chainspec/src/spec.rs`). The SDK pin `41e1c20` parses the same
    keys.
  - The consensus layer reads only `alphaTime`, from the same mutated genesis
    (`bin/gravity_node/src/main.rs`, `consensus_hardforks_from_genesis_extra_fields`).
  - greth gates TestnetOwnerFix/V2 on `chain_id == LONGEVITY_TESTNET_CHAIN_ID`
    (`7_771_625`, exported from `crates/chainspec/src/gravity.rs`); Gamma has no
    chain-id gate and activates purely from `gammaTime`.
- Related changelogs/code evidence: none in this repository (first entry in
  `docs/changelogs/`). SDK #826 and #828 synced the testnet genesis schedule;
  greth #435 added TestnetOwnerFixV2.
- Runtime evidence (2026-09-30, read-only `eth_getBlockByNumber`,
  `eth_getCode`, `eth_call` against the public testnet RPC): for each
  timestamp, the first block with `timestamp >= value` has the previous block
  exactly one second earlier, and the fork's state effect appears in that block.

  | Fork | Timestamp | First block | Observed effect |
  | --- | --- | --- | --- |
  | Prague | `1780909442` | `1084564` | EIP-2935 history contract code appears (0 → 83 bytes) |
  | Alpha | `1783328400` | `10787823` | Matches the first Alpha block reported in greth #442 |
  | Beta / Osaka | `1785812400` | — | No directly observable state effect; value from genesis snapshot |
  | Gamma | `1787108400` | `25666608` | `NativeOracle` runtime swapped (4602 → 3904 bytes) |
  | TestnetOwnerFix | `1788490800` | `30922944` | StakePool `pendingOwner` set to the wrong CLI address |
  | TestnetOwnerFixV2 | `1788948000` | `32664162` | `pendingOwner` set to the cast-derived address |

- Constraints/assumptions/unknowns:
  - Hardcoded values must equal the timestamps the chain actually used;
    any other value makes the binary diverge on replay.
  - Beta/Osaka rest on the genesis snapshot (synced from live nodes in
    mono-grav #103), not on a state observation.
  - TestnetGammaBridge is not implemented in greth; its genesis key and
    activation time do not exist yet.
  - End-to-end clusters use chain id `1337`, so testnet overrides do not touch
    them. Benchmark configs reference `7771625` only as an RPC chain id.

## 2. Problem Model and End-to-End Behavior

- Root cause: testnet fork times are per-node runtime input, so correctness
  depends on every operator's genesis file staying in sync with a schedule that
  changes through rolling upgrades. The repository genesis has already fallen
  behind.
- Goals:
  - Testnet fork conditions are fixed by the binary and identical on every node
    built from the same commit.
  - Every fork greth reads from genesis is covered by the table for both owned
    chains, so no Gravity fork key passes through unchecked.
- Non-goals:
  - TestnetGammaBridge (added with the greth bump that implements it).
  - `gravityMinBaseFee` (not a fork timestamp).
  - greth changes, mono-grav genesis archives, mainnet Gamma scheduling
    (planned for `v1.10.2`).
- B1 — A testnet node parses genesis with chain id `7771625`: the resulting
  `ChainSpec` schedules Prague, Osaka, Alpha, Beta, Gamma, TestnetOwnerFix and
  TestnetOwnerFixV2 at the table timestamps regardless of genesis content.
- B2 — Testnet genesis missing or disagreeing on a field (for example no
  `testnetOwnerFixV2Time`): the binary value wins and one grep-friendly
  `WARN [GravityChainSpecParser]` line names the field, genesis value and
  forced value.
- B3 — Testnet genesis already matching the table: no warning.
- B4 — Mainnet: existing four fields unchanged; `gammaTime`,
  `testnetOwnerFixTime`, `testnetOwnerFixV2Time` are forced absent (removed
  with a warning if present). Live mainnet genesis has none of them, so the
  live schedule is unchanged.
- B5 — Chain ids outside the table (devnet, e2e `1337`): unchanged pass-through.
- F1 — A hardcoded value that differs from the chain's real activation would
  make replay diverge; guarded by per-value pin tests that record the
  activation block evidence.
- Idempotency/compatibility: parsing is deterministic and idempotent. Nodes
  whose genesis already matches see no behavior change; the upgrade is safe to
  roll node by node on both networks.

## 3. Research, Findings, and Architecture Decision

- Repository evidence: the mainnet override table, its warning events, and
  its pin tests are the established pattern for binary-authoritative fork
  times; greth already exports `LONGEVITY_TESTNET_CHAIN_ID`.
- External research: not needed. The change extends an existing local
  mechanism; correctness depends only on this chain's recorded activations.
- Validated premises (agreed 2026-09-30):
  1. Extend the existing `HARDCODED` table rather than introduce a new
     mechanism, reusing greth's testnet chain-id constant.
  2. Values equal on-chain activations, so live nodes see no behavior change.
  3. TestnetGammaBridge is deferred until greth implements it.
- Approaches:
  - A — Extend the table with the three missing Gravity fields, add a testnet
    entry, set the new fields to `None` for mainnet. Minimal, keeps the
    two-state `Some`/`None` contract, closes the Gamma pass-through gap on
    mainnet.
  - B — Same table, but let mainnet pass the new fields through unchanged.
    Needs a third "untouched" state and leaves a single-node Gamma
    misconfiguration able to fork one mainnet node.
  - C — Keep genesis-driven testnet times and only fix the repository genesis
    file. Status quo; does not prevent the next drift.
- D1 — Choose A: the binary owns every Gravity fork key for both listed chains.
- D2 — Mainnet sets `gamma_time`, `testnet_owner_fix_time`,
  `testnet_owner_fix_v2_time` to `None`. OwnerFix fields are inert on mainnet
  either way because of greth's chain-id gate; `None` for Gamma removes a
  per-node divergence path until `v1.10.2` pins the mainnet value.
- D3 — Testnet chain id comes from greth's `LONGEVITY_TESTNET_CHAIN_ID`
  instead of a new SDK literal.
- D4 — TestnetGammaBridge is out of scope; the greth-bump PR that introduces
  it adds its field and timestamp.
- R1 — Beta/Osaka value is not independently state-verified.
- R2 — Code paths that build a `ChainSpec` without this parser bypass the
  override (existing, documented bypass risk); testnet now shares it.
- Closest references: `bin/gravity_node/src/chainspec.rs` (table, events, pin
  tests), greth `crates/chainspec/src/spec.rs` (`From<Genesis>` fork keys).

## 4. Implementation Design

- Change/ownership map: only `bin/gravity_node/src/chainspec.rs` (the parser
  already owns binary-authoritative fork times). No change to `main.rs`
  (the consensus layer keeps reading `alphaTime` from the mutated genesis),
  greth, genesis files, or deployment artifacts.
- Call/data flow (unchanged funnel): `--chain <genesis>` →
  `GravityChainSpecParser::parse` → `parse_genesis` → `apply_overrides`
  looks up `HARDCODED` by `chain_id` → forces typed fields
  (`prague_time`, `osaka_time`) and Gravity `extra_fields` → stderr WARN per
  changed field → `From<Genesis> for ChainSpec` in greth.
- Critical interfaces:

  ```rust
  use greth::reth_chainspec::{ChainSpec, LONGEVITY_TESTNET_CHAIN_ID};

  struct GravityForkOverrides {
      prague_time: Option<u64>,
      osaka_time: Option<u64>,
      alpha_time: Option<u64>,
      beta_time: Option<u64>,
      gamma_time: Option<u64>,
      testnet_owner_fix_time: Option<u64>,
      testnet_owner_fix_v2_time: Option<u64>,
  }

  impl GravityForkOverrides {
      /// Gravity forks keyed by the genesis `extra_fields` name greth reads.
      fn extra_field_forks(&self) -> [(&'static str, Option<u64>); 5] {
          [
              ("alphaTime", self.alpha_time),
              ("betaTime", self.beta_time),
              ("gammaTime", self.gamma_time),
              ("testnetOwnerFixTime", self.testnet_owner_fix_time),
              ("testnetOwnerFixV2Time", self.testnet_owner_fix_v2_time),
          ]
      }
  }
  ```

  Table entries (D1, D2, D3):

  | Field | Mainnet `127001` | Testnet `LONGEVITY_TESTNET_CHAIN_ID` |
  | --- | --- | --- |
  | `prague_time` | `Some(1_782_709_200)` (unchanged) | `Some(1_780_909_442)` |
  | `osaka_time` | `Some(GRAVITY_MAINNET_BETA_OSAKA_TIME)` (unchanged) | `Some(LONGEVITY_TESTNET_BETA_OSAKA_TIME)` = `1_785_812_400` |
  | `alpha_time` | `Some(GRAVITY_MAINNET_ALPHA_TIME)` (unchanged) | `Some(1_783_328_400)` |
  | `beta_time` | `Some(GRAVITY_MAINNET_BETA_OSAKA_TIME)` (unchanged) | `Some(LONGEVITY_TESTNET_BETA_OSAKA_TIME)` |
  | `gamma_time` | `None` | `Some(1_787_108_400)` |
  | `testnet_owner_fix_time` | `None` | `Some(1_788_490_800)` |
  | `testnet_owner_fix_v2_time` | `None` | `Some(1_788_948_000)` |

  Each testnet value carries a comment with its first on-chain activation
  block from section 1.
- Algorithm: `apply_overrides` keeps the two typed-field blocks and replaces
  the duplicated `alphaTime` / `betaTime` blocks with one loop over
  `extra_field_forks()`: read `extra_fields[key].as_u64()`; if it differs from
  the table value, push an `OverrideEvent { fork: key, .. }`, then insert
  `Some(t)` or remove on `None`. Event order becomes `prague_time`,
  `osaka_time`, `alphaTime`, `betaTime`, `gammaTime`, `testnetOwnerFixTime`,
  `testnetOwnerFixV2Time`. Deterministic, no runtime inputs (module invariant
  preserved).
- Errors/security/observability: no new error path. Operators see the existing
  `WARN [GravityChainSpecParser] chainId <id>: binary forces <key> = <v>;
  genesis says <v>.` line; a testnet node on the repository genesis sees one
  line for `testnetOwnerFixV2Time` (genesis `<unset>`). Module docs that say the
  table covers mainnet only are updated to name both chains.
- Migration/compatibility/rollout: no data migration. Values equal on-chain
  activations, so testnet nodes replay identically; mainnet schedule is
  unchanged (live genesis has none of the three new keys). Mixed-version
  fleets are safe. Rollback is replacing the binary. Per the 2026-09-30
  decision, genesis files are not edited: once hardcoded, genesis fork fields
  no longer drive behavior.
- Authoritative docs: module-level rustdoc in `chainspec.rs`; this changelog.

### Serial Implementation Checklist

- [x] Tests first (RED where meaningful): extend the test genesis helper with
  the three new keys; add the testnet schedule, repository-genesis and mainnet
  key-removal tests; move pass-through tests to an unlisted chain id. Run the
  focused command and confirm the new testnet/mainnet tests fail for the
  expected reason. Maps B1-B5, F1.
- [x] Extend `GravityForkOverrides`, add `extra_field_forks()`, fold the
  `extra_fields` handling into the loop, add the testnet entry and mainnet
  `None`s, import `LONGEVITY_TESTNET_CHAIN_ID`, update module docs. Maps D1-D3.
- [x] Focused tests green; fmt check and scoped clippy clean.

Code-stage notes (non-material):

- RED was taken after extending the struct but before adding the testnet
  entry, since the new tests read the new struct fields and could not compile
  earlier. 4 testnet tests failed for the missing entry; notably
  `repository_testnet_genesis_never_contradicts_table` failed with
  `TestnetOwnerFixV2: left Never, right Timestamp(1788948000)`, reproducing the
  genesis drift from section 1.
- `testnet_genesis_matching_the_table_is_silent` passes vacuously without the
  entry; it guards B3 only once the entry exists.
- Clippy deviation: the planned `cargo clippy -p gravity_node --all-targets
  --all-features -- -D warnings` fails on a pre-existing dead-code error in the
  untouched `crates/api` (`FailpointConfResponse is never constructed`), also
  with default features. Linting uses `--no-deps` to cover `gravity_node`
  only; the `crates/api` error is left as-is (out of scope).

## 5. Verification and E2E Design

- Testability: `apply_overrides` and `GravityChainSpecParser::parse` are pure
  functions of the genesis input; existing helpers `genesis_json`,
  `build_via_parser`, `parse_genesis_only` cover both layers. Expected values
  in new tests are literals, not read back from `HARDCODED`, except in the
  helper-based mainnet tests that already follow that pattern.
- Representative tests:

  ```rust
  const UNLISTED_CHAIN_ID: u64 = 1337; // e2e cluster chain id

  #[test]
  fn testnet_schedule_is_forced_when_genesis_omits_every_fork() {
      let cs = build_via_parser(LONGEVITY_TESTNET_CHAIN_ID, ForkInputs::default());
      assert_eq!(cs.hardforks.fork(EthereumHardfork::Prague), ForkCondition::Timestamp(1_780_909_442));
      assert_eq!(cs.hardforks.fork(EthereumHardfork::Osaka), ForkCondition::Timestamp(1_785_812_400));
      let g = &cs.gravity_hardforks;
      assert_eq!(g.fork(GravityHardfork::Alpha), ForkCondition::Timestamp(1_783_328_400));
      assert_eq!(g.fork(GravityHardfork::Beta), ForkCondition::Timestamp(1_785_812_400));
      assert_eq!(g.fork(GravityHardfork::Gamma), ForkCondition::Timestamp(1_787_108_400));
      assert_eq!(g.fork(GravityHardfork::TestnetOwnerFix), ForkCondition::Timestamp(1_788_490_800));
      assert_eq!(g.fork(GravityHardfork::TestnetOwnerFixV2), ForkCondition::Timestamp(1_788_948_000));
  }

  /// The shipped testnet genesis may omit fork keys, but must never carry a
  /// value that contradicts the binary.
  #[test]
  fn repository_testnet_genesis_never_contradicts_table() {
      let mut g: Genesis =
          serde_json::from_str(include_str!("../../../genesis/testnet/genesis.json")).unwrap();
      assert_eq!(g.config.chain_id, LONGEVITY_TESTNET_CHAIN_ID);
      for ev in apply_overrides(&mut g) {
          assert_eq!(ev.genesis, None, "genesis contradicts binary: {ev}");
      }
  }

  #[test]
  fn mainnet_removes_gamma_and_owner_fix_keys() {
      let mut g = parse_genesis_only(
          GRAVITY_MAINNET_CHAIN_ID,
          ForkInputs { gamma: Some(1), owner_fix: Some(2), owner_fix_v2: Some(3), ..matching_mainnet_forks() },
      );
      let forks: Vec<_> = apply_overrides(&mut g).iter().map(|e| e.fork).collect();
      assert_eq!(forks, ["gammaTime", "testnetOwnerFixTime", "testnetOwnerFixV2Time"]);
      let cs: ChainSpec = g.into();
      assert_eq!(cs.gravity_hardforks.fork(GravityHardfork::Gamma), ForkCondition::Never);
  }
  ```

  Also: a testnet pin test (one assertion per value, comment names the
  activation block); `apply_overrides_emits_all_events_when_all_disagree`
  extended to seven events in order; existing pass-through and
  "silent for unlisted chain" tests switched from `7_771_625` to
  `UNLISTED_CHAIN_ID`.
- Mutations these catch: a wrong or missing testnet value (schedule and pin
  tests); dropping a key from `extra_field_forks()` (schedule, mainnet
  removal, all-disagree tests); passing the new keys through on mainnet
  (mainnet removal test); a future genesis edit with a conflicting timestamp
  (repository genesis test).
- E2E Required: no — the change is a pure genesis → `ChainSpec` transform
  proven at the parser boundary with the real repository genesis; the startup
  funnel is unchanged. Replay of testnet and mainnet state with the `v1.10.1`
  binary is a release gate tracked in mono-grav version-status docs, not part
  of this PR.
- Exact commands (from the worktree root; the repository defines no
  `make lint-fix`, so the CI commands are used):
  - `RUSTFLAGS="--cfg tokio_unstable" cargo test -p gravity_node chainspec`
  - `cargo +nightly-2026-09-05 fmt --all -- --check`
  - `RUSTFLAGS="--cfg tokio_unstable" cargo clippy -p gravity_node --all-targets --all-features -- -D warnings`
- Excluded: full `cargo test --workspace` (unrelated crates, change is local to
  one module; CI runs it on the PR).

| Behavior | Evidence |
|----------|----------|
| B1 | `testnet_schedule_is_forced_when_genesis_omits_every_fork` |
| B2 | `repository_testnet_genesis_never_contradicts_table`; testnet event test for a disagreeing value |
| B3 | testnet silent-apply test with a fully matching genesis |
| B4 | `mainnet_removes_gamma_and_owner_fix_keys`; existing mainnet tests unchanged |
| B5 | pass-through tests on `UNLISTED_CHAIN_ID` |
| F1 | testnet pin test with activation-block comments |

## 6. Human Decisions and Interaction

- 2026-09-30: next testnet upgrade is `v1.10.1`; SDK hardcodes historical
  testnet fork times; TestnetGammaBridge deferred to the greth-bump PR (D4).
- 2026-09-30: mainnet impact reviewed — OwnerFix keys are inert on mainnet via
  greth's chain-id gate; `None` for all three new mainnet fields accepted (D2).
- 2026-09-30: genesis files are not edited; after hardcoding, genesis fork
  fields are unnecessary. The repository-genesis test therefore allows missing
  keys and rejects only contradicting values.
- 2026-09-30: sections 1-3 approved.

## 7. Outcome and Evidence

- Result: a node started with a Longevity testnet genesis (chain id
  `7771625`) schedules all seven gated forks at the on-chain activation
  timestamps regardless of genesis content. The repository testnet genesis,
  which lacks `testnetOwnerFixV2Time`, now yields the correct schedule (one
  startup WARN for that key) instead of never scheduling TestnetOwnerFixV2.
  Mainnet additionally pins `gammaTime`, `testnetOwnerFixTime` and
  `testnetOwnerFixV2Time` to absent.
- Reconciliation:

  | ID | Status | Evidence |
  | --- | --- | --- |
  | B1 | DONE | `testnet_schedule_is_forced_when_genesis_omits_every_fork` |
  | B2 | DONE | `testnet_overrides_a_genesis_value_that_disagrees`, `repository_testnet_genesis_never_contradicts_table` |
  | B3 | DONE | `testnet_genesis_matching_the_table_is_silent` |
  | B4 | DONE | `mainnet_removes_gamma_and_owner_fix_keys`; existing mainnet tests unchanged and green. SDK `genesis/mainnet/genesis.json` and production `gravity-mainnet-gitops` `genesis.json` (main, 2026-09-30) contain none of the three keys, so the live mainnet schedule and startup output are unchanged |
  | B5 | DONE | pass-through tests on `UNLISTED_CHAIN_ID` (1337) |
  | F1 | DONE | `gravity_testnet_timestamps_pinned_to_onchain_activations`; table comments name the activation blocks |
  | D1-D3 | DONE | `bin/gravity_node/src/chainspec.rs` |
  | D4 | DONE (deferred by design) | no TestnetGammaBridge field added |
  | R1 | Accepted | Beta/Osaka from the live genesis snapshot, not state-verified |
  | R2 | Checked | `GravityChainSpecParser` is the only `ChainSpec` construction path in the SDK (`cli.rs`, `main.rs`) |

- Changes: `bin/gravity_node/src/chainspec.rs` (table, loop, testnet entry,
  tests, module docs) and this changelog. No dependency, genesis, config or
  deployment change.
- Deviations: RED taken after the struct extension; clippy run with
  `--no-deps` because of a pre-existing `crates/api` dead-code error (details
  in section 4 notes).
- Verification (worktree root, final product tree `a0b2c967`):
  - `RUSTFLAGS="--cfg tokio_unstable" cargo test -p gravity_node` → exit 0,
    57 passed (38 in `chainspec`).
  - `cargo +nightly-2026-09-05 fmt --all -- --check` → exit 0.
  - `RUSTFLAGS="--cfg tokio_unstable" cargo clippy -p gravity_node --no-deps --all-targets --all-features -- -D warnings` → exit 0.
  - `make lint-fix`: not defined in this repository.
- E2E: not required (section 5); the stable boundary is the parser, exercised
  with the real repository testnet genesis.
- Migration/docs: none beyond module rustdoc.

## 8. Remaining Work

- TestnetGammaBridge: add its genesis key and timestamp to the table in the
  SDK PR that bumps greth to the implementation (D4).
- `v1.10.1` release gate (tracked in mono-grav version-status docs): replay
  testnet and mainnet state with the release binary.
- Pre-existing: `cargo clippy -p gravity_node` with dependencies fails on
  `crates/api` `FailpointConfResponse` dead code under `-D warnings`; not
  addressed here.
