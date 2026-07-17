"""
Unit tests for the storage_v2_fresh_sync (TC9) case-local helpers.

The case modules live in cluster_test_cases/storage_v2_fresh_sync/
(outside the gravity_e2e package), so they are loaded by file path here.
Covered:

- sf_lib.py: the SF x non-SF topology invariants, the SF-enable mode
  knob (env override, executable-mode gate), the equal-power constants;
- render_config.py: the dual binary-source rendering ({{SOURCE}} for the
  legacy core, {{SF_SOURCE}} for the SF nodes) and the [sf] validation;
- Config consistency: genesis.toml.tpl ports/stake vs cluster.toml.tpl
  and sf_lib's cross-checked constants; role-specific port constraints
  (pfn: only public_port; sf_val1 must carry vfn_port for sf_vfn2); port
  uniqueness inside the suite and against storage_v2_upgrade's block.
"""

import importlib.util
import sys
import tomllib
from pathlib import Path

import pytest

CASES_DIR = Path(__file__).resolve().parents[3] / "cluster_test_cases"
CASE_DIR = CASES_DIR / "storage_v2_fresh_sync"


def _load_case_module(filename: str, module_name: str):
    spec = importlib.util.spec_from_file_location(module_name, CASE_DIR / filename)
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


sf_lib = _load_case_module("sf_lib.py", "storage_fresh_sync_sf_lib")
render_config = _load_case_module(
    "render_config.py", "storage_fresh_sync_render_config"
)


# ---------------------------------------------------------------------------
# sf_lib: topology invariants
# ---------------------------------------------------------------------------


def test_node_sets_are_disjoint_and_complete():
    legacy, sf = set(sf_lib.LEGACY_NODE_IDS), set(sf_lib.SF_NODE_IDS)
    assert not legacy & sf
    assert len(legacy) == 4 and len(sf) == 5


def test_every_fullnode_has_a_pinned_upstream():
    all_nodes = set(sf_lib.LEGACY_NODE_IDS) | set(sf_lib.SF_NODE_IDS)
    validators = {"node1", "node2", "sf_val1"}
    fullnodes = all_nodes - validators
    assert set(sf_lib.PINNED_UPSTREAMS) == fullnodes
    for downstream, upstream in sf_lib.PINNED_UPSTREAMS.items():
        assert upstream in all_nodes, f"{downstream} pinned to unknown {upstream}"
        assert upstream != downstream


def test_matrix_covers_sf_and_legacy_upstreams():
    """The design's coverage matrix: SF fullnodes split across legacy and
    SF upstreams; the legacy controls stay strictly under legacy
    upstreams (no contamination)."""
    sf = set(sf_lib.SF_NODE_IDS)
    sf_fullnodes = {n for n in sf_lib.PINNED_UPSTREAMS if n in sf}
    upstream_kinds = {
        sf_lib.PINNED_UPSTREAMS[n] in sf for n in sf_fullnodes
    }
    assert upstream_kinds == {True, False}, "matrix needs both upstream kinds"
    for control in ("vfn1", "pfn1"):
        assert sf_lib.PINNED_UPSTREAMS[control] not in sf, (
            f"legacy control {control} must not depend on an SF upstream"
        )


def test_first_batch_upstreams_exist_from_phase_1():
    late = set(sf_lib.SF_NODE_IDS)
    for node_id in sf_lib.SF_FIRST_BATCH:
        assert sf_lib.PINNED_UPSTREAMS[node_id] in set(sf_lib.LEGACY_NODE_IDS), (
            f"{node_id} is first-batch but its upstream starts late"
        )
    # And the late chain: sf_pfn1 <- sf_vfn1 (first batch), sf_vfn2 <- sf_val1.
    assert sf_lib.PINNED_UPSTREAMS["sf_pfn1"] in sf_lib.SF_FIRST_BATCH
    assert sf_lib.PINNED_UPSTREAMS["sf_vfn2"] == "sf_val1"
    assert sf_lib.TX_ENTRY_NODE in sf_lib.LEGACY_NODE_IDS


# ---------------------------------------------------------------------------
# sf_lib: mode knob
# ---------------------------------------------------------------------------


def test_resolve_sf_mode_default_and_params():
    assert sf_lib.resolve_sf_mode({}, {}) == "migrate"
    assert sf_lib.resolve_sf_mode({"sf": {"mode": "migrate"}}, {}) == "migrate"


def test_resolve_sf_mode_env_overrides_params():
    assert (
        sf_lib.resolve_sf_mode(
            {"sf": {"mode": "flag"}}, {"GRAVITY_SF_MODE": "migrate"}
        )
        == "migrate"
    )


