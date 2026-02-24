# Security Audit Report — gravity-sdk

**Date:** 2026-02-23
**Scope:** Gravity-specific code in `crates/api/`, `bin/gravity_node/`, `bin/gravity_cli/`, `bin/sentinel/`, `crates/block-buffer-manager/`, `aptos-core/consensus/` modifications
**Repository:** https://github.com/Galxe/gravity-sdk
**Release:** gravity-testnet-v1.0.0

---

## Summary

| Severity | Findings | Status | Commit |
|----------|----------|--------|--------|
| HIGH | 2 | All fixed | [`a0bf499`](https://github.com/Richard1048576/gravity-sdk/commit/a0bf499) |
| MEDIUM | 3 | All fixed | [`a0bf499`](https://github.com/Richard1048576/gravity-sdk/commit/a0bf499), [`21e6898`](https://github.com/Richard1048576/gravity-sdk/commit/21e6898) |
| **Total** | **5** | **All fixed** | |

---

## HIGH Severity (2)

### GSDK-001: Unauthenticated `/set_failpoint` on Plaintext HTTP

**File:** `crates/api/src/https/mod.rs:113`
**Issue:** `/set_failpoint` endpoint on unauthenticated plaintext HTTP router. Calls `fail::cfg()` directly, allowing arbitrary failpoint injection. 14+ failpoints throughout consensus layer (`consensus::send::proposal`, `consensus::process_proposal`, etc.). An attacker with network access to port 8080 can halt block production.
**Fix:** Gated behind `#[cfg(debug_assertions)]` so the route is compiled out in release builds. Added runtime warning on startup if failpoints feature is enabled in a release build.
**Commit:** `a0bf499`

### GSDK-002: Unauthenticated `/mem_prof` on Plaintext HTTP

**File:** `crates/api/src/https/mod.rs:114`
**Issue:** `/mem_prof` endpoint on unauthenticated plaintext HTTP. Triggers jemalloc heap dump that may capture in-memory private keys, DKG transcripts, and session material. Dump file may be world-readable.
**Fix:** Gated behind `#[cfg(debug_assertions)]` same as GSDK-001. Heap dump files now written with `0600` permissions.
**Commit:** `a0bf499`

---

## MEDIUM Severity (2)

### GSDK-003: Consensus/DKG Endpoints on Plaintext HTTP

**File:** `crates/api/src/https/mod.rs:105-112`
**Issue:** DKG randomness, quorum certificates, ledger info, and consensus block data served on plaintext HTTP without TLS. Passive MITM can collect DKG randomness values and aggregate BLS signatures from QCs.
**Fix:** Moved sensitive routes (`/dkg/randomness`, `/consensus/latest_ledger_info`, `/consensus/ledger_info`, `/consensus/block`, `/consensus/qc`) to `https_routes` with TLS enforcement. Only `/consensus/validator_count` (public metadata) remains on HTTP.
**Commit:** `a0bf499`

### GSDK-004: SSRF in Sentinel Probe URL

**File:** `bin/sentinel/src/probe.rs:34`, `bin/sentinel/src/config.rs:14`
**Issue:** Sentinel probe URL accepted without validation. Attacker who modifies `sentinel.toml` can cause sentinel to probe cloud metadata endpoints (`169.254.169.254`) and potentially exfiltrate IAM credentials via webhook alerts.
**Fix:** Added URL validation at `Config::load()` time: scheme allowlist (http/https only), reject reserved IP ranges (loopback, link-local, RFC 1918 private). Sentinel refuses to start if validation fails.
**Commit:** `a0bf499`

### GSDK-005: GSDK-004 Fix DNS Bypass

**File:** `bin/sentinel/src/config.rs:72`
**Issue:** The GSDK-004 fix only checked IP literals against the blocklist. When the host was a DNS hostname (e.g., `metadata.google.internal`, `169.254.169.254.nip.io`), `host.parse::<IpAddr>()` failed and all IP checks were bypassed. Additionally, a userinfo prefix (`http://x@169.254.169.254/`) was not stripped, allowing further bypass.
**Fix:** Resolve DNS hostnames via `ToSocketAddrs` and check all resolved IPs against the blocklist. Reject unresolvable hostnames (fail-closed). Strip userinfo component before extracting the host.
**Commit:** `21e6898`

---

## Commits

| Commit | Description | Files Changed |
|--------|-------------|---------------|
| [`a0bf499`](https://github.com/Richard1048576/gravity-sdk/commit/a0bf499) | GSDK-001/002/003/004: admin routes, plaintext HTTP, sentinel SSRF | 4 files |
| [`21e6898`](https://github.com/Richard1048576/gravity-sdk/commit/21e6898) | GSDK-005: fix DNS bypass in probe URL validation | 1 file |

## Design Documents

- [Security Fixes Design](../plans/2026-02-23-gsdk-fixes-design.md)
