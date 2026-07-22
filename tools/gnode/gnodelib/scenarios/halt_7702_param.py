"""7702-race —— 7702 nonce 竞争停链机理的**参数化模板**（可搜索变体）。

在稳定默认场景 `7702-halt`（跨发送者、单委托、正确 chainId）之外，把机理拆成一组
可调旋钮，供 Coacker 红队 agent SEARCH 变体、而非只跑一个固定用例。复用
`halt_7702` 里的 `_deploy_target` / `_inject_race` / `probe_liveness` / `Verdict`，
故退出码契约（3=halt/panic，0=alive/revert，2=inconclusive/usage，1=error）不变。

旋钮（--param K=V，均经 PARAMS 类型强转；坏值→干净用法错误，非 traceback）：
  cross_sender_action  cross_sender(默认，tx0 由 faucet→A 触发委托 CREATE)
                       | self_sponsored(authority==sender，A 自发同发送者连续 nonce)
  nonce_offset  int(默认 0)  在途 stale 交易 tx1 的 nonce 相对基准 N 的偏移
  auth_count    int(默认 1)  单笔 SetCode 里装入的授权条数（>1 追加随机 EOA→T 压测授权表）
  chain_id      correct(默认) | zero(=0，7702 语义为任意链有效) | mismatched(=nodeid+1，应被拒)
  delegate_chain single(默认，A→T) | aba(A→B→A 委托环，当前原语暂不可构造→结构化 inconclusive)
  attempts      int(默认 3)  竞争成形的重试次数
  window        float(默认 20) 每次注入后的存活观察窗口秒数

未成形不下 halt 结论（inconclusive）；旋钮组合暂不可构造时返回结构化说明，绝不伪造 halt。
"""
from __future__ import annotations

from eth_account import Account
from web3 import Web3

from gravity_e2e.utils.eip7702 import build_signed_set_code_tx, sign_authorization

from ..env import faucet_account, make_web3, resolve_cluster, suggest_fees
from ..ops import _pid_alive
from ..verdict import Verdict, probe_liveness
from ._common import preflight_error
from .halt_7702 import (
    DESIGNATOR_PREFIX,
    _deploy_target,
    _inject_race,
    _receipt,
    _send_raw,
    _wait_nonce,
)

# 枚举型旋钮用 str 收，值域在 run() 内校验（坏值→usage_error）。
PARAMS = {
    "cross_sender_action": str,
    "nonce_offset": int,
    "auth_count": int,
    "chain_id": str,
    "delegate_chain": str,
    "attempts": int,
    "window": float,
}

_ACTIONS = ("cross_sender", "self_sponsored")
_CHAIN_ID_MODES = ("correct", "zero", "mismatched")
_DELEGATE_CHAINS = ("single", "aba")


def _resolve_auth_chain_id(mode: str, node_chain_id: int) -> int:
    """把 chain_id 旋钮解析成授权(authorization)里用的 chainId。"""
    if mode == "correct":
        return node_chain_id
    if mode == "zero":
        return 0  # EIP-7702：chainId=0 表示任意链有效
    return node_chain_id + 1  # mismatched：应被节点判为无效授权


