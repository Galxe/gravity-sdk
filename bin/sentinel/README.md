# Sentinel

A high-performance log file monitoring and alerting tool written in Rust.

## Overview

Sentinel is a daemon that watches log files for error patterns and sends alerts via webhook notifications. It uses event-driven file tailing for low latency and supports frequency-based whitelisting to control alert noise.

## Features

- **Glob Pattern Matching**: Automatically discover log files using glob patterns (e.g., `logs/*.log`)
- **Event-Driven Tailing**: Uses `linemux` (inotify/kqueue) for real-time log reading
- **Whitelist & Thresholds**: Supports CSV-based whitelist rules to ignore or rate-limit specific errors
- **Priority-Based Routing**: Route alerts to different webhooks by priority (P0/P1/P2)
- **Per-Priority Rate Limiting**: Independent rate limiting per priority level
- **Multiple Notification Channels**: Supports Feishu and Slack webhooks
- **Health Probe**: Optional HTTP endpoint monitoring with failure threshold (always P0)
- **Log Rotation Support**: Automatically handles file rotation, truncation, and recreation

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Watcher   │     │   Reader    │────▶│  Analyzer   │
│ (discover)  │     │ (linemux)   │     │  (regex)    │
└─────────────┘     └─────────────┘     └──────┬──────┘
       │                   ▲                   │
       └───────────────────┘            ┌──────▼──────┐
                                        │  Whitelist  │
                                        │ (threshold) │
                                        └──────┬──────┘
                                               │
┌─────────────┐                         ┌──────▼──────┐
│    Probe    │────────────────────────▶│  Notifier   │
│ (health)    │                         │ (webhook)   │
└─────────────┘                         └─────────────┘
```

## Installation

```bash
# Build from source
cargo build --release -p sentinel

# The binary will be at target/release/sentinel
```

## Usage

```bash
# Run with config file
./sentinel <config.toml>

# Example
./sentinel sentinel.toml
```

## Configuration

Create a configuration file based on `sentinel.toml.example`:

```toml
[general]
# How often to check for new files via Glob (milliseconds)
check_interval_ms = 2000

[monitoring]
# Glob patterns to match log files
file_patterns = ["logs/*.log", "test.log"]

# Only monitor files modified within this time window (seconds)
# 86400 = 24 hours
recent_file_threshold_seconds = 86400

# Regex pattern to identify error lines (case-insensitive)
error_pattern = "(?i)error|panic|fatal"

# Path to whitelist CSV file (optional)
whitelist_path = "whitelist.csv"

# Default priority for log alerts (p0, p1, p2). Default: p0
default_priority = "p0"

[alerting]
# Default webhooks (fallback for priorities without specific overrides)
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/default..."
slack_webhook = "https://hooks.slack.com/services/default..."

# Minimum interval between alerts per priority (seconds)
min_alert_interval = 5

# Per-priority webhook overrides (optional)
# If a priority has no override, the default webhooks above are used.
[alerting.priorities.p0]
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/critical-group..."

[alerting.priorities.p1]
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/ops-group..."

[alerting.priorities.p2]
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/general-group..."

[probe]
# Optional: HTTP health check endpoint
url = "http://localhost:8545"
check_interval_seconds = 10
failure_threshold = 3
```

### Priority Levels

| Priority | Description | Default Usage |
|----------|-------------|---------------|
| P0 | Highest priority | Probe alerts, default for log alerts |
| P1 | Medium priority | — |
| P2 | Lowest priority | — |

**Fallback logic**: When a priority has no webhook override in `[alerting.priorities.<level>]`, the top-level `[alerting]` webhooks are used.

## Components

### Watcher

Periodically scans the filesystem using configured glob patterns to discover new log files. Ensures files are only monitored if they have been modified recently.

### Reader

Uses platform-specific event notification (inotify on Linux, kqueue on macOS) to efficiently tail log files in real-time. Supports handling file rotation and recreation.

### Analyzer

Matches log lines against the configured `error_pattern` regex to identify potential issues.

### Whitelist

Filters error logs based on checking rules defined in a CSV file.
- **Ignore (-1)**: Permanently ignore logs matching the pattern
- **Threshold (>0)**: Only alert if the pattern appears more than N times within a 5-minute window

### Notifier

Sends alerts to configured webhook endpoints with rate limiting (`min_alert_interval`).

```
🚨 **Log Sentinel Alert** 🚨
File: `/path/to/file.log`
Error:
```
<error message>
```
[Frequency Alert: >10/5min]
```

### Probe (Optional)

Monitors an HTTP endpoint availability by sending periodic GET requests.

## Whitelist CSV Format

```csv
# Comment
"Connection refused",-1
"Timeout",10
```

- Column 1: Regex pattern (use `""` for literal quotes)
- Column 2: Threshold (-1 to ignore, >0 for frequency count)

## Environment Variables

- `RUST_LOG`: Set log level (e.g., `RUST_LOG=info`, `RUST_LOG=debug`)

```bash
RUST_LOG=info ./sentinel sentinel.toml
```

## Example Use Cases

### Monitor Gravity Node Logs

```toml
[general]
check_interval_ms = 2000

[monitoring]
file_patterns = [
    "/var/log/gravity/*.log",
    "/tmp/gravity-cluster/*/consensus_log/*.log"
]
recent_file_threshold_seconds = 86400
error_pattern = "(?i)error|panic|fatal|warn"
whitelist_path = "consensus_whitelist.csv"
default_priority = "p0"

[alerting]
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/default..."
min_alert_interval = 5

[alerting.priorities.p0]
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/critical-alerts..."

[alerting.priorities.p1]
feishu_webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/ops-alerts..."

[probe]
url = "http://localhost:8545"
check_interval_seconds = 30
failure_threshold = 3
```

## License

This project is part of the Gravity SDK.
