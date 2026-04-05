# Rolling Upgrade Test

## Usage

### 1. Configure test parameters

```bash
cp test_params.toml.example test_params.toml
```

Edit `test_params.toml`:

```toml
[source]
bin_path = "../target/quick-release/v1.0.0/gravity_node"
# or: github = "Galxe/gravity-sdk"
# or: rev = "v1.0.0"
# or: project_path = "../"

[genesis_contracts]
repo = "https://github.com/Galxe/gravity_chain_core_contracts.git"
ref = "gravity-testnet-v1.0.0"

[hardforks]
alphaBlock = 100
betaBlock = 100
gammaBlock = 10000
```

### 2. Render config files

```bash
python render_config.py
```

This generates `cluster.toml` and `genesis.toml` from templates.

To use a different params file:

```bash
python render_config.py my_scenario.toml
```

### 3. Run the test

```bash
# From project root
python gravity_e2e/runner.py rolling_upgrade
```

The upgrade target binary defaults to `target/quick-release/gravity_node`. Override via:

```bash
export GRAVITY_NEW_BINARY=/path/to/new/gravity_node
python gravity_e2e/runner.py rolling_upgrade
```
