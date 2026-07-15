"""
Unit tests for the storage_v2_upgrade case-local helpers.

The case modules live in cluster_test_cases/storage_v2_upgrade/ (outside
the gravity_e2e package), so they are loaded by file path here. Covered:

- upgrade_lib.py: vfn-first upgrade ordering and the storage/decode log
  scan (patterns, unwind accounting, offender caps).
- render_config.py: cluster.toml + genesis.toml rendering (the
  untracked-binary/hardfork/contracts override mechanism); validated on
  toy templates and on the real tracked templates, whose rendered output
  must parse as TOML.
- Config consistency: genesis.toml.tpl validator ports must match the
  node entries in cluster.toml.tpl (deploy.sh reads both), and the port
  block must not collide with other suites' defaults by construction of
  the topology (checked pairwise-unique here).
"""

import importlib.util
import sys
import tomllib
from pathlib import Path

import pytest

CASE_DIR = (
    Path(__file__).resolve().parents[3] / "cluster_test_cases" / "storage_v2_upgrade"
)


def _load_case_module(filename: str, module_name: str):
    """Load a case-local module by path (case dir is not a package)."""
    spec = importlib.util.spec_from_file_location(module_name, CASE_DIR / filename)
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


upgrade_lib = _load_case_module("upgrade_lib.py", "storage_upgrade_lib")
render_config = _load_case_module("render_config.py", "storage_upgrade_render_config")


# ---------------------------------------------------------------------------
# build_upgrade_order
# ---------------------------------------------------------------------------


def test_upgrade_order_puts_vfn_first_and_keeps_rest_stable():
    ids = ["node1", "node2", "node3", "node4", "node5", "vfn1"]
    assert upgrade_lib.build_upgrade_order(ids, first="vfn1") == [
        "vfn1",
        "node1",
        "node2",
        "node3",
        "node4",
        "node5",
    ]


def test_upgrade_order_first_already_in_front():
    assert upgrade_lib.build_upgrade_order(["vfn1", "node1"], first="vfn1") == [
        "vfn1",
        "node1",
    ]


def test_upgrade_order_rejects_unknown_first():
    with pytest.raises(ValueError):
        upgrade_lib.build_upgrade_order(["node1"], first="vfn1")


def test_upgrade_order_rejects_duplicates():
    with pytest.raises(ValueError):
        upgrade_lib.build_upgrade_order(["node1", "node1", "vfn1"], first="vfn1")


# ---------------------------------------------------------------------------
# scan_log_lines
# ---------------------------------------------------------------------------


def test_scan_clean_log_finds_nothing():
    lines = [
        "2026-07-15T10:00:00 INFO reth::node Block produced number=1234",
        "2026-07-15T10:00:01 WARN network: peer disconnected",
        "2026-07-15T10:00:02 ERROR consensus: timeout waiting for proposal",
    ]
    result = upgrade_lib.scan_log_lines(lines)
    assert result.lines_scanned == 3
    assert result.error_count == 0
    assert result.unwind_count == 0


def test_scan_flags_each_error_family():
    offenders = [
        "thread 'main' panicked at src/lib.rs:1:1:",
        "ERROR rocksdb: Corruption: block checksum mismatch",
        "ERROR provider: failed to decode value for table StorageChangeSets",
        "ERROR db: decode error while reading AccountChangeSets",
        "ERROR db: DecodeError(InvalidLength)",
        "ERROR provider: DatabaseError(Read(-30796))",
    ]
    for line in offenders:
        result = upgrade_lib.scan_log_lines([line])
        assert result.error_count == 1, f"pattern missed: {line}"
        assert result.error_lines == [line]


def test_scan_counts_unwind_separately_from_errors():
    lines = [
        "INFO pipeline: Unwinding to block 100",
        "INFO pipeline: unwind complete target=100",
    ]
    result = upgrade_lib.scan_log_lines(lines)
    assert result.error_count == 0
    assert result.unwind_count == 2
    assert len(result.unwind_lines) == 2


