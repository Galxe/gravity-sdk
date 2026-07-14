"""
Storage anchor collection / replay helper.

Turns liveness-style e2e cases into storage-correctness cases: before a
rolling upgrade / storage migration, collect a set of "anchors" (historical
on-chain facts: balances and storage slots at historical blocks, tx /
receipt contents, log ranges, block hashes); after the upgrade, replay the
same queries against any node and diff the results to catch silent
misreads or data corruption. Historical-state queries (eth_getBalance /
eth_getStorageAt at an old block) are the primary consumers of changesets
and therefore the core verification surface.

Collection targets are given explicitly by the caller (the case knows what
it deployed and which txs it sent) — there is no auto-discovery. Anchor
sets serialize to JSON so they survive node restarts and pytest stages.

This module is synchronous by design: cluster ``Node`` objects expose a
sync ``web3.Web3`` instance (``node.w3``, see ``cluster/node.py``) and
cluster cases call it directly even from async tests; wrap calls in
``asyncio.to_thread`` if you need them inside a tight async loop (as
``cluster_test_cases/rolling_upgrade`` already does for its w3 calls).

Typical usage (three steps)::

    from gravity_e2e.helpers.storage_anchors import (
        AnchorSpec, AnchorSet, collect_anchors, replay_anchors,
    )

    # 1. Before the upgrade: collect anchors from a healthy node.
    spec = AnchorSpec(
        balances=[(alice_addr, 100), (alice_addr, 120)],
        storage_slots=[(counter_addr, 0, 120)],
        tx_hashes=[transfer_tx_hash],
        block_numbers=[100, 120],
        log_ranges=[(100, 120, counter_addr)],
    )
    anchors = collect_anchors(node.w3, spec, meta={"node": node.node_id})

    # 2. Persist across restarts / pytest stages.
    anchors.save(output_dir / "anchors.json")
    # ... rolling upgrade happens, possibly in another pytest stage ...
    anchors = AnchorSet.load(output_dir / "anchors.json")

    # 3. After the upgrade: replay against any node and assert.
    report = replay_anchors(upgraded_node.w3, anchors)
    assert report.ok, report.summary()
"""

from __future__ import annotations

import json
import logging
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple, Union

from web3 import Web3
from web3.exceptions import TransactionNotFound

LOG = logging.getLogger(__name__)

# Bump when the on-disk JSON layout changes incompatibly.
FORMAT_VERSION = 1

# ---------------------------------------------------------------------------
# Normalization
#
# The same value can come back as HexBytes, raw bytes, int, or a hex string
# with varying case / zero-padding depending on client and code path. All
# values are canonicalized before storage and comparison so that encoding
# differences never show up as mismatches.
# ---------------------------------------------------------------------------


def _to_hex(value: Union[bytes, str, int], width: Optional[int] = None) -> str:
    """Canonical lowercase 0x-hex; zero-pads to ``width`` bytes if given."""
    if isinstance(value, (bytes, bytearray)):
        digits = bytes(value).hex()
    elif isinstance(value, str):
        digits = value.lower()
        if digits.startswith("0x"):
            digits = digits[2:]
        int(digits or "0", 16)  # validate
    elif isinstance(value, int):
        digits = format(value, "x")
    else:
        raise TypeError(f"cannot normalize {type(value).__name__} to hex: {value!r}")
    if width is not None:
        digits = digits.rjust(width * 2, "0")
    if len(digits) % 2:
        digits = "0" + digits
    return "0x" + digits


def _to_int(value: Union[bytes, str, int]) -> int:
    """Canonical int for quantity-like fields (balance, gasUsed, block number)."""
    if isinstance(value, int):  # bool included
        return int(value)
    if isinstance(value, (bytes, bytearray)):
        return int.from_bytes(bytes(value), "big")
    if isinstance(value, str):
        return int(value, 16) if value.lower().startswith("0x") else int(value)
    raise TypeError(f"cannot normalize {type(value).__name__} to int: {value!r}")


