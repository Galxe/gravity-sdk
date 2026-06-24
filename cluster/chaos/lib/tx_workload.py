#!/usr/bin/env python3
"""Small EVM transfer workload for Gravity chaos scenarios."""

from __future__ import annotations

import argparse
import json
import os
import random
import signal
import sys
import time
from pathlib import Path
from typing import Any

from eth_account import Account

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from cluster import Node, load_nodes, rpc_call, _load_toml, _resolve_path  # noqa: E402


DEFAULT_FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
STOP = False


def _signal_stop(_signum: int, _frame: Any) -> None:
    global STOP
    STOP = True


def normalize_key(value: str) -> str:
    value = value.strip()
    if not value:
        raise ValueError("empty private key")
    return value if value.startswith("0x") else f"0x{value}"


def json_default(value: Any) -> Any:
    if isinstance(value, bytes):
        return "0x" + value.hex()
    if hasattr(value, "hex"):
        text = value.hex()
        return text if str(text).startswith("0x") else f"0x{text}"
    return str(value)


def append_history(path: Path, event: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    event.setdefault("ts", time.time())
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(event, sort_keys=True, default=json_default) + "\n")


def rpc_url_label(node: Node) -> str:
    return f"http://{node.host}:{node.rpc_port}"


def rpc_hex_int(node: Node, method: str, params: list[Any] | None = None) -> int:
    result = rpc_call(node, method, params or [], timeout=float(os.environ.get("RPC_TIMEOUT", "5")))
    if result is None:
        raise RuntimeError(f"{method} returned null")
    return int(str(result), 16)


def load_account_keys(config_path: Path, explicit_keys: list[str], accounts_file: str | None) -> list[str]:
    keys: list[str] = []
    keys.extend(explicit_keys)

    env_keys = os.environ.get("CHAOS_TX_PRIVATE_KEYS", "")
    if env_keys:
        keys.extend(part for part in env_keys.replace("\n", ",").split(",") if part.strip())

    if accounts_file:
        keys.extend(load_keys_from_accounts_file(Path(accounts_file).expanduser()))
    else:
        artifacts_dir = os.environ.get("GRAVITY_ARTIFACTS_DIR")
        if artifacts_dir:
            candidate = Path(artifacts_dir) / "accounts.csv"
            if candidate.exists():
                keys.extend(load_keys_from_accounts_file(candidate))

    if not keys:
        key = faucet_key_from_config(config_path)
        if key:
            keys.append(key)

    if not keys and os.environ.get("CHAOS_TX_ALLOW_DEFAULT_FAUCET") == "1":
        keys.append(DEFAULT_FAUCET_KEY)

    unique: list[str] = []
    seen: set[str] = set()
    for key in keys:
        normalized = normalize_key(key)
        if normalized not in seen:
            seen.add(normalized)
            unique.append(normalized)
    return unique


def load_keys_from_accounts_file(path: Path) -> list[str]:
    if not path.exists():
        raise FileNotFoundError(path)
    keys: list[str] = []
    if path.suffix.lower() in {".json", ".jsonl"}:
        for line in path.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            obj = json.loads(line) if path.suffix.lower() == ".jsonl" else None
            if obj is None:
                data = json.loads(path.read_text(encoding="utf-8"))
                rows = data if isinstance(data, list) else data.get("accounts", [])
                for row in rows:
                    key = row.get("private_key") or row.get("privateKey") or row.get("key")
                    if key:
                        keys.append(str(key))
                break
            key = obj.get("private_key") or obj.get("privateKey") or obj.get("key")
            if key:
                keys.append(str(key))
        return keys

    import csv

    with path.open(newline="", encoding="utf-8") as fh:
        sample = fh.read(4096)
        fh.seek(0)
        has_header = "private" in sample.splitlines()[0].lower() if sample.splitlines() else False
        if has_header:
            for row in csv.DictReader(fh):
                key = row.get("private_key") or row.get("privateKey") or row.get("key")
                if key:
                    keys.append(key)
        else:
            for row in csv.reader(fh):
                for item in row:
                    value = item.strip()
                    if len(value.removeprefix("0x")) == 64:
                        keys.append(value)
                        break
    return keys


