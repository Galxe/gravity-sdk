"""revert-probe —— 普通 EVM revert 是否被优雅处理（对照 7702 的 NonceTooLow panic）。

探测目的：确认一笔「注定 revert 的调用」只会让该交易回滚（receipt.status=0），
链继续出块（verdict=revert / alive），而不会像执行层 NonceTooLow 那样把节点打 panic。
用来把「交易级失败」与「共识/执行层不可恢复错误」区分开——审计时判断某个异常
到底是良性 revert 还是真正的链级 DoS。

做法（全部走 gnode 现有 primitive 能力）：
  1) 部署一个运行时永远 REVERT 的合约 R（PUSH1 0 PUSH1 0 REVERT）。
  2) faucet 发一笔 to=R 的交易，预期执行时 revert。
  3) probe_liveness 观察链是否仍在出块，并读回执判定 revert vs alive vs halt/panic。
"""
from __future__ import annotations

from web3 import Web3

from ..env import faucet_account, make_web3, resolve_cluster, suggest_fees
from ..ops import _pid_alive
from ..verdict import Verdict, probe_liveness
from ._common import preflight_error, send_raw as _send_raw

# 运行时: PUSH1 0 PUSH1 0 REVERT  (总是 revert)
# init:  CODECOPY 出 5 字节 runtime 并 RETURN
REVERT_INITCODE = "0x6005600c60003960056000f360006000fd"

PARAMS = {"window": float}  # 观察窗口秒数


def run(*, preset, instance: int = 0, params: dict) -> dict:
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    node_id = cp.node_ids()[0]
    pid_alive = lambda: _pid_alive(cp.pid_file(node_id))

    result: dict = {"scenario": "revert-probe", "expected": "revert", "rpc": cp.rpc_url()}
    err = preflight_error(cp, w3)
    if err:
        result.update(err)
        return result

    chain_id = w3.eth.chain_id
    faucet = faucet_account()
    fees = suggest_fees(w3)
    steps: list[str] = []

    # 1) 部署永远 revert 的合约 R
    dep_tx = {
        "from": faucet.address, "data": REVERT_INITCODE, "value": 0,
        "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 200000,
        "chainId": chain_id, **fees,
    }
    dep_rcpt = w3.eth.wait_for_transaction_receipt(_send_raw(w3, faucet, dep_tx), timeout=60)
    R = dep_rcpt["contractAddress"]
    if not R or int(dep_rcpt["status"]) != 1:
        result["verdict"] = "error"
        result["detail"] = f"部署 revert 合约失败: {dict(dep_rcpt)}"
        return result
    result["revert_contract"] = R
    steps.append(f"部署永远 revert 的合约 R={R}")

    # 2) 调用 R，预期该交易 revert（gas 写死避免 estimate_gas 因预执行 revert 而抛错）
    call_tx = {
        "from": faucet.address, "to": R, "value": 0, "data": "0x",
        "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 100000,
        "chainId": chain_id, **fees,
    }
    call_hash = _send_raw(w3, faucet, call_tx)
    steps.append(f"发送注定 revert 的调用 tx={call_hash}")

    # 3) 判定：链是否仍在出块
    window = float(params.get("window", 10))
    probe = probe_liveness(w3, window_s=window, min_delta=1, pid_alive=pid_alive)
    result["liveness"] = probe.as_dict()

    verdict = probe.verdict
    detail = probe.detail
    if verdict == Verdict.ALIVE:
        try:
            r = w3.eth.get_transaction_receipt(call_hash)
            status = int(r["status"])
            result["call_receipt"] = {"status": status, "block": int(r["blockNumber"])}
            if status == 0:
                verdict = Verdict.REVERT
                detail = "链存活；调用交易按预期 revert（status=0），普通 revert 被优雅处理"
            else:
                detail = "链存活，但调用交易竟然成功（status=1）——合约未按预期 revert"
        except Exception as e:
            detail = f"链存活但读回执失败: {e}"

    result["verdict"] = verdict.value
    result["detail"] = detail
    result["steps"] = steps
    return result