def _slot_to_hex(slot: Union[int, str, bytes]) -> str:
    """Canonical minimal hex for a storage slot key (e.g. 0, "0x00" -> "0x0")."""
    return hex(_to_int(slot if not isinstance(slot, str) else int(slot, 16)))


def _canon_receipt_log(log: Any) -> Dict[str, Any]:
    return {
        "address": _to_hex(log["address"], 20),
        "topics": [_to_hex(topic, 32) for topic in log["topics"]],
        "data": _to_hex(log["data"]),
    }


def _canon_range_log(log: Any) -> Dict[str, Any]:
    canon = _canon_receipt_log(log)
    canon["block_number"] = _to_int(log["blockNumber"])
    canon["log_index"] = _to_int(log["logIndex"])
    canon["transaction_hash"] = _to_hex(log["transactionHash"], 32)
    return canon


# ---------------------------------------------------------------------------
# Fetchers: one per anchor kind. Used both at collection time (to build the
# expected value) and at replay time (to build the actual value), so both
# sides always go through identical canonicalization.
# ---------------------------------------------------------------------------


def _fetch_balance(w3: Any, params: Dict[str, Any]) -> int:
    address = Web3.to_checksum_address(params["address"])
    return _to_int(w3.eth.get_balance(address, params["block_number"]))


def _fetch_storage(w3: Any, params: Dict[str, Any]) -> str:
    contract = Web3.to_checksum_address(params["contract"])
    value = w3.eth.get_storage_at(
        contract, int(params["slot"], 16), params["block_number"]
    )
    return _to_hex(value, 32)


def _fetch_transaction(w3: Any, params: Dict[str, Any]) -> Dict[str, Any]:
    try:
        tx = w3.eth.get_transaction(params["tx_hash"])
    except TransactionNotFound:
        tx = None
    if tx is None:
        return {"exists": False, "block_hash": None, "block_number": None}
    return {
        "exists": True,
        "block_hash": _to_hex(tx["blockHash"], 32),
        "block_number": _to_int(tx["blockNumber"]),
    }


def _fetch_receipt(w3: Any, params: Dict[str, Any]) -> Dict[str, Any]:
    try:
        receipt = w3.eth.get_transaction_receipt(params["tx_hash"])
    except TransactionNotFound:
        receipt = None
    if receipt is None:
        return {"exists": False}
    return {
        "exists": True,
        "status": _to_int(receipt["status"]),
        "gas_used": _to_int(receipt["gasUsed"]),
        "block_hash": _to_hex(receipt["blockHash"], 32),
        "block_number": _to_int(receipt["blockNumber"]),
        "logs": [_canon_receipt_log(log) for log in receipt["logs"]],
    }


def _fetch_logs(w3: Any, params: Dict[str, Any]) -> List[Dict[str, Any]]:
    filter_params: Dict[str, Any] = {
        "fromBlock": params["from_block"],
        "toBlock": params["to_block"],
    }
    if params.get("address") is not None:
        filter_params["address"] = Web3.to_checksum_address(params["address"])
    logs = [_canon_range_log(log) for log in w3.eth.get_logs(filter_params)]
    logs.sort(key=lambda log: (log["block_number"], log["log_index"]))
    return logs


def _fetch_block_hash(w3: Any, params: Dict[str, Any]) -> Dict[str, Any]:
    block = w3.eth.get_block(params["block_number"])
    return {
        "number": _to_int(block["number"]),
        "hash": _to_hex(block["hash"], 32),
        "parent_hash": _to_hex(block["parentHash"], 32),
    }


_FETCHERS = {
    "balance": _fetch_balance,
    "storage": _fetch_storage,
    "transaction": _fetch_transaction,
    "receipt": _fetch_receipt,
    "logs": _fetch_logs,
    "block_hash": _fetch_block_hash,
}


# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------