def test_scan_caps_recorded_lines_but_counts_all():
    lines = ["ERROR db: DatabaseError(oops)"] * 200
    result = upgrade_lib.scan_log_lines(lines)
    assert result.error_count == 200
    assert len(result.error_lines) == upgrade_lib.MAX_RECORDED_LINES


def test_scan_summary_mentions_counts_and_offenders():
    result = upgrade_lib.scan_log_lines(
        ["ERROR rocksdb: Corruption: bad block", "INFO fine"]
    )
    summary = result.summary()
    assert "1 storage/decode errors" in summary
    assert "Corruption" in summary


# ---------------------------------------------------------------------------
# Alpha hardfork timeline helpers
# ---------------------------------------------------------------------------


def test_alpha_preflight_absent_is_ok():
    # Mainnet posture: alphaTime unscheduled -> Alpha never activates.
    assert upgrade_lib.alpha_preflight_error(None, now=1000.0, min_lead_s=1500) is None


def test_alpha_preflight_far_future_is_ok():
    assert (
        upgrade_lib.alpha_preflight_error(10_000, now=1000.0, min_lead_s=1500) is None
    )


def test_alpha_preflight_rejects_active_from_genesis():
    # The legacy alphaTime=0 artifact: Alpha active for all history.
    error = upgrade_lib.alpha_preflight_error(0, now=1000.0, min_lead_s=1500)
    assert error is not None
    assert "block_store.rs:773" in error


def test_alpha_preflight_rejects_insufficient_lead():
    error = upgrade_lib.alpha_preflight_error(2000, now=1000.0, min_lead_s=1500)
    assert error is not None
    # Exactly at the lead boundary is accepted.
    assert upgrade_lib.alpha_preflight_error(2500, now=1000.0, min_lead_s=1500) is None


def test_alpha_tail_wait_absent_skips():
    assert upgrade_lib.alpha_tail_wait_s(None, now=1000.0, max_wait_s=600) is None


def test_alpha_tail_wait_far_future_skips():
    assert upgrade_lib.alpha_tail_wait_s(10_000, now=1000.0, max_wait_s=600) is None


def test_alpha_tail_wait_reachable_returns_remaining():
    assert upgrade_lib.alpha_tail_wait_s(1300, now=1000.0, max_wait_s=600) == 300.0


def test_alpha_tail_wait_already_active_returns_zero():
    assert upgrade_lib.alpha_tail_wait_s(500, now=1000.0, max_wait_s=600) == 0.0


# ---------------------------------------------------------------------------
# render_config.py
# ---------------------------------------------------------------------------


def test_resolve_hardfork_value_int_passthrough():
    assert render_config.resolve_hardfork_value(8000, now=123) == 8000


def test_resolve_hardfork_value_relative_forms():
    assert render_config.resolve_hardfork_value("+30s", now=1000) == 1030
    assert render_config.resolve_hardfork_value("+45m", now=1000) == 1000 + 45 * 60
    assert render_config.resolve_hardfork_value("+2h", now=1000) == 1000 + 7200


def test_resolve_hardfork_value_rejects_garbage():
    for bad in ("45m", "+45", "+45x", "soon", 1.5):
        with pytest.raises(ValueError):
            render_config.resolve_hardfork_value(bad, now=1000)


def test_hardforks_to_toml_resolves_relative_values():
    text = render_config.hardforks_to_toml(
        {"alphaTime": "+45m", "gammaBlock": 8000}, now=1000
    )
    assert tomllib.loads(text) == {"alphaTime": 3700, "gammaBlock": 8000}


def test_render_cluster_toml_substitutes_source_on_every_node():
    template = "a = {{SOURCE}}\nb = {{SOURCE}}\n"
    out = render_config.render_cluster_toml(template, {"bin_path": "/opt/old"})
    assert out.count('{ bin_path = "/opt/old" }') == 2


