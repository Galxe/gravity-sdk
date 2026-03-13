# implement_sentinel_monitoring

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 111866ms
- **Steps**: 1

## Report

# Sentinel Binary — Implementation Analysis

## Overview

The sentinel is a log-monitoring daemon that tails log files via glob patterns, matches error lines against regex patterns, checks a whitelist for frequency-based suppression, and dispatches alerts to Feishu/Slack webhooks with priority-level routing (P0/P1/P2). An optional HTTP health probe runs in a background task.

---

## Files Involved

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, CLI arg parsing, orchestration loop |
| `config.rs` | TOML config deserialization, priority-level webhook resolution |
| `notifier.rs` | Webhook HTTP dispatch (Feishu + Slack), per-priority rate limiting |
| `watcher.rs` | Glob-based file discovery with recency filtering |
| `whitelist.rs` | CSV-loaded regex rules with frequency-based alert suppression |
| `analyzer.rs` | Single regex filter for identifying error lines |
| `probe.rs` | Periodic HTTP health check with failure-threshold alerting |
| `reader.rs` | Thin wrapper around `linemux::MuxedLines` for tailing files |

---

## Execution Path

### 1. Startup (`main.rs:23-60`)

1. `env_logger::init()` — initializes logging from `RUST_LOG` env var.
2. `env::args().collect()` — collects CLI args into `Vec<String>`. If `args.len() < 2`, prints usage and exits with code 1. No further validation of `args[1]` beyond passing it to `Config::load()`.
3. `Config::load(config_path)` — reads the TOML file from disk (`fs::read_to_string`), deserializes via `toml::from_str`.
4. If `config.monitoring.whitelist_path` is `Some`, loads `Whitelist::load(path)`. Otherwise uses `Whitelist::default()` (empty rules).
5. Constructs `Watcher`, `Reader`, `Analyzer`, `Notifier`.
6. `notifier.verify_webhooks()` — sends a startup message to every configured webhook URL. Fails hard (via `?` + `context()`) if any webhook is unreachable.
7. If `config.probe` is `Some`, spawns `Probe::run()` in a detached `tokio::spawn`.
8. Calls `watcher.discover()` for initial file set, then `reader.add_file()` for each.

### 2. Main Loop (`main.rs:75-125`)

Uses `tokio::select!` with two branches:

- **Event-driven branch**: `reader.next_line()` returns a `linemux::Line`. Calls `analyzer.is_error(line)` — if false, skips. If true, calls `whitelist.check(line)` which returns one of:
  - `Skip` — below threshold, no alert.
  - `Alert { count, priority }` — above threshold, sends alert at the rule's priority.
  - `AlwaysAlert` — no whitelist rule matched, sends alert at `Priority::P0`.

- **Periodic branch**: `interval.tick()` triggers `watcher.discover()` to find new files matching glob patterns and adds them to the reader.

---

## Key Functions

### `config.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Config::load` | `fn load<P: AsRef<Path>>(path: P) -> Result<Self>` | `fs::read_to_string(path)` then `toml::from_str(&content)`. No schema validation beyond serde derive. No URL format validation on webhook fields. |
| `AlertingConfig::get_webhooks` | `fn get_webhooks(&self, priority: Priority) -> (Option<&str>, Option<&str>)` | Looks up `self.priorities[priority]`; if found, uses its feishu/slack URLs with fallback to top-level defaults. Returns `(feishu, slack)` tuple. |
| `AlertingConfig::all_webhooks` | `fn all_webhooks(&self) -> Vec<(&str, &str)>` | Collects all unique non-empty webhook URLs across default + per-priority configs. Uses `HashSet` for dedup. |
| `default_min_alert_interval` | `fn default_min_alert_interval() -> u64` | Returns `5` (seconds). Used as `#[serde(default)]` for `min_alert_interval`. |

**Config structs deserialized from TOML:**
- `GeneralConfig`: `check_interval_ms: u64`
- `MonitoringConfig`: `file_patterns: Vec<String>`, `recent_file_threshold_seconds: u64`, `error_pattern: String`, `whitelist_path: Option<String>`
- `AlertingConfig`: `feishu_webhook: Option<String>`, `slack_webhook: Option<String>`, `min_alert_interval: u64`, `priorities: HashMap<Priority, PriorityAlertConfig>`
- `ProbeConfig`: `url: String`, `check_interval_seconds: u64`, `failure_threshold: u32`
- `Priority` enum: `P0`, `P1`, `P2` with case-insensitive serde aliases (`p0`/`P0`)

