"""场景共用小工具：签发原始交易、RPC 可达性、起手健康检查。

所有场景都应复用 `preflight_error`：起手就不健康 = 环境问题(ERROR)，
不是本次探测/攻击造成的 halt/panic —— 否则会把「集群本来就没起」误报成「命中链停」。
"""
from __future__ import annotations

from typing import Optional

from web3 import Web3

from ..ops import _pid_alive
from ..verdict import Verdict


def send_raw(w3: Web3, acct, tx: dict) -> str:
    signed = acct.sign_transaction(tx)
    return w3.eth.send_raw_transaction(signed.raw_transaction).to_0x_hex()


def rpc_ok(w3: Web3) -> bool:
    try:
        _ = w3.eth.block_number
        return True
    except Exception:
        return False


def preflight_error(cp, w3: Web3) -> Optional[dict]:
    """起手健康检查。不健康返回 {verdict: error, detail}（供场景直接 return），健康返回 None。"""
    node_id = cp.node_ids()[0]
    if not _pid_alive(cp.pid_file(node_id)):
        return {"verdict": Verdict.ERROR.value,
                "detail": f"起手节点进程就未运行（可能被上次攻击打挂）；先 `gnode up --preset {cp.name} --fresh` 再打"}
    if not rpc_ok(w3):
        return {"verdict": Verdict.ERROR.value,
                "detail": f"起手进程在但 RPC 无响应（疑似残留 halt）；先 `gnode up --preset {cp.name} --fresh` 再打"}
    return None
