# Testnet installation from a missing Bridge Receiver

The cross-repository design and acceptance flow are recorded in
[`mono-grav`'s primary changelog](https://github.com/Galxe/mono-grav/blob/codex/issue131-oracle-install-e2e/docs/changelogs/2026-10-15-oracle-install-missing-receiver.md).
This local record owns the SDK dependency pin and locked release evidence.

## 1. Current state

The SDK candidate at `3d7984d35a8b61fc3e3becadbd7ee34a7bc87db0`
pins Reth `0a52b0c24c007fe1745c7f0f26ac7abf9012a0a6`. That candidate
requires the destination Receiver to exist before task installation.
Current Testnet has no code at the native mint precompile's authorized
Receiver address, so the release must include Reth's fixed-address Receiver
installation together with the NativeOracle callback failure fix and task
installation.

## 2. Required behavior

- B1: A locked SDK release embeds the final Reth implementation that installs
  the Receiver, NativeOracle runtime and task/callback together at the
  configured activation timestamp.
- B2: Activation time and bridge inputs remain supplied dynamically through
  the chain configuration. This change does not import SDK PR #843's fixed
  historical fork timestamps.
- B3: The manifest, workspace dependency declaration and Cargo lock resolve
  exactly one final Reth revision, with no unrelated dependency updates.

## 3. Architecture

Reth owns execution and migration behavior; Contracts owns the runtime
source; Mono owns the two isolated acceptance flows. SDK changes only its
Reth pin and retains existing runtime configuration behavior.

## 4. Implementation

Update the Reth revision in `Cargo.toml`, the node dependency
declaration and `Cargo.lock`. Preserve all unrelated lock packages and
dependency edges. Validate the resulting graph using Cargo's actual locked,
offline metadata resolution on Gravity Dev before building.

## 5. Verification

All builds and tests run on Gravity Dev. No public chain receives writes.
Retain the original offline mainnet PFN baseline and guard thresholds.

- Resolve `cargo metadata --locked --offline --format-version 1` with the
  final SDK source and final Reth Git object available in the offline cache.
- Build `cargo build --locked -p gravity_node --release -j 4` using the
  existing pinned Rust 1.93 builder and isolated owned target directory.
- Bind the release ELF, Cargo-selected Reth source, frozen Contracts
  runtime/template and source manifest to acceptance evidence.
- E2E Required: yes. After root review is clean, execute one focused
  four-message callback flow and one complete twelve-message installer,
  original-validator replay and recovery flow from a missing Receiver.
  Do not repeat the historical ABC matrix or run five rounds.

## 6. Execution checklist

- [x] S1: Pin the final Reth revision without unrelated graph changes.
- [x] S2: Record locked Cargo metadata and successful release build.
- [x] S3: Export release and source provenance.
- [x] S4: Record both required final-tree E2E outcomes.
- [x] S5: Root reviews final diff and evidence; retain the local-only boundary.

## 7. Outcome

Final Reth revision: `b84b33c58e7db2566a8115e31efba5943d660183`.
Gravity Dev Cargo 1.93.0 passed locked offline metadata resolution.
The lock SHA256 is
`af2ecde2d634f84dcfd93f3ad0a9f680d109fe10305f12385276a30cae34be86`;
only the Reth source revision strings changed.

The tested SDK product revision is
`ceae46b6a97bd74cc9fddce273a3123a6be6bfb8`. Its locked Gravity Dev release
build passed with Rust 1.93.0; Cargo completed in 36m19s. The resulting ELF
SHA256 is
`3fecc79aa22e2fad3ad1f55c67e6c84f896ce73c9c75c61c9532c7a64275e1bd`.
The actual release job was `sdk-release-02`; job 01 failed operator mount
admission before any compiler ran. The original pre-generation snapshot
predates job 02, and all three generated Rust files were rewritten during
that build with unchanged bytes.

Both required single-run E2E flows passed with this ELF, including fresh
source reload, independent verification and archive reload:

- Focused callback flow: missing Receiver installed at H1235; two failed
  callbacks consumed their nonces and the later valid mint completed.
  Final nonce 4, exactly two mints, balance increase 13.
- Full installer flow: missing Receiver installed at H757; original core-4
  database replay and a second restart both succeeded. All twelve messages
  minted once, final nonce 12 and balance increase 138. The final soak
  lasted 240.411 seconds and advanced 1081 common blocks.

Local evidence is retained under
`/home/jingyue/.local/state/issue131/receiver-install-20261015/`:
`logs/sdk-release-02-outcome.json`,
`logs/gaptos-generation-origin-receiver-install-01.json`,
`artifacts/candidate/provenance.json`, both `logs/e2e/*/result.json` and
`logs/final-operator-outcome.json`. The primary Mono changelog records the
cross-repository checks and archive hashes. Both owned raw case directories
and build targets were removed after verification; the candidate and
acceptance archives remain. All four owned case/runtime units are inactive,
and the original offline mainnet PFN baseline is unchanged.

## 8. Remaining work

The user has now authorized preparing and publishing the four existing PRs for
review. Merge and public activation remain separate work; the mainnet PFN remains
untouched.

Review order: [Contracts #119](https://github.com/Galxe/gravity_chain_core_contracts/pull/119) owns callback semantics; [Reth #457](https://github.com/Galxe/gravity-reth/pull/457) owns atomic installation; [SDK #844](https://github.com/Galxe/gravity-sdk/pull/844) pins the tested Reth; [Mono #132](https://github.com/Galxe/mono-grav/pull/132) owns acceptance flows and evidence.