def _setup_delegation_param(
    w3: Web3, faucet, A, T: str, *, node_chain_id: int, auth_chain_id: int, auth_count: int
) -> dict:
    """参数化装委托：A→T 为首条授权，另追加 auth_count-1 条随机 EOA→T 压测授权表。

    授权(authorization)的 chainId 用 auth_chain_id（旋钮控制）；外层 SetCode 交易本身
    仍用节点真实 chain_id（node_chain_id），以隔离「授权链号」这一变量。
    """
    a_nonce0 = w3.eth.get_transaction_count(A.address)
    auths = [sign_authorization(A, chain_id=auth_chain_id, delegate=T, nonce=a_nonce0)]
    for _ in range(max(0, auth_count - 1)):
        extra = Account.create()
        auths.append(sign_authorization(extra, chain_id=auth_chain_id, delegate=T, nonce=0))
    inner_target = Account.create().address  # to=无码地址，避免此步就触发 CREATE
    fee = max(w3.eth.gas_price * 2, w3.to_wei(50, "gwei"))
    raw = build_signed_set_code_tx(
        faucet, chain_id=node_chain_id, nonce=w3.eth.get_transaction_count(faucet.address),
        to=inner_target, authorization_list=auths, gas=200000 + auth_count * 30000,
        max_fee_per_gas=fee, max_priority_fee_per_gas=fee,
    )
    h = w3.eth.send_raw_transaction(raw).to_0x_hex()
    r = w3.eth.wait_for_transaction_receipt(h, timeout=60)
    code = bytes(w3.eth.get_code(A.address))
    ok = int(r["status"]) == 1 and code.startswith(DESIGNATOR_PREFIX)
    return {"ok": ok, "code": code.hex(), "auth_count": auth_count,
            "nonce": w3.eth.get_transaction_count(A.address)}


def _inject_self_sponsored(w3: Web3, faucet, A, chain_id: int, *, nonce_offset: int = 0) -> dict:
    """自发起(authority==sender)同块注入：tx0(A→A 触发委托 CREATE) 先，tx1(A stale) 后。

    同一发送者的多笔交易在块内恒按 nonce 排序且同块，故 tx0 先于 tx1 是确定性的
    （比跨发送者按小费排序更可靠地复现竞争）。
    """
    N = w3.eth.get_transaction_count(A.address)
    head_before = w3.eth.block_number
    base = w3.eth.get_block("latest").get("baseFeePerGas") or w3.to_wei(50, "gwei")
    prio = w3.to_wei(2, "gwei")
    sink = Account.create().address
    # tx0：调用自身（已装委托码 → 执行 CREATE）；nonce=N，块内先执行 → 抬升 A.nonce 到 N+2。
    tx0 = {
        "from": A.address, "to": A.address, "value": 0, "data": "0x", "nonce": N,
        "gas": 500000, "chainId": chain_id,
        "maxPriorityFeePerGas": prio, "maxFeePerGas": base * 2 + prio,
    }
    # tx1：在途 stale 交易，nonce=N+1+offset（默认紧邻 tx0）；执行时 A.nonce 已 >N+1 → NonceTooLow。
    tx1 = {
        "from": A.address, "to": sink, "value": 0, "nonce": N + 1 + nonce_offset,
        "gas": 40000, "chainId": chain_id,
        "maxPriorityFeePerGas": prio, "maxFeePerGas": base * 2 + prio,
    }
    tx0_hash = _send_raw(w3, A, tx0)  # 同发送者：先发低 nonce
    tx1_hash = _send_raw(w3, A, tx1)  # 再发高 nonce，块内自然排在 tx0 之后
    return {"N": N, "head_before": head_before, "nonce_offset": nonce_offset,
            "tx0_self_create": tx0_hash, "tx1_from_A_stale": tx1_hash}


def _evaluate(w3: Web3, race: dict, tx0_key: str, tx1_key: str, probe) -> tuple:
    """把一次注入的回执 + liveness 判定成 (决定性?, verdict, detail, race_formed, receipts)。"""
    r0, r1 = _receipt(w3, race[tx0_key]), _receipt(w3, race[tx1_key])
    receipts = {"tx0": r0, "tx1": r1}
    if probe.verdict in (Verdict.HALT, Verdict.PANIC):
        return True, probe.verdict, probe.detail, None, receipts
    race_formed = bool(r0 and r1 and r0["block"] == r1["block"] and r0["tx_index"] < r1["tx_index"])
    if race_formed:
        if r1 and r1["status"] == 0:
            return True, Verdict.REVERT, "竞争已成形（tx0/tx1 同块且 tx0 在前），tx1 被优雅回滚，链不停 —— 修复生效", True, receipts
        return True, Verdict.ALIVE, "竞争已成形但链继续出块 —— 该路径已被缓解/修复", True, receipts
    return False, None, None, False, receipts


