"""
Per-suite lifecycle hooks (called by runner.py around the node lifecycle).

pre_start: enable Prague (EIP-7702) for THIS suite only. The shared devnet
genesis template does not set `pragueTime`, so a type-4 (set-code) transaction
would be rejected by the reth pool ("transaction type not supported") before it
can exercise the code path under test. Activating Prague at genesis for this
suite — without touching the shared genesis template or any other suite — lets
the type-4 tx reach execution. (This is also the coverage gap that let
gravity-audit #677 reach the testnet: the e2e harness never exercised any
Prague/7702 path.)

IMPORTANT ordering note: runner.py runs `deploy.sh` (step 2) BEFORE this hook
(step 2.5). deploy.sh copies the suite's `artifacts/genesis.json` to
`<base_dir>/genesis.json` and starts the node from THAT copy (deploy.sh:796,
GENESIS_PATH=<base_dir>/genesis.json). So patching only the source
`artifacts/genesis.json` here is too late — the node already read the unpatched
copy. We must patch the DEPLOYED copy at `<base_dir>/genesis.json` (which exists
by the time pre_start runs). We patch the source too so cached/`--resume` reuse
and on-disk inspection stay consistent.
"""

import json
import logging
from pathlib import Path

try:
    import tomllib as toml_reader  # py311+
except ModuleNotFoundError:  # pragma: no cover
    import tomli as toml_reader

LOG = logging.getLogger(__name__)


def _enable_prague(genesis_path: Path) -> bool:
    """Set pragueTime=0 in the given reth genesis.json. Returns True if applied."""
    if not genesis_path.exists():
        LOG.warning("genesis.json not found at %s; cannot enable Prague", genesis_path)
        return False
    genesis = json.loads(genesis_path.read_text())
    cfg = genesis.setdefault("config", {})
    if cfg.get("pragueTime") == 0:
        LOG.info("Prague already enabled (pragueTime=0) in %s", genesis_path)
        return True
    cfg["pragueTime"] = 0  # activate Prague (EIP-7702) from genesis
    genesis_path.write_text(json.dumps(genesis, indent=2))
    LOG.info("Enabled Prague (pragueTime=0) in %s for the 7702 suite", genesis_path)
    return True


def pre_start(test_dir: Path, env: dict, pytest_args=None):
    test_dir = Path(test_dir)

    # 1. The DEPLOYED genesis the node actually boots from. deploy.sh copies the
    #    suite genesis to <base_dir>/genesis.json (deploy.sh:796) and the node
    #    reads that path, so this is the copy that must carry pragueTime=0.
    base_dir = None
    cluster_toml = test_dir / "cluster.toml"
    if cluster_toml.exists():
        with open(cluster_toml, "rb") as f:
            base_dir = toml_reader.load(f).get("cluster", {}).get("base_dir")
    if base_dir:
        deployed = Path(base_dir) / "genesis.json"
        if not _enable_prague(deployed):
            LOG.error(
                "Deployed genesis %s missing — Prague will NOT be active and the "
                "7702 tx will be rejected before execution (false negative).",
                deployed,
            )
    else:
        LOG.error("Could not resolve cluster.base_dir from %s", cluster_toml)

    # 2. The source artifact, so cached/`--resume` reuse and inspection match.
    _enable_prague(test_dir / "artifacts" / "genesis.json")
