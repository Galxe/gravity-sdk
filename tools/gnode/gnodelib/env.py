"""路径 / 集群档位 / web3 工厂 / gas 估算 —— gnode 的运行环境层。

约定：所有 RPC 走本地节点，http_proxy 已在 gnode 启动器里绕过。
"""
from __future__ import annotations

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


def resolve_cluster(preset) -> ClusterPaths:
    """把档位名（"1node"/"prague"）或数字别名（1 / "1"）解析成 ClusterPaths。"""
    if isinstance(preset, int):
        preset = NODE_ALIASES.get(preset, str(preset))
    name = str(preset)
    if name.isdigit():  # argparse 传进来的 --nodes 是字符串，映射数字别名
        name = NODE_ALIASES.get(int(name), name)
    if name not in PRESETS:
        raise ValueError(f"未知档位 '{name}'（支持: {sorted(PRESETS)}；4-validator docker 档暂未接入）")
    preset_dir = PRESETS_DIR / name
    cluster_toml = preset_dir / "cluster.toml"
    genesis_toml = preset_dir / "genesis.toml"
    if not cluster_toml.exists():
        raise FileNotFoundError(f"cluster.toml 不存在: {cluster_toml}（先生成 gnode preset）")
    ccfg = _load_toml(cluster_toml)
    base_dir = Path(ccfg["cluster"]["base_dir"])
    chain_id = 1337
    if genesis_toml.exists():
        gcfg = _load_toml(genesis_toml)
        chain_id = int(gcfg.get("genesis", {}).get("chain_id", 1337))
    return ClusterPaths(
        name=name,
        preset_dir=preset_dir,
        cluster_toml=cluster_toml,
        genesis_toml=genesis_toml,
        base_dir=base_dir,
        artifacts_dir=preset_dir / "artifacts",
        chain_id=chain_id,
        prague=PRESETS[name]["prague"],
    )


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
