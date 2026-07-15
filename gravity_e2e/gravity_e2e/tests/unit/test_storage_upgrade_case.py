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


def test_alpha_preflight_future_within_budget_is_ok():
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


def test_alpha_preflight_rejects_unreachable_schedule():
    # A scheduled alphaTime beyond the case budget is a config error (the
    # activation crossing is mandatory coverage when scheduled), not a
    # silent skip.
    error = upgrade_lib.alpha_preflight_error(
        1000 + 7200 + 1, now=1000.0, min_lead_s=1500, max_lead_s=7200
    )
    assert error is not None
    assert "mainnet posture" in error
    # Exactly at the ceiling is accepted; no ceiling when max_lead_s absent.
    assert (
        upgrade_lib.alpha_preflight_error(
            1000 + 7200, now=1000.0, min_lead_s=1500, max_lead_s=7200
        )
        is None
    )
    assert (
        upgrade_lib.alpha_preflight_error(10_000_000, now=1000.0, min_lead_s=1500)
        is None
    )


def test_upgrade_completion_guard_absent_alpha_is_ok():
    assert upgrade_lib.upgrade_completion_error(None, completion_time=5000.0) is None


def test_upgrade_completion_guard_before_activation_is_ok():
    assert upgrade_lib.upgrade_completion_error(6000, completion_time=5999.0) is None


def test_upgrade_completion_guard_rejects_late_completion():
    # The operational rule, machine-enforced: every node must be on the
    # new binary BEFORE Alpha activates. Late completion = immediate fail.
    error = upgrade_lib.upgrade_completion_error(6000, completion_time=6000.0)
    assert error is not None
    assert "after every node" in error.lower()


def test_alpha_tail_wait_absent_skips():
    assert upgrade_lib.alpha_tail_wait_s(None, now=1000.0) is None


def test_alpha_tail_wait_scheduled_returns_remaining():
    assert upgrade_lib.alpha_tail_wait_s(1300, now=1000.0) == 300.0


def test_alpha_tail_wait_already_active_returns_zero():
    assert upgrade_lib.alpha_tail_wait_s(500, now=1000.0) == 0.0


# ---------------------------------------------------------------------------
# SYSTEM_CALLER balance trajectory (the Alpha semantic-flip direct proof)
# ---------------------------------------------------------------------------


def _sample(number, balance, gas_used=117_084, base_fee=50_000_000_000, tx_count=0):
    return upgrade_lib.BlockSample(
        number=number,
        gas_used=gas_used,
        base_fee=base_fee,
        tx_count=tx_count,
        balance=balance,
    )


def _debit_chain(start_block, start_balance, n, gas_used=117_084,
                 base_fee=50_000_000_000):
    """n+1 samples where every block debits exactly gas_used*base_fee."""
    samples = [_sample(start_block, start_balance, gas_used, base_fee)]
    for i in range(1, n + 1):
        samples.append(
            _sample(
                start_block + i,
                start_balance - i * gas_used * base_fee,
                gas_used,
                base_fee,
            )
        )
    return samples


def test_pre_alpha_debits_accept_exact_per_block_debit():
    samples = _debit_chain(100, 10**21, 4)
    assert upgrade_lib.pre_alpha_debit_error(samples) is None


def test_pre_alpha_debits_reject_constant_balance():
    # Balance not moving pre-activation would mean the exemption was
    # already live (alphaTime misplaced) — must fail.
    samples = [_sample(100, 10**21), _sample(101, 10**21)]
    error = upgrade_lib.pre_alpha_debit_error(samples)
    assert error is not None
    assert "decrease" in error


def test_pre_alpha_debits_reject_wrong_delta_on_empty_block():
    samples = _debit_chain(100, 10**21, 3)
    # Corrupt one balance so the wei-level reconciliation breaks.
    samples[2] = _sample(102, samples[2].balance - 1)
    error = upgrade_lib.pre_alpha_debit_error(samples)
    assert error is not None
    assert "gas_used" in error


