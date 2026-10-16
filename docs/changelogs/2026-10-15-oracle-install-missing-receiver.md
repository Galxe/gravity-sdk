# Integrate the combined testnet bridge hardfork

## 1. Background

[Issue #131](https://github.com/Galxe/mono-grav/issues/131) installs a missing
Bridge Receiver, the callback-failure NativeOracle fix, and the Oracle task/default
callback together at a scheduled hardfork. SDK integrates the Reth implementation;
[Mono's guide](https://github.com/Galxe/mono-grav/blob/codex/issue131-oracle-install-e2e/docs/changelogs/2026-10-15-oracle-install-missing-receiver.md)
owns the complete design, acceptance evidence and reproduction procedure.

## 2. SDK behavior

`GravityChainSpecParser` reads the chain configuration and calls Reth's shared
`oracle_task_install_config` validator before startup completes. On testnet chain
`7771625`, malformed present installer configuration fails startup. With both
installer keys absent, or on another chain ID, this validation returns no installer
configuration. Existing mainnet hardfork overrides remain unchanged by this PR.

The parser preserves the existing genesis extra fields. It performs no contract
installation or state migration; Reth executes the transition at the first block H
whose timestamp crosses the configured installation time. Gamma is a prerequisite
and is staged in the same overlay if H first crosses both fork boundaries.

## 3. Integration

Both Cargo manifests and all 113 Reth git lock sources select
`f028d188e207a5963af38120e76f7f406e563d6d`. This advances the previous SDK pin
`41e1c20` through Reth upstream `5235476` and the installer changes in
[Reth #457](https://github.com/Galxe/gravity-reth/pull/457), including upstream
committed-block RPC replay changes. Gaptos remains `a64f8ad`; locked Alloy 2.0.5
and primitives 1.6.0 are retained. There is no SDK database schema migration.

## 4. Configuration

Set integer Unix-seconds `oracleTaskInstallTime` and an `oracleTaskInstall` object
containing exactly `sourceId`, `taskName`, `taskConfig`, `receiverCodeHash` and
`trustedBridge`. Reth validates their encoding, bounds and nonzero identifiers,
and requires installation after genesis and `gammaTime`.

Activation time stays configurable for isolated testing. This PR does not add
SDK #843's fixed fork schedule. Public-network values require a separate freeze;
the parser tests' timestamps and addresses are fixtures, not deployment inputs.

## 5. Verification commands

Run on Gravity Dev with the recorded Rust 1.93 builder and isolated target:

```bash
cargo metadata --locked --offline --format-version 1
cargo test --locked -p gravity_node oracle_task_install
RUSTFLAGS="--cfg tokio_unstable" cargo build --locked -p gravity_node --release -j 4
```

For E2E, follow Mono's guide for the focused four-message callback flow and the
twelve-message installation/replay/recovery flow, once each with the selected
binary. The guide supplies prerequisites, isolated staging, exact invocation,
source-receipt checks, independent archive verification and cleanup.

## 6. Scope and ownership

Merge order is [Contracts #119](https://github.com/Galxe/gravity_chain_core_contracts/pull/119)
→ [Reth #457](https://github.com/Galxe/gravity-reth/pull/457)
→ [SDK #844](https://github.com/Galxe/gravity-sdk/pull/844)
→ [Mono #132](https://github.com/Galxe/mono-grav/pull/132).
Contracts owns callback semantics, Reth owns execution, SDK owns integration, and
Mono owns cross-repository acceptance. All builds and tests run on Gravity Dev.

## 7. Verified outcome

SDK product revision `4412093c7cb48c39843c2b16ab19eb5926c06ba1` passed locked/offline
metadata resolution and the locked release build with the Reth pin above. The
verified ELF SHA256 is
`d6b6930e8a6f4301314de37e9e54d89387070ef16ab9ca1aba1076bae4d880ec`.
Later commits, including this consolidation, change documentation only.

Both missing-Receiver flows passed once with that ELF: focused H1220 consumed four
messages, recorded two callback failures and minted only the two valid messages
(balance +13); full H457 completed twelve messages, original-database replay and
a second natural-epoch recovery. All four replicas ended at nonce 12 and balance
138, followed by a 240.407-second / 1081-block observation. Source-receipt reload,
independent/archive verification and cleanup passed. Mono retains the exact
source/binary bindings, archive hashes and commands.

The four parser tests and 55 full-node tests passed on earlier SDK revisions;
full-node tests and Clippy also passed on SDK `2213742`. Those are historical
results, not fresh unit/Clippy runs for `4412093`. This documentation consolidation
does not require a new build or repeat of the unchanged product's E2E.

## 8. Remaining work

Hosted required checks and review remain merge gates. Release publication and
public activation are separate steps. The callback batch-gas budget review remains
open in Contracts #119; the randomness recovery issue SDK #845 remains separate.