def faucet_key_from_config(config_path: Path) -> str | None:
    config_path = config_path.resolve()
    config = _load_toml(config_path)

    direct = (config.get("faucet_init") or {}).get("private_key")
    if direct:
        return str(direct)

    source = config.get("genesis_source") or {}
    candidates: list[Path] = []
    for key in ("genesis_toml", "genesis_config", "genesis_path"):
        value = source.get(key)
        if isinstance(value, str) and value.endswith(".toml"):
            candidates.append(_resolve_path(value, config_path.parent))
    candidates.extend(
        [
            config_path.parent / "genesis.toml",
            config_path.parent / "genesis.bridge.toml",
            config_path.parent / f"{config_path.stem}.genesis.toml",
        ]
    )

    for candidate in candidates:
        if not candidate.exists():
            continue
        try:
            genesis = _load_toml(candidate)
        except Exception:
            continue
        faucet = (genesis.get("genesis") or {}).get("faucet") or {}
        key = faucet.get("private_key")
        if key:
            return str(key)
    return None


def selected_nodes(config_path: Path, node_ids: str) -> list[Node]:
    nodes = [node for node in load_nodes(config_path) if node.rpc_port is not None]
    if node_ids:
        allowed = {item.strip() for item in node_ids.split(",") if item.strip()}
        nodes = [node for node in nodes if node.id in allowed]
    if not nodes:
        raise SystemExit("no RPC nodes selected")
    return nodes


def signed_raw_hex(account: Any, tx: dict[str, Any]) -> str:
    signed = Account.sign_transaction(tx, account.key)
    raw = getattr(signed, "raw_transaction", None) or getattr(signed, "rawTransaction")
    return raw.hex() if raw.hex().startswith("0x") else f"0x{raw.hex()}"


def wait_receipt(node: Node, tx_hash: str, timeout_s: float, poll_s: float) -> dict[str, Any] | None:
    deadline = time.monotonic() + timeout_s
    while not STOP and time.monotonic() < deadline:
        try:
            receipt = rpc_call(node, "eth_getTransactionReceipt", [tx_hash], timeout=5)
            if receipt:
                return receipt
        except Exception:
            pass
        time.sleep(poll_s)
    return None


