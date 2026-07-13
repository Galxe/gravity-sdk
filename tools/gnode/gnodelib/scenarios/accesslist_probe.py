"""accesslist-probe —— EIP-2930（type 1）access-list 交易是否被正常处理。

探测目的：确认一笔带 accessList 的 type-1 交易能正常落块（receipt.status=1）、
链继续出块（verdict=alive），以此作为「typed-tx 通路健康」的正对照——与 7702(type 4)
的 NonceTooLow panic 形成对比，帮助审计区分「typed-tx 本身能跑」和「特定类型触发的执行层缺陷」。

做法（全部走 gnode 现有 primitive 能力）：
  1) 部署一个 SSTORE 合约 S（写一个 slot 后 STOP）。
  2) faucet 发一笔 type=1、带 accessList（预热 S 的 slot 0）的交易调用 S。
  3) probe_liveness 观察链是否仍在出块，并读回执判定 alive vs halt/panic。
"""
from __future__ import annotations

from web3 import Web3

from ..env import faucet_account, make_web3, resolve_cluster, suggest_fees
from ..ops import _pid_alive
from ..verdict import Verdict, probe_liveness
from ._common import preflight_error, send_raw as _send_raw

# 运行时: PUSH1 1 PUSH1 0 SSTORE STOP  (写 slot0=1 后停)
# init:  CODECOPY 出 6 字节 runtime 并 RETURN
SSTORE_INITCODE = "0x6006600c60003960066000f3600160005500"

PARAMS = {"window": float}  # 观察窗口秒数


def run(*, preset, params: dict) -> dict:
    cp = resolve_cluster(preset)
    w3 = make_web3(cp.rpc_url())
    node_id = cp.node_ids()[0]
    pid_alive = lambda: _pid_alive(cp.pid_file(node_id))

    result: dict = {"scenario": "accesslist-probe", "expected": "alive", "rpc": cp.rpc_url()}
    err = preflight_error(cp, w3)
    if err:
        result.update(err)
        return result

    chain_id = w3.eth.chain_id
    faucet = faucet_account()
    fees = suggest_fees(w3)
    steps: list[str] = []

    # 1) 部署 SSTORE 合约 S
    dep_tx = {
        "from": faucet.address, "data": SSTORE_INITCODE, "value": 0,
        "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 200000,
        "chainId": chain_id, **fees,
    }
    dep_rcpt = w3.eth.wait_for_transaction_receipt(_send_raw(w3, faucet, dep_tx), timeout=60)
    S = dep_rcpt["contractAddress"]
    if not S or int(dep_rcpt["status"]) != 1:
        result["verdict"] = "error"
        result["detail"] = f"部署 SSTORE 合约失败: {dict(dep_rcpt)}"
        return result
    result["sstore_contract"] = S
    steps.append(f"部署 SSTORE 合约 S={S}")

    # 2) 发一笔 type=1（EIP-2930）交易，accessList 预热 S 的 slot0
    call_tx = {
        "type": 1,
        "from": faucet.address, "to": S, "value": 0, "data": "0x",
        "nonce": w3.eth.get_transaction_count(faucet.address), "gas": 100000,
        "gasPrice": fees.get("maxFeePerGas") or w3.eth.gas_price,
        "chainId": chain_id,
        "accessList": [
            {"address": S, "storageKeys": ["0x" + "00" * 32]},
        ],
    }
    call_hash = _send_raw(w3, faucet, call_tx)
    steps.append(f"发送 type-1 access-list 调用 tx={call_hash}")

    # 3) 判定：链是否仍在出块
    window = float(params.get("window", 8))
    probe = probe_liveness(w3, window_s=window, min_delta=1, pid_alive=pid_alive)
    result["liveness"] = probe.as_dict()

    verdict = probe.verdict
    detail = probe.detail
    if verdict == Verdict.ALIVE:
        try:
            r = w3.eth.get_transaction_receipt(call_hash)
            status = int(r["status"])
            result["call_receipt"] = {
                "status": status, "block": int(r["blockNumber"]),
                "type": int(r.get("type", 0)),
            }
            detail = (
                "链存活；type-1 access-list 交易正常落块（status=1）"
                if status == 1 else
                f"链存活，但 type-1 交易 status={status}（非预期）"
            )
        except Exception as e:
            detail = f"链存活但读回执失败: {e}"

    result["verdict"] = verdict.value
    result["detail"] = detail
    result["steps"] = steps
    return result
