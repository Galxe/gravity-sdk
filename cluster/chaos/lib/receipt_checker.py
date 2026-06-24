#!/usr/bin/env python3
"""Verify confirmed chaos workload receipts remain on the canonical chain."""

from __future__ import annotations

import argparse
import collections
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from cluster import Node, load_nodes, rpc_call  # noqa: E402


def load_history(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            events.append(event)
    return events


def load_confirmed_receipts(events: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    receipts: dict[str, dict[str, Any]] = {}
    for event in events:
        if event.get("type") != "tx_receipt":
            continue
        tx_hash = str(event.get("tx_hash") or "")
        if tx_hash:
            receipts[tx_hash] = event
    return receipts


def selected_nodes(config_path: Path, node_ids: str) -> list[Node]:
    nodes = [node for node in load_nodes(config_path) if node.rpc_port is not None]
    if node_ids:
        allowed = {item.strip() for item in node_ids.split(",") if item.strip()}
        nodes = [node for node in nodes if node.id in allowed]
    if not nodes:
        raise SystemExit("no RPC nodes selected")
    return nodes


def hex_int(value: Any) -> int:
    return int(str(value), 16)


def summarize_history(events: list[dict[str, Any]], max_examples: int) -> dict[str, Any]:
    counts = collections.Counter(str(event.get("type") or "unknown") for event in events)
    submits: dict[str, dict[str, Any]] = {}
    terminal_by_tx: dict[str, list[dict[str, Any]]] = {}
    error_events: list[dict[str, Any]] = []

    for event in events:
        event_type = str(event.get("type") or "")
        tx_hash = str(event.get("tx_hash") or "")
        if event_type == "tx_submit" and tx_hash:
            submits[tx_hash] = event
        elif event_type in {"tx_receipt", "tx_timeout", "tx_interrupted"} and tx_hash:
            terminal_by_tx.setdefault(tx_hash, []).append(event)
        elif event_type == "tx_error":
            error_events.append(event)

    receipt_txs = {
        str(event.get("tx_hash"))
        for event in events
        if event.get("type") == "tx_receipt" and event.get("tx_hash")
    }
    timeout_txs = {
        str(event.get("tx_hash"))
        for event in events
        if event.get("type") == "tx_timeout" and event.get("tx_hash")
    }
    interrupted_txs = {
        str(event.get("tx_hash"))
        for event in events
        if event.get("type") == "tx_interrupted" and event.get("tx_hash")
    }
    missing_terminal = sorted(tx_hash for tx_hash in submits if tx_hash not in terminal_by_tx)
    stop_events = [event for event in events if event.get("type") == "tx_workload_stop"]
    last_stop = stop_events[-1] if stop_events else {}

    return {
        "counts": dict(sorted(counts.items())),
        "submitted": len(submits),
        "receipts": len(receipt_txs),
        "timeouts": len(timeout_txs),
        "interrupted": len(interrupted_txs),
        "errors": len(error_events),
        "missing_terminal": len(missing_terminal),
        "missing_terminal_examples": missing_terminal[:max_examples],
        "workload_started": counts.get("tx_workload_start", 0) > 0,
        "workload_stopped": bool(stop_events),
        "last_workload_status": last_stop.get("status"),
        "last_workload_sent": last_stop.get("sent"),
        "last_workload_confirmed": last_stop.get("confirmed"),
        "error_examples": error_events[:max_examples],
    }


def evaluate_history(summary: dict[str, Any], args: argparse.Namespace) -> tuple[str, list[str], list[str]]:
    failures: list[str] = []
    warnings: list[str] = []

    if args.require_txs and summary["submitted"] == 0 and summary["receipts"] == 0:
        failures.append("no tx submissions or receipts found")
    if summary["errors"] and not args.allow_errors:
        failures.append(f"{summary['errors']} tx_error events")
    if summary["timeouts"] and not args.allow_timeouts:
        failures.append(f"{summary['timeouts']} tx_timeout events")
    if summary["missing_terminal"]:
        failures.append(f"{summary['missing_terminal']} submitted txs have no terminal event")
    if summary["workload_started"] and not summary["workload_stopped"] and not args.allow_missing_stop:
        failures.append("tx workload started but did not write tx_workload_stop")
    if summary.get("last_workload_status") == "fail" and not args.allow_failed_stop:
        failures.append("tx workload stop status is fail")

    if summary["interrupted"]:
        message = f"{summary['interrupted']} tx_interrupted events"
        if args.fail_interrupted:
            failures.append(message)
        else:
            warnings.append(message)

    if failures:
        return "fail", failures, warnings
    if warnings:
        return "inconclusive", failures, warnings
    return "pass", failures, warnings


def verify_one(nodes: list[Node], tx_hash: str, expected: dict[str, Any], min_nodes: int) -> dict[str, Any]:
    expected_block_hash = str(expected.get("block_hash") or "")
    expected_block_number = int(expected.get("block_number"))
    ok_nodes: list[str] = []
    failures: list[dict[str, str]] = []

    for node in nodes:
        try:
            receipt = rpc_call(node, "eth_getTransactionReceipt", [tx_hash], timeout=5)
            if not receipt:
                failures.append({"node": node.id, "error": "missing receipt"})
                continue
            block_hash = str(receipt.get("blockHash") or "")
            block_number = hex_int(receipt.get("blockNumber"))
            if block_hash.lower() != expected_block_hash.lower():
                failures.append(
                    {
                        "node": node.id,
                        "error": f"receipt block hash changed: {block_hash} != {expected_block_hash}",
                    }
                )
                continue
            if block_number != expected_block_number:
                failures.append(
                    {
                        "node": node.id,
                        "error": f"receipt block number changed: {block_number} != {expected_block_number}",
                    }
                )
                continue
            block = rpc_call(node, "eth_getBlockByNumber", [hex(block_number), False], timeout=5)
            if not block:
                failures.append({"node": node.id, "error": f"missing canonical block {block_number}"})
                continue
            canonical_hash = str(block.get("hash") or "")
            if canonical_hash.lower() != expected_block_hash.lower():
                failures.append(
                    {
                        "node": node.id,
                        "error": f"canonical block hash changed: {canonical_hash} != {expected_block_hash}",
                    }
                )
                continue
            ok_nodes.append(node.id)
        except Exception as exc:
            failures.append({"node": node.id, "error": str(exc)})

    return {
        "tx_hash": tx_hash,
        "expected_block_number": expected_block_number,
        "expected_block_hash": expected_block_hash,
        "ok": len(ok_nodes) >= min_nodes,
        "ok_nodes": ok_nodes,
        "failures": failures,
    }


def run_check(args: argparse.Namespace) -> int:
    config_path = Path(args.config).resolve()
    history_file = Path(args.history_file).expanduser().resolve()
    if not history_file.exists():
        report = {
            "ts": time.time(),
            "history_file": str(history_file),
            "checked": 0,
            "failed": 1,
            "pass": False,
            "status": "fail",
            "history_status": "fail",
            "history_failures": ["history file does not exist"],
            "history_warnings": [],
        }
        print(json.dumps(report, indent=2, sort_keys=True) if args.json else report["history_failures"][0])
        return 1

    events = load_history(history_file)
    summary = summarize_history(events, args.max_failures)
    history_status, history_failures, history_warnings = evaluate_history(summary, args)
    receipts = load_confirmed_receipts(events)
    nodes = selected_nodes(config_path, args.nodes)
    min_nodes = min(args.min_nodes, len(nodes))

    results = [
        verify_one(nodes, tx_hash, receipt, min_nodes)
        for tx_hash, receipt in sorted(receipts.items())
    ]
    failures = [item for item in results if not item["ok"]]
    if failures:
        receipt_status = "fail"
    elif results or not args.require_txs:
        receipt_status = "pass"
    elif history_status == "inconclusive":
        receipt_status = "inconclusive"
    else:
        receipt_status = "fail"

    if receipt_status == "fail" or history_status == "fail":
        status = "fail"
    elif receipt_status == "inconclusive" or history_status == "inconclusive":
        status = "inconclusive"
    else:
        status = "pass"
    overall_pass = status == "pass" or (status == "inconclusive" and not args.fail_on_inconclusive)
    report = {
        "ts": time.time(),
        "history_file": str(history_file),
        "checked": len(results),
        "failed": len(failures),
        "min_nodes": min_nodes,
        "nodes": [node.id for node in nodes],
        "pass": overall_pass,
        "status": status,
        "history_status": history_status,
        "history_summary": summary,
        "history_failures": history_failures,
        "history_warnings": history_warnings,
        "failures": failures[: args.max_failures],
    }
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(
            f"receipt-check checked={report['checked']} failed={report['failed']} "
            f"min_nodes={min_nodes} status={status} pass={str(report['pass']).lower()}"
        )
        if history_failures:
            print(f"  history failures: {history_failures}")
        if history_warnings:
            print(f"  history warnings: {history_warnings}")
        for failure in failures[: args.max_failures]:
            print(f"  FAIL {failure['tx_hash']}: {failure['failures']}")

    return 0 if report["pass"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True)
    parser.add_argument("--history-file", required=True)
    parser.add_argument("--nodes", default=os.environ.get("CHAOS_TX_CHECK_NODES", ""))
    parser.add_argument("--min-nodes", type=int, default=int(os.environ.get("CHAOS_TX_CHECK_MIN_NODES", "2")))
    parser.add_argument("--require-txs", action="store_true")
    parser.add_argument("--max-failures", type=int, default=20)
    parser.add_argument("--allow-timeouts", action="store_true", default=os.environ.get("CHAOS_TX_ALLOW_TIMEOUTS", "0") == "1")
    parser.add_argument("--allow-errors", action="store_true", default=os.environ.get("CHAOS_TX_ALLOW_ERRORS", "0") == "1")
    parser.add_argument("--allow-missing-stop", action="store_true", default=os.environ.get("CHAOS_TX_ALLOW_MISSING_STOP", "0") == "1")
    parser.add_argument("--allow-failed-stop", action="store_true", default=os.environ.get("CHAOS_TX_ALLOW_FAILED_STOP", "0") == "1")
    parser.add_argument("--fail-interrupted", action="store_true", default=os.environ.get("CHAOS_TX_FAIL_INTERRUPTED", "0") == "1")
    parser.add_argument("--fail-on-inconclusive", action="store_true", default=os.environ.get("CHAOS_TX_FAIL_ON_INCONCLUSIVE", "0") == "1")
    parser.add_argument("--json", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    return run_check(args)


if __name__ == "__main__":
    raise SystemExit(main())