def test_resolve_sf_mode_rejects_unknown():
    with pytest.raises(ValueError, match="mode must be one of"):
        sf_lib.resolve_sf_mode({"sf": {"mode": "bogus"}}, {})


def test_resolve_sf_mode_accepts_both_wired_forms():
    # Form B landed with greth's feat/sf-fresh-init; both forms execute.
    assert sf_lib.resolve_sf_mode({"sf": {"mode": "flag"}}, {}) == "flag"
    assert sf_lib.resolve_sf_mode({"sf": {"mode": "migrate"}}, {}) == "migrate"


def test_sf_start_extra_args():
    assert tuple(sf_lib.sf_start_extra_args("migrate")) == ()
    # Bare flag == true: clap's `num_args = 0..=1,
    # default_missing_value = "true"` on feat/sf-fresh-init storage.rs.
    assert tuple(sf_lib.sf_start_extra_args("flag")) == ("--storage.v2",)
    with pytest.raises(ValueError):
        sf_lib.sf_start_extra_args("bogus")


def test_inject_sf_v2_flag_reth_args_is_pure_and_preserving():
    original = {
        "reth_args": {"datadir": "/x", "dev": ""},
        "env_vars": {"BATCH_INSERT_TIME": 20},
    }
    injected = sf_lib.inject_sf_v2_flag_reth_args(original)
    # Empty value -> the generated start.sh emits the bare --storage.v2.
    assert injected["reth_args"][sf_lib.SF_FLAG_RETH_ARG] == ""
    assert injected["reth_args"]["datadir"] == "/x"
    assert injected["env_vars"] == {"BATCH_INSERT_TIME": 20}
    # Pure: the input dict is untouched.
    assert sf_lib.SF_FLAG_RETH_ARG not in original["reth_args"]
    # reth_args absent/null — created.
    assert (
        sf_lib.inject_sf_v2_flag_reth_args({})["reth_args"][
            sf_lib.SF_FLAG_RETH_ARG
        ]
        == ""
    )


def test_deploy_start_scripts_emit_bare_flags_for_empty_values():
    """The injection channel's contract: deploy.sh's generated start.sh
    turns an empty-valued reth_args entry into a bare `--key` — exactly
    how `--storage.v2` must reach clap (bare == true)."""
    deploy_sh = (CASES_DIR.parent.parent / "cluster" / "deploy.sh").read_text()
    assert 'reth_args_array+=( "--${key}" )' in deploy_sh


def test_flag_matches_greth_clap_definition():
    """Static consistency with the greth clap definition when the
    feat/sf-fresh-init worktree is present (skips elsewhere); the
    contract is also comment-anchored at sf_lib.SF_FLAG_RETH_ARG."""
    storage_rs = Path(
        "/home/neko/gravity/gravity-reth-sf-fresh/crates/node/core/src/args/storage.rs"
    )
    if not storage_rs.exists():
        pytest.skip("greth feat/sf-fresh-init worktree not present")
    src = storage_rs.read_text()
    assert f'long = "{sf_lib.SF_FLAG_RETH_ARG}"' in src
    assert 'default_missing_value = "true"' in src, (
        "bare-flag == true assumption broken — update sf_start_extra_args"
    )
    assert "num_args = 0..=1" in src


# ---------------------------------------------------------------------------
# render_config: dual-source rendering + [sf] validation
# ---------------------------------------------------------------------------

TOY_CLUSTER_TPL = (
    "[[nodes]]\nid = 'a'\nsource = {{SOURCE}}\n"
    "[[nodes]]\nid = 'b'\nsource = {{SF_SOURCE}}\n"
)


def test_render_cluster_toml_substitutes_both_sources():
    rendered = render_config.render_cluster_toml(
        TOY_CLUSTER_TPL, {"bin_path": "/old"}, {"bin_path": "/new"}
    )
    assert '{ bin_path = "/old" }' in rendered
    assert '{ bin_path = "/new" }' in rendered
    assert "{{" not in rendered


def test_render_cluster_toml_requires_both_tables():
    with pytest.raises(ValueError, match=r"empty \[source\]"):
        render_config.render_cluster_toml(TOY_CLUSTER_TPL, {}, {"bin_path": "/n"})
    with pytest.raises(ValueError, match=r"empty \[sf_source\]"):
        render_config.render_cluster_toml(TOY_CLUSTER_TPL, {"bin_path": "/o"}, {})