def run_workload(args: argparse.Namespace) -> int:
    config_path = Path(args.config).resolve()
    history_file = Path(args.history_file).expanduser().resolve()
    nodes = selected_nodes(config_path, args.nodes)
    keys = load_account_keys(config_path, args.private_key, args.accounts_file)
    if not keys:
        raise SystemExit(
            "no tx private keys found; set --private-key, CHAOS_TX_PRIVATE_KEYS, "
            "CHAOS_TX_ACCOUNTS_FILE, GRAVITY_ARTIFACTS_DIR/accounts.csv, or faucet config"
        )

    accounts = [Account.from_key(key) for key in keys[: args.max_accounts]]
    recipient = args.recipient or Account.create().address
    chain_id = rpc_hex_int(nodes[0], "eth_chainId")
    nonces = {
        account.address: rpc_hex_int(nodes[0], "eth_getTransactionCount", [account.address, "pending"])
        for account in accounts
    }

    append_history(
        history_file,
        {
            "type": "tx_workload_start",
            "nodes": [node.id for node in nodes],
            "accounts": [account.address for account in accounts],
            "recipient": recipient,
            "chain_id": chain_id,
            "interval_s": args.interval,
            "receipt_timeout_s": args.receipt_timeout,
        },
    )

    started = time.monotonic()
    sent = confirmed = failed = timeout = 0
    index = 0
    while not STOP:
        if args.duration > 0 and time.monotonic() - started >= args.duration:
            break
        if args.max_txs > 0 and sent >= args.max_txs:
            break

        node = nodes[index % len(nodes)]
        account = accounts[index % len(accounts)]
        index += 1
        nonce = nonces[account.address]
        tx = {
            "nonce": nonce,
            "to": recipient,
            "value": args.value_wei,
            "gas": args.gas,
            "gasPrice": args.gas_price_wei,
            "chainId": chain_id,
        }

        try:
            raw_hex = signed_raw_hex(account, tx)
            tx_hash = rpc_call(node, "eth_sendRawTransaction", [raw_hex], timeout=5)
            sent += 1
            nonces[account.address] += 1
            send_ts = time.time()
            append_history(
                history_file,
                {
                    "type": "tx_submit",
                    "node": node.id,
                    "rpc": rpc_url_label(node),
                    "from": account.address,
                    "to": recipient,
                    "nonce": nonce,
                    "tx_hash": tx_hash,
                    "gas": args.gas,
                    "gas_price_wei": args.gas_price_wei,
                    "value_wei": args.value_wei,
                },
            )
            receipt = wait_receipt(node, tx_hash, args.receipt_timeout, args.receipt_poll)
            if receipt:
                confirmed += 1
                append_history(
                    history_file,
                    {
                        "type": "tx_receipt",
                        "node": node.id,
                        "tx_hash": tx_hash,
                        "from": account.address,
                        "block_number": int(str(receipt["blockNumber"]), 16),
                        "block_hash": receipt["blockHash"],
                        "transaction_index": int(str(receipt["transactionIndex"]), 16),
                        "status": receipt.get("status"),
                        "latency_s": time.time() - send_ts,
                    },
                )
            elif STOP:
                append_history(
                    history_file,
                    {
                        "type": "tx_interrupted",
                        "node": node.id,
                        "tx_hash": tx_hash,
                        "from": account.address,
                        "nonce": nonce,
                    },
                )
            else:
                timeout += 1
                append_history(
                    history_file,
                    {
                        "type": "tx_timeout",
                        "node": node.id,
                        "tx_hash": tx_hash,
                        "from": account.address,
                        "nonce": nonce,
                        "timeout_s": args.receipt_timeout,
                    },
                )
                try:
                    nonces[account.address] = rpc_hex_int(
                        node, "eth_getTransactionCount", [account.address, "pending"]
                    )
                except Exception:
                    pass
        except Exception as exc:
            failed += 1
            append_history(
                history_file,
                {
                    "type": "tx_error",
                    "node": node.id,
                    "from": account.address,
                    "nonce": nonce,
                    "error": str(exc),
                },
            )
            try:
                nonces[account.address] = rpc_hex_int(
                    node, "eth_getTransactionCount", [account.address, "pending"]
                )
            except Exception:
                pass
            time.sleep(min(args.interval, 1.0))
            continue

        time.sleep(args.interval)

    status = "fail" if failed > 0 or (timeout > 0 and args.fail_on_timeout) else "pass"
    append_history(
        history_file,
        {
            "type": "tx_workload_stop",
            "status": status,
            "sent": sent,
            "confirmed": confirmed,
            "failed": failed,
            "timeout": timeout,
            "duration_s": time.monotonic() - started,
        },
    )
    print(
        json.dumps(
            {
                "history_file": str(history_file),
                "sent": sent,
                "confirmed": confirmed,
                "failed": failed,
                "timeout": timeout,
                "status": status,
            },
            sort_keys=True,
        )
    )
    return 0 if status == "pass" else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True)
    parser.add_argument("--history-file", required=True)
    parser.add_argument("--nodes", default=os.environ.get("CHAOS_TX_NODES", ""))
    parser.add_argument("--accounts-file", default=os.environ.get("CHAOS_TX_ACCOUNTS_FILE"))
    parser.add_argument("--private-key", action="append", default=[])
    parser.add_argument("--max-accounts", type=int, default=int(os.environ.get("CHAOS_TX_MAX_ACCOUNTS", "8")))
    parser.add_argument("--duration", type=float, default=float(os.environ.get("CHAOS_TX_DURATION", "0")))
    parser.add_argument("--max-txs", type=int, default=int(os.environ.get("CHAOS_TX_MAX_TXS", "0")))
    parser.add_argument("--interval", type=float, default=float(os.environ.get("CHAOS_TX_INTERVAL", "1")))
    parser.add_argument("--receipt-timeout", type=float, default=float(os.environ.get("CHAOS_TX_RECEIPT_TIMEOUT", "30")))
    parser.add_argument("--receipt-poll", type=float, default=float(os.environ.get("CHAOS_TX_RECEIPT_POLL", "0.5")))
    parser.add_argument(
        "--fail-on-timeout",
        action="store_true",
        default=os.environ.get("CHAOS_TX_FAIL_ON_TIMEOUT", "1") == "1",
    )
    parser.add_argument("--gas", type=int, default=int(os.environ.get("CHAOS_TX_GAS", "21000")))
    parser.add_argument("--gas-price-wei", type=int, default=int(os.environ.get("CHAOS_TX_GAS_PRICE_WEI", "100000000000")))
    parser.add_argument("--value-wei", type=int, default=int(os.environ.get("CHAOS_TX_VALUE_WEI", "0")))
    parser.add_argument("--recipient", default=os.environ.get("CHAOS_TX_RECIPIENT", ""))
    return parser


def main() -> int:
    signal.signal(signal.SIGTERM, _signal_stop)
    signal.signal(signal.SIGINT, _signal_stop)
    args = build_parser().parse_args()
    return run_workload(args)


if __name__ == "__main__":
    raise SystemExit(main())