@dataclass
class AnchorSpec:
    """What to collect. All targets are explicit; nothing is auto-discovered.

    Attributes:
        balances: (address, block_number) pairs for eth_getBalance.
        storage_slots: (contract, slot, block_number) triples for
            eth_getStorageAt; slot may be an int or a hex string.
        tx_hashes: tx hashes; each yields a transaction anchor (existence +
            inclusion block) and a receipt anchor (status/gasUsed/logs).
        block_numbers: block numbers for eth_getBlockByNumber hash anchors.
        log_ranges: (from_block, to_block) or (from_block, to_block, address)
            tuples for eth_getLogs; address None means no address filter.
    """

    balances: List[Tuple[str, int]] = field(default_factory=list)
    storage_slots: List[Tuple[str, Union[int, str], int]] = field(default_factory=list)
    tx_hashes: List[str] = field(default_factory=list)
    block_numbers: List[int] = field(default_factory=list)
    log_ranges: List[Sequence] = field(default_factory=list)


@dataclass
class Anchor:
    """One collected fact: query params plus the canonicalized expected value."""

    kind: str
    params: Dict[str, Any]
    expected: Any

    @property
    def anchor_id(self) -> str:
        """Short stable human-readable identifier for reports."""
        p = self.params
        if self.kind == "balance":
            return f"balance[{p['address']}@{p['block_number']}]"
        if self.kind == "storage":
            return f"storage[{p['contract']}:{p['slot']}@{p['block_number']}]"
        if self.kind in ("transaction", "receipt"):
            return f"{self.kind}[{p['tx_hash']}]"
        if self.kind == "logs":
            return f"logs[{p['from_block']}..{p['to_block']}@{p['address'] or '*'}]"
        if self.kind == "block_hash":
            return f"block_hash[{p['block_number']}]"
        return f"{self.kind}[{p}]"


@dataclass
class AnchorSet:
    """A collected, serializable set of anchors."""

    anchors: List[Anchor] = field(default_factory=list)
    meta: Dict[str, Any] = field(default_factory=dict)

    def __len__(self) -> int:
        return len(self.anchors)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "version": FORMAT_VERSION,
            "meta": self.meta,
            "anchors": [asdict(anchor) for anchor in self.anchors],
        }

    def save(self, path: Union[str, Path]) -> None:
        """Write the anchor set to a JSON file (creates parent dirs)."""
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.to_dict(), indent=2, sort_keys=True))
        LOG.info("Saved %d anchors to %s", len(self.anchors), path)

    @classmethod
    def load(cls, path: Union[str, Path]) -> "AnchorSet":
        """Read an anchor set previously written by :meth:`save`."""
        data = json.loads(Path(path).read_text())
        version = data.get("version")
        if version != FORMAT_VERSION:
            raise ValueError(
                f"Unsupported anchor file version {version!r} in {path} "
                f"(expected {FORMAT_VERSION})"
            )
        anchors = [
            Anchor(kind=a["kind"], params=a["params"], expected=a["expected"])
            for a in data.get("anchors", [])
        ]
        LOG.info("Loaded %d anchors from %s", len(anchors), path)
        return cls(anchors=anchors, meta=data.get("meta", {}))


@dataclass
class AnchorMismatch:
    """One replay divergence: which anchor, what was expected, what came back."""

    anchor_id: str
    kind: str
    expected: Any
    actual: Any


@dataclass
class ReplayReport:
    """Aggregated replay outcome; never raises on mismatches (no fail-fast)."""

    total: int = 0
    mismatches: List[AnchorMismatch] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.mismatches

    @property
    def matched(self) -> int:
        return self.total - len(self.mismatches)

    def summary(self) -> str:
        """Human-readable multi-line summary, suitable for pytest assert msgs."""
        lines = [
            f"anchor replay: {self.matched}/{self.total} matched, "
            f"{len(self.mismatches)} mismatched"
        ]
        for m in self.mismatches:
            lines.append(
                f"  - {m.anchor_id}: expected={_truncate(m.expected)} "
                f"actual={_truncate(m.actual)}"
            )
        return "\n".join(lines)


