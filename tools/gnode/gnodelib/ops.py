"""集群生命周期 + 链上交互操作。

up/down/status/logs 包装 cluster/ 下的 shell 脚本（不重造编排）；
state/deploy/send 用 web3 直连本地 RPC。
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

from eth_account import Account
from web3 import Web3

from .env import (
    CLUSTER_DIR,
    NODE_BIN,
    CLI_BIN,
    GSDK_ROOT,
    ClusterPaths,
    faucet_account,
    make_web3,
    resolve_cluster,
    suggest_fees,
)
from .verdict import probe_liveness


def log(msg: str) -> None:
    print(f"[gnode] {msg}", file=sys.stderr, flush=True)


# ----------------------------------------------------------------------------
# 底层：跑 cluster 脚本 / 进程存活 / RPC 等待
# ----------------------------------------------------------------------------

def _run_script(script: str, args: list[str], *, env: Optional[dict] = None, check: bool = True) -> int:
    """在 CLUSTER_DIR 下跑一个 bash 脚本，实时透传输出。"""
    cmd = ["bash", script, *args]
    log(f"运行: {script} {' '.join(str(a) for a in args)} (cwd={CLUSTER_DIR})")
    proc = subprocess.run(cmd, cwd=str(CLUSTER_DIR), env=env or os.environ.copy(), stdin=subprocess.DEVNULL)
    if check and proc.returncode != 0:
        raise RuntimeError(f"脚本失败 ({proc.returncode}): {script} {args}")
    return proc.returncode


def _script_env(cp: ClusterPaths) -> dict:
    """cluster 脚本需要的环境：把 genesis 产物重定向到 preset 自己的 artifacts/。"""
    env = os.environ.copy()
    env["GRAVITY_ARTIFACTS_DIR"] = str(cp.artifacts_dir)
    env["GENESIS_CONFIG_FILE"] = str(cp.genesis_toml)
    return env


def _pid_alive(pid_file: Path) -> bool:
    try:
        pid = int(pid_file.read_text().strip())
    except Exception:
        return False
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def _rpc_up(w3: Web3) -> bool:
    try:
        _ = w3.eth.block_number
        return True
    except Exception:
        return False


def _require_rpc(cp: ClusterPaths, w3: Web3) -> Optional[int]:
    """RPC 不可达时打印一致的友好提示并返回退出码 2；可达返回 None。
    与 state/attack 的下机处理保持一致（deploy/send 也走这条，不再甩 urllib3 内部错误）。"""
    if _rpc_up(w3):
        return None
    proc_up = _pid_alive(cp.pid_file(cp.node_ids()[0]))
    print(json.dumps({
        "reachable": False,
        "node_process": "up" if proc_up else "down",
        "detail": (f"进程在但 RPC 无响应（疑似 halt），先 `gnode up --preset {cp.name} --fresh`"
                   if proc_up else f"节点未运行，先 `gnode up --preset {cp.name}`"),
    }, indent=2, ensure_ascii=False))
    return 2


def _wait_rpc(w3: Web3, timeout: int = 60) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if _rpc_up(w3):
            return True
        time.sleep(1.0)
    return False


def _ensure_binaries() -> None:
    if NODE_BIN.exists() and CLI_BIN.exists():
        return
    log("gravity_node / gravity_cli 未构建，开始 cargo build（首次很慢，会拉 reth/aptos 依赖）...")
    env = os.environ.copy()
    env["RUSTFLAGS"] = "--cfg tokio_unstable"
    for binname in ("gravity_node", "gravity_cli"):
        cmd = [
            "cargo", "build", "--manifest-path", str(GSDK_ROOT / "Cargo.toml"),
            "--bin", binname, "--profile", "quick-release",
        ]
        log(f"运行: {' '.join(cmd)}")
        if subprocess.run(cmd, env=env).returncode != 0:
            raise RuntimeError(f"构建 {binname} 失败")


# ----------------------------------------------------------------------------
# 命令实现
# ----------------------------------------------------------------------------

def cmd_up(preset, *, instance: int = 0, fresh: bool = False) -> int:
    from .env import alloc_instance
    if instance == "auto" or instance is None:
        instance = alloc_instance(preset)
        log(f"自动分配 instance={instance}")
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    env = _script_env(cp)
    cfg = str(cp.cluster_toml)                    # 本 instance（偏移端口/独立 base_dir）
    tmpl_cfg = str(cp.preset_dir / "cluster.toml")  # 模板（genesis.toml 同目录）供 init/genesis
    gen = str(cp.genesis_toml)

    node_ids = cp.node_ids()
    already = all(_pid_alive(cp.pid_file(nid)) for nid in node_ids) and _rpc_up(w3)
    if already and not fresh:
        log(f"集群已在运行（{cp.name} inst={cp.instance}, base_dir={cp.base_dir}），RPC={cp.rpc_url()} block={w3.eth.block_number}")
        return 0

    _ensure_binaries()
    genesis_json = cp.artifacts_dir / "genesis.json"
    identity = cp.artifacts_dir / node_ids[0] / "config" / "identity.yaml"

    _run_script("stop.sh", ["--config", cfg], env=env, check=False)  # 停本 instance 旧进程
    subprocess.run(["rm", "-rf", str(cp.base_dir)])                   # 只清本 instance base_dir
    if fresh and cp.instance == 0:
        # 共享 genesis 由 instance 0 “拥有”，--fresh 才重建；instance>0 的 --fresh 不动共享 genesis
        log("--fresh (instance 0)：清理共享 artifacts，重新 genesis")
        subprocess.run(["rm", "-rf", str(cp.artifacts_dir)])
    elif fresh:
        log(f"--fresh (instance {cp.instance})：只重置本实例 base_dir，共享 genesis 保留")

    # 共享 genesis/identity 只生成一次（用模板配置，端口无关）；多 instance 复用
    if not identity.exists():
        _run_script("init.sh", [tmpl_cfg], env=env)
    if not genesis_json.exists():
        _run_script("genesis.sh", [gen], env=env)
    _run_script("deploy.sh", [cfg], env=env)
    _run_script("start.sh", ["--config", cfg], env=env)

    log(f"等待 RPC 就绪 {cp.rpc_url()} ...")
    if not _wait_rpc(w3, timeout=90):
        log(f"RPC 未在 90s 内就绪，请检查日志: gnode logs --preset {cp.name} --instance {cp.instance} --which reth")
        return 1
    log(f"集群已就绪：{cp.name} inst={cp.instance} RPC={cp.rpc_url()} chainId={w3.eth.chain_id} block={w3.eth.block_number} prague={cp.prague}")
    return 0


def cmd_down(preset, *, instance: int = 0) -> int:
    cp = resolve_cluster(preset, int(instance))
    _run_script("stop.sh", ["--config", str(cp.cluster_toml)], env=_script_env(cp), check=False)
    log(f"已发送停止命令（{cp.name} inst={cp.instance}）")
    return 0


def cmd_status(preset, *, instance: int = 0, show_all: bool = False) -> int:
    from .env import PRESETS, MAX_INSTANCES
    names = list(PRESETS) if show_all else [preset]
    for name in names:
        # 指定 --instance 时只看那一个；否则（含 show_all）列出所有 base_dir 存在的 instance（0 号始终列）
        if not show_all and instance:
            insts = [int(instance)]
        else:
            insts = [i for i in range(MAX_INSTANCES)
                     if i == 0 or Path(f"/tmp/gnode-{resolve_cluster(name).name}{'' if i == 0 else f'-{i}'}").exists()]
        for inst in insts:
            cp = resolve_cluster(name, inst)
            for nid in cp.node_ids():
                w3 = make_web3(cp.rpc_url(nid), timeout=3)
                alive = _pid_alive(cp.pid_file(nid))
                block = "-"
                try:
                    block = str(int(w3.eth.block_number))
                except Exception:
                    pass
                print(f"{cp.name:<8} inst={inst:<3} {cp.rpc_url(nid):<28} {('up' if alive else 'down'):<6} block={block}")
    return 0


def cmd_logs(preset, which: str, follow: bool, lines: int, *, instance: int = 0) -> int:
    cp = resolve_cluster(preset, int(instance))
    # 默认第一个节点
    nid = cp.node_ids()[0]
    files = cp.log_files(nid)
    targets = list(files.values()) if which == "all" else [files[which]]
    targets = [t for t in targets if t.exists()]
    if not targets:
        log(f"无日志文件（which={which}）：{[str(files[k]) for k in files]}")
        return 1
    cmd = ["tail", f"-n{lines}"]
    if follow:
        cmd.append("-F")
    cmd += [str(t) for t in targets]
    return subprocess.run(cmd).returncode


def cmd_state(preset, addr: str, *, instance: int = 0) -> int:
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    try:
        addr = Web3.to_checksum_address(addr)
    except Exception:
        raise ValueError(f"地址不合法: {addr!r}（需为 0x 开头的 40 位十六进制地址）")
    node_id = cp.node_ids()[0]

    # 先判可达性：RPC 连不上就直接报明确状态（down/unreachable），不跑 halt 探测、不误报 panic
    if not _rpc_up(w3):
        proc_up = _pid_alive(cp.pid_file(node_id))
        out = {
            "address": addr,
            "reachable": False,
            "node_process": "up" if proc_up else "down",
            "detail": (f"进程在但 RPC 无响应（疑似 halt），先 `gnode up --preset {cp.name} --fresh`"
                       if proc_up else f"节点未运行，先 `gnode up --preset {cp.name}`"),
        }
        print(json.dumps(out, indent=2, ensure_ascii=False))
        return 2

    out = {"address": addr, "reachable": True}
    out["balance_wei"] = int(w3.eth.get_balance(addr))
    out["balance_eth"] = float(w3.from_wei(out["balance_wei"], "ether"))
    out["nonce"] = int(w3.eth.get_transaction_count(addr))
    code = w3.eth.get_code(addr)
    out["code_size"] = len(code)
    out["code"] = code.to_0x_hex() if code else "0x"
    # 附带一次快速 halt 探测（此时 RPC 已知可达）
    probe = probe_liveness(w3, window_s=6.0, pid_alive=lambda: _pid_alive(cp.pid_file(node_id)))
    out["liveness"] = probe.as_dict()
    print(json.dumps(out, indent=2, ensure_ascii=False))
    return 0


def _load_artifact(path: Path) -> tuple[Optional[list], str]:
    """支持 {abi,bytecode} JSON 或纯 bytecode（.hex/.txt/.bin）。返回 (abi, bytecode_hex)。"""
    raw = path.read_text().strip()
    if raw.startswith("{"):
        obj = json.loads(raw)
        bytecode = obj.get("bytecode") or obj.get("bin") or obj.get("object")
        if isinstance(bytecode, dict):  # solc 标准输出 {object: ...}
            bytecode = bytecode.get("object")
        abi = obj.get("abi")
        if not bytecode:
            raise ValueError(f"artifact 缺少 bytecode: {path}")
        return abi, bytecode if bytecode.startswith("0x") else "0x" + bytecode
    # 纯 bytecode
    return None, raw if raw.startswith("0x") else "0x" + raw


def send_raw_deploy(w3: Web3, acct, bytecode: str, *, gas: Optional[int] = None) -> dict:
    """用原始 bytecode 部署（不依赖 ABI）。返回 {tx_hash, contract, status, block, gas_used}。"""
    fees = suggest_fees(w3)
    tx = {
        "from": acct.address,
        "nonce": w3.eth.get_transaction_count(acct.address),
        "data": bytecode,
        "value": 0,
        "chainId": w3.eth.chain_id,
        **fees,
    }
    tx["gas"] = gas or int(w3.eth.estimate_gas(tx) * 12 // 10)
    signed = acct.sign_transaction(tx)
    h = w3.eth.send_raw_transaction(signed.raw_transaction)
    rcpt = w3.eth.wait_for_transaction_receipt(h, timeout=120)
    return {
        "tx_hash": h.to_0x_hex(),
        "contract": rcpt.get("contractAddress"),
        "status": int(rcpt["status"]),
        "block": int(rcpt["blockNumber"]),
        "gas_used": int(rcpt["gasUsed"]),
    }


def _checksum_addrs(v):
    """递归把「恰好 20 字节的 0x 十六进制串」规范成 EIP-55 checksum 地址，
    其余原样（bytes4 之类短 hex 不动）。让 deploy --args 像 send 一样容忍小写地址。"""
    if isinstance(v, str) and len(v) == 42 and v.startswith("0x"):
        try:
            int(v, 16)
            return Web3.to_checksum_address(v)
        except ValueError:
            return v
    if isinstance(v, list):
        return [_checksum_addrs(x) for x in v]
    return v


def cmd_deploy(preset, artifact_path: str, *, args_json: Optional[str] = None, instance: int = 0) -> int:
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    rc = _require_rpc(cp, w3)
    if rc is not None:
        return rc
    abi, bytecode = _load_artifact(Path(artifact_path))
    acct = faucet_account()

    if args_json:
        ctor_args = json.loads(args_json)
        if not isinstance(ctor_args, list):
            raise ValueError("--args 需为 JSON 数组，如 '[42,\"0xabc\"]'")
        if not abi:
            raise ValueError("传了 --args 但 artifact 无 abi，无法编码构造函数参数（请用带 abi 的 artifact）")
        # 用 abi 把构造参数编码进 initcode（data_in_transaction 已是 0x 十六进制串）；
        # 先把地址型参数规范成 checksum，容忍小写地址（与 send 的 to 处理一致）
        contract = w3.eth.contract(abi=abi, bytecode=bytecode)
        bytecode = contract.constructor(*_checksum_addrs(ctor_args)).data_in_transaction
        log(f"从 {acct.address} 部署 {artifact_path}（构造参数 {ctor_args}）...")
    else:
        log(f"从 {acct.address} 部署 {artifact_path} ...")

    res = send_raw_deploy(w3, acct, bytecode)
    print(json.dumps(res, indent=2, ensure_ascii=False))
    return 0 if res["status"] == 1 else 2


def cmd_send(preset, tx_path: str, *, no_wait: bool = False, instance: int = 0) -> int:
    """tx.json 规格见 `gnode send --help`。支持 type(2/1/4)、accessList、authorizationList、
    raw(已签名原始交易) 与 --no-wait（不等回执，用于同块/连发）。缺省用 faucet 签名。"""
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    rc = _require_rpc(cp, w3)
    if rc is not None:
        return rc
    spec = json.loads(Path(tx_path).read_text())
    raw, sender, to = _build_signed_from_spec(w3, spec)
    h = w3.eth.send_raw_transaction(raw)
    return _report_sent(w3, h, no_wait, sender=sender, to=to)


def _build_signed_from_spec(w3: Web3, spec: dict):
    """把一条 tx.json 规格构造并签名成 (raw, sender, to)，供 send / send-batch 复用。

    raw 是可直接喂给 eth_sendRawTransaction 的原始交易；sender/to 用于回执补全
    （raw 直广播路径下未知，返回 None）。规格字段/校验与 `gnode send --help` 一致。
    """
    # 拒绝未知/拼错的字段，避免像把 "value" 写成 "valeu" 这种被静默忽略
    known = {"to", "value", "data", "nonce", "gas", "gasPrice", "type", "accessList",
             "authorizationList", "raw", "privkey"}
    unknown = set(spec) - known
    if unknown:
        raise ValueError(f"tx.json 含未知字段 {sorted(unknown)}；合法字段: {sorted(known)}")

    # 已签名原始交易：直接广播，忽略其他字段
    if spec.get("raw"):
        raw = spec["raw"]
        return (raw if raw.startswith("0x") else "0x" + raw), None, None

    acct = Account.from_key(spec["privkey"]) if spec.get("privkey") else faucet_account()
    # value 友好校验：接受十进制或 0x 十六进制，必须非负
    raw_val = spec.get("value", 0)
    try:
        value = int(raw_val, 0) if isinstance(raw_val, str) else int(raw_val)
    except (ValueError, TypeError):
        raise ValueError(f"value={raw_val!r} 需为非负整数（十进制或 0x 十六进制）")
    if value < 0:
        raise ValueError(f"value={value} 非法：需为非负整数")
    if value >= 2 ** 256:
        raise ValueError(f"value 超出 uint256 上限（{value} ≥ 2^256）")
    tx: dict = {
        "from": acct.address,
        "chainId": w3.eth.chain_id,
        "nonce": spec.get("nonce", w3.eth.get_transaction_count(acct.address)),
        "value": value,
    }
    if spec.get("to"):
        try:
            tx["to"] = Web3.to_checksum_address(spec["to"])
        except Exception:
            raise ValueError(f"to={spec['to']!r} 不是合法地址")
    if spec.get("data"):
        tx["data"] = spec["data"]
    req_type = int(spec["type"]) if spec.get("type") is not None else None
    # 0(legacy) 不能带 type 字段（eth_account 不认 type=0）；1/2/4 显式设置
    if req_type not in (None, 0):
        tx["type"] = req_type
    if req_type == 1:
        tx.setdefault("accessList", [])  # type-1(EIP-2930) 需带 accessList 字段
    if spec.get("accessList") is not None:
        tx["accessList"] = spec["accessList"]
    if spec.get("authorizationList") is not None:
        # type-4 SetCode：已签名的授权(带 r/s/yParity)直接用；未签名的(给 delegate+signerKey)自动签，
        # 免去手写 eth_account 的 auth 签名。self-sponsored(签名者即发送者)时 auth.nonce 默认 = tx.nonce+1。
        tx["authorizationList"] = _prepare_auth_list(w3, spec["authorizationList"], tx, acct)
        tx["type"] = req_type = 4
        # 7702 SetCode 交易必须有 to（不允许合约创建）且需带 accessList 字段；缺省补上
        tx.setdefault("to", acct.address)
        tx.setdefault("accessList", [])
    # gas 费按交易类型区分：type 0/1(legacy/access-list)用 gasPrice；type 2(默认)/4 用 EIP-1559 三件套
    fees = suggest_fees(w3)
    if req_type in (0, 1):
        gp = spec.get("gasPrice")
        tx["gasPrice"] = (int(gp, 0) if isinstance(gp, str) else int(gp)) if gp is not None else fees["maxFeePerGas"]
    else:
        tx.update(fees)
    try:
        tx["gas"] = spec.get("gas") or int(w3.eth.estimate_gas(tx) * 12 // 10)
    except Exception:
        # 部分 type-4/自定义交易 estimate 会失败，给个保守默认，允许 spec 覆盖
        tx["gas"] = spec.get("gas") or 500000

    signed = acct.sign_transaction(tx)
    return signed.raw_transaction, acct.address, tx.get("to")


def cmd_send_batch(preset, batch_path: str, *, instance: int = 0) -> int:
    """按数组顺序连发一批交易，尽力打进同一个块并保持块内顺序（同块有序批量原语）。

    batch.json = 一个 tx 规格数组，每个元素字段同 `gnode send`（to/value/data/nonce/type/
    authorizationList/raw/privkey/...）。先对全部规格逐条构造并签名，再**不等回执**地按
    数组顺序背靠背广播（避免签名耗时打散提交），最后拉回执报告各笔落在哪个块/块内下标。

    ⚠️ 顺序保证的诚实边界：本地 RPC 不提供比「同发送者按 nonce 排序 / 跨发送者按小费排序」
    更强的块内定序原语。
      · 同一发送者的连续 nonce：块内顺序确定（nonce 递增），是 nonce 竞争最可靠的复现姿势；
      · 跨发送者：块内先后由 fee-priority 决定，请自行在各笔 spec 里用 maxPriorityFeePerGas
        把想排前的交易设更高小费（best-effort，非 100% 保证同块/同序）。
    """
    cp = resolve_cluster(preset, int(instance))
    w3 = make_web3(cp.rpc_url())
    rc = _require_rpc(cp, w3)
    if rc is not None:
        return rc
    specs = json.loads(Path(batch_path).read_text())
    if not isinstance(specs, list) or not specs:
        raise ValueError("batch.json 需为非空的 tx 规格数组，如 '[{...},{...}]'")

    # 阶段一：先把所有交易签好（签名/estimate 可能较慢，提前做完，广播阶段才能紧凑连发）。
    prepared = []
    for i, spec in enumerate(specs):
        if not isinstance(spec, dict):
            raise ValueError(f"batch[{i}] 需为对象(tx 规格)，得到 {type(spec).__name__}")
        raw, sender, to = _build_signed_from_spec(w3, spec)
        prepared.append((raw, sender, to))

    # 阶段二：按数组顺序背靠背广播，不等回执（--no-wait 语义），尽量同块同序。
    sent = []
    for i, (raw, sender, to) in enumerate(prepared):
        try:
            h = w3.eth.send_raw_transaction(raw)
            sent.append({"index": i, "tx_hash": h.to_0x_hex(), "from": sender, "to": to, "error": None})
        except Exception as e:  # noqa: BLE001 —— 单笔失败不应中断整批，记录后继续
            sent.append({"index": i, "tx_hash": None, "from": sender, "to": to,
                         "error": f"{type(e).__name__}: {e}"})
    log(f"已按顺序广播 {sum(1 for s in sent if s['tx_hash'])}/{len(prepared)} 笔，拉回执确认落块 ...")

    # 阶段三：拉回执，报告各笔落在哪个块 / 块内下标，便于判断是否同块按序。
    for s in sent:
        if not s["tx_hash"]:
            continue
        try:
            r = w3.eth.wait_for_transaction_receipt(s["tx_hash"], timeout=120)
            s["block"] = int(r["blockNumber"])
            s["tx_index"] = int(r["transactionIndex"])
            s["status"] = int(r["status"])
        except Exception as e:  # noqa: BLE001
            s["block"] = s["tx_index"] = s["status"] = None
            s["error"] = f"receipt: {type(e).__name__}: {e}"

    landed = [s for s in sent if s.get("block") is not None]
    blocks = {s["block"] for s in landed}
    same_block = len(blocks) == 1 and len(landed) == len(sent)
    # 块内是否严格按数组顺序（index 升序 ⇔ tx_index 升序）
    ordered = same_block and all(
        landed[k]["tx_index"] < landed[k + 1]["tx_index"] for k in range(len(landed) - 1)
    )
    out = {
        "count": len(sent),
        "same_block": same_block,
        "ordered_in_block": ordered,
        "blocks": sorted(blocks),
        "txs": sent,
        "note": ("全部同块且按数组顺序落块" if ordered else
                 "未全部同块/未严格按序 —— 跨发送者定序靠 fee-priority，best-effort；"
                 "同发送者连续 nonce 才有确定性块内序"),
    }
    print(json.dumps(out, indent=2, ensure_ascii=False))
    # 退出码：全部成功落块=0；有交易失败/未落块=2（inconclusive/部分失败），与 send 的非 1-status 归 2 一致
    all_ok = len(landed) == len(sent) and all(s.get("status") == 1 for s in landed)
    return 0 if all_ok else 2


def _prepare_auth_list(w3: Web3, entries: list, tx: dict, tx_signer) -> list:
    """把 authorizationList 里未签名的条目自动签好（reuse gravity_e2e.utils.eip7702）。

    条目二选一：
      已签名: {chainId,address,nonce,yParity,r,s} —— 原样返回。
      待签名: {delegate|address, signerKey, nonce?} —— 用 signerKey 签 delegate；
              nonce 缺省：签名者==发送者(self-sponsored)时取 tx.nonce+1，否则取签名者当前 nonce。
    """
    from gravity_e2e.utils.eip7702 import sign_authorization

    chain_id = tx["chainId"]
    out = []
    for e in entries:
        if "r" in e and "s" in e:  # 已签名，原样用
            out.append(e)
            continue
        signer_key = e.get("signerKey") or e.get("privkey")
        if not signer_key:
            raise ValueError("authorizationList 条目未签名且缺 signerKey，无法自动签名")
        signer = Account.from_key(signer_key)
        delegate = Web3.to_checksum_address(e.get("delegate") or e["address"])
        if "nonce" in e:
            nonce = int(e["nonce"])
        elif signer.address.lower() == tx_signer.address.lower():
            nonce = int(tx["nonce"]) + 1  # self-sponsored：发送者 nonce 先自增，auth 要 +1
        else:
            nonce = w3.eth.get_transaction_count(signer.address)
        out.append(sign_authorization(signer, chain_id=chain_id, delegate=delegate, nonce=nonce))
    return out


def _report_sent(w3: Web3, h, no_wait: bool, *, sender, to) -> int:
    tx_hash = h.to_0x_hex()
    if no_wait:
        log(f"已发送 tx {tx_hash}（--no-wait，不等回执）")
        print(json.dumps({"tx_hash": tx_hash, "waited": False, "from": sender, "to": to},
                         indent=2, ensure_ascii=False))
        return 0
    log(f"已发送 tx {tx_hash}，等待回执 ...")
    rcpt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    out = {
        "tx_hash": tx_hash,
        "status": int(rcpt["status"]),
        "block": int(rcpt["blockNumber"]),
        "gas_used": int(rcpt["gasUsed"]),
        # raw 交易我们不知道 from/to，从回执补回（普通路径 sender/to 已知则优先用）
        "from": sender or rcpt.get("from"),
        "to": to if to is not None else rcpt.get("to"),
    }
    print(json.dumps(out, indent=2, ensure_ascii=False))
    return 0 if out["status"] == 1 else 2