def test_pre_alpha_debits_skip_exact_check_on_nonempty_blocks():
    # Blocks with user txs debit more than the system share; only strict
    # decrease is required there, exact reconciliation comes from empty
    # blocks. With min_exact=1 a single clean empty pair suffices.
    samples = [
        _sample(100, 10**21),
        _sample(101, 10**21 - 7_000_000_000_000_000, tx_count=3),
        _sample(102, 10**21 - 7_000_000_000_000_000 - 117_084 * 50_000_000_000),
    ]
    assert upgrade_lib.pre_alpha_debit_error(samples, min_exact=1) is None


def test_pre_alpha_debits_require_enough_exact_confirmations():
    samples = [
        _sample(100, 10**21),
        _sample(101, 10**21 - 1, tx_count=5),
        _sample(102, 10**21 - 2, tx_count=5),
    ]
    error = upgrade_lib.pre_alpha_debit_error(samples, min_exact=2)
    assert error is not None
    assert "exact" in error


def test_pre_alpha_debits_reject_gaps_and_short_input():
    assert upgrade_lib.pre_alpha_debit_error([_sample(100, 1)]) is not None
    samples = [_sample(100, 10**21), _sample(102, 10**21 - 1)]
    error = upgrade_lib.pre_alpha_debit_error(samples)
    assert error is not None
    assert "consecutive" in error


def test_post_alpha_constancy_accepts_frozen_balance():
    assert upgrade_lib.post_alpha_constancy_error({200: 0, 201: 0, 202: 0}) is None


def test_post_alpha_constancy_rejects_moving_balance():
    error = upgrade_lib.post_alpha_constancy_error({200: 10, 201: 9})
    assert error is not None
    assert "still" in error


def test_post_alpha_constancy_rejects_short_input():
    assert upgrade_lib.post_alpha_constancy_error({200: 0}) is not None


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


def test_resolve_hardfork_value_component_schedule_sums_offsets():
    # The brief's formula: alphaTime = render_time + upgrade_budget +
    # stability_window + margin, each component tunable in test_params.
    components = {
        "upgrade_budget": "+27m",
        "stability_window": "+13m",
        "margin": "+10m",
    }
    assert (
        render_config.resolve_hardfork_value(components, now=1000)
        == 1000 + (27 + 13 + 10) * 60
    )


def test_resolve_hardfork_value_component_schedule_rejects_bad_parts():
    with pytest.raises(ValueError):
        render_config.resolve_hardfork_value({}, now=1000)
    for bad in ({"budget": "45m"}, {"budget": 45}, {"budget": None}):
        with pytest.raises(ValueError):
            render_config.resolve_hardfork_value(bad, now=1000)


def test_hardforks_to_toml_resolves_relative_values():
    text = render_config.hardforks_to_toml(
        {"alphaTime": "+45m", "gammaBlock": 8000}, now=1000
    )
    assert tomllib.loads(text) == {"alphaTime": 3700, "gammaBlock": 8000}


def test_hardforks_to_toml_resolves_component_schedule():
    text = render_config.hardforks_to_toml(
        {
            "betaBlock": 100,
            "alphaTime": {"upgrade_budget": "+30m", "margin": "+20m"},
        },
        now=1000,
    )
    assert tomllib.loads(text) == {"betaBlock": 100, "alphaTime": 1000 + 3000}


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
            # The recommended shape (test_params.toml.example): beta early,
            # gamma late, alphaTime as a component schedule.
            {
                "betaBlock": 100,
                "gammaBlock": 6000,
                "alphaTime": {
                    "upgrade_budget": "+27m",
                    "stability_window": "+13m",
                    "margin": "+10m",
                },
            },
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
    assert genesis["genesis"]["hardforks"]["gammaBlock"] == 6000
    assert genesis["genesis"]["hardforks"]["alphaTime"] == 1_000_000 + 50 * 60


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