def _truncate(value: Any, limit: int = 256) -> str:
    text = repr(value)
    return text if len(text) <= limit else text[:limit] + "...(truncated)"


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def collect_anchors(
    w3: Any, spec: AnchorSpec, meta: Optional[Dict[str, Any]] = None
) -> AnchorSet:
    """Query every target in ``spec`` via ``w3`` and snapshot the results.

    Args:
        w3: sync ``web3.Web3`` instance (e.g. ``node.w3``).
        spec: explicit collection targets.
        meta: optional caller metadata stored alongside the anchors
            (e.g. source node id, upgrade stage).

    Returns:
        AnchorSet ready to :meth:`AnchorSet.save` and later replay.

    Raises:
        Any query error propagates: collection runs pre-upgrade on a healthy
        node, so a failure there is a case/environment bug the caller must see.
    """
    anchors: List[Anchor] = []
    for address, block_number in spec.balances:
        params = {"address": _to_hex(address, 20), "block_number": int(block_number)}
        anchors.append(Anchor("balance", params, _fetch_balance(w3, params)))
    for contract, slot, block_number in spec.storage_slots:
        params = {
            "contract": _to_hex(contract, 20),
            "slot": _slot_to_hex(slot),
            "block_number": int(block_number),
        }
        anchors.append(Anchor("storage", params, _fetch_storage(w3, params)))
    for tx_hash in spec.tx_hashes:
        params = {"tx_hash": _to_hex(tx_hash, 32)}
        anchors.append(Anchor("transaction", params, _fetch_transaction(w3, params)))
        anchors.append(Anchor("receipt", dict(params), _fetch_receipt(w3, params)))
    for block_number in spec.block_numbers:
        params = {"block_number": int(block_number)}
        anchors.append(Anchor("block_hash", params, _fetch_block_hash(w3, params)))
    for log_range in spec.log_ranges:
        from_block, to_block = log_range[0], log_range[1]
        address = log_range[2] if len(log_range) > 2 else None
        params = {
            "from_block": int(from_block),
            "to_block": int(to_block),
            "address": _to_hex(address, 20) if address is not None else None,
        }
        anchors.append(Anchor("logs", params, _fetch_logs(w3, params)))
    LOG.info("Collected %d storage anchors", len(anchors))
    return AnchorSet(anchors=anchors, meta=dict(meta or {}))


def replay_anchors(w3: Any, anchor_set: AnchorSet) -> ReplayReport:
    """Re-query every anchor via ``w3`` and diff against expected values.

    Never fail-fast: every anchor is checked, and query errors are recorded
    as mismatches (actual = ``"<replay error: ...>"``) instead of raised, so
    one broken RPC path cannot hide other corruptions.

    Args:
        w3: sync ``web3.Web3`` instance of the node under verification.
        anchor_set: previously collected (or loaded) anchors.

    Returns:
        ReplayReport; use ``assert report.ok, report.summary()`` in tests.
    """
    report = ReplayReport(total=len(anchor_set.anchors))
    for anchor in anchor_set.anchors:
        fetcher = _FETCHERS.get(anchor.kind)
        if fetcher is None:
            report.mismatches.append(
                AnchorMismatch(
                    anchor_id=anchor.anchor_id,
                    kind=anchor.kind,
                    expected=anchor.expected,
                    actual=f"<unknown anchor kind: {anchor.kind!r}>",
                )
            )
            continue
        try:
            actual = fetcher(w3, anchor.params)
        except Exception as exc:  # noqa: BLE001 - deliberately no fail-fast
            LOG.warning("Replay query failed for %s: %s", anchor.anchor_id, exc)
            report.mismatches.append(
                AnchorMismatch(
                    anchor_id=anchor.anchor_id,
                    kind=anchor.kind,
                    expected=anchor.expected,
                    actual=f"<replay error: {type(exc).__name__}: {exc}>",
                )
            )
            continue
        if actual != anchor.expected:
            report.mismatches.append(
                AnchorMismatch(
                    anchor_id=anchor.anchor_id,
                    kind=anchor.kind,
                    expected=anchor.expected,
                    actual=actual,
                )
            )
    if report.ok:
        LOG.info("Anchor replay OK: %d/%d matched", report.matched, report.total)
    else:
        LOG.warning("Anchor replay found mismatches:\n%s", report.summary())
    return report
