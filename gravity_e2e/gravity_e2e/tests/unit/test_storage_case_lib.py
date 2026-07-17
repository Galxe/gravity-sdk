"""
Unit tests for gravity_e2e.helpers.storage_case_lib (shared pure helpers of
the storage-v2 cases). Moved from test_storage_baseline_case.py when the
library graduated from case-local (storage_v2_baseline) to shared:

- anchor-spec construction from recorded history;
- offline db env derivation (encodes the cluster/deploy.sh datadir layout);
- set(uint256) calldata encoding (checked against the canonical selector
  0x60fe47b1, independently visible in the tracked
  prague/contracts/Counter.json bytecode).
"""

from pathlib import Path

import pytest

from gravity_e2e.helpers import storage_case_lib as lib
from gravity_e2e.helpers.storage_anchors import Anchor, AnchorSet


def _history(**overrides):
    """A valid OnChainHistory with readable, distinct block numbers."""
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
    return lib.OnChainHistory(**kwargs)


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
    assert data.endswith(
        "00000000000000000000000000000000000000000000000000000000deadbeef"
    )


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


def test_derive_offline_env_other_node_id():
    env = lib.derive_offline_env("/tmp/gravity-cluster-x", "vfn1")
    assert env.binary == Path("/tmp/gravity-cluster-x/vfn1/bin/gravity_node")
    assert env.datadir == Path("/tmp/gravity-cluster-x/vfn1/data/reth")


# ---------------------------------------------------------------------------
# assert_history_is_anchorable: positive controls on collected anchors
# ---------------------------------------------------------------------------

TRANSFERS_WEI = [10**18, 2 * 10**18]
SET_VALUES = [1, 2]


def _anchor(kind, params, expected):
    return Anchor(kind=kind, params=params, expected=expected)


def _good_anchor_set(h) -> AnchorSet:
    """An AnchorSet consistent with _history() + TRANSFERS_WEI/SET_VALUES."""
    anchors = []
    cumulative = 0
    for point, amount in zip(h.transfers, TRANSFERS_WEI):
        cumulative += amount
        anchors.append(
            _anchor(
                "balance",
                {"address": h.recipient.lower(), "block_number": point.block_number},
                cumulative,
            )
        )
    for point, value in zip(h.sets, SET_VALUES):
        anchors.append(
            _anchor(
                "storage",
                {
                    "contract": h.contract.lower(),
                    "slot": "0x0",
                    "block_number": point.block_number,
                },
                "0x" + format(value, "064x"),
            )
        )
    for point in (*h.transfers, h.deploy, *h.sets):
        anchors.append(
            _anchor("transaction", {"tx_hash": point.tx_hash}, {"exists": True})
        )
        anchors.append(
            _anchor("receipt", {"tx_hash": point.tx_hash}, {"exists": True})
        )
        anchors.append(
            _anchor("block_hash", {"block_number": point.block_number}, {})
        )
    anchors.append(
        _anchor(
            "logs",
            {
                "from_block": h.deploy.block_number,
                "to_block": h.sets[-1].block_number,
                "address": h.contract.lower(),
            },
            [{"log": i} for i in range(len(h.sets))],
        )
    )
    return AnchorSet(anchors=anchors)


def test_assert_history_is_anchorable_accepts_consistent_set():
    h = _history()
    lib.assert_history_is_anchorable(
        _good_anchor_set(h), h, TRANSFERS_WEI, SET_VALUES
    )


def test_assert_history_is_anchorable_rejects_missing_kind():
    h = _history()
    anchor_set = _good_anchor_set(h)
    anchor_set.anchors = [a for a in anchor_set.anchors if a.kind != "storage"]
    with pytest.raises(AssertionError, match="no storage anchors"):
        lib.assert_history_is_anchorable(anchor_set, h, TRANSFERS_WEI, SET_VALUES)


def test_assert_history_is_anchorable_rejects_wrong_slot_value():
    h = _history()
    anchor_set = _good_anchor_set(h)
    for anchor in anchor_set.anchors:
        if anchor.kind == "storage":
            anchor.expected = "0x" + format(99, "064x")
    with pytest.raises(AssertionError, match="slot 0"):
        lib.assert_history_is_anchorable(anchor_set, h, TRANSFERS_WEI, SET_VALUES)


def test_assert_history_is_anchorable_rejects_wrong_balance():
    h = _history()
    anchor_set = _good_anchor_set(h)
    for anchor in anchor_set.anchors:
        if anchor.kind == "balance":
            anchor.expected = 0
    with pytest.raises(AssertionError, match="recipient balance"):
        lib.assert_history_is_anchorable(anchor_set, h, TRANSFERS_WEI, SET_VALUES)


