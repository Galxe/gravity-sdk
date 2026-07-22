"""场景注册表：名字 -> 运行函数。"""
from __future__ import annotations

from typing import Callable

from . import halt_7702
from . import halt_7702_param
from . import revert_probe
from . import accesslist_probe

# name -> (描述, 运行函数(nodes:int, params:dict)->dict)
_SCENARIOS: dict[str, tuple[str, Callable]] = {
    "7702-halt": (
        "EIP-7702 委托到含 CREATE 的合约，同块 nonce 竞争触发 revm NonceTooLow → 链停",
        halt_7702.run,
    ),
    "7702-race": (
        "7702 nonce 竞争的参数化模板（旋钮: cross_sender_action/nonce_offset/auth_count/"
        "chain_id/delegate_chain/attempts/window）—— 供搜索停链变体",
        halt_7702_param.run,
    ),
    "revert-probe": (
        "部署永远 revert 的合约并调用，验证普通 EVM revert 被优雅处理（链不停）",
        revert_probe.run,
    ),
    "accesslist-probe": (
        "发一笔 EIP-2930 type-1 access-list 交易，验证 typed-tx 通路健康、链不停（正对照）",
        accesslist_probe.run,
    ),
}


def list_scenarios() -> list[tuple[str, str]]:
    return [(name, desc) for name, (desc, _fn) in _SCENARIOS.items()]


def run(name: str, *, preset, instance: int = 0, params: dict) -> dict:
    if name not in _SCENARIOS:
        avail = ", ".join(_SCENARIOS)
        return {"scenario": name, "verdict": "error", "detail": f"未知场景 '{name}'。可用: {avail}"}
    _desc, fn = _SCENARIOS[name]

    # 按场景声明的 PARAMS 校验 + 强转参数：拒绝拼错/未知的 key，坏值给干净错误而非 traceback。
    import sys
    spec = getattr(sys.modules.get(fn.__module__), "PARAMS", None)
    if spec is not None:
        clean: dict = {}
        for k, v in params.items():
            if k not in spec:
                return {"scenario": name, "verdict": "error", "usage_error": True,
                        "detail": f"未知场景参数 '{k}'；{name} 支持: {sorted(spec) or '（无）'}"}
            caster = spec[k]
            try:
                clean[k] = caster(v)
            except (ValueError, TypeError):
                return {"scenario": name, "verdict": "error", "usage_error": True,
                        "detail": f"参数 {k}={v!r} 无法转成 {getattr(caster, '__name__', caster)}"}
        params = clean

    # 兜底：场景内部任何异常（如注入到一半节点被打挂、RPC 断开）都转成结构化结果，
    # 而不是抛 traceback，保证 --json 契约永远成立。
    try:
        return fn(preset=preset, instance=instance, params=params)
    except Exception as e:  # noqa: BLE001 —— 面向 agent 的工具，任何异常都要给结构化输出
        import traceback
        return {
            "scenario": name,
            "verdict": "error",
            "detail": f"场景执行抛异常: {type(e).__name__}: {e}",
            "traceback": traceback.format_exc().splitlines()[-6:],
        }
