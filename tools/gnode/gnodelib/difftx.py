"""gnode difftx / difftx-exec —— 把 Rust 差分预言机（D1）串进 gnode 的统一 UX。

设计要点：
  - 纯静态差分，无需集群 —— 不碰 resolve_cluster / RPC。
  - 直接 `cargo run` 兄弟 gravity-reth worktree 里的 difftx / difftx_exec 二进制，
    透传子进程 stdout/stderr（继承父进程 fd，实时流式），并原样传播退出码：
        3 = 发现停链缺口 / 执行背离（gap/divergence）
        0 = 干净（两侧一致）
  - 命中（非 0）时补一行指向 difftx-repro/*.json 复现工件的提示。
  - worktree 缺失 / cargo 不在 PATH → 干净的一行错误 + 退出码 2（用法错误），
    绝不抛 traceback。

Rust bin 把 difftx-repro/ 写在**运行时 cwd** 下；为使工件位置确定，
本模块把子进程 cwd 固定为 GRAVITY_RETH_DIR，工件即落在 <reth>/difftx-repro/。
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

# 兄弟 gravity-reth worktree 默认位置；可用环境变量 GRAVITY_RETH_DIR 覆盖。
DEFAULT_RETH_DIR = "/mnt/data2/kenji/galxe/gravity-reth-difftx"

# kind -> (cargo --features, --bin, 人读说明)
_VARIANTS = {
    "difftx": ("difftx", "difftx",
               "tx_filter ⊇ revm 交易级差分矩阵 + 7702 自检（纯静态，无需集群）"),
    "difftx-exec": ("difftx_exec", "difftx_exec",
                    "serial(revm) ⟷ grevm 执行差分（纯静态，无需集群）"),
}


def _reth_dir() -> Path:
    return Path(os.environ.get("GRAVITY_RETH_DIR", DEFAULT_RETH_DIR)).expanduser()


def cmd_difftx(kind: str) -> int:
    """运行差分预言机二进制并透传其输出/退出码。kind ∈ {'difftx','difftx-exec'}。"""
    if kind not in _VARIANTS:
        print(f"[gnode] error: 未知 difftx 变体 '{kind}'（支持: {sorted(_VARIANTS)}）",
              file=sys.stderr)
        return 2
    features, binname, _desc = _VARIANTS[kind]

    reth = _reth_dir()
    manifest = reth / "Cargo.toml"
    # —— 环境预检：worktree / manifest / cargo 缺失都归「用法错误」(2)，给一行干净提示 ——
    if not reth.is_dir():
        print(f"[gnode] error: gravity-reth worktree 不存在: {reth}\n"
              f"        设置环境变量 GRAVITY_RETH_DIR 指向含 difftx 二进制的 worktree。",
              file=sys.stderr)
        return 2
    if not manifest.is_file():
        print(f"[gnode] error: 找不到 Cargo.toml: {manifest}\n"
              f"        GRAVITY_RETH_DIR 应指向 gravity-reth 仓库根（含 Cargo.toml）。",
              file=sys.stderr)
        return 2
    if shutil.which("cargo") is None:
        print("[gnode] error: 找不到 cargo（Rust 工具链未安装或不在 PATH）；"
              "请确保 ~/.cargo/bin 在 PATH。", file=sys.stderr)
        return 2

    cmd = [
        "cargo", "run", "-q",
        "--manifest-path", str(manifest),
        "-p", "reth-pipe-exec-layer-ext-v2",
        "--features", features,
        "--bin", binname,
    ]
    print(f"[gnode] difftx: {_VARIANTS[kind][2]}", file=sys.stderr)
    print(f"[gnode] 运行: {' '.join(cmd)}  (cwd={reth})", file=sys.stderr)

    # 子进程 stdout/stderr 继承父进程 → 实时流式；cwd 固定到 reth 使 difftx-repro/ 位置确定。
    try:
        proc = subprocess.run(cmd, cwd=str(reth))
    except FileNotFoundError as e:
        # cargo 二进制在 which 之后又消失等极端情况
        print(f"[gnode] error: 无法执行 cargo: {e}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("[gnode] difftx 被中断", file=sys.stderr)
        return 1

    rc = proc.returncode
    # 退出码原样透传。仅 rc==3 才是「发现 gap/背离」；其它非零（如 cargo 编译失败 101、
    # 运行错误 1）不是 gap 判定，别误报——否则 agent 会把编译失败当成停链缺口。
    if rc == 3:
        repro = reth / "difftx-repro"
        print(f"\n[gnode] difftx 退出码 3 —— 发现停链缺口/执行背离(gap/divergence)。"
              f"\n[gnode] 复现工件（gnode send 兼容）见: {repro}/*.json",
              file=sys.stderr)
    elif rc not in (0, 3):
        print(f"\n[gnode] difftx 退出码 {rc} —— 非 gap 判定（cargo 编译/运行错误等）；"
              f"请检查上面的构建/运行输出。", file=sys.stderr)
    return rc
