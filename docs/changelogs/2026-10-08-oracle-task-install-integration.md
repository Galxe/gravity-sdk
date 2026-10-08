# chore: integrate the timestamp bridge-task installer

Status: SDK integration and locked parser/unit/Clippy/format checks complete;
final candidate release and five-mode E2E remain pending in mono-grav.
The user authorized isolated Gravity Dev build/test execution and parallel agents.

## 1. Background and Current State

For [mono-grav Issue #131](https://github.com/Galxe/mono-grav/issues/131), SDK
main `3b982931f887f245bfbccf3329e0a025593427cb` still pins greth
`41e1c20c665def93c3484230992f153ed93f1f06`. It enables forward epoch sync by
default and retains gaptos `a64f8adc274bf2681df796766ef9a5b195fee44b`.

## 2. Problem Model and End-to-End Behavior

SDK must integrate the actual greth installer consistently through the node
manifest, root patch and lockfile, then produce a candidate node binary on
Gravity Dev with source/lock/toolchain/ELF provenance. Natural epoch adoption
and authenticated block replay should retain their existing implementations.
No local product tests or build will substitute for the requested Dev run.

## 3. Research, Findings, and Architecture Decision

The primary workspace architecture record is
[gravity-reth's installer changelog](https://github.com/Galxe/gravity-reth/blob/0c0bfd023a5b4c4a52721e0a797ac89f52663c11/docs/changelogs/2026-10-08-oracle-task-install-hardfork.md).
SDK pins this exact implementation commit; cross-repository PR links belong in
the PR body.

Activation uses the proposed independent `oracleTaskInstallTime` JSON key,
in Unix seconds, gated to 7771625 with explicit frozen installation parameters.
Forward epoch recovery sends historical blocks through execution; retain the
default mode and prove installer execution at H in runtime evidence.
Candidate provenance must not replace the fixed pristine binaries used by the
existing four-mode baseline regression suite. Build prerequisites are confined
to a task-specific, resource-limited Dev builder.

## 4. Implementation Design

Update the node's greth dependency, the root `reth-primitives-traits` patch,
and every corresponding Cargo.lock git source to one exact installer commit.
Keep gaptos `a64f8adc274bf2681df796766ef9a5b195fee44b` and reject unrelated
lock drift. Unpublished greth commits may be imported into an isolated Dev
bare mirror from a Git bundle; Cargo's manifest URL and exact commit stay
unchanged. This is source transport, not an alternate dependency identity.

The SDK's old greth revision predates current greth main. Advancing from
`41e1c20` to installer-on-main also integrates seven upstream commits
(Galxe/gravity-reth#440, Galxe/gravity-reth#444, Galxe/gravity-reth#445,
Galxe/gravity-reth#446, Galxe/gravity-reth#450, Galxe/gravity-reth#451 and
Galxe/gravity-reth#453; 98 changed files before the installer), including committed-block RPC replay and test changes. Candidate
verification covers this upstream integration as well as the installer; the
resulting binary is not described as an installer-only delta.

`GravityChainSpecParser` currently changes only the mainnet 127001 Alpha,
Beta, Prague and Osaka fields. It preserves target chain 7771625 custom
genesis fields. It now calls greth's shared installer-config validator at
startup, with parser regression tests, so invalid target-chain inputs fail
before node startup. Execution-side validation remains
authoritative for consumers that bypass the SDK parser. No provider/task-engine
runtime or forward-sync flag changes are made.

Build only on Gravity Dev in
`/home/ubuntu/issue131-build-20261008T1323Z`; local work is source inspection,
editing, Git and transfer. Builder prerequisites follow SDK
`docker/gravity_node/Dockerfile`, Rust 1.93.0, in a dedicated Docker container
limited to 4 CPU and 16 GiB with 16 GiB total memory plus swap. Cargo uses two
jobs and dedicated `cargo-home`. SDK parser/unit tests use `target/sdk-unit`,
CI Clippy uses `target/sdk-clippy`, and release uses `target/sdk`; standalone
reth uses `target/reth`. Profiles and debug information remain unchanged.
Reclaim only reviewed task-owned caches after retaining evidence and confirming
no live references.

Bind `mono-grav.candidate-provenance/v1` to exact SDK/reth/gaptos commits,
Cargo.lock/tree SHA256, immutable builder image ID, rustc/cargo versions,
`RUSTFLAGS="--cfg tokio_unstable"`, locked release command and ELF SHA256.
`GIT_SHA`, `GIT_BRANCH`, `GIT_TAG` and `BUILD_DATE` must be absent when reading
build information and starting nodes. Actual `/proc/PID/exe` identity/hash
must match the candidate; `--version` is supplementary evidence.

## 5. Verification

E2E Required: yes, owned by mono-grav's installer changelog. All commands run
on Gravity Dev. Build command:

```sh
RUSTFLAGS="--cfg tokio_unstable" cargo build --locked -p gravity_node --release -j 2
```

Run focused SDK chainspec parser tests against the same locked source and
builder, then the node's full unit suite. The applicable Rust CI check is
`cargo clippy --locked --all-targets --all-features -- -D warnings`, with
`CARGO_INCREMENTAL=0`, two Cargo jobs and
`RUSTFLAGS="--cfg tokio_unstable -C link-arg=-fuse-ld=mold"`. Its Mold tool is
confined to the isolated tool mount; the release command retains the flags
above. The dependency check must retain the public greth URL and verify the
node revision, root patch and lock sources agree exactly. Final formatting
uses `cargo +nightly-2026-09-05 fmt --all -- --check` on Dev.

SDK's workspace unit-test workflow is explicitly disabled (`if: false`);
its documented command is `cargo test --workspace --exclude smoke-test`.
No `make lint-fix` target exists. Do not treat its absence as absence of the
active Clippy check, or report the disabled workspace suite as passed.
Downstream coverage includes greth installer/Gamma tests,
changed execution consumers, the new installer four-node flow, and the existing
four modes using the final candidate in the unconfigured-installer stages.
The mono-grav owner records exact final test commands, fixture bindings,
independent verification and selected A-restore exclusion, if applicable.

Protect all seven current persistent process identities throughout every job.
Four persistent cores must retain common-block agreement, bridge nonce 49,
progress `[49,1190]`, recipient balance 1225 and grow; PFN must grow independently.
The legacy VFN and Anvil retain their own unchanged static endpoints and
identities, matching the established persistent recovery health contract.
Never require legacy VFN to agree with the new four-core chain. Old failed
guards/directories remain untouched. Stop only owned jobs on guard/resource
failure and retain the new failed evidence.

## 6. Human Decisions and Implementation Checklist

The parent task reports reviewed schema/runtime and E2E plans and authorizes
implementation, source transport and isolated Dev builds/tests. No real
Testnet/Mainnet deployment or protected-service mutation is authorized.

- [x] Read Issue131, SDK manifests/Dockerfile, custom chainspec parser and guards.
- [x] Establish separate current protected-process baseline after stability,
  core semantics and growth checks; preserve historical failed baselines.
- [x] Finish immutable issue-specific builder and record toolchain/image provenance.
- [x] Integrate exact greth commit, shared startup validation and focused tests.
- [x] Regenerate/review the SDK lock without unrelated dependency changes.
- [x] Pass locked parser/full node tests, applicable CI Clippy and pinned fmt on Dev.
- [x] Complete narrow owning source review and preserve the final-tree check evidence.
- [ ] Build final candidate, emit manifest and hand off exact artifact to E2E.
- [ ] Record final release/protection/ELF and E2E acceptance in mono-grav/PR evidence.

## 7. Current Evidence

SDK manifests, root patch and all 113 git lock sources pin reth
`0c0bfd023a5b4c4a52721e0a797ac89f52663c11`. Remote locked metadata resolves
that exact commit and unchanged gaptos. Final Cargo.lock SHA256 is
`abc33e63379750e188c7d2276f837ce314a660243ba0438f8bfdd9c51b53e7db`.
The minimal resolver update changes four dependency edges, without adding,
removing or changing package versions or unrelated git revisions.
SDK's locked Alloy 2.0.5/primitives 1.6.0 combination now compiles and passes
the node tests and applicable Clippy command on Rust 1.93.0. The standalone
reth graph uses Alloy 2.4.1 and remains a separate validation boundary.

| Check | Result | Evidence entry |
| --- | --- | --- |
| Exact dependency identity and locked metadata | Passed; no version or unrelated source drift | `logs/sdk-lock-metadata-02*`; `sources/sdk-lock-evidence/summary.json` and `metadata.json` |
| SDK parser tests | 4 passed; 51 filtered | `logs/sdk-parser-locked-01*`; `cargo test --locked -p gravity_node oracle_task_install` |
| Full SDK node unit target | 55 passed, including the 4 parser tests; no failures or ignored tests | `logs/sdk-gravity-node-full-01*`; `cargo test --locked -p gravity_node` |
| SDK CI Clippy | Passed with original CI flags and unchanged profiles | `logs/sdk-clippy-ci-01*`; command/environment in §5 |
| SDK final formatting | Passed on pinned nightly; no Rust source changes | `logs/sdk-fmt-check-01*`; command in §5 |
| SDK locked release and ELF provenance | Pending fixed SDK commit; tracked by mono-grav and PR evidence | `target/sdk`; `artifacts/candidate/provenance.json` |
| Standalone reth parser/EVM/real staged and epoch installer | Behavioral tests passed; broad check failure boundaries retained by reth owner | Reth changelog and per-job `logs/*-{source,outcome}.json` |
| Standalone reth CI unit/integration groups | 2487 and 359 tests passed respectively | `logs/reth-nextest-ci-{unit,integration}-01*` |
| Standalone engine E2E/Gravity pipe/required docs | 21 and 2 tests passed; 150 private documentation entrypoints generated | `logs/reth-nextest-ci-engine-e2e-01*`, `logs/reth-nextest-ci-gravity-pipe-01*`, `logs/reth-required-docs-01*`; explicit CI exclusions remain |
| Final candidate five-mode E2E | Pending; no runtime pass is claimed here | mono-grav owns runtime assertions, archives and independent acceptance |

SDK parser, full-node, Clippy and fmt checks bind source snapshot SHA256
`f0c06825cc54a5d8ea8af9e8e75da26c1f3a113d94a17478e3b0f4ec401be376`.
Only this outcome documentation is updated after those checks; Rust source,
manifests and Cargo.lock remain byte-identical. Both node test runs and Clippy
finished with exit 0, no supervisor failure, no container OOM and healthy
post-job protection. Existing build-script/future-incompatibility warnings
remain in logs; Clippy success is the recorded command outcome, not a claim
that every raw log line was warning-free. The disabled workspace test workflow
was not executed and is not reported as passed.

All evidence is under
`gravity-dev:/home/ubuntu/issue131-build-20261008T1323Z`. Each product job binds
its exact source snapshot, builder image, command/environment, target directory,
container result and protection samples. Long jobs use persistent systemd
supervision and stop only the label-checked owned container. Integration tests
use Docker's independent bridge network; heavy jobs remain serial.

SDK's initial immutable image is
`sha256:99cc98e6500bd9db7c6ff38e673e9c74b2f96e129cbf076a1be4357ce41e0d4b`,
Rust 1.93.0/Cargo 1.93.0, recorded in `builder-final-manifest.json`.
Standalone reth uses image
`sha256:32c3006127a0d5912e0b93258ad2ed8baed4669dfbcf6f0ac95e31bce65fa193`,
Rust 1.94.1/Cargo 1.94.1 (`builder-reth-manifest.json`), because the initial
`reth-parser-initial` job failed the actual Alloy 2.4.1 MSRV check before tests.
The final candidate manifest must record the compiler actually used for SDK.
The dedicated Mold tool serves the CI Clippy flags. Release retains the stated
RUSTFLAGS and original profile, with no additional Mold selection; build records
identify the actual linker configuration.

The separate current protected baseline is `protected-baseline.json`, SHA256
`c39d627f204914d477c001cea6abfdc1fa873bbdc815296b6518ea4305e6a5d1`.
Three initial samples over 40 seconds retained all seven identities/core state
and static legacy endpoints; core height grew 181 and PFN grew 830. Current PFN
PID 147562/start ticks 206110978 is distinct from the retained October 8 failed
guard baseline. The initial Issue131 guard's incorrect VFN/core comparison and
its corrected sampling evidence also remain preserved. Host reserves are at
least 24 GiB available RAM and 60 GiB disk; full host swap is unchanged.

Failure evidence remains immutable. Reth's changelog indexes compiler/fixture
repairs, independently reproduced baseline stage and lint failures, and static
attribution boundaries. `reth-nextest-workspace-01` stopped at the disk reserve
before tests; `reth-nextest-workspace-02` was deliberately stopped after partial
execution because the documented standard-engine and Gravity-pipe tests require
opposite environment settings. Neither is a full workspace pass. Subsequent CI
groups retain their feature sets, exclusions and separate results; they do not
silently replace the original required-command failure. Cache recovery audits
retain exact approved paths, ELF hashes, references and unchanged job evidence;
all historical guards, failed attempts, source and E2E data remain untouched.

## 8. Final Owning Review and Candidate Handoff

The narrow SDK review confirms one public greth URL/exact revision across the
node manifest, root patch and lock; unchanged gaptos/Alloy versions; preserved
7771625 genesis extra fields; startup use of the shared validator; and four
focused parser regressions. Other chain configuration remains ignored by the
installer validator, and the existing mainnet Alpha override remains covered.
No task-engine, provider, forward-sync flag or execution implementation is
changed in SDK. Parent source review gates the final commit and release build.

The SDK commit is fixed before the locked release build. Subsequent release,
actual `/proc/PID/exe` hash verification and five-mode E2E results are recorded
in mono-grav's changelog, artifacts and linked PR bodies; they do not require
changing SDK source or this changelog after the candidate is built. Rebuilding
is required if a later SDK source commit is needed.

The candidate export represents tracked-file contents plus original Git link
and submodule metadata. Internal file symlinks are materialized as regular
bytes; directory links and gitlinks are declared without recursive traversal.
Stage-0 coverage and actual SDK Cargo Reth/Gaptos checkout identity/cleanliness
are onsite exporter evidence. The frozen independent archive verifier checks
captured source bytes and digests, not original Git inventory completeness.
The exporter also binds the actual release target and cargo-home container
mounts, locked metadata, toolchain/image, source pins and candidate ELF hash.

Retained failures include `sdk-lock-metadata-01` (the initial SDK image lacked
Python, before Cargo execution) and the SDK unit-cache audit's final assertion
about the not-yet-created Clippy directory. The approved cache removal was
confined to `target/sdk-unit`; its exact tested ELF/provenance and 38 evidence
files remain unchanged. `logs/cache-sdk-unit-removal-audit-failure.json` and
`logs/cache-sdk-unit-approved-removal.json` preserve the failure, recovery and
explicit disk-observation limits. No failure was rewritten as a passing attempt.