def run(*, preset, instance: int = 0, params: dict) -> dict:
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    faucet = faucet_account()
    node_id = cp.node_ids()[0]
    pid_alive = lambda: _pid_alive(cp.pid_file(node_id))
    result: dict = {"scenario": "7702-race", "expected": "halt", "rpc": cp.rpc_url()}

    # —— 旋钮值域校验（坏值→usage_error，退出码 2；与坏 --preset 一致）——
    action = params.get("cross_sender_action", "cross_sender")
    if action not in _ACTIONS:
        result.update({"verdict": Verdict.ERROR.value, "usage_error": True,
                       "detail": f"cross_sender_action={action!r} 非法，取值 {list(_ACTIONS)}"})
        return result
    chain_id_mode = params.get("chain_id", "correct")
    if chain_id_mode not in _CHAIN_ID_MODES:
        result.update({"verdict": Verdict.ERROR.value, "usage_error": True,
                       "detail": f"chain_id={chain_id_mode!r} 非法，取值 {list(_CHAIN_ID_MODES)}"})
        return result
    delegate_chain = params.get("delegate_chain", "single")
    if delegate_chain not in _DELEGATE_CHAINS:
        result.update({"verdict": Verdict.ERROR.value, "usage_error": True,
                       "detail": f"delegate_chain={delegate_chain!r} 非法，取值 {list(_DELEGATE_CHAINS)}"})
        return result
    nonce_offset = int(params.get("nonce_offset", 0))
    # nonce_offset 只对 self_sponsored 注入器生效（_inject_race 不吃该参数）。cross_sender 下
    # 若设了非零 nonce_offset,过去会被静默忽略、却仍记进 params_used——误导调用方以为生效了。
    # 这里显式拒绝该组合(usage_error),而不是假装接受。
    if action == "cross_sender" and nonce_offset != 0:
        result.update({"verdict": Verdict.ERROR.value, "usage_error": True,
                       "detail": "nonce_offset 仅对 cross_sender_action=self_sponsored 生效；"
                                 "cross_sender 模式下不支持,请改用 self_sponsored 或去掉 nonce_offset。"})
        return result
    auth_count = int(params.get("auth_count", 1))
    if auth_count < 1:
        result.update({"verdict": Verdict.ERROR.value, "usage_error": True,
                       "detail": f"auth_count 需 ≥1，得到 {auth_count}"})
        return result
    attempts = int(params.get("attempts", 3))
    if attempts < 1:
        result.update({"verdict": Verdict.ERROR.value, "usage_error": True,
                       "detail": f"attempts 需 ≥1，得到 {attempts}"})
        return result
    window = float(params.get("window", 20))
    result["params_used"] = {"cross_sender_action": action, "nonce_offset": nonce_offset,
                             "auth_count": auth_count, "chain_id": chain_id_mode,
                             "delegate_chain": delegate_chain, "attempts": attempts, "window": window}

    # —— 尚不可构造的旋钮组合：结构化 inconclusive，绝不伪造 halt ——
    if delegate_chain == "aba":
        result.update({
            "verdict": Verdict.INCONCLUSIVE.value,
            "detail": "delegate_chain=aba（A→B→A 委托环）当前原语暂不可构造：需部署两个互指的委托目标"
                      "并给两个 EOA 分别装成环形 7702 designator，_deploy_target/_setup_delegation 暂只做"
                      "单跳 A→T。请扩展委托装配原语后再搜此变体（本次不下 halt 结论）。",
        })
        return result

    # —— 健康检查必须先于任何 RPC 调用 ——
    err = preflight_error(cp, w3)
    if err:
        result.update(err)
        return result

    node_chain_id = w3.eth.chain_id
    result["chain_id_node"] = node_chain_id
    auth_chain_id = _resolve_auth_chain_id(chain_id_mode, node_chain_id)
    result["auth_chain_id"] = auth_chain_id
    fees = suggest_fees(w3)
    steps: list[str] = []

    # 委托目标 T（含 CREATE）只需部署一次
    T, derr = _deploy_target(w3, faucet, node_chain_id, fees)
    if not T:
        result.update({"verdict": Verdict.ERROR.value, "detail": derr["detail"]})
        return result
    result["delegate_target"] = T
    steps.append(f"部署委托目标 T={T}（运行时含 CREATE）")

    tries: list = []
    for i in range(1, attempts + 1):
        A = Account.create()
        fund = {
            "from": faucet.address, "to": A.address, "value": w3.to_wei(10, "ether"),
            "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 21000,
            "chainId": node_chain_id, **fees,
        }
        _send_raw(w3, faucet, fund)
        _wait_nonce(w3, faucet.address, fund["nonce"] + 1)

        setup = _setup_delegation_param(
            w3, faucet, A, T, node_chain_id=node_chain_id,
            auth_chain_id=auth_chain_id, auth_count=auth_count,
        )
        if not setup["ok"]:
            # chainId=mismatched 的预期结果就是授权被拒 → 委托没装上（负对照，非 halt）。
            if chain_id_mode == "mismatched":
                result.update({
                    "verdict": Verdict.INCONCLUSIVE.value,
                    "detail": f"chain_id=mismatched（auth_chain_id={auth_chain_id}≠node {node_chain_id}）"
                              f"→ 授权被判无效，A 未装委托码(code={setup['code']})，委托 CREATE 不触发、"
                              f"竞争无法成形。这是预期的负对照，非链停。",
                    "setup": setup, "steps": steps,
                })
                return result
            result.update({"verdict": Verdict.ERROR.value, "steps": steps,
                           "detail": f"安装 7702 委托失败: code={setup['code']}"})
            return result

        if action == "self_sponsored":
            race = _inject_self_sponsored(w3, faucet, A, node_chain_id, nonce_offset=nonce_offset)
            tx0_key, tx1_key = "tx0_self_create", "tx1_from_A_stale"
        else:
            race = _inject_race(w3, faucet, A, node_chain_id)
            tx0_key, tx1_key = "tx0_faucet_to_A", "tx1_from_A_stale"

        probe = probe_liveness(w3, window_s=window, min_delta=2, pid_alive=pid_alive)
        decisive, verdict, detail, race_formed, receipts = _evaluate(w3, race, tx0_key, tx1_key, probe)
        attempt_rec = {"attempt": i, "attacker_eoa": A.address, "nonce_race": race,
                       "liveness": probe.as_dict(), "receipts": receipts}
        tries.append(attempt_rec)

        if decisive:
            steps.append(f"[try {i}] 决定性判定：{detail}")
            result.update({"verdict": verdict.value, "detail": detail, "attacker_eoa": A.address,
                           "nonce_race": race, "liveness": probe.as_dict(),
                           "attempts_made": i, "tries": tries, "steps": steps})
            return result

        r0, r1 = receipts["tx0"], receipts["tx1"]
        why = "两笔未落在同一区块" if (r0 and r1 and r0["block"] != r1["block"]) else "块内顺序不满足 tx0<tx1 或有交易缺失"
        steps.append(f"[try {i}] 竞争未成形（{why}），重试")

    result.update({
        "verdict": Verdict.INCONCLUSIVE.value,
        "detail": f"{attempts} 次注入均未能让 tx0/tx1 同块且按序（竞争未成形），无法判定；"
                  f"可加大 --param attempts=N，或改用 gravity-reth 的 pipe 测试 harness 做确定性喂块。",
        "attempts_made": attempts, "tries": tries, "steps": steps,
    })
    return result
