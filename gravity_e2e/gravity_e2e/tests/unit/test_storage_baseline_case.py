"""
Unit tests for the storage_v2_baseline case-local helpers.

The case's render_config.py lives in cluster_test_cases/storage_v2_baseline/
(outside the gravity_e2e package), so it is loaded by file path here.
Covered:

- render_config.py: cluster.toml rendering (the untracked-binary-override
  mechanism); validated both on a toy template and on the real tracked
  cluster.toml.tpl, whose rendered output must parse as TOML.
- Config consistency: genesis.toml validator ports must match the node
  entry in cluster.toml.tpl (deploy.sh reads both).

The pure history/offline-env helpers this case consumes moved to
gravity_e2e.helpers.storage_case_lib (shared with storage_v2_upgrade) and
are covered by test_storage_case_lib.py.
"""

import importlib.util
import sys
import tomllib
from pathlib import Path

import pytest

CASE_DIR = (
    Path(__file__).resolve().parents[3] / "cluster_test_cases" / "storage_v2_baseline"
)


def _load_case_module(filename: str, module_name: str):
    """Load a case-local module by path (case dir is not a package)."""
    spec = importlib.util.spec_from_file_location(module_name, CASE_DIR / filename)
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


render_config = _load_case_module(
    "render_config.py", "storage_baseline_render_config"
)


# ---------------------------------------------------------------------------
# render_config.py: the untracked binary-override path
# ---------------------------------------------------------------------------


def test_render_cluster_toml_substitutes_source():
    template = 'x = 1\nsource = {{SOURCE}}\n'
    out = render_config.render_cluster_toml(
        template, {"bin_path": "/opt/gravity_node"}
    )
    assert 'source = { bin_path = "/opt/gravity_node" }' in out


def test_render_cluster_toml_missing_placeholder_raises():
    with pytest.raises(ValueError):
        render_config.render_cluster_toml("x = 1\n", {"bin_path": "/opt/g"})


def test_render_cluster_toml_empty_source_raises():
    with pytest.raises(ValueError):
        render_config.render_cluster_toml("source = {{SOURCE}}\n", {})


def test_real_template_renders_to_parseable_cluster_toml():
    template = (CASE_DIR / "cluster.toml.tpl").read_text()
    rendered = render_config.render_cluster_toml(
        template, {"bin_path": "/opt/greth/gravity_node"}
    )
    config = tomllib.loads(rendered)
    (node,) = config["nodes"]
    assert node["id"] == "node1"
    assert node["source"] == {"bin_path": "/opt/greth/gravity_node"}


# ---------------------------------------------------------------------------
# Tracked config consistency: genesis.toml validator entry vs cluster.toml.tpl
# ---------------------------------------------------------------------------


def test_genesis_validator_ports_match_cluster_template():
    template = (CASE_DIR / "cluster.toml.tpl").read_text()
    cluster = tomllib.loads(
        render_config.render_cluster_toml(template, {"project_path": "../"})
    )
    genesis = tomllib.loads((CASE_DIR / "genesis.toml").read_text())

    (node,) = cluster["nodes"]
    (validator,) = genesis["genesis_validators"]
    assert validator["id"] == node["id"]
    assert validator["validator_port"] == node["validator_port"]
    assert validator["vfn_port"] == node["vfn_port"]
    assert validator["host"] == node["host"]
