"""三态判定：halt / revert / alive（+ 进程 panic）。

攻击场景的核心断言：一次注入之后，链是「停摆(halt)」、「交易回滚但链活(revert)」
还是「安然无恙(alive)」。single-node 下 state-fork 表现为节点 panic/halt，
多节点档另可跨节点比 state_root。
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional

from web3 import Web3


class Verdict(str, Enum):
    ALIVE = "alive"                # 链继续正常出块
    HALT = "halt"                  # 出块停摆（RPC 卡死或高度不再增长）
    PANIC = "panic"                # 节点进程直接挂掉
    REVERT = "revert"              # 目标交易回滚，但链仍在出块
    INCONCLUSIVE = "inconclusive"  # 注入未按预期成形（如未同块/未按序），结论不成立，应重试
    ERROR = "error"                # 场景执行出错（如节点不健康、部署失败）


@dataclass
class HaltProbe:
    verdict: Verdict
    start_block: Optional[int]
    end_block: Optional[int]
    window_s: float
    detail: str = ""
    extra: dict = field(default_factory=dict)

    def as_dict(self) -> dict:
        return {
            "verdict": self.verdict.value,
            "start_block": self.start_block,
            "end_block": self.end_block,
            "window_s": self.window_s,
            "detail": self.detail,
            **({"extra": self.extra} if self.extra else {}),
        }


def _safe_block_number(w3: Web3) -> Optional[int]:
    try:
        return int(w3.eth.block_number)
    except Exception:
        return None


def probe_liveness(
    w3: Web3,
    *,
    window_s: float = 12.0,
    poll_s: float = 1.0,
    min_delta: int = 1,
    pid_alive=None,
) -> HaltProbe:
    """在 window_s 窗口内观察出块是否推进，判定 alive / halt / panic。

    pid_alive: 可选 callable()->bool，用于区分「halt(进程在但不出块)」与「panic(进程没了)」。
    """
    start = _safe_block_number(w3)
    if start is None:
        # 一开始就连不上 RPC
        if pid_alive is not None and not pid_alive():
            return HaltProbe(Verdict.PANIC, None, None, 0.0, "RPC 无响应且进程已退出")
        return HaltProbe(Verdict.HALT, None, None, 0.0, "RPC 无响应（进程仍在）")

    deadline = time.time() + window_s
    end = start
    while time.time() < deadline:
        time.sleep(poll_s)
        cur = _safe_block_number(w3)
        if cur is None:
            if pid_alive is not None and not pid_alive():
                return HaltProbe(Verdict.PANIC, start, end, window_s, "观察期内 RPC 断开且进程退出")
            return HaltProbe(Verdict.HALT, start, end, window_s, "观察期内 RPC 断开（进程仍在）")
        end = cur
        if cur - start >= min_delta:
            return HaltProbe(
                Verdict.ALIVE, start, end, window_s,
                f"出块推进 {start}→{end}",
            )

    # 窗口内高度没有推进到阈值
    if pid_alive is not None and not pid_alive():
        return HaltProbe(Verdict.PANIC, start, end, window_s, "进程已退出且未出块")
    return HaltProbe(
        Verdict.HALT, start, end, window_s,
        f"{window_s:.0f}s 内高度停在 {start}（未推进 ≥{min_delta}），判定 halt",
    )
