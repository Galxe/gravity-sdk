# Integrate timestamp Oracle Task installation

For [Issue #131](https://github.com/Galxe/mono-grav/issues/131), SDK pins Reth's
installer and rejects malformed target-chain configuration before node startup.
The [primary cross-repository record](https://github.com/Galxe/mono-grav/blob/codex/issue131-oracle-install-e2e/docs/changelogs/2026-10-08-oracle-task-install-e2e.md)
owns the complete acceptance flow; the
[Reth owning record](https://github.com/Galxe/gravity-reth/pull/457)
owns configuration and consensus execution.

## Integration

Keep one exact Reth revision across the node manifest, root
`reth-primitives-traits` patch and all 113 git lock sources. The original advance
from `41e1c20` to base `5235476` integrates seven upstream PRs
[#440](https://github.com/Galxe/gravity-reth/pull/440),
[#444](https://github.com/Galxe/gravity-reth/pull/444),
[#445](https://github.com/Galxe/gravity-reth/pull/445),
[#446](https://github.com/Galxe/gravity-reth/pull/446),
[#450](https://github.com/Galxe/gravity-reth/pull/450),
[#451](https://github.com/Galxe/gravity-reth/pull/451) and
[#453](https://github.com/Galxe/gravity-reth/pull/453): 98 changed files before the
installer, including committed-block RPC replay. The dependency update therefore
covers upstream integration as well as the installer.

The original minimal lock update changes four dependency edges without package
additions/removals/version drift. Keep Gaptos upstream `a64f8ad` and locked Alloy
2.0.5 / primitives 1.6.0. This source checkpoint pins the simplified Reth revision
`1bb275f0a085461560de6773b4257abc024773af` consistently. The actual locked SDK node
requires a fresh Dev build and verification.

`GravityChainSpecParser` preserves chain `7771625` genesis extra fields and calls
Reth's shared strict validator. Execution validation remains authoritative for
consumers bypassing the parser. Existing mainnet `127001` overrides and
unconfigured/non-target behavior remain covered. Provider/task-engine and default
forward-sync behavior are unchanged. No SDK database schema or contract ABI changes.

## Dev verification and binary handoff

All executable checks run on Gravity Dev in the isolated, resource-limited task
builder. Commands use the actual SDK lock and Rust 1.93.0; standalone Reth's
Rust 1.94.1 / Alloy 2.4.1 graph is a separate verification boundary.

Verification at this source checkpoint is pending. Subsequent Dev check,
release and E2E outcomes are recorded in the primary cross-repository record
and linked PR bodies.

```bash
cargo test --locked -p gravity_node oracle_task_install
cargo test --locked -p gravity_node
CARGO_INCREMENTAL=0 RUSTFLAGS='--cfg tokio_unstable -C link-arg=-fuse-ld=mold' cargo clippy --locked --all-targets --all-features -- -D warnings
cargo +nightly-2026-09-05 fmt --all -- --check
RUSTFLAGS='--cfg tokio_unstable' cargo build --locked -p gravity_node --release -j 2
```

| Scope | Historical SDK `74aaeba` result |
| --- | --- |
| Locked identity / parser / full node | Consistent; 4 parser and 55 full-node tests passed |
| Active CI Clippy / pinned formatting / locked release | Passed with recorded flags |
| New Reth pin, simplified source checks and release | Pending at this source checkpoint; subsequent outcomes live in the primary record and linked PR bodies |
| New candidate's affected E2E | Pending at this source checkpoint; Mono owns runtime acceptance |

SDK's workspace test workflow is disabled (`if: false`); it was not executed.
The passing active Clippy command does not imply all raw log lines were warning-free.
Historical parser/node/Clippy/fmt outcomes bind their captured source snapshot;
release provenance binds its actual inputs. Later edits cannot relabel those results.

The `mono-grav.candidate-provenance/v1` manifest binds full SDK/Reth/Gaptos
revisions, source/Cargo.lock hashes, builder image, toolchain, command/flags and ELF.
`GIT_SHA`, `GIT_BRANCH`, `GIT_TAG`, `BUILD_DATE` overrides must be absent during
build and runtime; actual `/proc/PID/exe` must match. Release includes three generated
Gaptos bindings; build-clean metadata does not describe that overlay. Captured-source
byte verification and original inventory/generation validation are distinct.

The [immutable original SDK record](https://github.com/Galxe/gravity-sdk/blob/74aaeba9a10752296eebb0943e410b95d3f85cf2/docs/changelogs/2026-10-08-oracle-task-install-integration.md)
retains exact snapshots, failed attempts and build evidence under Dev
`/home/ubuntu/issue131-build-20261008T1323Z/`. Reth broad-lint failures and incomplete
workspace runs retain their owning limits. Dev PASS does not establish hosted
required checks or approval. No activation/deployment is selected.
