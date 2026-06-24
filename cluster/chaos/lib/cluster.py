#!/usr/bin/env python3
"""Small helpers for Gravity chaos scripts.

This module intentionally stays dependency-light: stdlib on Python 3.11+, with
the existing repo `toml` package fallback for older local Python versions.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import subprocess
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - local Python 3.10 fallback
    import toml as tomllib  # type: ignore


@dataclass(frozen=True)
class Node:
    id: str
    role: str
    host: str
    rpc_port: int | None
    validator_port: int | None
    vfn_port: int | None
    public_port: int | None
    data_dir: str
    stake: int
    validator: bool


def _load_toml(path: Path) -> dict[str, Any]:
    mode = "rb" if tomllib.__name__ == "tomllib" else "r"
    with path.open(mode) as fh:
        return tomllib.load(fh)


def _resolve_path(path: str, base: Path) -> Path:
    candidate = Path(path).expanduser()
    if candidate.is_absolute():
        return candidate
    return (base / candidate).resolve()


def _genesis_candidates(config: dict[str, Any], config_path: Path) -> list[Path]:
    base = config_path.parent
    candidates: list[Path] = []
    source = config.get("genesis_source") or {}
    for key in ("genesis_toml", "genesis_config", "genesis_path"):
        value = source.get(key)
        if isinstance(value, str) and value.endswith(".toml"):
            candidates.append(_resolve_path(value, base))
    candidates.extend(
        [
            base / "genesis.toml",
            base / f"{config_path.stem}.genesis.toml",
        ]
    )
    seen: set[Path] = set()
    unique: list[Path] = []
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved not in seen:
            seen.add(resolved)
            unique.append(resolved)
    return unique


def _stake_map(config: dict[str, Any], config_path: Path) -> dict[str, int]:
    stakes: dict[str, int] = {}
    for path in _genesis_candidates(config, config_path):
        if not path.exists():
            continue
        try:
            genesis = _load_toml(path)
        except Exception:
            continue
        for val in genesis.get("genesis_validators", []):
            node_id = str(val.get("id") or "")
            if not node_id:
                continue
            raw = val.get("voting_power") or val.get("stake_amount") or 1
            try:
                stakes[node_id] = int(str(raw), 0)
            except ValueError:
                stakes[node_id] = 1
        if stakes:
            break
    return stakes


def load_nodes(config_path: Path) -> list[Node]:
    config_path = config_path.resolve()
    config = _load_toml(config_path)
    base_dir = str((config.get("cluster") or {}).get("base_dir") or "")
    stakes = _stake_map(config, config_path)
    nodes: list[Node] = []

    for raw in config.get("nodes", []):
        node_id = str(raw["id"])
        role = str(raw.get("role") or "")
        host = str(raw.get("host") or "127.0.0.1")
        rpc_port = raw.get("rpc_port")
        validator_port = raw.get("validator_port") or raw.get("p2p_port")
        vfn_port = raw.get("vfn_port")
        public_port = raw.get("public_port")
        data_dir = str(raw.get("data_dir") or os.path.join(base_dir, node_id))
        validator = role in {"genesis", "validator"} or (
            "validator_port" in raw and role not in {"vfn", "pfn"}
        )
        stake_raw = (
            raw.get("voting_power")
            or raw.get("stake_amount")
            or stakes.get(node_id)
            or 1
        )
        try:
            stake = int(str(stake_raw), 0)
        except ValueError:
            stake = 1
        nodes.append(
            Node(
                id=node_id,
                role=role,
                host=host,
                rpc_port=int(rpc_port) if rpc_port is not None else None,
                validator_port=int(validator_port) if validator_port is not None else None,
                vfn_port=int(vfn_port) if vfn_port is not None else None,
                public_port=int(public_port) if public_port is not None else None,
                data_dir=data_dir,
                stake=stake,
                validator=validator,
            )
        )
    return nodes


def node_by_id(nodes: list[Node], node_id: str) -> Node:
    for node in nodes:
        if node.id == node_id:
            return node
    raise SystemExit(f"unknown node: {node_id}")


def validator_nodes(config_path: Path) -> list[Node]:
    validators = [node for node in load_nodes(config_path) if node.validator]
    if not validators:
        raise SystemExit("no validators found")
    total = sum(node.stake for node in validators)
    if total <= 0:
        raise SystemExit("no validator stake found")
    return validators


def stake_sum(nodes: list[Node]) -> int:
    return sum(node.stake for node in nodes)


def has_quorum(stake: int, total: int) -> bool:
    return stake * 3 > total * 2


def no_quorum(stake: int, total: int) -> bool:
    return not has_quorum(stake, total)


def node_csv(nodes: list[Node]) -> str:
    return ",".join(node.id for node in nodes)


def parse_node_csv(value: str) -> list[str]:
    return [item.strip() for item in value.replace(";", ",").split(",") if item.strip()]


def selected_validators(validators: list[Node], csv_value: str, label: str) -> list[Node]:
    by_id = {node.id: node for node in validators}
    selected: list[Node] = []
    seen: set[str] = set()
    for node_id in parse_node_csv(csv_value):
        if node_id in seen:
            raise SystemExit(f"{label} contains duplicate node: {node_id}")
        if node_id not in by_id:
            raise SystemExit(f"{label} contains unknown validator: {node_id}")
        seen.add(node_id)
        selected.append(by_id[node_id])
    if not selected:
        raise SystemExit(f"{label} must not be empty")
    return selected


def choose_quorum_safe_victim(validators: list[Node], requested: str = "") -> dict[str, Any]:
    total = stake_sum(validators)
    if len(validators) < 2:
        raise SystemExit("at least two validators are required")

    candidates = [
        node for node in validators
        if has_quorum(total - node.stake, total)
    ]
    if requested and requested != "random":
        victim = node_by_id(validators, requested)
        if victim not in candidates:
            raise SystemExit(
                f"isolating {victim.id} leaves remaining stake without quorum: "
                f"victim_stake={victim.stake} total_stake={total}"
            )
    else:
        if not candidates:
            raise SystemExit(
                "no single validator can be isolated while the remaining validators keep quorum"
            )
        victim = random.choice(candidates)

    majority = [node for node in validators if node.id != victim.id]
    probe = max(majority, key=lambda node: (node.stake, node.id))
    majority_stake = stake_sum(majority)
    return {
        "victim": victim.id,
        "probe": probe.id,
        "majority_csv": node_csv(majority),
        "victim_stake": str(victim.stake),
        "majority_stake": str(majority_stake),
        "total_stake": str(total),
        "remaining_has_quorum": has_quorum(majority_stake, total),
    }


def choose_majority_minority_split(validators: list[Node], minority_csv: str = "") -> dict[str, Any]:
    total = stake_sum(validators)
    if minority_csv and minority_csv != "random":
        minority = selected_validators(validators, minority_csv, "minority")
        minority_ids = {node.id for node in minority}
        majority = [node for node in validators if node.id not in minority_ids]
        if not majority:
            raise SystemExit("majority side must not be empty")
        minority_stake = stake_sum(minority)
        majority_stake = stake_sum(majority)
        if not has_quorum(majority_stake, total):
            raise SystemExit(
                "requested minority leaves majority without quorum: "
                f"minority={node_csv(minority)} minority_stake={minority_stake} total_stake={total}"
            )
    else:
        single = choose_quorum_safe_victim(validators)
        minority = [node_by_id(validators, single["victim"])]
        majority = [node for node in validators if node.id != single["victim"]]
        minority_stake = stake_sum(minority)
        majority_stake = stake_sum(majority)

    probe = max(majority, key=lambda node: (node.stake, node.id))
    return {
        "majority_csv": node_csv(majority),
        "minority_csv": node_csv(minority),
        "probe": probe.id,
        "majority_stake": str(majority_stake),
        "minority_stake": str(minority_stake),
        "total_stake": str(total),
    }


def choose_no_quorum_split(
    validators: list[Node],
    left_csv: str = "",
    right_csv: str = "",
) -> dict[str, Any]:
    total = stake_sum(validators)
    if left_csv or right_csv:
        if not left_csv or not right_csv:
            raise SystemExit("explicit no-quorum split requires both --left and --right")
        left = selected_validators(validators, left_csv, "left")
        right = selected_validators(validators, right_csv, "right")
        left_ids = {node.id for node in left}
        right_ids = {node.id for node in right}
        overlap = sorted(left_ids & right_ids)
        if overlap:
            raise SystemExit(f"split groups overlap: {','.join(overlap)}")
        all_ids = {node.id for node in validators}
        covered = left_ids | right_ids
        missing = sorted(all_ids - covered)
        extra = sorted(covered - all_ids)
        if missing:
            raise SystemExit(f"split groups must cover all validators; missing: {','.join(missing)}")
        if extra:
            raise SystemExit(f"split groups include non-validators: {','.join(extra)}")
    else:
        if len(validators) < 2:
            raise SystemExit("at least two validators are required for a split")
        too_large = [node for node in validators if has_quorum(node.stake, total)]
        if too_large:
            node = max(too_large, key=lambda item: item.stake)
            raise SystemExit(
                "cannot build a two-way no-quorum split because one validator has quorum: "
                f"{node.id} stake={node.stake} total_stake={total}"
            )

        eligible_singletons = [
            node for node in validators
            if node.stake * 3 >= total and no_quorum(node.stake, total)
        ]
        if eligible_singletons:
            left = [random.choice(eligible_singletons)]
        else:
            shuffled = validators[:]
            random.shuffle(shuffled)
            left = []
            left_stake = 0
            for node in shuffled:
                left.append(node)
                left_stake += node.stake
                if left_stake * 3 >= total:
                    break
        left_ids = {node.id for node in left}
        right = [node for node in validators if node.id not in left_ids]

    left_stake = stake_sum(left)
    right_stake = stake_sum(right)
    if not left or not right:
        raise SystemExit("both split sides must be non-empty")
    if has_quorum(left_stake, total) or has_quorum(right_stake, total):
        raise SystemExit(
            "split is not no-quorum on both sides: "
            f"left_stake={left_stake} right_stake={right_stake} total_stake={total}"
        )
    return {
        "left_csv": node_csv(left),
        "right_csv": node_csv(right),
        "left_stake": str(left_stake),
        "right_stake": str(right_stake),
        "total_stake": str(total),
    }


def print_topology_result(result: dict[str, Any], args: argparse.Namespace, fields: list[str]) -> None:
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print("\t".join(str(result[field]) for field in fields))


def pid_status(node: Node) -> dict[str, Any]:
    pid_file = Path(node.data_dir) / "script" / "node.pid"
    if not pid_file.exists():
        return {"pid": None, "process_alive": False, "pid_file": str(pid_file)}
    try:
        pid = int(pid_file.read_text().strip())
    except Exception:
        return {"pid": None, "process_alive": False, "pid_file": str(pid_file)}
    try:
        os.kill(pid, 0)
        alive = True
    except ProcessLookupError:
        alive = False
    except PermissionError:
        alive = True
    return {"pid": pid, "process_alive": alive, "pid_file": str(pid_file)}


def _rpc_payload(method: str, params: list[Any] | None) -> bytes:
    return json.dumps(
        {"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}
    ).encode()


def _rpc_result(body: bytes) -> Any:
    decoded = json.loads(body.decode())
    if "error" in decoded:
        raise RuntimeError(decoded["error"])
    return decoded.get("result")


def _docker_rpc_call(node: Node, payload: bytes, timeout: float) -> Any:
    network = os.environ.get("CHAOS_DOCKER_RPC_NETWORK")
    if not network:
        raise RuntimeError("CHAOS_DOCKER_RPC_NETWORK is required for docker RPC sampling")
    image = os.environ.get("CHAOS_DOCKER_RPC_IMAGE", "curlimages/curl:8.10.1")
    curl_timeout = str(max(timeout, 1.0))
    proc = subprocess.run(
        [
            "docker",
            "run",
            "--rm",
            "--network",
            network,
            image,
            "-sS",
            "-m",
            curl_timeout,
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            payload.decode(),
            f"http://{node.host}:{node.rpc_port}",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout + 8,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode(errors="replace").strip() or "docker RPC failed")
    return _rpc_result(proc.stdout)


def rpc_call(node: Node, method: str, params: list[Any] | None = None, timeout: float = 2.0) -> Any:
    if node.rpc_port is None:
        raise RuntimeError("node has no rpc_port")
    payload = _rpc_payload(method, params)
    if os.environ.get("CHAOS_BACKEND") == "docker" and os.environ.get("CHAOS_DOCKER_RPC_NETWORK"):
        return _docker_rpc_call(node, payload, timeout)
    req = urllib.request.Request(
        f"http://{node.host}:{node.rpc_port}",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(req, timeout=timeout) as resp:
        return _rpc_result(resp.read())


def sample_node(node: Node, block_number: int | None = None) -> dict[str, Any]:
    record = {"node": node.id, **pid_status(node)}
    try:
        if block_number is None:
            height_hex = rpc_call(node, "eth_blockNumber")
            height = int(height_hex, 16)
        else:
            height = block_number
        block = rpc_call(node, "eth_getBlockByNumber", [hex(height), False])
        if not block:
            raise RuntimeError(f"missing block at height {height}")
        record.update(
            {
                "rpc_ok": True,
                "height": height,
                "hash": block.get("hash"),
                "state_root": block.get("stateRoot"),
                "receipts_root": block.get("receiptsRoot"),
            }
        )
    except Exception as exc:
        record.update({"rpc_ok": False, "error": str(exc)})
    return record


def sample_workers(count: int) -> int:
    raw = os.environ.get("CHAOS_SAMPLE_WORKERS")
    if raw:
        try:
            return max(1, min(count, int(raw)))
        except ValueError:
            pass
    return max(1, min(count, 16))


def sample_nodes(nodes: list[Node], only: set[str] | None = None) -> list[dict[str, Any]]:
    selected = [n for n in nodes if only is None or n.id in only]
    if len(selected) <= 1:
        return [sample_node(node) for node in selected]

    results: list[dict[str, Any] | None] = [None] * len(selected)
    with ThreadPoolExecutor(max_workers=sample_workers(len(selected))) as executor:
        future_to_index = {
            executor.submit(sample_node, node): index
            for index, node in enumerate(selected)
        }
        for future in as_completed(future_to_index):
            index = future_to_index[future]
            node = selected[index]
            try:
                results[index] = future.result()
            except Exception as exc:  # pragma: no cover - sample_node normally captures RPC errors.
                results[index] = {
                    "node": node.id,
                    **pid_status(node),
                    "rpc_ok": False,
                    "error": str(exc),
                }
    return [result for result in results if result is not None]


LOG_PATTERNS = [
    re.compile(pattern, re.IGNORECASE)
    for pattern in (
        r"thread .* panicked",
        r"\bpanicked at\b",
        r"\bpanic occurred\b",
        r"\bfatal\b",
        r"\babort(?:ed)?\b",
        r"segmentation fault",
    )
]


def log_candidates(node: Node) -> list[Path]:
    base = Path(node.data_dir)
    candidates = [
        base / "logs" / "debug.log",
        base / "debug.log",
    ]
    candidates.extend((base / "logs" / "consensus_log").glob("*.log"))
    candidates.extend((base / "consensus_log").glob("*.log"))
    candidates.extend((base / "logs" / "execution_logs").glob("**/reth.log"))
    candidates.extend((base / "execution_logs").glob("**/reth.log"))
    seen: set[Path] = set()
    unique: list[Path] = []
    for path in candidates:
        if path.exists() and path.is_file() and path not in seen:
            seen.add(path)
            unique.append(path)
    return unique


def _docker_container_for_node(node: Node) -> str | None:
    candidates = [f"{os.environ.get('CHAOS_DOCKER_PREFIX', '')}{node.id}", node.id]
    for candidate in candidates:
        if not candidate:
            continue
        proc = subprocess.run(
            ["docker", "inspect", candidate],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if proc.returncode == 0:
            return candidate

    proc = subprocess.run(
        ["docker", "ps", "-a", "--format", "{{.Names}}"],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return None
    for name in proc.stdout.splitlines():
        if name == node.id or re.search(rf"(^|[_-]){re.escape(node.id)}($|[_-])", name):
            return name
    return None


def scan_docker_logs(nodes: list[Node], *, since: float | None, max_matches: int) -> dict[str, Any]:
    script = r'''
paths="/gravity/data/execution_logs/dev/reth.log /gravity/data/consensus_log/validator.log /gravity/data/debug.log /gravity/data/logs/debug.log"
for path in $paths; do
    [ -f "$path" ] || continue
    if [ -n "${SINCE:-}" ]; then
        mtime="$(stat -c %Y "$path" 2>/dev/null || echo 0)"
        if [ "$mtime" -lt "$SINCE" ]; then
            printf '__SKIP__ %s\n' "$path"
            continue
        fi
    fi
    printf '__FILE__ %s\n' "$path"
    tail -n "${LOG_TAIL_LINES:-2000}" "$path" |
        grep -Ein -m "${MAX_MATCHES:-20}" 'thread .* panicked|panicked at|panic occurred|\bfatal\b|\babort(ed)?\b|segmentation fault' || true
done
'''
    matches: list[dict[str, Any]] = []
    scanned_files = 0
    skipped_old_files = 0
    since_env = str(int(since)) if since is not None else ""

    for node in nodes:
        container = _docker_container_for_node(node)
        if not container:
            matches.append(
                {
                    "node": node.id,
                    "file": None,
                    "line": None,
                    "text": "failed to resolve docker container for log scan",
                }
            )
            continue
        proc = subprocess.run(
            [
                "docker",
                "exec",
                "--env",
                f"SINCE={since_env}",
                "--env",
                f"MAX_MATCHES={max_matches}",
                "--env",
                f"LOG_TAIL_LINES={os.environ.get('CHAOS_DOCKER_LOG_TAIL_LINES', '2000')}",
                container,
                "sh",
                "-lc",
                script,
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=10,
            check=False,
        )
        if proc.returncode != 0:
            matches.append(
                {
                    "node": node.id,
                    "file": None,
                    "line": None,
                    "text": proc.stderr.strip() or "docker log scan failed",
                }
            )
            continue
        current_file: str | None = None
        for line in proc.stdout.splitlines():
            if line.startswith("__SKIP__ "):
                skipped_old_files += 1
                current_file = None
                continue
            if line.startswith("__FILE__ "):
                scanned_files += 1
                current_file = line[len("__FILE__ ") :]
                continue
            if current_file is None:
                continue
            lineno_raw, _, text = line.partition(":")
            try:
                lineno = int(lineno_raw)
            except ValueError:
                lineno = None
                text = line
            matches.append(
                {
                    "node": node.id,
                    "file": f"{container}:{current_file}",
                    "line": lineno,
                    "text": text[-500:],
                }
            )
            if len(matches) >= max_matches:
                return {
                    "scanned_files": scanned_files,
                    "skipped_old_files": skipped_old_files,
                    "truncated": True,
                    "matches": matches,
                    "source": "docker",
                }

    return {
        "scanned_files": scanned_files,
        "skipped_old_files": skipped_old_files,
        "truncated": False,
        "matches": matches,
        "source": "docker",
    }


def scan_logs(nodes: list[Node], *, since: float | None, max_matches: int) -> dict[str, Any]:
    if os.environ.get("CHAOS_BACKEND") == "docker":
        return scan_docker_logs(nodes, since=since, max_matches=max_matches)

    matches: list[dict[str, Any]] = []
    scanned_files = 0
    skipped_old_files = 0
    for node in nodes:
        for path in log_candidates(node):
            try:
                stat = path.stat()
            except OSError:
                continue
            if since is not None and stat.st_mtime < since:
                skipped_old_files += 1
                continue
            scanned_files += 1
            try:
                lines = path.read_text(errors="replace").splitlines()
            except OSError as exc:
                matches.append(
                    {
                        "node": node.id,
                        "file": str(path),
                        "line": None,
                        "text": f"failed to read log: {exc}",
                    }
                )
                continue
            for lineno, line in enumerate(lines, 1):
                if since is not None:
                    # If the whole file changed after `since`, still scan all lines:
                    # log formats vary, and mtime is the only reliable common filter.
                    pass
                if any(pattern.search(line) for pattern in LOG_PATTERNS):
                    matches.append(
                        {
                            "node": node.id,
                            "file": str(path),
                            "line": lineno,
                            "text": line[-500:],
                        }
                    )
                    if len(matches) >= max_matches:
                        return {
                            "scanned_files": scanned_files,
                            "skipped_old_files": skipped_old_files,
                            "truncated": True,
                            "matches": matches,
                        }
    return {
        "scanned_files": scanned_files,
        "skipped_old_files": skipped_old_files,
        "truncated": False,
        "matches": matches,
    }


def oracle_report(
    nodes: list[Node],
    *,
    height_diff_max: int,
    sample_interval: float,
    common_depth: int,
    skip_process: bool,
    skip_advancing: bool,
    skip_log_scan: bool,
    log_since: float | None,
    log_max_matches: int,
) -> tuple[dict[str, Any], bool]:
    first = sample_nodes(nodes)
    if sample_interval > 0:
        time.sleep(sample_interval)
    second = sample_nodes(nodes)

    by_id_first = {item["node"]: item for item in first}
    by_id_second = {item["node"]: item for item in second}
    usable = [
        item
        for item in second
        if (skip_process or item.get("process_alive"))
        and item.get("rpc_ok")
        and item.get("height") is not None
    ]

    checks: list[dict[str, Any]] = []

    all_processes_alive = skip_process or all(item.get("process_alive") for item in second)
    checks.append({"name": "processes_alive", "pass": all_processes_alive})

    all_rpc_ok = len(usable) == len(nodes)
    checks.append({"name": "rpc_ok", "pass": all_rpc_ok})

    heights = [int(item["height"]) for item in usable]
    spread = max(heights) - min(heights) if heights else None
    checks.append(
        {
            "name": "height_spread",
            "pass": spread is not None and spread <= height_diff_max,
            "detail": {"spread": spread, "max_allowed": height_diff_max},
        }
    )

    validator_by_id = {n.id: n for n in nodes if n.validator}
    total_stake = sum(n.stake for n in validator_by_id.values())
    advancing_stake = 0
    advancing_nodes: list[str] = []
    if not skip_advancing:
        for node_id, node in validator_by_id.items():
            before = by_id_first.get(node_id) or {}
            after = by_id_second.get(node_id) or {}
            if (
                before.get("rpc_ok")
                and after.get("rpc_ok")
                and after.get("height") is not None
                and before.get("height") is not None
                and int(after["height"]) > int(before["height"])
            ):
                advancing_stake += node.stake
                advancing_nodes.append(node_id)
        checks.append(
            {
                "name": "validator_stake_advancing",
                "pass": total_stake > 0 and advancing_stake * 3 > total_stake * 2,
                "detail": {
                    "advancing_nodes": advancing_nodes,
                    "advancing_stake": str(advancing_stake),
                    "total_stake": str(total_stake),
                },
            }
        )

    common_block_samples: list[dict[str, Any]] = []
    if usable:
        common_height = max(0, min(int(item["height"]) for item in usable) - common_depth)
        for node in nodes:
            common_block_samples.append(sample_node(node, common_height))
        hashes = {
            item.get("hash")
            for item in common_block_samples
            if item.get("rpc_ok") and item.get("hash")
        }
        state_roots = {
            item.get("state_root")
            for item in common_block_samples
            if item.get("rpc_ok") and item.get("state_root")
        }
        checks.append(
            {
                "name": "common_height_no_fork",
                "pass": len(hashes) == 1 and len(common_block_samples) == len(nodes),
                "detail": {"height": common_height, "hashes": sorted(hashes)},
            }
        )
        checks.append(
            {
                "name": "common_height_state_root",
                "pass": len(state_roots) == 1 and len(common_block_samples) == len(nodes),
                "detail": {"height": common_height, "state_roots": sorted(state_roots)},
            }
        )

    log_scan = None
    if not skip_log_scan:
        log_scan = scan_logs(nodes, since=log_since, max_matches=log_max_matches)
        checks.append(
            {
                "name": "panic_log_scan",
                "pass": not log_scan["matches"],
                "detail": {
                    "matches": log_scan["matches"],
                    "scanned_files": log_scan["scanned_files"],
                    "skipped_old_files": log_scan["skipped_old_files"],
                    "truncated": log_scan["truncated"],
                },
            }
        )

    report = {
        "ts": time.time(),
        "checks": checks,
        "first_sample": first,
        "second_sample": second,
        "common_block_sample": common_block_samples,
    }
    if log_scan is not None:
        report["log_scan"] = log_scan
    ok = all(check["pass"] for check in checks)
    return report, ok


def cmd_nodes(args: argparse.Namespace) -> int:
    nodes = load_nodes(Path(args.config))
    if args.validators:
        nodes = [node for node in nodes if node.validator]
    if args.json:
        print(json.dumps([asdict(node) for node in nodes], indent=2))
    else:
        for node in nodes:
            print(node.id)
    return 0


def cmd_node_field(args: argparse.Namespace) -> int:
    node = node_by_id(load_nodes(Path(args.config)), args.node)
    print(getattr(node, args.field))
    return 0


def cmd_node_ports(args: argparse.Namespace) -> int:
    node = node_by_id(load_nodes(Path(args.config)), args.node)
    ports = [
        port
        for port in (node.validator_port, node.vfn_port, node.public_port)
        if port is not None
    ]
    if args.json:
        print(json.dumps({"node": node.id, "host": node.host, "ports": ports}, indent=2))
    else:
        print(" ".join(str(port) for port in ports))
    return 0


def cmd_peer_ports(args: argparse.Namespace) -> int:
    nodes = load_nodes(Path(args.config))
    target = node_by_id(nodes, args.node)
    ports = sorted(
        {
            port
            for node in nodes
            if node.id != target.id
            for port in (node.validator_port, node.vfn_port, node.public_port)
            if port is not None
        }
    )
    if args.json:
        print(json.dumps({"node": target.id, "peer_ports": ports}, indent=2))
    else:
        print(" ".join(str(port) for port in ports))
    return 0


def cmd_peer_hosts(args: argparse.Namespace) -> int:
    nodes = load_nodes(Path(args.config))
    target = node_by_id(nodes, args.node)
    hosts = sorted({node.host for node in nodes if node.id != target.id and node.host != target.host})
    if args.json:
        print(json.dumps({"node": target.id, "host": target.host, "peer_hosts": hosts}, indent=2))
    else:
        print(" ".join(hosts))
    return 0


def cmd_majority_victims(args: argparse.Namespace) -> int:
    validators = [node for node in load_nodes(Path(args.config)) if node.validator]
    total = sum(node.stake for node in validators)
    if total <= 0:
        raise SystemExit("no validator stake found")
    selected: list[Node] = []
    selected_stake = 0
    for node in sorted(validators, key=lambda item: item.stake, reverse=True):
        selected.append(node)
        selected_stake += node.stake
        if selected_stake * 3 > total:
            break
    if args.json:
        print(
            json.dumps(
                {
                    "total_stake": str(total),
                    "selected_stake": str(selected_stake),
                    "victims": [asdict(node) for node in selected],
                },
                indent=2,
            )
        )
    else:
        print(" ".join(node.id for node in selected))
    return 0


def cmd_choose_single_victim(args: argparse.Namespace) -> int:
    result = choose_quorum_safe_victim(validator_nodes(Path(args.config)), args.node)
    print_topology_result(
        result,
        args,
        ["victim", "probe", "majority_csv", "victim_stake", "majority_stake", "total_stake"],
    )
    return 0


def cmd_choose_majority_minority_split(args: argparse.Namespace) -> int:
    result = choose_majority_minority_split(validator_nodes(Path(args.config)), args.minority)
    print_topology_result(
        result,
        args,
        ["majority_csv", "minority_csv", "probe", "majority_stake", "minority_stake", "total_stake"],
    )
    return 0


def cmd_choose_no_quorum_split(args: argparse.Namespace) -> int:
    result = choose_no_quorum_split(validator_nodes(Path(args.config)), args.left, args.right)
    print_topology_result(
        result,
        args,
        ["left_csv", "right_csv", "left_stake", "right_stake", "total_stake"],
    )
    return 0


def cmd_snapshot(args: argparse.Namespace) -> int:
    nodes = load_nodes(Path(args.config))
    if args.validators:
        nodes = [node for node in nodes if node.validator]
    only = set(args.nodes.split(",")) if args.nodes else None
    exclude = set(args.exclude.split(",")) if args.exclude else set()
    selected = [node for node in nodes if (only is None or node.id in only) and node.id not in exclude]
    print(json.dumps({"ts": time.time(), "nodes": sample_nodes(selected)}, indent=2))
    return 0


def cmd_oracle(args: argparse.Namespace) -> int:
    nodes = load_nodes(Path(args.config))
    if args.validators:
        nodes = [node for node in nodes if node.validator]
    report, ok = oracle_report(
        nodes,
        height_diff_max=args.height_diff_max,
        sample_interval=args.sample_interval,
        common_depth=args.common_depth,
        skip_process=args.skip_process,
        skip_advancing=args.skip_advancing,
        skip_log_scan=args.skip_log_scan,
        log_since=args.log_since,
        log_max_matches=args.log_max_matches,
    )
    print(json.dumps(report, indent=2))
    return 0 if ok else 1


def cmd_assert_stalled(args: argparse.Namespace) -> int:
    nodes = [node for node in load_nodes(Path(args.config)) if node.validator]
    exclude = set(args.exclude.split(",")) if args.exclude else set()
    selected = [node for node in nodes if node.id not in exclude]
    first = sample_nodes(selected)
    time.sleep(args.hold)
    second = sample_nodes(selected)
    by_id_first = {item["node"]: item for item in first}
    advanced: list[dict[str, Any]] = []
    for after in second:
        before = by_id_first.get(after["node"]) or {}
        if (
            before.get("rpc_ok")
            and after.get("rpc_ok")
            and before.get("height") is not None
            and after.get("height") is not None
            and int(after["height"]) > int(before["height"])
        ):
            advanced.append(
                {
                    "node": after["node"],
                    "before": before["height"],
                    "after": after["height"],
                }
            )
    report = {
        "ts": time.time(),
        "hold": args.hold,
        "excluded": sorted(exclude),
        "first_sample": first,
        "second_sample": second,
        "advanced": advanced,
        "pass": not advanced,
    }
    print(json.dumps(report, indent=2))
    return 0 if not advanced else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)

    def add_config(p: argparse.ArgumentParser) -> None:
        p.add_argument("--config", required=True)

    p = sub.add_parser("nodes")
    add_config(p)
    p.add_argument("--validators", action="store_true")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_nodes)

    p = sub.add_parser("node-field")
    add_config(p)
    p.add_argument("node")
    p.add_argument("field")
    p.set_defaults(func=cmd_node_field)

    p = sub.add_parser("node-ports")
    add_config(p)
    p.add_argument("node")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_node_ports)

    p = sub.add_parser("peer-ports")
    add_config(p)
    p.add_argument("node")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_peer_ports)

    p = sub.add_parser("peer-hosts")
    add_config(p)
    p.add_argument("node")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_peer_hosts)

    p = sub.add_parser("majority-victims")
    add_config(p)
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_majority_victims)

    p = sub.add_parser("choose-single-victim")
    add_config(p)
    p.add_argument("--node", default="")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_choose_single_victim)

    p = sub.add_parser("choose-majority-minority-split")
    add_config(p)
    p.add_argument("--minority", default="")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_choose_majority_minority_split)

    p = sub.add_parser("choose-no-quorum-split")
    add_config(p)
    p.add_argument("--left", default="")
    p.add_argument("--right", default="")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_choose_no_quorum_split)

    p = sub.add_parser("snapshot")
    add_config(p)
    p.add_argument("--validators", action="store_true")
    p.add_argument("--nodes", default="")
    p.add_argument("--exclude", default="")
    p.set_defaults(func=cmd_snapshot)

    p = sub.add_parser("oracle")
    add_config(p)
    p.add_argument("--validators", action="store_true")
    p.add_argument("--height-diff-max", type=int, default=10)
    p.add_argument("--sample-interval", type=float, default=3.0)
    p.add_argument("--common-depth", type=int, default=2)
    p.add_argument("--skip-process", action="store_true")
    p.add_argument("--skip-advancing", action="store_true")
    p.add_argument("--skip-log-scan", action="store_true")
    p.add_argument("--log-since", type=float, default=None)
    p.add_argument("--log-max-matches", type=int, default=20)
    p.set_defaults(func=cmd_oracle)

    p = sub.add_parser("assert-stalled")
    add_config(p)
    p.add_argument("--exclude", default="")
    p.add_argument("--hold", type=float, default=20.0)
    p.set_defaults(func=cmd_assert_stalled)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
