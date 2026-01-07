# Large-Scale Account Benchmark Test

This document describes how to run large-scale account benchmark tests for Gravity SDK.

## Overview

Large-scale account testing is designed to evaluate Gravity node performance under high load conditions with millions of accounts. This test simulates real-world scenarios where the network handles a large number of unique accounts sending transactions concurrently.

## Hardware Requirements

### Recommended Instance

- **Instance Type**: `c4-highcpu-32` (or equivalent)
- **vCPUs**: 32
- **Memory**: 64 GB
- **Disk**: 1024 GB, 30,000 IOPS, 590 MB/s throughput

### Resource Allocation

| Component | vCPUs | Memory |
|-----------|-------|--------|
| Gravity Node | 16 | 32 GB |
| Benchmark Client | 16 | 32 GB |

> **Note**: While the Gravity node itself only requires 16 vCPUs and 32 GB memory, the benchmark client requires significant resources because:
> - Transaction signing is CPU-intensive at high TPS
> - Client must maintain nonce state for millions of accounts in memory
> - Concurrent transaction sending requires substantial memory allocation

## Quick Start: 10 Million Accounts Test

Run the following command from the repository root to start a 10 million account benchmark test:

```bash
cd scripts
BENCH_CONFIG_PATH=./bench_config.10M_accounts.toml DURATION=0 ./e2e_test.sh main
```

### Parameters Explained

| Parameter | Value | Description |
|-----------|-------|-------------|
| `BENCH_CONFIG_PATH` | `./bench_config.10M_accounts.toml` | Path to the 10M accounts config |
| `DURATION` | `0` | Run indefinitely (manual stop required) |
| `main` | - | Git branch to test |

## Configuration Details

The `bench_config.10M_accounts.toml` configuration includes:

```toml
# Target TPS
target_tps = 14000

# Number of accounts to create and use
[accounts]
num_accounts = 10000000  # 10 million

# Performance tuning
[performance]
num_senders = 500        # Concurrent sender threads
max_pool_size = 40000    # Transaction pool size
duration_secs = 0        # Run indefinitely
```

## Monitoring

During the test, you can monitor performance through:

1. **Benchmark Logs**: Check TPS and transaction statistics
   ```bash
   # Inside the container
   tail -f /gravity_bench/log.*.log
   ```

2. **Metrics Endpoint**: Access detailed metrics at `http://localhost:9001/metrics`

3. **Prometheus & Grafana**: For comprehensive monitoring, see [e2e_test_usage.md](./e2e_test_usage.md#metrics-monitoring)

## Stopping the Test

Since `DURATION=0` runs the test indefinitely, stop it manually:

```bash
# Find and stop the container
docker ps
docker stop <container_id>
```

Or press `Ctrl+C` in the terminal running the test.
