# attack_sentinel_monitoring

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 184690ms
- **Steps**: 1

## Report

# Sentinel Binary — Security Audit (Red Team Attack Report)

---

## Critical Severity

### 1. SSRF via Unconstrained Webhook and Probe URLs

**Files:** `notifier.rs:37,53,84` / `probe.rs:35` / `config.rs:42,68-72`

No URL validation exists anywhere. Webhook URLs (`feishu_webhook`, `slack_webhook`) and probe URL (`probe.url`) are read from TOML config and passed directly to `reqwest::Client::post()` / `::get()`. An attacker who can modify the config file (or an operator who misconfigures it) can:

- Target internal services: `http://169.254.169.254/latest/meta-data/` (cloud metadata), `http://localhost:8500/v1/agent/self` (Consul), etc.
- Exfiltrate log content to arbitrary endpoints — every alert body contains raw log lines, which may include secrets, tokens, or PII.
- Port-scan internal networks by observing timing/error differences.

Additionally, `reqwest::Client::new()` in `notifier.rs:22` is created with **no timeout, no redirect policy, and no TLS verification config**. This means:
- Infinite hangs on unresponsive internal targets (DoS of the sentinel itself).
- Open redirects can chain SSRF to arbitrary destinations.

**Attack:** Set `feishu_webhook = "http://169.254.169.254/latest/meta-data/iam/security-credentials/"` in TOML. Sentinel will POST log content to the cloud metadata endpoint on every alert cycle.

---

### 2. Log Content Injection into Webhook Payloads (Data Exfiltration / Injection)

**File:** `notifier.rs:109-110`

```rust
let text = format!(
    "🚨 **Log Sentinel Alert** [{priority}] 🚨\nFile: `{file}`\nError:\n```\n{message}\n```"
);
```

Raw, unsanitized log line content is interpolated directly into the webhook payload. This has two consequences:

- **Data exfiltration:** Any secrets accidentally logged (API keys, tokens, database credentials, session IDs) are forwarded verbatim to external webhook services (Feishu/Slack). This violates least-privilege and data minimization principles.
- **Slack/Feishu payload injection:** Crafted log lines containing JSON control characters or Slack mrkdwn formatting (`@here`, `@channel`, `<@U12345>`) can manipulate the alert message, tag arbitrary users, or break JSON payload structure.

---

### 3. Negative Threshold Bypass — Silent Alert Suppression

**File:** `whitelist.rs:125-146`

```rust
if rule.threshold == -1 {
    return CheckResult::Skip;  // Only -1 is guarded
}
// ...
if count > rule.threshold as u32 {  // line 146
```

Only `threshold == -1` is explicitly handled as "always suppress." Any other negative value (e.g., `-2`, `-100`) is cast to `u32`, wrapping to an astronomically large number (e.g., `-2` becomes `4294967294`). This means:

- A rule with `threshold = -2` will **never** trigger an alert because the count can never exceed ~4.3 billion within a 5-minute window.
- An attacker who can edit the whitelist CSV can silently suppress all alerts for a specific error pattern by setting a negative threshold other than `-1`, which looks like a legitimate configuration value to a casual reviewer.

**Attack:** Add line `critical_error_pattern,-2,p0` to whitelist CSV. All matching critical errors are silently swallowed forever. No warning is logged.

---

## High Severity

### 4. Per-Priority Rate Limiting Causes Cross-Error Alert Suppression

**File:** `notifier.rs:97-107`

Rate limiting is keyed **per priority level globally**, not per error message or per file:

```rust
if let Some(last) = times.get(&priority) {
    if now.duration_since(*last) < Duration::from_secs(self.config.min_alert_interval) {
        return Ok(());  // Silently dropped
    }
}
```

**Attack scenario:** An attacker who can write to a monitored log file causes a flood of low-severity P0 errors (e.g., benign-looking "connection reset" messages). Each one resets the P0 rate-limit window. Genuine critical P0 alerts arriving within `min_alert_interval` seconds (default: 5s) of the flood are **silently dropped** — `Ok(())` is returned, no log is emitted, no error propagated. The operator has no visibility that alerts were suppressed.

This is especially dangerous because the default `min_alert_interval` of 5 seconds is short enough to seem safe but long enough for a sustained attacker to block every real alert.

---

### 5. Symlink Following in File Discovery — Arbitrary File Monitoring

**File:** `watcher.rs:41`

```rust
if let Ok(metadata) = fs::metadata(path) {  // follows symlinks
```

`fs::metadata()` follows symlinks (unlike `fs::symlink_metadata()`). Combined with unconstrained glob patterns from config, an attacker who can create symlinks in a monitored directory can:

- Cause sentinel to tail arbitrary files outside the intended log directory (e.g., `/etc/shadow`, application config files containing secrets).
- Those file contents then flow through the alert pipeline and get sent to external webhooks (see #2), achieving **arbitrary file read + exfiltration**.

**Attack chain:** Create symlink `/var/log/app/evil.log -> /etc/environment` in a globbed directory. Sentinel picks it up, tails it, and any line matching `error_pattern` gets sent to the webhook endpoint.

---

### 6. ReDoS via Whitelist Patterns

**File:** `whitelist.rs:32-37`

```rust
let pattern = match Regex::new(pattern_str) {
    Ok(re) => re,
    Err(_) => {
        Regex::new(&regex::escape(pattern_str))?  // fallback to literal
    }
};
```

Whitelist patterns from the CSV file are compiled as regex with no complexity limits, size limits, or timeout. A malicious or poorly-written pattern (e.g., `(a+)+$`) will cause catastrophic backtracking when matched against log lines. Since `whitelist.check()` runs **synchronously on every error log line** in the main tokio task, a ReDoS pattern will:

- Block the entire main event loop.
- Halt all file monitoring, new file discovery, and alert processing.
- Effectively DoS the sentinel — no alerts fire for any error.

The same risk applies to `analyzer.rs:10` where `error_pattern` from config is compiled without safeguards.

---

### 7. `std::sync::Mutex` in Async Context — Potential Deadlock

**File:** `notifier.rs:7,16,98`

```rust
use std::sync::Mutex;
// ...
let mut times = self.last_alert_times.lock().unwrap();
```

A `std::sync::Mutex` is used inside an async function. While the lock is held briefly in this specific code, `lock().unwrap()` will **panic and crash the entire process** if the mutex is poisoned (i.e., if a previous holder panicked). Since `Notifier` is cloned and shared with the `Probe` task (`main.rs:55`), a panic in the probe's alert path poisons the mutex and crashes the main monitoring loop on the next alert attempt.

---

## Medium Severity

### 8. Silent Error Swallowing in Reader

**File:** `reader.rs:21`

```rust
pub async fn next_line(&mut self) -> Option<Line> {
    self.lines.next_line().await.ok().flatten()
}
```

All `linemux` errors (file deletion, permission changes, inotify failures, fd exhaustion) are converted to `None` via `.ok()`. In the main loop (`main.rs:78`), `None` simply means "no line available" — the sentinel silently stops monitoring files that encounter errors with no alert, no log, and no recovery mechanism. An attacker can cause targeted file monitoring to cease by triggering filesystem errors.

---

### 9. Unbounded Memory Growth in Watcher

**File:** `watcher.rs:13,29`

```rust
known_files: HashSet<PathBuf>,  // grows forever, never shrinks
```

`known_files` is append-only. In environments with log rotation (which is the common case for the exact files this tool monitors), old files are removed but never purged from the `HashSet`. Over days/weeks of runtime, this causes unbounded memory growth. More importantly, rotated-away file paths stay in `known_files`, so if a new log file is created with the same path (standard rotation behavior), **it will never be picked up** because `HashSet::insert` returns `false` for existing entries.

**Impact:** After log rotation, sentinel silently stops monitoring the new log file. No alerts fire for new errors. This is a silent monitoring failure.

---

### 10. Hardcoded Slack Channel — Misconfigured Alert Routing

**File:** `notifier.rs:50`

```rust
"channel": "#alerts-devops",
```

The Slack channel is hardcoded. Per-priority webhook routing was explicitly implemented (the whole `priorities` config system), but the channel override is always `#alerts-devops`. If operators configure different Slack webhook URLs expecting messages to route to different channels based on priority, the hardcoded channel override may silently redirect all alerts to the wrong channel (depending on Slack webhook configuration).

---

## Summary

| # | Finding | Severity | File(s) |
|---|---------|----------|---------|
| 1 | SSRF via unconstrained webhook/probe URLs | **Critical** | notifier.rs, probe.rs, config.rs |
| 2 | Log content injection / secret exfiltration via webhooks | **Critical** | notifier.rs |
| 3 | Negative threshold bypass silently suppresses all alerts | **Critical** | whitelist.rs |
| 4 | Global per-priority rate limiting enables cross-error suppression | **High** | notifier.rs |
| 5 | Symlink following enables arbitrary file read + exfiltration | **High** | watcher.rs |
| 6 | ReDoS in whitelist/analyzer patterns DoSes entire sentinel | **High** | whitelist.rs, analyzer.rs |
| 7 | `std::sync::Mutex` poisoning crashes process | **High** | notifier.rs |
| 8 | Silent error swallowing stops monitoring without notice | **Medium** | reader.rs |
| 9 | Unbounded `known_files` growth + post-rotation monitoring failure | **Medium** | watcher.rs |
| 10 | Hardcoded Slack channel defeats priority routing intent | **Medium** | notifier.rs |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Sentinel Binary — Security Audit (Red Team Attack Report)
 | 184690ms |