def test_render_cluster_toml_requires_both_placeholders():
    with pytest.raises(ValueError, match="SF_SOURCE"):
        render_config.render_cluster_toml(
            "source = {{SOURCE}}", {"bin_path": "/o"}, {"bin_path": "/n"}
        )


def test_validate_sf_mode():
    assert render_config.validate_sf_mode({}) == "migrate"
    assert render_config.validate_sf_mode({"mode": "flag"}) == "flag"
    with pytest.raises(ValueError, match="mode must be one of"):
        render_config.validate_sf_mode({"mode": "bogus"})


def test_real_templates_render_to_valid_toml():
    cluster_tpl = (CASE_DIR / "cluster.toml.tpl").read_text()
    rendered = render_config.render_cluster_toml(
        cluster_tpl, {"bin_path": "/old/gravity_node"},
        {"bin_path": "/new/gravity_node"},
    )
    cluster = tomllib.loads(rendered)
    assert {n["id"] for n in cluster["nodes"]} == set(
        sf_lib.LEGACY_NODE_IDS
    ) | set(sf_lib.SF_NODE_IDS)

    genesis_tpl = (CASE_DIR / "genesis.toml.tpl").read_text()
    rendered_genesis = render_config.render_genesis_toml(
        genesis_tpl,
        {"betaBlock": 100, "alphaTime": "+40m"},
        {"repo": "https://example/repo.git", "ref": "main"},
        now=1_000_000,
    )
    genesis = tomllib.loads(rendered_genesis)
    assert genesis["genesis"]["hardforks"]["alphaTime"] == 1_000_000 + 40 * 60


# ---------------------------------------------------------------------------
# Config consistency: tpl x tpl x sf_lib
# ---------------------------------------------------------------------------


def _rendered_cluster():
    tpl = (CASE_DIR / "cluster.toml.tpl").read_text()
    rendered = render_config.render_cluster_toml(
        tpl, {"bin_path": "/o"}, {"bin_path": "/n"}
    )
    return tomllib.loads(rendered)


def _rendered_genesis():
    tpl = (CASE_DIR / "genesis.toml.tpl").read_text()
    rendered = render_config.render_genesis_toml(
        tpl, {}, {"repo": "r", "ref": "f"}
    )
    return tomllib.loads(rendered)


def test_genesis_validators_match_cluster_ports_and_stake():
    cluster = {n["id"]: n for n in _rendered_cluster()["nodes"]}
    genesis = _rendered_genesis()
    validators = {v["id"]: v for v in genesis["genesis_validators"]}
    assert set(validators) == {"node1", "node2"}
    for vid, v in validators.items():
        assert v["validator_port"] == cluster[vid]["validator_port"]
        assert v["vfn_port"] == cluster[vid]["vfn_port"]
        assert int(v["stake_amount"]) == sf_lib.GENESIS_STAKE_WEI
        assert int(v["voting_power"]) == sf_lib.GENESIS_STAKE_WEI


def test_join_stake_is_equal_power():
    # L3's prerequisite: sf_val1 joins with EXACTLY the genesis stake.
    assert int(float(sf_lib.JOIN_STAKE_ETH) * 10**18) == sf_lib.GENESIS_STAKE_WEI


def test_epoch_interval_matches_genesis():
    genesis = _rendered_genesis()
    assert (
        genesis["genesis"]["epoch_interval_micros"]
        == sf_lib.EPOCH_INTERVAL_S * 1_000_000
    )


def test_voting_power_increase_limit_within_contract_cap():
    # The contract hard-caps this knob at MAX_VOTING_POWER_INCREASE_LIMIT = 50
    # (genesis reverts above it). The +50% equal-power join still lands because
    # the first activation per epoch bypasses the limit (whale clause,
    # ValidatorManagement.sol `addedPower > 0`).
    genesis = _rendered_genesis()
    pct = genesis["genesis"]["validator_config"]["voting_power_increase_limit_pct"]
    assert 0 < pct <= 50


def test_role_specific_ports():
    nodes = {n["id"]: n for n in _rendered_cluster()["nodes"]}
    for node_id, node in nodes.items():
        role = node["role"]
        if role == "pfn":
            assert "validator_port" not in node and "vfn_port" not in node, (
                f"{node_id}: pfn must not carry validator/vfn ports"
            )
            assert "public_port" in node
        if role in ("genesis", "validator"):
            assert "public_port" not in node, (
                f"{node_id}: validator roles must not carry public_port"
            )
    # sf_val1 is sf_vfn2's upstream: the VFN listener must exist.
    assert "vfn_port" in nodes["sf_val1"]
    # Upstreams of pfns must expose a Public listener.
    for pfn in ("pfn1", "sf_pfn1", "sf_pfn2"):
        upstream = sf_lib.PINNED_UPSTREAMS[pfn]
        assert "public_port" in nodes[upstream], (
            f"{pfn}'s upstream {upstream} lacks a public_port"
        )


