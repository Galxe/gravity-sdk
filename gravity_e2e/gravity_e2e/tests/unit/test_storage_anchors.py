"""
Unit tests for gravity_e2e.helpers.storage_anchors

All tests run against an in-memory fake w3 object (no cluster, no network).
The fake is dict-backed so tests can tamper with the "chain" to simulate
silent data corruption after a storage migration.
"""

import json

import pytest
from web3 import Web3
from web3.exceptions import TransactionNotFound

from gravity_e2e.helpers.storage_anchors import (
    Anchor,
    AnchorSet,
    AnchorSpec,
    collect_anchors,
    replay_anchors,
)

ADDR = "0x" + "aa" * 20
CONTRACT = "0x" + "bb" * 20
OTHER_CONTRACT = "0x" + "cc" * 20
TX1 = "0x" + "11" * 32
TX2 = "0x" + "22" * 32
BLOCK5_HASH = "0x" + "ab" * 32
BLOCK5_PARENT = "0x" + "cd" * 32
TOPIC0 = "0x" + "77" * 32
TAMPERED_HASH = "0x" + "ef" * 32

ALL_KINDS = {"balance", "storage", "transaction", "receipt", "logs", "block_hash"}


def hx(hexstr: str) -> bytes:
    """Raw bytes from 0x-hex. Stands in for the HexBytes values a real w3
    returns: HexBytes subclasses bytes, so the same normalization branch is
    exercised without a direct dependency on the hexbytes package (which is
    not in requirements.txt)."""
    return bytes.fromhex(hexstr[2:] if hexstr.startswith("0x") else hexstr)


def make_chain():
    """Fresh fake chain state; each test gets its own copy to tamper with."""
    log_a = {
        "address": CONTRACT,
        "topics": [hx(TOPIC0)],
        "data": hx("0x" + "00" * 31 + "2a"),
        "blockNumber": 5,
        "logIndex": 0,
        "transactionHash": hx(TX1),
    }
    log_b = {
        "address": OTHER_CONTRACT,
        "topics": [hx(TOPIC0), hx(TX2)],
        "data": hx("0x"),
        "blockNumber": 6,
        "logIndex": 1,
        "transactionHash": hx(TX2),
    }
    return {
        "balances": {(ADDR, 5): 10**18, (ADDR, 7): 2 * 10**18},
        "storage": {
            (CONTRACT, 0, 5): hx("0x" + "00" * 31 + "01"),
            (CONTRACT, 1, 5): "0x2a",  # short hexstr on purpose
        },
        "txs": {TX1: {"blockHash": hx(BLOCK5_HASH), "blockNumber": 5}},
        "receipts": {
            TX1: {
                "status": 1,
                "gasUsed": 21000,
                "blockHash": hx(BLOCK5_HASH),
                "blockNumber": 5,
                "logs": [dict(log_a)],
            }
        },
        # intentionally unsorted: canonicalization must sort
        "logs": [log_b, log_a],
        "blocks": {
            5: {
                "number": 5,
                "hash": hx(BLOCK5_HASH),
                "parentHash": hx(BLOCK5_PARENT),
            }
        },
    }


class FakeEth:
    """Dict-backed stand-in for w3.eth (sync web3.py surface)."""

    def __init__(self, chain):
        self.chain = chain

    def get_balance(self, address, block_identifier):
        return self.chain["balances"][(address.lower(), block_identifier)]

    def get_storage_at(self, address, slot, block_identifier):
        return self.chain["storage"][(address.lower(), slot, block_identifier)]

    def get_transaction(self, tx_hash):
        tx = self.chain["txs"].get(tx_hash.lower())
        if tx is None:
            raise TransactionNotFound(f"Transaction {tx_hash} not found")
        return tx

    def get_transaction_receipt(self, tx_hash):
        receipt = self.chain["receipts"].get(tx_hash.lower())
        if receipt is None:
            raise TransactionNotFound(f"Receipt for {tx_hash} not found")
        return receipt

    def get_logs(self, filter_params):
        from_block = filter_params["fromBlock"]
        to_block = filter_params["toBlock"]
        address = filter_params.get("address")
        return [
            log
            for log in self.chain["logs"]
            if from_block <= log["blockNumber"] <= to_block
            and (address is None or log["address"].lower() == address.lower())
        ]

    def get_block(self, block_number):
        return self.chain["blocks"][block_number]


class FakeW3:
    def __init__(self, chain):
        self.eth = FakeEth(chain)


def make_spec():
    return AnchorSpec(
        # checksummed input on purpose: params must be normalized to lowercase
        balances=[(Web3.to_checksum_address(ADDR), 5), (ADDR, 7)],
        # one int slot, one hexstr slot
        storage_slots=[(CONTRACT, 0, 5), (CONTRACT, "0x1", 5)],
        tx_hashes=[TX1],
        block_numbers=[5],
        # one 3-tuple with address filter, one 2-tuple without
        log_ranges=[(5, 6, CONTRACT), (5, 6)],
    )


def collect_baseline():
    return collect_anchors(FakeW3(make_chain()), make_spec())


def anchors_by_kind(anchor_set, kind):
    return [a for a in anchor_set.anchors if a.kind == kind]