def test_render_cluster_toml_missing_placeholder_raises():
    with pytest.raises(ValueError):
        render_config.render_cluster_toml("x = 1\n", {"bin_path": "/opt/g"})


def test_render_cluster_toml_empty_source_raises():
    with pytest.raises(ValueError):
        render_config.render_cluster_toml("source = {{SOURCE}}\n", {})


def test_render_genesis_toml_substitutes_all_placeholders():
    template = (
        "repo = \"{{GENESIS_CONTRACTS_REPO}}\"\n"
        "ref = \"{{GENESIS_CONTRACTS_REF}}\"\n"
        "[genesis.hardforks]\n{{HARDFORKS}}\n"
    )
    out = render_config.render_genesis_toml(
        template,
        {"alphaTime": 0, "gammaBlock": 10000},
        {"repo": "https://example/repo.git", "ref": "main"},
    )
    parsed = tomllib.loads(out)
    assert parsed["repo"] == "https://example/repo.git"
    assert parsed["ref"] == "main"
    assert parsed["genesis"]["hardforks"] == {"alphaTime": 0, "gammaBlock": 10000}


def test_render_genesis_toml_requires_contract_pin():
    template = (
        "repo = \"{{GENESIS_CONTRACTS_REPO}}\"\n"
        "ref = \"{{GENESIS_CONTRACTS_REF}}\"\n{{HARDFORKS}}\n"
    )
    with pytest.raises(ValueError):
        render_config.render_genesis_toml(template, {}, {"repo": "x"})
    with pytest.raises(ValueError):
        render_config.render_genesis_toml(template, {}, {"ref": "x"})


# ---------------------------------------------------------------------------
# Real tracked templates
# ---------------------------------------------------------------------------


def _render_real_templates():
    cluster = tomllib.loads(
        render_config.render_cluster_toml(
            (CASE_DIR / "cluster.toml.tpl").read_text(),
            {"bin_path": "/opt/v175/gravity_node"},
        )
    )
    genesis = tomllib.loads(
        render_config.render_genesis_toml(
            (CASE_DIR / "genesis.toml.tpl").read_text(),
            # The recommended shape: relative alphaTime, gamma late.
            {"alphaTime": "+45m", "betaBlock": 100, "gammaBlock": 8000},
            {"repo": "https://example/repo.git", "ref": "main"},
            now=1_000_000,
        )
    )
    return cluster, genesis


def test_real_templates_render_to_parseable_toml():
    cluster, genesis = _render_real_templates()
    nodes = {n["id"]: n for n in cluster["nodes"]}
    assert set(nodes) == {"node1", "node2", "node3", "node4", "node5", "vfn1"}
    for node in nodes.values():
        assert node["source"] == {"bin_path": "/opt/v175/gravity_node"}
    assert nodes["vfn1"]["role"] == "vfn"
    assert nodes["node5"]["role"] == "validator"
    assert genesis["genesis"]["hardforks"]["gammaBlock"] == 8000
    assert genesis["genesis"]["hardforks"]["alphaTime"] == 1_000_000 + 45 * 60


def test_genesis_validator_ports_match_cluster_template():
    cluster, genesis = _render_real_templates()
    nodes = {n["id"]: n for n in cluster["nodes"]}
    validators = genesis["genesis_validators"]
    assert [v["id"] for v in validators] == ["node1", "node2", "node3", "node4"]
    for validator in validators:
        node = nodes[validator["id"]]
        assert validator["validator_port"] == node["validator_port"]
        assert validator["vfn_port"] == node["vfn_port"]
        assert validator["host"] == node["host"]


def test_cluster_ports_are_pairwise_unique():
    cluster, _ = _render_real_templates()
    ports = []
    for node in cluster["nodes"]:
        for key, value in node.items():
            if key.endswith("_port") or key == "https_port":
                ports.append(value)
    assert len(ports) == len(set(ports)), f"duplicate ports in template: {ports}"