PORT_KEYS = (
    "validator_port",
    "vfn_port",
    "public_port",
    "rpc_port",
    "metrics_port",
    "inspection_port",
    "https_port",
    "authrpc_port",
    "reth_p2p_port",
)


def _all_ports(cluster_dict):
    ports = []
    for node in cluster_dict["nodes"]:
        for key in PORT_KEYS:
            if key in node:
                ports.append(node[key])
    return ports


def test_ports_pairwise_unique_and_disjoint_from_upgrade_suite():
    ports = _all_ports(_rendered_cluster())
    assert len(ports) == len(set(ports)), "duplicate ports inside the suite"

    upgrade_tpl = (CASES_DIR / "storage_v2_upgrade" / "cluster.toml.tpl").read_text()
    upgrade = tomllib.loads(upgrade_tpl.replace("{{SOURCE}}", '{ bin_path = "/o" }'))
    upgrade_ports = set(_all_ports(upgrade))
    overlap = upgrade_ports & set(ports)
    assert not overlap, f"ports collide with storage_v2_upgrade: {overlap}"


def test_seeds_encode_the_pinned_upstreams():
    nodes = {n["id"]: n for n in _rendered_cluster()["nodes"]}
    for downstream, upstream in sf_lib.PINNED_UPSTREAMS.items():
        seeds = nodes[downstream].get("seeds", [])
        froms = {s["from"] for s in seeds}
        assert froms == {upstream}, (
            f"{downstream}: seeds {froms} != pinned upstream {{{upstream!r}}}"
        )
        if nodes[downstream]["role"] == "vfn":
            assert nodes[downstream].get("discovery_method") == "none", (
                f"{downstream}: vfn with static seeds must pin discovery off"
            )


# ---------------------------------------------------------------------------
# Account discipline (constraint (4)): background sender vs faucet
# ---------------------------------------------------------------------------

CASE_SOURCE = (CASE_DIR / "test_storage_v2_fresh_sync.py").read_text()


def test_background_sender_never_uses_the_faucet_account():
    """Regression lock for the live-attempt2 nonce race ("nonce too low"
    -> "replacement transaction underpriced"): the TxSender runs across
    phases 3-10 concurrently with explicit faucet transactions, so it
    must be constructed with the dedicated phase-2 account — never with
    cluster.faucet."""
    marker = "ctx.tx_sender = TxSender("
    assert marker in CASE_SOURCE, "TxSender wiring moved — update this lock"
    for start in range(0, len(CASE_SOURCE)):
        idx = CASE_SOURCE.find(marker, start)
        if idx == -1:
            break
        window = CASE_SOURCE[idx : idx + 400]
        assert "cluster.faucet" not in window.split(")")[0], (
            "TxSender is fed the faucet account — that races every "
            "foreground faucet tx (B-batch history, governance)"
        )
        assert "bg_sender_account" in window, (
            "TxSender must run on the dedicated phase-2 account"
        )
        start = idx + 1


def test_bg_sender_account_is_funded_before_the_sender_starts():
    funding = CASE_SOURCE.find("background-sender funding")
    sender_start = CASE_SOURCE.find("ctx.tx_sender = TxSender(")
    assert funding != -1, "phase 2 must fund the dedicated sender account"
    assert sender_start != -1
    # Phase 2 (funding) is defined before the test body (sender start) in
    # source order AND executed before it; the source-order check catches
    # someone moving the funding after the sender wiring.
    assert funding < sender_start


def test_faucet_init_matches_the_fund_flow_model():
    """The three-account model: bench[0] = sf_val1's join signer (one
    VALIDATOR-role node), bench[1] = the bank. [faucet_init] must produce
    exactly sf_lib.FAUCET_INIT_NUM_ACCOUNTS accounts — fewer starves the
    join or the bank, more silently dilutes the swept-faucet split."""
    cluster = _rendered_cluster()
    validator_role_nodes = [
        n for n in cluster["nodes"] if n["role"] == "validator"
    ]
    faucet_init = cluster.get("faucet_init")
    assert faucet_init, "cluster.toml.tpl must carry [faucet_init] (join needs it)"
    assert faucet_init["num_accounts"] == sf_lib.FAUCET_INIT_NUM_ACCOUNTS
    assert len(validator_role_nodes) == sf_lib.BANK_BENCH_INDEX, (
        "bench rows before the bank must all be VALIDATOR-role join "
        "signers (manager assigns accounts.csv rows to VALIDATOR nodes "
        "in order)"
    )


