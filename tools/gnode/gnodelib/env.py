"""路径 / 集群档位 / web3 工厂 / gas 估算 —— gnode 的运行环境层。

约定：所有 RPC 走本地节点，http_proxy 已在 gnode 启动器里绕过。
"""
from __future__ import annotations

import re
import socket
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from eth_account import Account
from eth_account.signers.local import LocalAccount
from web3 import Web3

# ---- 关键路径（相对本文件推导，避免依赖 cwd）----
_GNODELIB_DIR = Path(__file__).resolve().parent
GNODE_DIR = _GNODELIB_DIR.parent            # tools/gnode
GSDK_ROOT = GNODE_DIR.parent.parent         # gravity-sdk
CLUSTER_DIR = GSDK_ROOT / "cluster"
E2E_DIR = GSDK_ROOT / "gravity_e2e"
TARGET_DIR = GSDK_ROOT / "target" / "quick-release"
NODE_BIN = TARGET_DIR / "gravity_node"
CLI_BIN = TARGET_DIR / "gravity_cli"

# gnode 自持的集群档位（配置在 tools/gnode/presets/<name>/，base_dir/端口独立，避免和他人共享 /tmp 冲突）
PRESETS_DIR = GNODE_DIR / "presets"
# 默认走 1node 快速档（“快速启动”优先）；prague 档启用 EIP-7702（7702-halt 场景必需）
PRESETS = {
    "1node": {"prague": False},
    "prague": {"prague": True},
}
# --nodes 的数字别名（向后兼容；目前只有 1node，3node 档未接入）
NODE_ALIASES = {1: "1node"}

# 多实例隔离：instance N 的所有端口 = 基准端口 + N*STRIDE，base_dir 带 -N 后缀。
# 所有 instance 共享同一份 genesis+identity（单验证者节点不校验对端口，实测可行），
# 因此只有 instance 首次不存在时 deploy+start，genesis 只生成一次 → 每个 instance 都很快。
PORT_STRIDE = 100
MAX_INSTANCES = 32
GNODE_WORK = Path("/tmp/gnode-work")  # 每个 instance 的物化 cluster.toml 放这里
_PORT_KEYS = ["validator_port", "vfn_port", "rpc_port", "metrics_port",
              "inspection_port", "https_port", "authrpc_port", "reth_p2p_port"]


def _apply_port_offset(text: str, offset: int) -> str:
    """把 cluster.toml 里各端口字段整体 +offset（只动已知端口键，避免误伤其它数字）。"""
    if offset == 0:
        return text
    for k in _PORT_KEYS:
        text = re.sub(rf"({k}\s*=\s*)(\d+)", lambda m: f"{m.group(1)}{int(m.group(2)) + offset}", text)
    return text