class TestCollect:
    def test_all_anchor_kinds_collected(self):
        anchor_set = collect_baseline()
        kinds = {a.kind for a in anchor_set.anchors}
        assert kinds == ALL_KINDS
        # 2 balances + 2 slots + 1 tx + 1 receipt + 1 block + 2 log ranges
        assert len(anchor_set) == 9

    def test_balance_anchor_canonical(self):
        anchor = anchors_by_kind(collect_baseline(), "balance")[0]
        assert anchor.params == {"address": ADDR, "block_number": 5}
        assert anchor.expected == 10**18

    def test_storage_anchor_canonical(self):
        slot0, slot1 = anchors_by_kind(collect_baseline(), "storage")
        assert slot0.params == {"contract": CONTRACT, "slot": "0x0", "block_number": 5}
        assert slot0.expected == "0x" + "00" * 31 + "01"
        # short hexstr value must be zero-padded to 32 bytes
        assert slot1.params["slot"] == "0x1"
        assert slot1.expected == "0x" + "00" * 31 + "2a"

    def test_transaction_anchor_canonical(self):
        anchor = anchors_by_kind(collect_baseline(), "transaction")[0]
        assert anchor.params == {"tx_hash": TX1}
        assert anchor.expected == {
            "exists": True,
            "block_hash": BLOCK5_HASH,
            "block_number": 5,
        }

    def test_receipt_anchor_canonical(self):
        anchor = anchors_by_kind(collect_baseline(), "receipt")[0]
        assert anchor.params == {"tx_hash": TX1}
        assert anchor.expected == {
            "exists": True,
            "status": 1,
            "gas_used": 21000,
            "block_hash": BLOCK5_HASH,
            "block_number": 5,
            "logs": [
                {
                    "address": CONTRACT,
                    "topics": [TOPIC0],
                    "data": "0x" + "00" * 31 + "2a",
                }
            ],
        }

    def test_block_hash_anchor_canonical(self):
        anchor = anchors_by_kind(collect_baseline(), "block_hash")[0]
        assert anchor.params == {"block_number": 5}
        assert anchor.expected == {
            "number": 5,
            "hash": BLOCK5_HASH,
            "parent_hash": BLOCK5_PARENT,
        }

    def test_logs_anchors_canonical_and_sorted(self):
        filtered, unfiltered = anchors_by_kind(collect_baseline(), "logs")
        assert filtered.params == {"from_block": 5, "to_block": 6, "address": CONTRACT}
        assert [log["block_number"] for log in filtered.expected] == [5]
        assert unfiltered.params == {"from_block": 5, "to_block": 6, "address": None}
        # fake returns logs unsorted; canonical form must sort by (block, index)
        assert [log["block_number"] for log in unfiltered.expected] == [5, 6]
        assert unfiltered.expected[0] == {
            "address": CONTRACT,
            "topics": [TOPIC0],
            "data": "0x" + "00" * 31 + "2a",
            "block_number": 5,
            "log_index": 0,
            "transaction_hash": TX1,
        }

    def test_empty_spec(self):
        anchor_set = collect_anchors(FakeW3(make_chain()), AnchorSpec())
        assert len(anchor_set) == 0
        report = replay_anchors(FakeW3(make_chain()), anchor_set)
        assert report.ok


class TestReplay:
    def test_all_match(self):
        anchor_set = collect_baseline()
        report = replay_anchors(FakeW3(make_chain()), anchor_set)
        assert report.ok
        assert report.mismatches == []
        assert report.total == 9
        assert report.matched == 9
        assert "9/9" in report.summary()

    @pytest.mark.parametrize(
        "kind,tamper",
        [
            ("balance", lambda c: c["balances"].__setitem__((ADDR, 5), 10**18 + 1)),
            (
                "storage",
                lambda c: c["storage"].__setitem__(
                    (CONTRACT, 0, 5), hx("0x" + "00" * 31 + "02")
                ),
            ),
            ("transaction", lambda c: c["txs"].pop(TX1)),
            ("receipt", lambda c: c["receipts"][TX1].__setitem__("gasUsed", 22000)),
            # drop the block-6 log: only the unfiltered range sees it
            ("logs", lambda c: c.__setitem__("logs", c["logs"][1:])),
            (
                "block_hash",
                lambda c: c["blocks"][5].__setitem__("hash", hx(TAMPERED_HASH)),
            ),
        ],
    )
    def test_each_kind_mismatch_detected(self, kind, tamper):
        anchor_set = collect_baseline()
        chain = make_chain()
        tamper(chain)
        report = replay_anchors(FakeW3(chain), anchor_set)
        assert not report.ok
        assert len(report.mismatches) == 1
        mismatch = report.mismatches[0]
        assert mismatch.kind == kind
        assert kind in mismatch.anchor_id
        assert mismatch.expected != mismatch.actual

    def test_deleted_tx_reported_as_not_existing(self):
        anchor_set = collect_baseline()
        chain = make_chain()
        del chain["txs"][TX1]
        report = replay_anchors(FakeW3(chain), anchor_set)
        (mismatch,) = report.mismatches
        assert mismatch.actual["exists"] is False

    def test_no_fail_fast_all_mismatches_reported(self):
        anchor_set = collect_baseline()
        chain = make_chain()
        chain["balances"][(ADDR, 5)] = 1
        chain["receipts"][TX1]["gasUsed"] = 22000
        chain["blocks"][5]["hash"] = hx(TAMPERED_HASH)
        report = replay_anchors(FakeW3(chain), anchor_set)
        assert not report.ok
        assert len(report.mismatches) == 3
        assert {m.kind for m in report.mismatches} == {
            "balance",
            "receipt",
            "block_hash",
        }
        summary = report.summary()
        assert "6/9" in summary
        for mismatch in report.mismatches:
            assert mismatch.anchor_id in summary

    def test_unknown_anchor_kind_reported_not_raised(self):
        """Forward-compat: an anchor file from a newer helper version must not crash."""
        anchor_set = AnchorSet(
            anchors=[Anchor(kind="debug_trace", params={"tx_hash": TX1}, expected={})]
        )
        report = replay_anchors(FakeW3(make_chain()), anchor_set)
        assert not report.ok
        assert "unknown anchor kind" in str(report.mismatches[0].actual)

    def test_replay_query_error_reported_not_raised(self):
        anchor_set = collect_baseline()

        class BrokenEth(FakeEth):
            def get_balance(self, address, block_identifier):
                raise RuntimeError("boom")

        w3 = FakeW3(make_chain())
        w3.eth = BrokenEth(make_chain())
        report = replay_anchors(w3, anchor_set)
        # both balance anchors fail, everything else still gets checked
        assert report.total == 9
        assert len(report.mismatches) == 2
        for mismatch in report.mismatches:
            assert mismatch.kind == "balance"
            assert "RuntimeError" in str(mismatch.actual)