def test_bench_partition_is_disjoint():
    assert sf_lib.JOIN_BENCH_INDEX != sf_lib.BANK_BENCH_INDEX
    assert 0 <= sf_lib.JOIN_BENCH_INDEX < sf_lib.FAUCET_INIT_NUM_ACCOUNTS
    assert 0 <= sf_lib.BANK_BENCH_INDEX < sf_lib.FAUCET_INIT_NUM_ACCOUNTS


def test_faucet_never_sends_value_transactions():
    """Post-sweep, the faucet holds ~0.5 ETH of gas budget only
    (gravity_bench main.rs:384-411 sweeps its balance at suite init —
    live attempt3 died funding 1 ETH from a 0.488 ETH faucet). Every
    TransactionBuilder (the value-transfer path) must be constructed
    with the bank or another funded account, never the faucet; the
    faucet's only legitimate use is the governance recipe's gas."""
    import re

    for match in re.finditer(r"TransactionBuilder\(([^)]*)\)", CASE_SOURCE):
        args = match.group(1)
        assert "faucet" not in args, (
            f"TransactionBuilder fed the faucet account: "
            f"TransactionBuilder({args}) — use the bank (constraint (4))"
        )


# ---------------------------------------------------------------------------
# Wipe discipline: fresh chain data, intact deployment skeleton
# ---------------------------------------------------------------------------

from types import SimpleNamespace  # noqa: E402


def _load_main_case_module():
    sys.path.insert(0, str(CASE_DIR))
    try:
        return _load_case_module(
            "test_storage_v2_fresh_sync.py", "storage_fresh_sync_case_main"
        )
    finally:
        sys.path.remove(str(CASE_DIR))


def test_wipe_clears_chain_data_but_preserves_skeleton(tmp_path):
    """Live-attempt4 lesson: the per-node script/ pair IS the node's pid
    bookkeeping (start.sh rewrites node.pid on every start). The wipe
    must reduce a runner-started node to fresh-init semantics — empty
    chain data — while the deployment skeleton survives untouched."""
    case = _load_main_case_module()

    node_dir = tmp_path / "sf_vfn1"
    (node_dir / "data" / "reth" / "db").mkdir(parents=True)
    (node_dir / "data" / "reth" / "db" / "mdbx.dat").write_text("x")
    (node_dir / "data" / "consensus_db").mkdir()
    (node_dir / "data" / "consensus_db" / "000001.log").write_text("x")
    (node_dir / "data" / "secure_storage.json").write_text("{}")
    (node_dir / "script").mkdir()
    (node_dir / "script" / "start.sh").write_text("#!/bin/bash")
    (node_dir / "script" / "stop.sh").write_text("#!/bin/bash")
    (node_dir / "script" / "node.pid").write_text("12345")
    (node_dir / "bin").mkdir()
    (node_dir / "bin" / "gravity_node").write_text("ELF")
    (node_dir / "config").mkdir()
    (node_dir / "config" / "identity.yaml").write_text("id")
    (node_dir / "logs").mkdir()
    (node_dir / "logs" / "debug.log").write_text("boot noise")
    (node_dir / "execution_logs").mkdir()
    (node_dir / "execution_logs" / "node.log").write_text("boot noise")

    node = SimpleNamespace(
        id="sf_vfn1",
        _infra_path=node_dir,
        pid_file=node_dir / "script" / "node.pid",
        start_script=node_dir / "script" / "start.sh",
        stop_script=node_dir / "script" / "stop.sh",
    )
    case.wipe_node_to_fresh(node)

    data_dir = node_dir / "data"
    assert data_dir.is_dir() and not any(data_dir.iterdir()), (
        "chain data must be gone, the data dir itself must remain"
    )
    assert (node_dir / "script" / "start.sh").exists()
    assert (node_dir / "script" / "stop.sh").exists()
    assert not (node_dir / "script" / "node.pid").exists(), (
        "stale pid bookkeeping must be dropped"
    )
    assert (node_dir / "bin" / "gravity_node").exists()
    assert (node_dir / "config" / "identity.yaml").exists()
    assert not (node_dir / "logs" / "debug.log").exists()
    assert not any((node_dir / "execution_logs").iterdir())


