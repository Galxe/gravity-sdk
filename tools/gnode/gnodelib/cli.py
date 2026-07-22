"""gnode 命令行分发。"""
from __future__ import annotations

import argparse
import json
import os
import sys

from . import ops
from .scenarios import registry


def _add_preset(p: argparse.ArgumentParser, default: str = "1node") -> None:
    p.add_argument(
        "--preset", "--nodes", dest="preset", default=default,
        help="集群档位（1node|prague，或数字 1=1node；默认 %(default)s。prague 启用 EIP-7702。"
             "status 不带该参数时列出所有档位）",
    )
    p.add_argument(
        "--instance", dest="instance", default=os.environ.get("GNODE_INSTANCE"),
        help="实例号（并发隔离：端口+N*100、base_dir 带 -N；缺省读 GNODE_INSTANCE，"
             "未设时 up 自动分配空闲号、其它命令用 0）。所有 instance 共享同一份 genesis。",
    )


def _inst(args, default=0):
    v = getattr(args, "instance", None)
    if v is None:
        return default
    return "auto" if str(v) == "auto" else int(v)


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="gnode", description="Gravity 集群端到端攻击/PoC 工具")
    sub = p.add_subparsers(dest="cmd", required=True)

    up = sub.add_parser("up", help="拉起本地集群")
    _add_preset(up)
    up.add_argument("--fresh", action="store_true", help="清理 artifacts/base_dir 后重新 genesis")

    down = sub.add_parser("down", help="停止集群")
    _add_preset(down)

    st = sub.add_parser("status", help="各节点存活/RPC/区块高度")
    _add_preset(st)

    lg = sub.add_parser("logs", help="查看节点日志")
    _add_preset(lg)
    lg.add_argument("--which", choices=["reth", "consensus", "debug", "all"], default="reth")
    lg.add_argument("--follow", "-f", action="store_true", help="持续跟随")
    lg.add_argument("--lines", "-n", type=int, default=40)

    stt = sub.add_parser("state", help="查 balance/nonce/code + halt 探测")
    _add_preset(stt)
    stt.add_argument("addr", help="要查询的地址")

    dp = sub.add_parser(
        "deploy", help="部署合约（{abi,bytecode} 或纯 bytecode）",
        epilog="artifact 文件可为：① {\"abi\":[...],\"bytecode\":\"0x..\"}（solc/forge 产物）"
               "② 纯 bytecode 十六进制（.hex/.bin，带不带 0x 均可）。用 faucet 自动签名部署。",
    )
    _add_preset(dp)
    dp.add_argument("artifact", help="合约 artifact 文件路径")
    dp.add_argument("--args", help="构造函数参数，JSON 数组，如 '[42,\"0xabc\"]'（需 artifact 带 abi）")

    sd = sub.add_parser(
        "send", help="发一笔交易（tx.json 规格）",
        epilog=(
            "tx.json 字段（均可选，缺省用 faucet 签名）：\n"
            "  to        目标地址（缺省=合约创建）\n"
            "  value     wei（非负整数；十进制或 0x 十六进制字符串）\n"
            "  data      calldata 十六进制\n"
            "  nonce     缺省=账户当前 nonce\n"
            "  gas       缺省=estimate*1.2\n"
            "  gasPrice  仅 type 0/1(legacy/access-list) 用；缺省自动取合理值\n"
            "  type      交易类型：2(EIP-1559,默认) / 0(legacy) / 1(access-list) / 4(EIP-7702 SetCode)\n"
            "  accessList        type=1 的 access list\n"
            "  authorizationList type=4 的 7702 授权列表。条目可为：\n"
            "                    已签名 {chainId,address,nonce,yParity,r,s}；或\n"
            "                    待签名 {delegate,signerKey,nonce?} —— 自动签，\n"
            "                    self-sponsored 时 nonce 缺省=tx.nonce+1\n"
            "  raw       已签名的原始交易十六进制（给了它则忽略其他字段，直接广播）\n"
            "  privkey   自定义签名私钥（缺省=faucet）\n"
            "示例: {\"to\":\"0x..dEaD\",\"value\":1000000000000000000}\n"
            "7702示例: {\"type\":4,\"authorizationList\":[{\"delegate\":\"0x..T\",\"signerKey\":\"0x..\"}]}"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    _add_preset(sd)
    sd.add_argument("tx", help="交易规格 JSON 文件")
    sd.add_argument("--no-wait", action="store_true", help="不等回执，发出即返回（用于同块/连发场景）")

    sb = sub.add_parser(
        "send-batch",
        help="按顺序连发一批交易，尽力同块同序（同块有序批量原语，用于 nonce 竞争）",
        epilog=(
            "batch.json = tx 规格数组，每个元素字段同 `gnode send`。\n"
            "先全部签好再背靠背广播(不等回执)，最后报告各笔落块/块内下标。\n"
            "顺序保证：同发送者连续 nonce = 块内序确定；跨发送者 = 靠 maxPriorityFeePerGas\n"
            "小费定序(best-effort，非 100%)。示例:\n"
            '  [{"to":"0x..A","nonce":5},{"to":"0x..B","nonce":6}]'
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    _add_preset(sb)
    sb.add_argument("batch", help="批量交易规格 JSON 文件（tx 规格数组）")

    at = sub.add_parser("attack", help="运行内置攻击场景")
    _add_preset(at, default="prague")
    at.add_argument("scenario", help="场景名（见 gnode scenarios）")
    at.add_argument("--json", action="store_true", help="仅输出 JSON 结果")
    at.add_argument("--verbose", "-v", action="store_true", help="人类可读摘要后附完整 JSON")
    at.add_argument("--param", action="append", default=[], metavar="K=V", help="场景参数，可重复（如 attempts=5）")

    sub.add_parser("scenarios", help="列出内置攻击场景")

    # —— 差分预言机（D1，Rust bin 封装）：纯静态，无需集群 ——
    sub.add_parser(
        "difftx",
        help="跑 Rust 差分预言机 tx_filter⊇revm 矩阵（无需集群；退出码 3=发现停链缺口）",
        epilog="需要兄弟 gravity-reth worktree（默认 /mnt/data2/kenji/galxe/gravity-reth-difftx，"
               "可用环境变量 GRAVITY_RETH_DIR 覆盖）。透传 cargo bin 的 stdout 与退出码。",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub.add_parser(
        "difftx-exec",
        help="跑 Rust serial⟷grevm 执行差分预言机（无需集群；退出码 3=发现执行背离）",
        epilog="同 difftx，但走 --features difftx_exec --bin difftx_exec；GRAVITY_RETH_DIR 同上。",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    return p


def main(argv: list[str]) -> int:
    args = build_parser().parse_args(argv)

    # 非法 --preset 属于用法错误 → 退出码 2（与 argparse 用法错误一致），在派发前统一校验
    if getattr(args, "preset", None) is not None:
        from .env import PRESETS, NODE_ALIASES
        name = args.preset
        if str(name).isdigit():
            name = NODE_ALIASES.get(int(name), name)
        if name not in PRESETS:
            print(f"[gnode] error: 未知档位 '{args.preset}'（支持: {sorted(PRESETS)}，或数字 "
                  f"{sorted(NODE_ALIASES)}）", file=sys.stderr)
            return 2

    # 顶层兜底：任何子命令抛异常都转成一行干净的 stderr 提示 + 退出码 1，
    # 而不是把 Python traceback 甩给用户/agent（好错误信息就在异常 message 里）。
    try:
        return _dispatch(args, argv)
    except (SystemExit, KeyboardInterrupt):
        raise
    except Exception as e:  # noqa: BLE001 —— 面向 agent 的 CLI，任何异常都要给干净输出
        print(f"[gnode] error: {type(e).__name__}: {e}", file=sys.stderr)
        return 1


def _dispatch(args, argv: list[str]) -> int:
    if args.cmd == "up":
        # 未指定 instance 且未设 GNODE_INSTANCE → 自动分配空闲实例
        return ops.cmd_up(args.preset, instance=_inst(args, "auto"), fresh=args.fresh)
    if args.cmd == "down":
        return ops.cmd_down(args.preset, instance=_inst(args))
    if args.cmd == "status":
        # 没显式指定档位时，列出所有 preset/instance 的状态（避免默认档让人误以为集群没起）
        return ops.cmd_status(args.preset, instance=_inst(args),
                              show_all=("--preset" not in argv and "--nodes" not in argv))
    if args.cmd == "logs":
        return ops.cmd_logs(args.preset, args.which, args.follow, args.lines, instance=_inst(args))
    if args.cmd == "state":
        return ops.cmd_state(args.preset, args.addr, instance=_inst(args))
    if args.cmd == "deploy":
        return ops.cmd_deploy(args.preset, args.artifact, args_json=args.args, instance=_inst(args))
    if args.cmd == "send":
        return ops.cmd_send(args.preset, args.tx, no_wait=args.no_wait, instance=_inst(args))
    if args.cmd == "send-batch":
        return ops.cmd_send_batch(args.preset, args.batch, instance=_inst(args))
    if args.cmd == "scenarios":
        for name, desc in registry.list_scenarios():
            print(f"{name:<16} {desc}")
        return 0
    if args.cmd in ("difftx", "difftx-exec"):
        # 差分预言机无需集群：不解析 --preset，直接 shell out 到 Rust bin，透传退出码。
        from . import difftx
        return difftx.cmd_difftx(args.cmd)
    if args.cmd == "attack":
        # 未知场景属于用法错误 → 退出码 2（与坏 --preset 一致），输出保持 scenario 字段
        known = {n for n, _ in registry.list_scenarios()}
        if args.scenario not in known:
            result = {"scenario": args.scenario, "verdict": "error",
                      "detail": f"未知场景 '{args.scenario}'。可用: {', '.join(sorted(known))}"}
            if args.json:
                print(json.dumps(result, indent=2, ensure_ascii=False))
            else:
                _print_attack_result(args.scenario, result, verbose=args.verbose)
            return 2
        params = {}
        for kv in args.param:
            k, _, v = kv.partition("=")
            params[k.strip()] = v.strip()
        result = registry.run(args.scenario, preset=args.preset, instance=_inst(args), params=params)
        if args.json:
            print(json.dumps(result, indent=2, ensure_ascii=False))
        else:
            _print_attack_result(args.scenario, result, verbose=args.verbose)
        # 退出码给 agent/脚本明确信号：3=命中(halt/panic)，0=链存活(alive/revert)，
        #                              2=结论不成立(inconclusive)/用法错误，1=运行时出错(error)
        verdict = result.get("verdict")
        if result.get("usage_error"):  # 参数用法错误归 2（与坏 --preset / 未知场景一致）
            return 2
        return {"halt": 3, "panic": 3, "alive": 0, "revert": 0,
                "inconclusive": 2, "error": 1}.get(verdict, 1)

    return 1


def _print_attack_result(name: str, result: dict, *, verbose: bool = False) -> None:
    print(f"=== attack: {name} ===")
    verdict = result.get("verdict")
    print(f"verdict : {verdict}")
    if result.get("expected"):
        # halt 与 panic 同属「链停摆」家族：expected=halt 时，verdict=panic（进程崩）也算命中
        stopped = {"halt", "panic"}
        exp = result["expected"]
        hit = verdict == exp or (exp in stopped and verdict in stopped)
        print(f"expected: {exp}  ->  {'✅ 命中' if hit else '❌ 未命中（见 detail）'}")
    if result.get("detail"):
        print(f"detail  : {result['detail']}")
    for step in result.get("steps", []):
        print(f"  - {step}")
    if verbose:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print("（完整结构化结果加 --json 或 -v 查看）")