### `notifier.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Notifier::new` | `fn new(config: AlertingConfig) -> Self` | Creates `reqwest::Client::new()` (no TLS config, no redirect policy, no timeout). Initializes empty `HashMap<Priority, Instant>` behind `Arc<Mutex>`. |
| `Notifier::send` | `async fn send(&self, text: &str, priority: Priority) -> Result<()>` | Gets webhooks via `config.get_webhooks(priority)`. POSTs JSON to each non-empty URL. Feishu payload: `{"msg_type":"text","content":{"text":...}}`. Slack payload: `{"text":...,"channel":"#alerts-devops","username":"System-Monitor"}`. Channel is hardcoded. |
| `Notifier::verify_webhooks` | `async fn verify_webhooks(&self) -> Result<()>` | Iterates `config.all_webhooks()`, POSTs a startup message to each. Bails if list is empty or any POST fails. |
| `Notifier::alert` | `async fn alert(&self, message: &str, file: &str, priority: Priority) -> Result<()>` | Rate-limits per priority: acquires `Mutex`, checks if `now - last_alert_time[priority] < min_alert_interval`. If within window, returns `Ok(())` silently. Otherwise records timestamp and calls `self.send()`. Errors from `send()` are logged to stderr but swallowed (`Ok(())` always returned). |

**Rate limiting details**: Uses `std::sync::Mutex` (not `tokio::Mutex`) in async context. The lock is held only briefly (check + insert) before being dropped, then `send()` is called outside the lock. Rate limiting is per-priority-level globally, not per-message-content — a P0 alert for one error suppresses all other P0 alerts within the interval.

### `watcher.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Watcher::new` | `fn new(config: MonitoringConfig) -> Self` | Stores config and initializes empty `HashSet<PathBuf>` for known files. |
| `Watcher::discover` | `fn discover(&mut self) -> Result<Vec<PathBuf>>` | Iterates `config.file_patterns`, calls `glob::glob(pattern)` for each. For each matched path, calls `should_monitor()` and checks `known_files.insert()` for dedup. |
| `Watcher::should_monitor` | `fn should_monitor(&self, path: &Path, now: u64) -> bool` | Calls `fs::metadata(path)` — this follows symlinks (not `fs::symlink_metadata`). Checks if the file's `modified` time is within `recent_file_threshold_seconds` of `now`. |

**Glob patterns** come directly from the TOML config `file_patterns: Vec<String>` and are passed to `glob::glob()` without sanitization.

### `whitelist.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `WhitelistRule::new` | `fn new(pattern_str: &str, threshold: i32, priority: Priority) -> Result<Self>` | Attempts `Regex::new(pattern_str)`. On failure, falls back to `Regex::new(&regex::escape(pattern_str))` (literal match). Initializes empty `VecDeque<Instant>` for timestamp tracking. |
| `WhitelistRule::matches` | `fn matches(&self, line: &str) -> bool` | Calls `self.pattern.is_match(line)`. |
| `Whitelist::load` | `fn load<P: AsRef<Path>>(path: P) -> Result<Self>` | Opens CSV file with `has_headers(false)`, `comment(Some(b'#'))`, `flexible(true)`. Parses columns: `pattern, threshold [, priority]`. Skips records with `< 2` fields or empty pattern. Priority defaults to `P0` if missing or unrecognized. |
| `Whitelist::check` | `fn check(&mut self, line: &str) -> CheckResult` | Iterates rules in order, returns on first match. If `threshold == -1`, returns `Skip` (always suppress). Otherwise maintains a sliding window (`VecDeque`) of timestamps within `WINDOW_SECONDS` (300s = 5 min). If count > threshold, returns `Alert { count, priority }`. If count <= threshold, returns `Skip`. If no rule matches, returns `AlwaysAlert`. |

**Threshold type**: `threshold` is `i32` but is compared as `u32` on line 148: `count > rule.threshold as u32`. A negative threshold other than `-1` (e.g., `-2`) would cast to a large `u32` value (4294967294), effectively making the rule never trigger an alert. Only `-1` is explicitly handled.

### `analyzer.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Analyzer::new` | `fn new(error_pattern: &str) -> Result<Self>` | Compiles `error_pattern` from config via `Regex::new()`. Returns error if pattern is invalid regex. |
| `Analyzer::is_error` | `fn is_error(&self, line: &str) -> bool` | Returns `self.error_regex.is_match(line)`. |

