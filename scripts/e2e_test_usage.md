# Gravity SDK E2E Test Script Documentation

This document describes how to use the End-to-End (E2E) test script [scripts/e2e_test.sh](file:///home/kenji/galxe/gravity-sdk/scripts/e2e_test.sh). This script executes the entire testing pipeline (build, deploy, node start, benchmark) inside a Docker container, ensuring a distinct and reproducible environment.

## Usage

### Command

Run the script from the root of the repository, providing the target git branch as an argument:

```bash
bash scripts/e2e_test.sh <branch_name>
```

**Example:**
```bash
# Test the 'main' branch
bash scripts/e2e_test.sh main

# Test a feature branch
bash scripts/e2e_test.sh fix/consensus-issue
```

### Prerequisites
- **Docker**: Must be installed and running. `rust` is NOT required on the host machine as the build happens inside the container.
- **Network**: The script binds port `9001` on the host. Ensure it is available.

## Parameters

The script accepts the following parameters via environment variables (optional) and command-line arguments:

| Argument / Variable | Description |
|---------------------|-------------|
| `$1` (Argument) | **Required**. The git branch name to checkout and test (e.g., `main`). |
| `TEST_GENESIS` | The genesis JSON content (defined inside the wrapper script). |
| `GIT_REF` | Passed automatically from `$1`. |
| `CLONE_URL` | The repository URL to clone inside Docker. |
| `DURATION` | Benchmark duration in seconds (Default: 300). |

## Logs & Monitoring

The test runs inside a Docker container (`rust:1.88.0-bookworm`).

### Vital Logs
- **Benchmark Logs**: Located at `/gravity_bench/log.*.log` inside the container.
- **Node Logs**: The node runs in the background. logs are typically in `/tmp/node1/` inside the container.

### How to Inspect Logs

Since the process runs inside Docker, you have two main ways to view logs:

#### 1. Real-time Output (Standard)
The script runs `docker run` in interactive mode (`-i`), so the standard output of the test steps (build progress, benchmark summary) will be streamed directly to your terminal.

#### 2. attaching to the Container (Deep Dive)
To inspect files (like detailed debug logs) or check the node status *while the test is running*:

1.  **Find the Container ID**:
    Open a new terminal and run:
    ```bash
    docker ps
    # Look for the container using image 'rust:1.88.0-bookworm'
    ```

2.  **Enter the Container**:
    Use `docker exec` to open a shell inside the running container:
    ```bash
    docker exec -it <container_id> bash
    ```

3.  **View Logs**:
    Once inside:
    ```bash
    # View Benchmark logs
    tail -f /gravity_bench/log.*.log

    # View Node logs/status
    # (Assuming logs are redirected or managed by start.sh in /tmp/node1)
    ls -l /tmp/node1/
    ```

#### 3. Attaching to Main Process
If you ran the script in the background or want to re-attach to the main script output:
```bash
docker attach <container_id>
```
*Note: This connects you to the main execution process. Pressing `Ctrl+C` here might terminate the test.*

## Script Logic Overview

1.  **Genesis Injection**: Pipes `TEST_GENESIS` (defined in the host script) into the Docker container.
2.  **Environment Setup**: Installs dependencies (`clang`, `llvm`, `node`, `python3`) inside Docker.
3.  **Clone & Configure**: Clones the SDK branch, writes genesis, and updates `reth_config.json` using `jq`.
4.  **Build & Deploy**: builds `gravity_node` and deploys it to `/tmp/node1`.
5.  **Benchmark**: Downloads `gravity_bench`, configures it (100 TPS, 100 Accounts), and runs it.
6.  **Verification**: Analyzes the benchmark logs using a Python script to verify success.