def test_deploy_start_scripts_rewrite_the_pid_file():
    """The self-healing restart cycles (storage_v2_upgrade's phases 9/16,
    this case's D-form hook) depend on every per-node start.sh REWRITING
    script/node.pid after spawning gravity_node — validator, pfn and vfn
    heredocs alike. If deploy.sh loses that line, stops silently become
    no-ops ("No PID file found", exit 0)."""
    deploy_sh = (CASES_DIR.parent.parent / "cluster" / "deploy.sh").read_text()
    writes = deploy_sh.count('echo $pid > "${WORKSPACE}/script/node.pid"')
    assert writes >= 3, (
        f"expected the validator/pfn/vfn start.sh heredocs to each write "
        f"the pid file, found {writes} write sites"
    )


# ---------------------------------------------------------------------------
# Compressed Alpha schedule vs the case guards (attempt5 chain-age lesson)
# ---------------------------------------------------------------------------


def test_example_alpha_schedule_clears_the_case_guards():
    """The example schedule is compressed (+12m/+5m/+5m — every extra
    minute of schedule is extra chain age the SF nodes replay from 0 at
    ~1.4-1.9 blk/s net). It must still clear the phase-1 lead floor with
    slack for the runner's init/deploy window (~4-5 min between render
    and the guard)."""
    example = tomllib.loads(
        (CASE_DIR / "test_params.toml.example").read_text()
    )
    components = example["hardforks"]["alphaTime"]
    total_s = 0
    for name, value in components.items():
        assert value.startswith("+") and value.endswith("m"), (
            f"component {name}={value!r} must be a '+NNm' offset"
        )
        total_s += int(value[1:-1]) * 60
    assert total_s == 22 * 60, "schedule drifted from the documented tier"

    case = _load_main_case_module()
    runner_init_budget_s = 5 * 60
    assert total_s - runner_init_budget_s >= case.ALPHA_MIN_LEAD_S, (
        "schedule leaves less than the phase-1 lead floor after a "
        "worst-case runner init window"
    )
    assert case.ALPHA_MIN_LEAD_S < total_s
    assert case.ALPHA_UPGRADE_MARGIN_S < case.ALPHA_MIN_LEAD_S


# ---------------------------------------------------------------------------
# Sync-tick injection: from-0 sync runs on the live loaded chain
# ---------------------------------------------------------------------------


def _func_source(name: str) -> str:
    """The body of one top-level (async) function in the case source."""
    import re

    match = re.search(
        rf"^(?:async )?def {name}\(.*?(?=^(?:async )?def |\Z)",
        CASE_SOURCE,
        re.DOTALL | re.MULTILINE,
    )
    assert match, f"function {name} not found in the case source"
    return match.group(0)


def test_freeze_and_quiet_machinery_is_retired():
    """The tick investigation refuted the capacity readings behind
    quiet_chain/frozen_tip (deep sync was tick-limited, not capacity
    limited) — the machinery must stay retired; from-0 sync runs against
    the live, loaded chain."""
    assert "frozen_tip" not in CASE_SOURCE.replace(
        "frozen_tip / quiet_chain", ""
    ), "frozen_tip machinery must stay retired (git history has it)"
    for phase in (
        "phase_4_sf_first_batch",
        "phase_7_sf_val1_join",
        "phase_8_sf_vfn2_matrix_close",
    ):
        src = _func_source(phase)
        assert "quiet_chain" not in src and ".pause(" not in src, (
            f"{phase}: from-0 sync must run under load on the live chain"
        )


def test_sync_side_is_untouched():
    """The nodes under test keep their sync path stock: no sync-driver
    env injection anywhere in the case (the feasibility lever is the
    CHAIN's pace, not the DUT)."""
    assert "GRAVITY_REQUEST_SYNC_INFO_INTERVAL_MS" not in CASE_SOURCE
    sf_lib_source = (CASE_DIR / "sf_lib.py").read_text()
    assert "GRAVITY_REQUEST_SYNC_INFO_INTERVAL_MS" not in sf_lib_source


# ---------------------------------------------------------------------------
# Chain-rate assumptions: central constants + derivations + deploy wiring
# ---------------------------------------------------------------------------