def test_assert_history_is_anchorable_rejects_missing_logs():
    h = _history()
    anchor_set = _good_anchor_set(h)
    for anchor in anchor_set.anchors:
        if anchor.kind == "logs":
            anchor.expected = []
    with pytest.raises(AssertionError, match="ValueSet logs"):
        lib.assert_history_is_anchorable(anchor_set, h, TRANSFERS_WEI, SET_VALUES)


# ---------------------------------------------------------------------------
# assert_sf_layout / assert_upgraded_legacy_layout (injected probes)
# ---------------------------------------------------------------------------

from gravity_e2e.helpers.offline_db import (  # noqa: E402
    ACCOUNT_CHANGESETS_TABLE,
    STORAGE_CHANGESETS_TABLE,
    OfflineDbEnv,
    SettingsState,
)

ENV = OfflineDbEnv(
    binary="/x/bin/gravity_node",
    datadir="/x/data/reth",
    chain="/x/genesis.json",
    static_files_dir="/x/data/reth",
)


class StubProbe:
    def __init__(self, state, error=None):
        self.state = state
        self.error = error

    def summary(self):
        return f"stub settings probe: {self.state}"


class StubLayout:
    def __init__(self, populated: bool):
        names = [Path("static_file_x_0_1")] if populated else []
        self.static_files_dir = Path("/x/data/reth")
        self.exists = True
        self.account_segments = list(names)
        self.storage_segments = list(names)
        self.account_sidecars = list(names)
        self.storage_sidecars = list(names)

    @property
    def has_segment_files(self):
        return bool(self.account_segments or self.storage_segments)

    @property
    def has_sidecar_files(self):
        return bool(self.account_sidecars or self.storage_sidecars)


class StubCount:
    def __init__(self, count, error=None):
        self.count = count
        self.error = error

    def summary(self):
        return f"stub count: {self.count}"


def _probes(state, populated_sf, count):
    return dict(
        read_settings=lambda env: StubProbe(state),
        inspect_sf=lambda datadir, sf_dir: StubLayout(populated_sf),
        count_entries=lambda env, table: StubCount(count),
    )


def test_assert_sf_layout_happy_path_returns_zero_counts():
    counts = lib.assert_sf_layout(
        ENV, "t", **_probes(SettingsState.PRESENT_STATIC_FILES, True, 0)
    )
    assert counts == {ACCOUNT_CHANGESETS_TABLE: 0, STORAGE_CHANGESETS_TABLE: 0}


def test_assert_sf_layout_rejects_unflipped_settings():
    with pytest.raises(AssertionError, match="PRESENT_STATIC_FILES"):
        lib.assert_sf_layout(ENV, "t", **_probes(SettingsState.MISSING, True, 0))


def test_assert_sf_layout_rejects_missing_segments():
    with pytest.raises(AssertionError, match="segments"):
        lib.assert_sf_layout(
            ENV, "t", **_probes(SettingsState.PRESENT_STATIC_FILES, False, 0)
        )


def test_assert_sf_layout_rejects_populated_tables():
    with pytest.raises(AssertionError, match="still has"):
        lib.assert_sf_layout(
            ENV, "t", **_probes(SettingsState.PRESENT_STATIC_FILES, True, 7)
        )


def test_assert_upgraded_legacy_layout_happy_path_returns_counts():
    counts = lib.assert_upgraded_legacy_layout(
        ENV, "t", **_probes(SettingsState.MISSING, False, 12)
    )
    assert counts == {ACCOUNT_CHANGESETS_TABLE: 12, STORAGE_CHANGESETS_TABLE: 12}


def test_assert_upgraded_legacy_layout_rejects_present_settings():
    with pytest.raises(AssertionError, match="gravity_storage_settings entry"):
        lib.assert_upgraded_legacy_layout(
            ENV, "t", **_probes(SettingsState.PRESENT_LEGACY, False, 12)
        )


def test_assert_upgraded_legacy_layout_rejects_sf_files():
    with pytest.raises(AssertionError, match="segment files"):
        lib.assert_upgraded_legacy_layout(
            ENV, "t", **_probes(SettingsState.MISSING, True, 12)
        )


def test_assert_upgraded_legacy_layout_rejects_empty_tables():
    with pytest.raises(AssertionError, match="is empty"):
        lib.assert_upgraded_legacy_layout(
            ENV, "t", **_probes(SettingsState.MISSING, False, 0)
        )
