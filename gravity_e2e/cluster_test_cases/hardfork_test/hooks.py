"""
Pre-start hook for hardfork test suite.

Injects gravityHardforks configuration (gammaBlock) into the generated
genesis.json before the node starts. This is necessary because the
genesis tool in gravity_chain_core_contracts does not natively support
Gravity-specific hardfork fields — they must be added to the chain
config's extra_fields after genesis generation.

IMPORTANT: deploy.sh copies genesis.json from artifacts to cluster base_dir
BEFORE hooks run. We need to patch BOTH copies so the node picks up the change.

Usage:
    Set GAMMA_BLOCK env var to control when the hardfork triggers.
    Default: 500 blocks (must be higher than faucet init block count ~380).
"""

import json
import logging
import os
from pathlib import Path

LOG = logging.getLogger(__name__)


def _inject_gamma_block(genesis_path: Path, gamma_block: int):
    """Inject gravityHardforks into a single genesis.json file."""
    if not genesis_path.exists():
        LOG.warning(f"genesis.json not found at {genesis_path}, skipping")
        return False

    with open(genesis_path) as f:
        genesis = json.load(f)

    if "config" not in genesis:
        genesis["config"] = {}

    genesis["config"]["gravityHardforks"] = {
        "alphaBlock": 0,
        "betaBlock": 0,
        "gammaBlock": gamma_block,
    }

    with open(genesis_path, "w") as f:
        json.dump(genesis, f, indent=2)

    LOG.info(f"  ✅ Patched {genesis_path}")
    return True


def pre_start(test_dir, env, pytest_args):
    """
    Inject gravityHardforks.gammaBlock into genesis.json.

    Called by runner.py after deploy but before node start.
    Patches BOTH the artifacts copy and the deployed base_dir copy.
    """
    gamma_block = int(os.environ.get("GAMMA_BLOCK", "500"))
    LOG.info(f"🔧 Injecting gravityHardforks (gammaBlock={gamma_block})")

    patched_count = 0

    # 1. Patch the artifacts copy
    artifacts_dir = env.get("GRAVITY_ARTIFACTS_DIR", str(Path(test_dir) / "artifacts"))
    artifacts_genesis = Path(artifacts_dir) / "genesis.json"
    if _inject_gamma_block(artifacts_genesis, gamma_block):
        patched_count += 1

    # 2. Patch the deployed copy in cluster base_dir
    #    deploy.sh copies genesis.json to $base_dir/genesis.json
    #    The node reads from this copy, so we MUST patch it.
    cluster_config = Path(test_dir) / "cluster.toml"
    if cluster_config.exists():
        try:
            import tomli
        except ImportError:
            try:
                import tomllib as tomli
            except ImportError:
                import toml as tomli

        with open(cluster_config, "rb") as f:
            config = tomli.load(f)
        base_dir = config.get("cluster", {}).get("base_dir", "")
        if base_dir:
            deployed_genesis = Path(base_dir) / "genesis.json"
            if _inject_gamma_block(deployed_genesis, gamma_block):
                patched_count += 1

    LOG.info(f"✅ Patched {patched_count} genesis.json file(s) with gammaBlock={gamma_block}")

    # Export for test code to read
    os.environ["GAMMA_BLOCK"] = str(gamma_block)
