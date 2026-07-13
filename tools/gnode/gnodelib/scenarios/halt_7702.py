"""7702-halt —— EIP-7702 委托码 CREATE 抬升 nonce，同块 nonce 竞争打停链。

漏洞机理（对应 gravity-audit 7702 nonce 竞争）：
  - EOA A 委托到合约 T，T 的运行时代码里含 CREATE。
  - 同一个区块内放两笔来自 A 的连续 nonce 交易：
      tx0 (from=A, to=A, nonce=N)  —— 执行时先把 A.nonce N→N+1（交易自增），
                                      再因委托码 CREATE 把 A.nonce N+1→N+2。
      tx1 (from=A, nonce=N+1)      —— 入池时 A.nonce=N，(N, N+1) 是合法连续序列被放行；
                                      但轮到执行时 A.nonce 已是 N+2 > N+1 → revm NonceTooLow。
  - 被准入门放行、却在执行层 NonceTooLow 的已入块交易，会让 pipe 执行不可恢复 → 出块停摆(halt)。

为什么用「同一发送者 A 的连续 nonce」而不是跨发送者：同发送者的多笔交易在区块内
恒按 nonce 排序且会被打进同一个块，从而确定性地保证 tx0 先于 tx1、且同块——
比「tx0 to=A 由他人发」的跨发送者排序更可靠地复现。

判定：注入后用 probe_liveness 观察出块是否停摆（halt/panic）还是继续（alive/revert）。
"""
from __future__ import annotations

from eth_account import Account
from web3 import Web3

from gravity_e2e.utils.eip7702 import build_signed_set_code_tx, sign_authorization

from ..env import faucet_account, make_web3, resolve_cluster, suggest_fees
from ..ops import _pid_alive
from ..verdict import Verdict, probe_liveness
from ._common import preflight_error

PARAMS = {"attempts": int, "window": float}  # attempts=注入重试次数, window=每次观察窗口秒数

# T 的运行时代码无条件执行 CREATE(0,0,0) 后 STOP —— 委托执行时抬升 A.nonce。
#   runtime: PUSH1 0 (size) PUSH1 0 (offset) PUSH1 0 (value) CREATE POP STOP
#   init:    CODECOPY 出上面 9 字节 runtime 并 RETURN
CREATE_CONTRACT_INITCODE = "0x6009600c60003960096000f3600060006000f05000"

DESIGNATOR_PREFIX = bytes.fromhex("ef0100")  # EIP-7702 delegation designator


def _send_raw(w3: Web3, acct, tx: dict) -> str:
    signed = acct.sign_transaction(tx)
    return w3.eth.send_raw_transaction(signed.raw_transaction).to_0x_hex()


def _receipt(w3: Web3, tx_hash: str):
    try:
        r = w3.eth.get_transaction_receipt(tx_hash)
        return {
            "status": int(r["status"]),
            "block": int(r["blockNumber"]),
            "tx_index": int(r["transactionIndex"]),
            "gas_used": int(r["gasUsed"]),
        }
    except Exception:
        return None


def _wait_nonce(w3: Web3, addr: str, target: int, timeout: int = 30) -> None:
    import time
    deadline = time.time() + timeout
    while time.time() < deadline:
        if w3.eth.get_transaction_count(addr) >= target:
            return
        time.sleep(0.5)


def _deploy_target(w3: Web3, faucet, chain_id: int, fees: dict) -> tuple[str, dict]:
    """部署含 CREATE 的委托目标 T（整个场景只需一次）。返回 (T_addr | "", err)。"""
    dep_tx = {
        "from": faucet.address, "data": CREATE_CONTRACT_INITCODE, "value": 0,
        "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 200000,
        "chainId": chain_id, **fees,
    }
    dep_hash = _send_raw(w3, faucet, dep_tx)
    r = w3.eth.wait_for_transaction_receipt(dep_hash, timeout=60)
    T = r["contractAddress"]
    if not T or int(r["status"]) != 1:
        return "", {"detail": f"部署 CREATE 目标合约失败: {dict(r)}"}
    return T, {}


