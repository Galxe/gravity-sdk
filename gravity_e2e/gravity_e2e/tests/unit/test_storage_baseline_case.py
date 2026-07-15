"""
Unit tests for the storage_v2_baseline case-local helpers.

The case modules live in cluster_test_cases/storage_v2_baseline/ (outside
the gravity_e2e package), so they are loaded by file path here. Covered:

- storage_baseline_lib.py: anchor-spec construction from recorded history,
  offline db env derivation (encodes the cluster/deploy.sh datadir layout),
  and set(uint256) calldata encoding (checked against the canonical
  selector 0x60fe47b1, independently visible in the tracked
  prague/contracts/Counter.json bytecode).
- render_config.py: cluster.toml rendering (the untracked-binary-override
  mechanism); validated both on a toy template and on the real tracked
  cluster.toml.tpl, whose rendered output must parse as TOML.
- Config consistency: genesis.toml validator ports must match the node
  entry in cluster.toml.tpl (deploy.sh reads both).
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


lib = _load_case_module("storage_baseline_lib.py", "storage_baseline_lib")
render_config = _load_case_module(
    "render_config.py", "storage_baseline_render_config"
)


def _history(**overrides):
    """A valid BaselineHistory with readable, distinct block numbers."""
    kwargs = dict(
        faucet="0x" + "aa" * 20,
        recipient="0x" + "bb" * 20,
        contract="0x" + "cc" * 20,
        transfers=[
            lib.TxPoint("0x" + "11" * 32, 10),
            lib.TxPoint("0x" + "22" * 32, 14),
        ],
        deploy=lib.TxPoint("0x" + "33" * 32, 18),
        sets=[
            lib.TxPoint("0x" + "44" * 32, 21),
            lib.TxPoint("0x" + "55" * 32, 25),
        ],
    )
    kwargs.update(overrides)
    return lib.BaselineHistory(**kwargs)


# ---------------------------------------------------------------------------
# encode_set_call
# ---------------------------------------------------------------------------


def test_encode_set_call_matches_canonical_selector():
    # keccak256("set(uint256)")[:4] == 60fe47b1 — the same selector is
    # visible in the tracked prague/contracts/Counter.json runtime bytecode.
    assert lib.encode_set_call(1) == "0x60fe47b1" + "0" * 63 + "1"


def test_encode_set_call_pads_to_32_bytes():
    data = lib.encode_set_call(0xDEADBEEF)
    assert data.startswith("0x60fe47b1")
    assert len(data) == 2 + 8 + 64
    assert data.endswith("00000000000000000000000000000000000000000000000000000000deadbeef")


def test_encode_set_call_rejects_out_of_range_values():
    with pytest.raises(ValueError):
        lib.encode_set_call(-1)
    with pytest.raises(ValueError):
        lib.encode_set_call(2**256)


# ---------------------------------------------------------------------------
# build_anchor_spec
# ---------------------------------------------------------------------------


def test_build_anchor_spec_covers_all_six_anchor_kinds():
    spec = lib.build_anchor_spec(_history())
    # tx_hashes yield both the transaction and the receipt anchor kinds, so
    # the five non-empty spec fields cover all six kinds of storage_anchors.
    assert spec.balances, "balance anchors missing"
    assert spec.storage_slots, "storage anchors missing"
    assert spec.tx_hashes, "transaction/receipt anchors missing"
    assert spec.block_numbers, "block_hash anchors missing"
    assert spec.log_ranges, "logs anchors missing"


def test_build_anchor_spec_balances_sample_historical_transfer_blocks():
    h = _history()
    spec = lib.build_anchor_spec(h)
    assert (h.recipient, 10) in spec.balances
    assert (h.recipient, 14) in spec.balances
    assert (h.faucet, 14) in spec.balances


def test_build_anchor_spec_storage_slots_sample_each_set_block():
    h = _history()
    spec = lib.build_anchor_spec(h)
    assert (h.contract, 0, 21) in spec.storage_slots
    assert (h.contract, 0, 25) in spec.storage_slots


def test_build_anchor_spec_tx_hashes_unique_and_complete():
    h = _history()
    spec = lib.build_anchor_spec(h)
    expected = {
        h.transfers[0].tx_hash,
        h.transfers[1].tx_hash,
        h.deploy.tx_hash,
        h.sets[0].tx_hash,
        h.sets[1].tx_hash,
    }
    assert set(spec.tx_hashes) == expected
    assert len(spec.tx_hashes) == len(set(spec.tx_hashes))


def test_build_anchor_spec_block_numbers_sorted_unique():
    spec = lib.build_anchor_spec(_history())
    assert spec.block_numbers == sorted(set(spec.block_numbers))
    for block in (10, 14, 18, 21, 25):
        assert block in spec.block_numbers


def test_build_anchor_spec_log_range_spans_contract_history():
    h = _history()
    spec = lib.build_anchor_spec(h)
    assert spec.log_ranges == [(18, 25, h.contract)]


def test_build_anchor_spec_log_range_uses_min_max_regardless_of_order():
    # Deploy block recorded after the set blocks (defensive): the range must
    # still span min..max, never invert.
    h = _history(deploy=lib.TxPoint("0x" + "33" * 32, 30))
    spec = lib.build_anchor_spec(h)
    assert spec.log_ranges == [(21, 30, h.contract)]


def test_build_anchor_spec_rejects_empty_history():
    with pytest.raises(ValueError):
        lib.build_anchor_spec(_history(transfers=[]))
    with pytest.raises(ValueError):
        lib.build_anchor_spec(_history(sets=[]))


# ---------------------------------------------------------------------------
# derive_offline_env: must match the layout cluster/deploy.sh materializes
# (data_dir=<base>/<node>, STORAGE_DIR=<data_dir>/data, reth datadir and
# datadir.static-files both ${STORAGE_DIR}/reth per
# cluster/templates/reth_config.json.tpl, chain=<base>/genesis.json).
# ---------------------------------------------------------------------------


def test_derive_offline_env_layout():
    env = lib.derive_offline_env("/tmp/gravity-cluster-x", "node1")
    assert env.binary == Path("/tmp/gravity-cluster-x/node1/bin/gravity_node")
    assert env.datadir == Path("/tmp/gravity-cluster-x/node1/data/reth")
    assert env.chain == Path("/tmp/gravity-cluster-x/genesis.json")
    # The deployed node runs with --datadir.static-files pointed at the
    # datadir itself (NOT <datadir>/static_files), so the offline commands
    # must do the same.
    assert env.static_files_dir == env.datadir


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
