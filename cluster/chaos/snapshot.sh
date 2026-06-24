#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLUSTER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="${CLUSTER_TOML:-$CLUSTER_DIR/cluster.toml}"
OUT_DIR="${CHAOS_SNAPSHOT_DIR:-$SCRIPT_DIR/snapshots/snapshot-$(date +%Y%m%d-%H%M%S)}"
TAIL_LINES="${CHAOS_SNAPSHOT_TAIL_LINES:-200}"
CHAOS_BACKEND="${CHAOS_BACKEND:-local}"
CHAOS_START_TS="${CHAOS_START_TS:-$(date +%s)}"

usage() {
    cat <<'EOF'
Usage:
  snapshot.sh [--config <cluster.toml>] [--out <dir>]

Collects a failure snapshot without healing or restarting anything.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        --out)
            OUT_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown parameter: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

mkdir -p "$OUT_DIR"

{
    echo "created_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "config=$CONFIG_FILE"
    echo "backend=$CHAOS_BACKEND"
    echo "chaos_start_ts=$CHAOS_START_TS"
    echo "pwd=$(pwd)"
} > "$OUT_DIR/meta.txt"

bash "$SCRIPT_DIR/chaos.sh" --config "$CONFIG_FILE" snapshot > "$OUT_DIR/rpc-snapshot.json" 2>"$OUT_DIR/rpc-snapshot.err" || true
LOG_SINCE="$CHAOS_START_TS" bash "$SCRIPT_DIR/oracle.sh" --config "$CONFIG_FILE" --skip-advancing > "$OUT_DIR/oracle.json" 2>"$OUT_DIR/oracle.err" || true
bash "$SCRIPT_DIR/chaos.sh" --config "$CONFIG_FILE" net-status > "$OUT_DIR/net-status.txt" 2>"$OUT_DIR/net-status.err" || true

python3 "$SCRIPT_DIR/lib/cluster.py" nodes --config "$CONFIG_FILE" --json > "$OUT_DIR/nodes.json"

if [ "$CHAOS_BACKEND" = "docker" ]; then
    docker ps -a --format '{{.ID}} {{.Names}} {{.Image}} {{.Status}} {{.Ports}}' > "$OUT_DIR/docker-ps.txt" 2>"$OUT_DIR/docker-ps.err" || true
fi

python3 - "$OUT_DIR/nodes.json" "$OUT_DIR" "$TAIL_LINES" <<'PY'
import json
import os
import shutil
import sys
from pathlib import Path

nodes_path = Path(sys.argv[1])
out_dir = Path(sys.argv[2])
tail_lines = int(sys.argv[3])
nodes = json.loads(nodes_path.read_text())

def tail_file(src: Path, dst: Path) -> None:
    try:
        lines = src.read_text(errors="replace").splitlines()
    except Exception as exc:
        dst.write_text(f"failed to read {src}: {exc}\n")
        return
    dst.write_text("\n".join(lines[-tail_lines:]) + ("\n" if lines else ""))

for node in nodes:
    node_id = node["id"]
    data_dir = Path(node["data_dir"])
    node_out = out_dir / "nodes" / node_id
    node_out.mkdir(parents=True, exist_ok=True)

    pid_file = data_dir / "script" / "node.pid"
    if pid_file.exists():
        pid = pid_file.read_text().strip()
        (node_out / "pid.txt").write_text(pid + "\n")
        proc = Path("/proc") / pid
        if proc.exists():
            for name in ("status", "limits"):
                src = proc / name
                if src.exists():
                    shutil.copyfile(src, node_out / f"proc-{name}")
            fd = proc / "fd"
            if fd.exists():
                try:
                    (node_out / "fd-count.txt").write_text(str(len(list(fd.iterdir()))) + "\n")
                except Exception:
                    pass

    candidates = [
        data_dir / "logs" / "debug.log",
        data_dir / "logs" / "consensus_log" / "consensus.log",
    ]
    candidates.extend((data_dir / "logs" / "consensus_log").glob("*.log"))
    candidates.extend((data_dir / "logs" / "execution_logs").glob("**/reth.log"))
    seen = set()
    for src in candidates:
        if not src.exists() or src in seen:
            continue
        seen.add(src)
        rel = src.relative_to(data_dir)
        dst = node_out / (str(rel).replace("/", "__") + ".tail")
        tail_file(src, dst)
PY

echo "$OUT_DIR"