def _setup_delegation(w3: Web3, faucet, A, T: str, chain_id: int) -> dict:
    """给 A 装 7702 委托（faucet 代付；to=无码地址，避免此步就触发 CREATE）。"""
    a_nonce0 = w3.eth.get_transaction_count(A.address)
    auth = sign_authorization(A, chain_id=chain_id, delegate=T, nonce=a_nonce0)
    inner_target = Account.create().address
    fee = max(w3.eth.gas_price * 2, w3.to_wei(50, "gwei"))
    raw = build_signed_set_code_tx(
        faucet, chain_id=chain_id, nonce=w3.eth.get_transaction_count(faucet.address),
        to=inner_target, authorization_list=[auth], gas=200000,
        max_fee_per_gas=fee, max_priority_fee_per_gas=fee,
    )
    h = w3.eth.send_raw_transaction(raw).to_0x_hex()
    r = w3.eth.wait_for_transaction_receipt(h, timeout=60)
    code = bytes(w3.eth.get_code(A.address))
    ok = int(r["status"]) == 1 and code.startswith(DESIGNATOR_PREFIX)
    return {"ok": ok, "code": code.hex(), "nonce": w3.eth.get_transaction_count(A.address)}


def _inject_race(w3: Web3, faucet, A, chain_id: int) -> dict:
    """跨发送者同块注入 tx1(from=A, stale) + tx0(faucet→A 触发 CREATE, 高小费排前)。"""
    N = w3.eth.get_transaction_count(A.address)
    head_before = w3.eth.block_number
    base = w3.eth.get_block("latest").get("baseFeePerGas") or w3.to_wei(50, "gwei")
    lo, hi = w3.to_wei(2, "gwei"), w3.to_wei(200, "gwei")
    sink = Account.create().address
    tx1 = {
        "from": A.address, "to": sink, "value": 0, "nonce": N, "gas": 40000,
        "chainId": chain_id, "maxPriorityFeePerGas": lo, "maxFeePerGas": base * 2 + lo,
    }
    tx0 = {
        "from": faucet.address, "to": A.address, "value": 0, "data": "0x",
        "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 500000,
        "chainId": chain_id, "maxPriorityFeePerGas": hi, "maxFeePerGas": base * 2 + hi,
    }
    tx1_hash = _send_raw(w3, A, tx1)        # 先把 A 的在途交易灌进池
    tx0_hash = _send_raw(w3, faucet, tx0)   # 再发高小费触发交易，争取同块且排在 tx1 前
    return {"N": N, "head_before": head_before, "tx0_faucet_to_A": tx0_hash, "tx1_from_A_stale": tx1_hash}


