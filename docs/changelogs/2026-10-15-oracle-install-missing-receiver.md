# Testnet installation from a missing Bridge Receiver

The cross-repository design and acceptance flow are recorded in
[`mono-grav`'s primary changelog](../../../mono-grav-bridge119/docs/changelogs/2026-10-15-oracle-install-missing-receiver.md).
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
- [ ] S2: Record locked Cargo metadata and successful release build.
- [ ] S3: Export release and source provenance.
- [ ] S4: Record both required final-tree E2E outcomes.
- [ ] S5: Root reviews final diff and evidence; retain the local-only boundary.

## 7. Outcome

Final Reth revision: `b84b33c58e7db2566a8115e31efba5943d660183`.
Gravity Dev Cargo 1.93.0 passed locked offline metadata resolution.
The lock SHA256 is
`af2ecde2d634f84dcfd93f3ad0a9f680d109fe10305f12385276a30cae34be86`;
only the Reth source revision strings changed. Release and E2E evidence are
pending.

## 8. Remaining work

This run prepares and verifies local release candidates. Publication and
deployment remain later work; the mainnet PFN remains untouched.
