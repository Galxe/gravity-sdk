# GSDK Security Fixes Design

Date: 2026-02-23

## GSDK-001: Unauthenticated `/set_failpoint` Endpoint

**Problem:** `/set_failpoint` is registered on the plaintext HTTP router (`http_routes`) with no authentication middleware. The endpoint calls `fail::cfg()` directly, allowing remote injection of failpoints that control consensus behavior (e.g., `consensus::send::proposal` stops block production). Any attacker with network access to the validator's HTTP port (default 8080) can halt the validator.

**Fix:** Gate the `/set_failpoint` route behind `#[cfg(debug_assertions)]` so it is compiled out in release builds. This ensures the endpoint is only available during development/testing. Additionally, add a runtime warning on startup if the `failpoints` feature is enabled in a non-debug build configuration.

**Alternative considered:** Moving to `https_routes` with bearer token auth. Rejected because failpoints should never be available in production — even with authentication, the risk of credential compromise doesn't justify having the endpoint.

**Files:** `crates/api/src/https/mod.rs`

## GSDK-002: Unauthenticated `/mem_prof` Endpoint

**Problem:** `/mem_prof` is on the same unauthenticated HTTP router. It triggers jemalloc heap profiling via `mallctl("prof.active")` and `mallctl("prof.dump")`. Heap dumps capture all in-memory data including potentially private keys, DKG transcript material, and session tokens. Dump files may be written with default permissions (world-readable).

**Fix:** Same approach as GSDK-001 — gate behind `#[cfg(debug_assertions)]`. Additionally, if profiling is enabled in debug builds, ensure heap dump files are written with `0600` permissions using explicit `fs::set_permissions()` after dump.

**Files:** `crates/api/src/https/mod.rs`, `crates/api/src/https/heap_profiler.rs`

## GSDK-003: Consensus/DKG Endpoints on Plaintext HTTP

**Problem:** Seven routes serve sensitive consensus state over plaintext HTTP without TLS:
- `/dkg/randomness/:block_number` — per-block DKG randomness seeds
- `/consensus/latest_ledger_info` — current epoch state
- `/consensus/ledger_info/:epoch` — epoch transitions with validator sets
- `/consensus/block/:epoch/:round` — full consensus block data
- `/consensus/qc/:epoch/:round` — quorum certificates with aggregate BLS signatures
- `/consensus/validator_count/:epoch` — epoch validator count (public metadata)
- `/dkg/status` — current DKG session status

A passive MITM on the same network segment can collect DKG randomness, QC signatures, and full consensus block data.

**Fix:** Move sensitive routes to `https_routes` which enforces TLS via `ensure_https` middleware. Keep `/consensus/validator_count/:epoch` on HTTP as it's purely public metadata. Update `gravity_cli` DKG subcommands to use HTTPS URLs for these endpoints.

**Files:** `crates/api/src/https/mod.rs`, `bin/gravity_cli/src/dkg.rs`

## GSDK-004: SSRF in Sentinel Probe URL

**Problem:** `ProbeConfig.url` field in `sentinel.toml` is passed verbatim to `reqwest::get()` with no validation of scheme, host, or IP range. If an attacker gains write access to the config file (via compromised CI/CD, overly permissive file ACLs, or shared config management), they can set the probe URL to `http://169.254.169.254/latest/meta-data/iam/security-credentials/` and exfiltrate IAM role names through sentinel's alerting webhooks.

**Fix:** Add URL validation in `Config::load()`:
1. Parse URL with `url::Url::parse()`
2. Scheme allowlist: only `http` and `https`
3. Host IP range check: reject loopback (`127.0.0.0/8`), link-local (`169.254.0.0/16`), and RFC 1918 private ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`)
4. Return hard error and refuse to start on validation failure

**Files:** `bin/sentinel/src/config.rs`, `bin/sentinel/src/probe.rs`
