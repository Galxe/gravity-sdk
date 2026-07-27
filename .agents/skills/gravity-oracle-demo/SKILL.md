---
name: gravity-oracle-demo
description: Run, demo, or debug the Gravity Binance and Polymarket Oracle E2E suites.
---

# Gravity Oracle Demo

Use `gravity_e2e/docs/ORACLE_E2E.md` as the canonical suite matrix and
runbook. Do not duplicate its commands or revision table here.

## Routing

- Use `oracle_demo` for deterministic review or the combined local frontend.
- Use `polymarket_mock` for three-outcome match settlement and claim behavior.
- Use `binance_price_feed_multivalidator` for explicitly approved live Binance
  validation and voting-power quorum.

## Safety

- Ask before public API or RPC traffic unless the user approved that exact run.
- Never commit credentials, `.env` files, private RPC URLs, user-specific
  absolute paths, generated runtime config, or cluster artifacts.
- Use `--force-init` after changing genesis or a pinned contracts revision.
- Stop clusters started by the current task unless `--keep-running` was
  explicitly requested.

For real Polymarket mirrors, require a reviewed manifest with frozen rules,
condition and question IDs, CTF address, outcome labels, slot mapping, source
block range, and metadata hashes.
