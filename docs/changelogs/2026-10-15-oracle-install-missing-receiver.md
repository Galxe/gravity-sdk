# Testnet installation from a missing Bridge Receiver

## 1. Background and Current State

The target testnet lacks the authorized Receiver. Its installation must include
Contracts PR119 NativeOracle and bridge Task/default callback together. Reth's
review fix also handles an epoch block that first crosses both Gamma and the
installation timestamp. [Mono's primary plan and report](https://github.com/Galxe/mono-grav/blob/codex/issue131-oracle-install-e2e/docs/changelogs/2026-10-15-oracle-install-missing-receiver.md) owns the
cross-repository design and acceptance evidence.

## 2. Required Behavior

B1: A locked SDK release embeds the Reth implementation that atomically stages
Gamma when needed and installs Receiver, NativeOracle and task/callback at H.
B2: Activation and bridge inputs remain dynamic chain configuration; this change
does not import SDK PR843's fixed historical timestamps. B3: Both manifests and
the lock resolve one Reth revision without unrelated dependency changes.

## 3. Architecture Decision

Reth owns execution/migration behavior; Contracts owns frozen runtime source;
Mono owns isolated acceptance flows. SDK owns the exact Reth pin, startup
configuration validation, locked dependency graph and release provenance. This
review fix changes only the Reth revision strings in the two manifests and lock.

## 4. Implementation Design

Pin Reth `f028d188e207a5963af38120e76f7f406e563d6d` in workspace and node
manifests and every corresponding lock source. Preserve all other lock packages
and dependency edges. Use actual locked/offline Cargo metadata resolution on
Gravity Dev, then build the release and bind its ELF to the selected Reth source.

- [x] Pin the reviewed Reth implementation without unrelated graph changes.
- [x] Validate actual locked/offline Cargo metadata on Gravity Dev.
- [x] Build the new release and independently verify binary/source provenance.
- [x] Complete both final-binary acceptance flows and reconcile evidence.

## 5. Verification and E2E Design

All builds and tests run on Gravity Dev. Preserve the original offline mainnet PFN
baseline and guard thresholds. Run `cargo metadata --locked --offline
--format-version 1`, then `RUSTFLAGS="--cfg tokio_unstable" cargo build --locked -p gravity_node --release -j 4`
with the existing Rust 1.93 builder and owned isolated target directory.

E2E Required: yes. Run the focused four-message callback flow and complete
twelve-message installer, original-validator replay and recovery flow from a
missing Receiver once each with the new ELF. Bind release, Cargo-selected Reth,
frozen Contracts artifacts and source manifest to the resulting archives.

## 6. Human Decisions and Interaction

The user authorized preparing and publishing the four existing PRs for review,
including the Gamma-crossing fix, with one Gravity Dev operator. Local work is
static editing/review; no local formatting, build or tests. Merge and public
activation remain separate work.

Review order: [Contracts #119](https://github.com/Galxe/gravity_chain_core_contracts/pull/119) owns callback semantics; [Reth #457](https://github.com/Galxe/gravity-reth/pull/457) owns atomic installation; [SDK #844](https://github.com/Galxe/gravity-sdk/pull/844) pins Reth; [Mono #132](https://github.com/Galxe/mono-grav/pull/132) owns acceptance flows and evidence.

## 7. Outcome and Evidence

SDK product revision: `4412093c7cb48c39843c2b16ab19eb5926c06ba1`.
Selected Reth: `f028d188e207a5963af38120e76f7f406e563d6d`.
Cargo.lock SHA256: `7fd08cc89440b17f07536d0d9671a3ee09b6103a5569ae86be5703f7244b8805`.
Gravity Dev's actual locked/offline Cargo metadata resolution passed. Static
comparison confirms the two manifests and lock contain only the Reth revision
replacement. The Rust 1.93 locked release build passed with
`RUSTFLAGS="--cfg tokio_unstable"`. Its independently verified ELF SHA256 is
`d6b6930e8a6f4301314de37e9e54d89387070ef16ab9ca1aba1076bae4d880ec`;
source-tree SHA256 is
`f1f5816cee1c4b70a671829608a8157aab81689cee878a93456ba1d16a127081`.
Reth formatting produced zero diff; all 21 installer/RPC tests, one staged history
test and both canonical epoch cases passed. Canonical cases require separate
processes because the node harness initializes process-global configuration.
The focused runtime passed at H1220 with four matching replicas, nonce 4,
progress `[4,13]`, balance 13 and two mints. Fresh four-receipt source reload,
independent verification, archive reload and raw cleanup passed; acceptance archive SHA256 is
`f1733b11197bf29cffe31a3268b2624ca8d5183d01d65ad77164917c62dcd194`.
The full H457 installer/replay/recovery flow passed with the same ELF, including
original-database replay and a second recovery. All four replicas ended at nonce
12, progress `[12,167]`, balance 138 and 12 mints. The final soak ran
240.407 seconds and advanced common blocks 2932 to 4013 (+1081), above
the 768-block minimum. Fresh source reload verified 12 receipts; independent
verification, archive reload and raw cleanup passed. Full acceptance archive SHA256:
`483dd88db3c57c0c593fb5d39aca66472bd8e1b84bd044a43cb81917f06c71f2`.
Both acceptance archives remained unchanged after cleanup; the original offline
mainnet PFN baseline remained intact. Local evidence is retained under
`/home/jingyue/.local/state/issue131/cross-fork-install-recovered-20261015/`.

The [initial SDK verification record](https://github.com/Galxe/gravity-sdk/blob/ddd0dddcd07c1a3ab92e166f745dc1653164b7c1/docs/changelogs/2026-10-15-oracle-install-missing-receiver.md)
retains the earlier release and acceptance evidence bound to SDK `ceae46b` and
Reth `b84b33c`. It does not verify the new dependency pin. [Mono's primary report](https://github.com/Galxe/mono-grav/blob/codex/issue131-oracle-install-e2e/docs/changelogs/2026-10-15-oracle-install-missing-receiver.md)
owns current binary/source bindings, archive hashes and cleanup evidence.

## 8. Remaining Work

The Gravity Dev release build and acceptance are complete. This documentation
closeout follows the tested SDK product revision. Hosted required checks, review,
merge, release publication and public activation remain separate steps. The shared
callback batch-gas budget boundary remains unresolved; see Mono's primary report.
