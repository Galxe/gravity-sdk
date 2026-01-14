# Gravity Cluster Management Tools

This directory contains utility scripts to initialize, deploy, and manage a local Gravity Devnet cluster.

## Prerequisite

1.  **Rust**: Installed via rustup.
2.  **Foundry**: Installed via `foundryup` (Required for genesis generation).
3.  **Python 3**: For configuration parsing.
4.  **envsubst**: Usually part of `gettext` package.

## Directory Structure

- `cluster.toml`: Main configuration file. Define your nodes here.
- `Makefile`: Entry point for all commands.
- `init.sh`: Generates artifacts (Keys, Genesis, Waypoint) into `./output`.
- `deploy.sh`: Deploys configurations to running directory (default `/tmp/gravity-cluster`).
- `start.sh` / `stop.sh` / `status.sh`: Cluster lifecycle control.
- `templates/`: Configuration templates.
- `output/`: Generated artifacts (Do not edit manually, regen via `make init`).

## Usage

### 1. Configure

Copy the example configuration:
```bash
cp cluster.toml.example cluster.toml
```
Edit `cluster.toml` to set your desired `base_dir` and node configuration.

### 2. Initialize (Generate Keys & Genesis)

This step generates the validator keys, aggregates them, and builds a fresh genesis block.
Artifacts are stored in `cluster/output/`.

```bash
make init
```

*Note: This step clones `gravity-genesis-contract` and runs `forge build`, so the first run takes a few minutes.*

### 3. Deploy (Configure Environment)

This step cleans the target `base_dir` (e.g., `/tmp/gravity-cluster`) and deploys the necessary configuration files (`validator.yaml`, `reth_config.json`) and scripts.

```bash
make deploy
```

### 4. Start Cluster

```bash
make start
```

### Other Commands

- **Check Status**: `make status`
- **Stop Cluster**: `make stop`
- **Clean Artifacts**: `make clean` (Removes `./output`)
- **Redeploy & Restart**: `make deploy_start`

## Debugging

- **Node Logs**: Check `{base_dir}/{node_id}/consensus_log/validator.log`.
- **Reth Logs**: Check `{base_dir}/{node_id}/logs/`.