def test_chain_rate_derivations():
    """Every rate-coupled constant flows from the central block; a
    future re-pacing is PROPOSER_SLEEP_MS + these locks."""
    assert sf_lib.PROPOSER_SLEEP_MS == 1000
    assert sf_lib.CHAIN_BLOCK_RATE_BPS == pytest.approx(
        1000.0 / (sf_lib.PROPOSER_SLEEP_MS + sf_lib.ROUND_OVERHEAD_MS)
    )
    # ~1 blk/s target band.
    assert 0.8 <= sf_lib.CHAIN_BLOCK_RATE_BPS <= 1.1
    # The sync driver must decisively outrun the slowed chain.
    assert sf_lib.SYNC_RATE_FLOOR_BPS >= 4 * sf_lib.CHAIN_BLOCK_RATE_BPS
    assert sf_lib.NET_CATCHUP_FLOOR_BPS == pytest.approx(
        sf_lib.SYNC_RATE_FLOOR_BPS - sf_lib.CHAIN_BLOCK_RATE_BPS
    )
    assert sf_lib.NET_CATCHUP_FLOOR_BPS >= 2.5
    assert sf_lib.BLOCKS_PER_EPOCH == int(
        sf_lib.EPOCH_INTERVAL_S * sf_lib.CHAIN_BLOCK_RATE_BPS
    )
    assert sf_lib.CATCHUP_STALL_WINDOW_S == max(
        2 * sf_lib.EPOCH_INTERVAL_S,
        3 * sf_lib.BLOCKS_PER_EPOCH / sf_lib.SYNC_RATE_FLOOR_BPS,
    )
    # Epoch-boundary flats (DKG etc.) must never read as stalls.
    assert sf_lib.CATCHUP_STALL_WINDOW_S >= 2 * sf_lib.EPOCH_INTERVAL_S


def test_gamma_block_sits_after_fleet_completion():
    """gammaBlock derivation: fleet completion ≈ 14 min of chain time
    (~render+19 min minus ~5 min pre-chain runner work) ≈ 790 blocks at
    the slowed rate; gamma must land after that with margin, yet be
    crossed within ~25 min of chain time."""
    completion_blocks = 14 * 60 * sf_lib.CHAIN_BLOCK_RATE_BPS
    assert sf_lib.GAMMA_BLOCK > completion_blocks * 1.2
    assert sf_lib.GAMMA_BLOCK <= 25 * 60 * sf_lib.CHAIN_BLOCK_RATE_BPS

    example = tomllib.loads(
        (CASE_DIR / "test_params.toml.example").read_text()
    )
    assert example["hardforks"]["gammaBlock"] == sf_lib.GAMMA_BLOCK


def test_proposer_pacing_template():
    """The case-local validator reth_config.json.tpl (runner auto-picks
    it as RETH_CONFIG_TPL) must bake APTOS_PROPOSER_SLEEP_MS into
    env_vars — round_manager.rs:389-396's unconditional per-round
    proposer sleep is the pacing knob (quorum_store_poll_time_ms is dead
    code in this fork: quorum_store_client.rs:124 `done = true`). Only
    the validator template is overridden — fullnodes never propose and
    stay on the stock templates."""
    import re

    tpl = (CASE_DIR / "reth_config.json.tpl").read_text()
    match = re.search(r'"APTOS_PROPOSER_SLEEP_MS":\s*(\d+)', tpl)
    assert match, "the template must carry the proposer-sleep env var"
    assert int(match.group(1)) == sf_lib.PROPOSER_SLEEP_MS
    assert '"env_vars"' in tpl
    # The stock template entries must survive the override.
    assert '"BATCH_INSERT_TIME"' in tpl
    assert not (CASE_DIR / "reth_config_vfn.json.tpl").exists()
    assert not (CASE_DIR / "reth_config_pfn.json.tpl").exists()


def test_pacing_verification_wiring():
    """phase 1 must verify the pacing reached every validator-role
    node's deployed config (node1, node2, sf_val1) — fail fast instead
    of timing out on wrong-rate budgets an hour later."""
    phase1 = _func_source("phase_1_bootstrap_legacy_core")
    assert "assert_proposer_pacing_configured(" in phase1
    for validator in ('"node1"', '"node2"', '"sf_val1"'):
        assert validator in phase1
    checker = _func_source("assert_proposer_pacing_configured")
    assert "APTOS_PROPOSER_SLEEP_MS" in checker
    assert "PROPOSER_SLEEP_MS" in checker


def test_catchup_calls_use_the_derived_constants():
    for func in ("sync_to_tip", "restart_node_and_catch_up"):
        src = _func_source(func)
        assert "CATCHUP_STALL_WINDOW_S" in src, (
            f"{func}: stall window must come from the central rate block"
        )
        assert "NET_CATCHUP_FLOOR_BPS" in src, (
            f"{func}: budget floor must come from the central rate block"
        )