def run(*, preset, params: dict) -> dict:
    cp = resolve_cluster(preset)
    w3 = make_web3(cp.rpc_url())
    faucet = faucet_account()
    node_id = cp.node_ids()[0]
    pid_alive = lambda: _pid_alive(cp.pid_file(node_id))
    result: dict = {"scenario": "7702-halt", "expected": "halt", "rpc": cp.rpc_url()}

    # —— 健康检查必须放在任何 RPC 调用之前（打挂后再跑会命中这里，而不是抛 traceback）——
    err = preflight_error(cp, w3)
    if err:
        result.update(err)
        return result

    chain_id = w3.eth.chain_id
    result["chain_id"] = chain_id
    fees = suggest_fees(w3)
    steps: list[str] = []
    attempts = int(params.get("attempts", 3))
    if attempts < 1:
        result["verdict"] = Verdict.ERROR.value
        result["detail"] = f"attempts 需为 ≥1 的整数，得到 {attempts}"
        return result

    # 委托目标 T 只需部署一次
    T, err = _deploy_target(w3, faucet, chain_id, fees)
    if not T:
        result["verdict"] = Verdict.ERROR.value
        result["detail"] = err["detail"]
        return result
    result["delegate_target"] = T
    steps.append(f"部署委托目标 T={T}（运行时含 CREATE）")

    # 竞争是否成形依赖块内共存 + 排序（跨发送者按小费排序，非 100% 稳），故重试若干次。
    window = float(params.get("window", 20))
    tries = []
    for i in range(1, attempts + 1):
        A = Account.create()
        # 充钱
        fund = {
            "from": faucet.address, "to": A.address, "value": w3.to_wei(10, "ether"),
            "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 21000,
            "chainId": chain_id, **fees,
        }
        _send_raw(w3, faucet, fund)
        _wait_nonce(w3, faucet.address, fund["nonce"] + 1)

        setup = _setup_delegation(w3, faucet, A, T, chain_id)
        if not setup["ok"]:
            result["verdict"] = Verdict.ERROR.value
            result["detail"] = f"安装 7702 委托失败: code={setup['code']}"
            result["steps"] = steps
            return result

        race = _inject_race(w3, faucet, A, chain_id)
        probe = probe_liveness(w3, window_s=window, min_delta=2, pid_alive=pid_alive)
        attempt_rec = {"attempt": i, "attacker_eoa": A.address, "nonce_race": race,
                       "liveness": probe.as_dict()}

        # 命中：链停 / 进程崩 —— 直接结案
        if probe.verdict in (Verdict.HALT, Verdict.PANIC):
            r0, r1 = _receipt(w3, race["tx0_faucet_to_A"]), _receipt(w3, race["tx1_from_A_stale"])
            attempt_rec["receipts"] = {"tx0": r0, "tx1": r1}
            tries.append(attempt_rec)
            steps.append(f"[try {i}] 命中：{probe.detail}")
            result.update({"verdict": probe.verdict.value, "detail": probe.detail,
                           "attacker_eoa": A.address, "nonce_race": race,
                           "liveness": probe.as_dict(), "attempts_made": i, "tries": tries, "steps": steps})
            return result

        # 链还活着：检查竞争是否真的成形（tx0、tx1 同块且 tx0 在前）
        r0, r1 = _receipt(w3, race["tx0_faucet_to_A"]), _receipt(w3, race["tx1_from_A_stale"])
        attempt_rec["receipts"] = {"tx0": r0, "tx1": r1}
        tries.append(attempt_rec)
        race_formed = bool(r0 and r1 and r0["block"] == r1["block"] and r0["tx_index"] < r1["tx_index"])
        if race_formed:
            # 真的同块按序注入了，链却没停 —— 说明该路径已被缓解/修复
            if r1 and r1["status"] == 0:
                verdict, detail = Verdict.REVERT, "竞争已成形（tx0/tx1 同块且 tx0 在前），tx1 被优雅回滚，链不停 —— 修复生效"
            else:
                verdict, detail = Verdict.ALIVE, "竞争已成形但链继续出块 —— 该路径已被缓解/修复"
            steps.append(f"[try {i}] 竞争成形但未 halt：{detail}")
            result.update({"verdict": verdict.value, "detail": detail, "attacker_eoa": A.address,
                           "nonce_race": race, "liveness": probe.as_dict(),
                           "attempts_made": i, "tries": tries, "steps": steps})
            return result

        # 竞争没成形（未同块 / 顺序反了）—— 重试
        why = "两笔未落在同一区块" if (r0 and r1 and r0["block"] != r1["block"]) else "块内顺序不满足 tx0<tx1 或有交易缺失"
        steps.append(f"[try {i}] 竞争未成形（{why}），重试")

    # 多次都没能让两笔同块按序 —— 不下 halt/alive 结论，报 inconclusive 让调用方重试
    result.update({
        "verdict": Verdict.INCONCLUSIVE.value,
        "detail": f"{attempts} 次注入均未能让 tx0/tx1 同块且按序（竞争未成形），无法判定；"
                  f"可加大 --param attempts=N 重试，或改用 gravity-reth 的 pipe 测试 harness 做确定性喂块。",
        "attempts_made": attempts, "tries": tries, "steps": steps,
    })
    return result