### `probe.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Probe::new` | `fn new(config: ProbeConfig, notifier: Notifier) -> Self` | Creates `reqwest::Client` with 10-second timeout. Falls back to `Client::new()` (no timeout) if builder fails. |
| `Probe::run` | `async fn run(self)` | Infinite loop: sends HTTP GET to `config.url` every `check_interval_seconds`. Increments `failures` counter on non-2xx or network error. When `failures >= failure_threshold`, sends P0 alert via `notifier.alert()` and resets `failures = 0`. On recovery (success after failures), resets counter and logs. |

**Probe URL** comes from `config.probe.url` (a `String`), passed directly to `self.client.get(&self.config.url)`. No URL validation or scheme restriction.

### `reader.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Reader::new` | `fn new() -> Result<Self>` | Creates `linemux::MuxedLines::new()`. |
| `Reader::add_file` | `async fn add_file(&mut self, path: impl Into<PathBuf>) -> Result<()>` | Delegates to `self.lines.add_file(path)`. `linemux` uses inotify/kqueue to watch for file changes and streams new lines. |
| `Reader::next_line` | `async fn next_line(&mut self) -> Option<Line>` | Returns `self.lines.next_line().await.ok().flatten()`. Errors are silently dropped (converted to `None` via `.ok()`). |

---

## State Changes

| Component | State | Mutability |
|-----------|-------|------------|
| `Watcher::known_files` | `HashSet<PathBuf>` — grows as new files are discovered, never shrinks | `&mut self` |
| `WhitelistRule::timestamps` | `VecDeque<Instant>` — sliding window, entries added on match and expired entries removed from front | `&mut self` via `Whitelist::check` |
| `Notifier::last_alert_times` | `Arc<Mutex<HashMap<Priority, Instant>>>` — stores last alert time per priority level, updated on each non-rate-limited alert | Shared via `Arc<Mutex>` |
| `Probe::failures` | Local `u32` counter — incremented on failure, reset on recovery or threshold trigger | Local variable in `run()` |
| `Reader::lines` | `MuxedLines` — internal file watch state, files added but never removed | `&mut self` |

---

## External Dependencies

| Crate | Version | Usage |
|-------|---------|-------|
| `tokio` | 1.0 (full) | Async runtime, timers, select, spawn |
| `reqwest` | 0.11 (json) | HTTP client for webhook POSTs and probe GETs |
| `serde` / `serde_json` | workspace | JSON payload construction, config deserialization |
| `toml` | 0.8 | TOML config file parsing |
| `regex` | 1.10 | Error pattern matching, whitelist pattern matching |
| `glob` | 0.3 | File discovery by glob pattern |
| `linemux` | 0.3 | File tailing (inotify/kqueue-based line-by-line streaming) |
| `csv` | workspace | Whitelist CSV file parsing |
| `sha2` / `hex` | — | Listed in Cargo.toml but **not used** in any source file |
| `chrono` | — | Listed in Cargo.toml but **not used** in any source file |
| `anyhow` | — | Error handling throughout |
| `log` / `env_logger` | — | Logging initialization; actual output uses `println!`/`eprintln!` |

---

## Data Flow Summary

```
TOML Config File
      │
      ▼
  Config::load() ──→ MonitoringConfig ──→ Watcher (glob patterns)
      │                                        │
      │                                        ▼
      │                                  glob::glob() ──→ PathBuf list
      │                                        │
      │                                        ▼
      │                                  Reader::add_file() ──→ linemux tailing
      │
      ├──→ AlertingConfig ──→ Notifier (webhook URLs, rate limit config)
      │
      ├──→ error_pattern ──→ Analyzer (compiled Regex)
      │
      ├──→ whitelist_path ──→ Whitelist::load() (CSV → Vec<WhitelistRule>)
      │
      └──→ ProbeConfig ──→ Probe (health check URL, threshold)

Main Loop:
  linemux line event
      │
      ▼
  Analyzer::is_error() ──[false]──→ skip
      │ [true]
      ▼
  Whitelist::check() ──→ Skip | Alert{count,priority} | AlwaysAlert
      │                    │              │
      │                [skip]    [send alert with rule priority]
      ▼
  Notifier::alert(msg, file, priority)
      │
      ▼
  Rate limit check (per-priority, min_alert_interval seconds)
      │ [pass]
      ▼
  Notifier::send() ──→ HTTP POST to Feishu/Slack webhook URLs
```

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Sentinel Binary — Implementation Analysis

## Overview

Th | 111866ms |