def test_probe_restarts_stay_under_load():
    """Short-gap probe restarts keep the load on — only the rolling
    upgrade swap windows, pfn1's own probe (the sender ingress) and the
    L3 necessity probe pause the sender."""
    for func in ("restart_node_and_catch_up", "offline_sf_probe_and_restart"):
        src = _func_source(func)
        assert ".pause(" not in src, (
            f"{func}: probe-style catch-up must not pause the load"
        )


def test_sync_to_tip_chases_the_live_tip():
    src = _func_source("sync_to_tip")
    assert 'get_node("node1")' in src, (
        "the catch-up reference is the live tip (node1)"
    )
    assert "FROZEN" not in src and "FREEZE" not in src


# ---------------------------------------------------------------------------
# D-form first-stop safety (attempt8: early-init SIGTERM ignored 109s)
# ---------------------------------------------------------------------------


def test_first_stop_ready_criteria():
    ready = sf_lib.first_stop_ready
    min_up = sf_lib.FIRST_STOP_MIN_UPTIME_S
    h = sf_lib.FIRST_STOP_READY_HEIGHT
    max_wait = sf_lib.FIRST_STOP_MAX_WAIT_S

    # Hard uptime floor: never ready before it, whatever the height.
    assert not ready(min_up - 1, h * 100)
    assert not ready(1, 10**9)
    # Past the floor: synced-enough qualifies immediately...
    assert ready(min_up, h)
    assert not ready(min_up, h - 1)
    # ...and the wall-clock cap qualifies regardless of height (RPC
    # flaps report -1).
    assert ready(max_wait, -1)
    assert not ready(max_wait - 1, h - 1)
    # The loop always terminates: by max_wait it is ready for ANY height.
    assert ready(max_wait, 0)
    # Sane ordering of the constants themselves.
    assert 0 < min_up <= max_wait


def test_first_stop_wiring_and_sigkill_scoping():
    """start_fresh_sf_node must gate its first stop on first_stop_ready
    and may use the SIGKILL last resort (fresh data, zero value); the
    probe paths must never pass allow_sigkill."""
    enable = _func_source("start_fresh_sf_node")
    assert "first_stop_ready(" in enable, (
        "the D-form first stop must wait for a stable runtime (attempt8)"
    )
    assert "allow_sigkill=True" in enable

    phase1 = _func_source("phase_1_bootstrap_legacy_core")
    assert "allow_sigkill=True" in phase1, (
        "the pre-wipe stops may escalate (data is erased next line)"
    )

    for func in (
        "offline_sf_probe_and_restart",
        "phase_5_layout_probes",
        "phase_9_l3_necessity_probe",
    ):
        assert "allow_sigkill" not in _func_source(func), (
            f"{func}: probe/long-lived stops must stay strict — "
            f"auto-SIGKILL would mask real shutdown regressions"
        )


# ---------------------------------------------------------------------------
# Form B ("flag") wiring: mode differential + SF-only injection
# ---------------------------------------------------------------------------


def test_flag_mode_bypasses_the_d_form_machinery():
    """Mode differential: in flag mode the SF node is BORN on SF — the
    enable path must return before the stable-runtime wait / offline
    migration / restart machinery; migrate mode keeps all of it."""
    enable = _func_source("start_fresh_sf_node")
    flag_idx = enable.index('if ctx.sf_mode == "flag"')
    d_form_idx = enable.index("first_stop_ready(")
    assert flag_idx < d_form_idx, "the flag branch must come first"
    assert "return" in enable[flag_idx:d_form_idx], (
        "flag mode must return before the D-form machinery"
    )
    # The D-form machinery itself must remain intact (compatibility).
    assert "migrate_changesets(" in enable
    assert "allow_sigkill=True" in enable


def test_flag_injection_is_sf_only_and_mode_gated():
    """The --storage.v2 deploy-config patch runs only in flag mode, only
    inside the phase-1 SF loop, and the injector hard-refuses legacy
    nodes (they must fresh-init legacy if ever re-inited)."""
    phase1 = _func_source("phase_1_bootstrap_legacy_core")
    patch_idx = phase1.index("inject_sf_v2_flag(")
    gate_idx = phase1.index('if ctx.sf_mode == "flag"')
    assert gate_idx < patch_idx, "the patch must be flag-mode gated"

    injector = _func_source("inject_sf_v2_flag")
    assert "SF_NODE_IDS" in injector, (
        "the injector must guard against legacy nodes"
    )
    # Exactly one call site (the phase-1 SF loop) plus the definition.
    assert CASE_SOURCE.count("inject_sf_v2_flag(") == 2