class TestSaveLoad:
    def test_json_roundtrip_preserves_anchors(self, tmp_path):
        anchor_set = collect_anchors(
            FakeW3(make_chain()), make_spec(), meta={"stage": "pre-upgrade"}
        )
        path = tmp_path / "anchors.json"
        anchor_set.save(path)

        raw = json.loads(path.read_text())
        assert raw["version"] == 1
        assert raw["meta"] == {"stage": "pre-upgrade"}

        loaded = AnchorSet.load(path)
        assert loaded.meta == anchor_set.meta
        assert loaded.anchors == anchor_set.anchors

    def test_replay_after_roundtrip_matches(self, tmp_path):
        anchor_set = collect_baseline()
        path = tmp_path / "anchors.json"
        anchor_set.save(path)
        loaded = AnchorSet.load(path)
        report = replay_anchors(FakeW3(make_chain()), loaded)
        assert report.ok, report.summary()

    def test_replay_after_roundtrip_still_detects_mismatch(self, tmp_path):
        anchor_set = collect_baseline()
        path = tmp_path / "anchors.json"
        anchor_set.save(path)
        loaded = AnchorSet.load(path)
        chain = make_chain()
        chain["balances"][(ADDR, 5)] = 1
        report = replay_anchors(FakeW3(chain), loaded)
        assert len(report.mismatches) == 1
        assert report.mismatches[0].kind == "balance"

    def test_load_rejects_unknown_version(self, tmp_path):
        path = tmp_path / "anchors.json"
        path.write_text(json.dumps({"version": 999, "meta": {}, "anchors": []}))
        with pytest.raises(ValueError, match="version"):
            AnchorSet.load(path)


class TestHexNormalization:
    def test_representation_changes_do_not_mismatch(self):
        """Same values, wildly different encodings: must NOT be reported."""
        anchor_set = collect_baseline()

        variant = make_chain()
        # balance as hex string instead of int
        variant["balances"][(ADDR, 5)] = hex(10**18)
        # storage as short / uppercase strings instead of raw bytes
        variant["storage"][(CONTRACT, 0, 5)] = "0x1"
        variant["storage"][(CONTRACT, 1, 5)] = "0X2A"
        # tx hash field as uppercase hex string (collected from raw bytes)
        variant["txs"][TX1] = {
            "blockHash": BLOCK5_HASH.upper().replace("0X", "0x"),
            "blockNumber": "0x5",
        }
        # receipt quantities as hex strings, log payloads as uppercase strings
        variant["receipts"][TX1] = {
            "status": "0x1",
            "gasUsed": "0x5208",
            "blockHash": BLOCK5_HASH.upper().replace("0X", "0x"),
            "blockNumber": "0x5",
            "logs": [
                {
                    "address": Web3.to_checksum_address(CONTRACT),
                    "topics": [TOPIC0.upper().replace("0X", "0x")],
                    "data": "0x" + "00" * 31 + "2A",
                }
            ],
        }
        # block hashes as uppercase hex strings (collected from raw bytes)
        variant["blocks"][5] = {
            "number": "0x5",
            "hash": BLOCK5_HASH.upper().replace("0X", "0x"),
            "parentHash": BLOCK5_PARENT.upper().replace("0X", "0x"),
        }

        report = replay_anchors(FakeW3(variant), anchor_set)
        assert report.ok, report.summary()