def _port_free(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        return s.connect_ex(("127.0.0.1", port)) != 0

# genesis.toml 里的 faucet = Anvil dev #0，私钥公开可用（本地 devnet）
FAUCET_PRIVKEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"


@dataclass
class ClusterPaths:
    """一个集群档位的全部相关路径。"""

    name: str
    preset_dir: Path
    cluster_toml: Path
    genesis_toml: Path
    base_dir: Path
    artifacts_dir: Path
    chain_id: int
    prague: bool = False
    instance: int = 0

    def node_ids(self) -> list[str]:
        cfg = _load_toml(self.cluster_toml)
        return [n["id"] for n in cfg.get("nodes", [])]

    def rpc_url(self, node_id: Optional[str] = None) -> str:
        cfg = _load_toml(self.cluster_toml)
        nodes = cfg.get("nodes", [])
        node = nodes[0] if node_id is None else next(n for n in nodes if n["id"] == node_id)
        host = node.get("host", "127.0.0.1")
        return f"http://{host}:{node['rpc_port']}"

    def node_dir(self, node_id: str) -> Path:
        return self.base_dir / node_id

    def log_files(self, node_id: str) -> dict[str, Path]:
        d = self.node_dir(node_id)
        return {
            "reth": d / "execution_logs" / "dev" / "reth.log",
            "consensus": d / "consensus_log" / "validator.log",
            "debug": d / "logs" / "debug.log",
        }

    def pid_file(self, node_id: str) -> Path:
        return self.node_dir(node_id) / "script" / "node.pid"


def _load_toml(path: Path) -> dict:
    with open(path, "rb") as f:
        return tomllib.load(f)


def _preset_name(preset) -> str:
    if isinstance(preset, int):
        preset = NODE_ALIASES.get(preset, str(preset))
    name = str(preset)
    if name.isdigit():  # argparse 传进来的 --nodes 是字符串，映射数字别名
        name = NODE_ALIASES.get(int(name), name)
    if name not in PRESETS:
        raise ValueError(f"未知档位 '{name}'（支持: {sorted(PRESETS)}；4-validator docker 档暂未接入）")
    return name


def resolve_cluster(preset, instance: int = 0) -> ClusterPaths:
    """把档位名 + instance 号解析成 ClusterPaths。

    instance 0 = 基准端口 + base_dir /tmp/gnode-<name>；instance N = 端口 +N*STRIDE、
    base_dir /tmp/gnode-<name>-N。所有 instance 共享 preset 的 genesis+identity(artifacts/)，
    只物化一份带偏移端口的 cluster.toml 到 /tmp/gnode-work/<name>-N/。
    """
    name = _preset_name(preset)
    if not (0 <= instance < MAX_INSTANCES):
        raise ValueError(f"instance 需在 [0,{MAX_INSTANCES}) 内，得到 {instance}")
    preset_dir = PRESETS_DIR / name
    tmpl = preset_dir / "cluster.toml"
    genesis_toml = preset_dir / "genesis.toml"
    if not tmpl.exists():
        raise FileNotFoundError(f"cluster.toml 模板不存在: {tmpl}")
    shared_artifacts = preset_dir / "artifacts"  # genesis+identity 全 instance 共享

    suffix = "" if instance == 0 else f"-{instance}"
    base_dir = Path(f"/tmp/gnode-{name}{suffix}")

    # 物化本 instance 的 cluster.toml：偏移端口 + 独立 base_dir + genesis_source 指向共享 artifacts
    text = _apply_port_offset(tmpl.read_text(), instance * PORT_STRIDE)
    text = re.sub(r'base_dir\s*=\s*"[^"]*"', f'base_dir = "{base_dir}"', text)
    text = re.sub(r'genesis_path\s*=\s*"[^"]*"', f'genesis_path = "{shared_artifacts}/genesis.json"', text)
    text = re.sub(r'waypoint_path\s*=\s*"[^"]*"', f'waypoint_path = "{shared_artifacts}/waypoint.txt"', text)
    work = GNODE_WORK / f"{name}-{instance}"
    work.mkdir(parents=True, exist_ok=True)
    cluster_toml = work / "cluster.toml"
    cluster_toml.write_text(text)

    chain_id = 1337
    if genesis_toml.exists():
        chain_id = int(_load_toml(genesis_toml).get("genesis", {}).get("chain_id", 1337))
    return ClusterPaths(
        name=name,
        preset_dir=preset_dir,
        cluster_toml=cluster_toml,
        genesis_toml=genesis_toml,
        base_dir=base_dir,
        artifacts_dir=shared_artifacts,
        chain_id=chain_id,
        prague=PRESETS[name]["prague"],
        instance=instance,
    )


def alloc_instance(preset) -> int:
    """自动分配一个空闲 instance：base_dir 不存在且 RPC 端口空闲的最小号。"""
    name = _preset_name(preset)
    base_rpc = int(_load_toml(PRESETS_DIR / name / "cluster.toml")["nodes"][0]["rpc_port"])
    for i in range(MAX_INSTANCES):
        suffix = "" if i == 0 else f"-{i}"
        if not Path(f"/tmp/gnode-{name}{suffix}").exists() and _port_free(base_rpc + i * PORT_STRIDE):
            return i
    raise RuntimeError(f"没有空闲 instance（0..{MAX_INSTANCES} 都被占用）")


def make_web3(rpc_url: str, timeout: int = 15) -> Web3:
    return Web3(Web3.HTTPProvider(rpc_url, request_kwargs={"timeout": timeout}))


def faucet_account() -> LocalAccount:
    return Account.from_key(FAUCET_PRIVKEY)


def suggest_fees(w3: Web3, priority_gwei: int = 2) -> dict:
    """基于最新区块 baseFee 给出 EIP-1559 gas 参数（genesis 设了 gravityMinBaseFee=50gwei）。"""
    latest = w3.eth.get_block("latest")
    base = latest.get("baseFeePerGas") or w3.to_wei(50, "gwei")
    prio = w3.to_wei(priority_gwei, "gwei")
    return {
        "maxPriorityFeePerGas": prio,
        # 2x baseFee 余量，避免高度推进后 baseFee 上浮导致 underpriced
        "maxFeePerGas": base * 2 + prio,
    }
