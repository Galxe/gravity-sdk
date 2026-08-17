"""Install legacy Oracle runtimes and stage bridge events around OracleV1."""

import json
import logging
import os
from pathlib import Path
import sys

from web3 import Web3


LOG = logging.getLogger(__name__)
PROJECT_ROOT = Path(__file__).resolve().parents[3]
SIGNED_TESTNET_GENESIS = PROJECT_ROOT / "genesis" / "testnet" / "genesis.json"
METADATA_FILE = "oracle_v1_bridge_hardfork_metadata.json"
SUMMARY_FILE = "oracle_v1_bridge_hardfork_summary.json"

ORACLE_V1_CONTRACTS = (
    {
        "name": "NativeOracle",
        "address": "0x00000000000000000000000000000001625f4000",
        "preForkCodeHash": "0x30dd3888ce26735c0d6c5a036b48a1de668dd5506efa7588ce450f976da28255",
        "postForkCodeHash": "0x981087ccdaa0b7843960782e99b078ccdd3820b331f86ce337d9750c5565d984",
    },
    {
        "name": "OracleTaskConfig",
        "address": "0x00000000000000000000000000000001625f1009",
        "preForkCodeHash": "0x74127baf705119810746598b2695ff5fa38f94bd778f0edae46799ffd3606bda",
        "postForkCodeHash": "0xa21bf93e6123b0104b9ea851b8154fb342a5b576c22c71f15f851e266faa9f7f",
    },
)

_mock = None


def _alloc_key(alloc: dict, address: str) -> str:
    normalized = address.removeprefix("0x").lower()
    for key in alloc:
        if key.removeprefix("0x").lower() == normalized:
            return key
    raise RuntimeError(f"genesis alloc is missing system contract {address}")


def _runtime_hash(code: str) -> str:
    if not code.startswith("0x") or len(code) % 2:
        raise RuntimeError("system-contract runtime is not canonical hex")
    return Web3.to_hex(Web3.keccak(hexstr=code)).lower()


def _install_legacy_runtimes(test_dir: Path) -> dict:
    genesis_path = test_dir / "artifacts" / "genesis.json"
    generated = json.loads(genesis_path.read_text())
    signed = json.loads(SIGNED_TESTNET_GENESIS.read_text())

    configured = generated.get("config", {}).get("oracleV1Block")
    override = os.environ.get("ORACLE_V1_BRIDGE_ACTIVATION_BLOCK")
    activation_block = int(override) if override else configured
    if not isinstance(activation_block, int) or activation_block <= 0:
        raise RuntimeError("config.oracleV1Block must be a positive integer")
    generated.setdefault("config", {})["oracleV1Block"] = activation_block

    generated_alloc = generated.get("alloc", {})
    signed_alloc = signed.get("alloc", {})
    evidence = []
    for contract in ORACLE_V1_CONTRACTS:
        generated_key = _alloc_key(generated_alloc, contract["address"])
        signed_key = _alloc_key(signed_alloc, contract["address"])
        signed_code = signed_alloc[signed_key].get("code", "")
        signed_hash = _runtime_hash(signed_code)
        if signed_hash != contract["preForkCodeHash"]:
            raise RuntimeError(
                f"signed {contract['name']} hash {signed_hash} does not match "
                f"{contract['preForkCodeHash']}"
            )
        generated_hash = _runtime_hash(
            generated_alloc[generated_key].get("code", "")
        )
        if generated_hash not in {
            contract["preForkCodeHash"],
            contract["postForkCodeHash"],
        }:
            raise RuntimeError(
                f"generated {contract['name']} hash {generated_hash} is unknown"
            )
        generated_alloc[generated_key]["code"] = signed_code
        evidence.append(dict(contract))

    temporary = genesis_path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(generated, indent=2) + "\n")
    temporary.replace(genesis_path)
    return {"activationBlock": activation_block, "contracts": evidence}


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    hardfork = _install_legacy_runtimes(test_dir)
    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    (artifacts / SUMMARY_FILE).unlink(missing_ok=True)
    (artifacts / METADATA_FILE).write_text(
        json.dumps({"oracleV1Hardfork": hardfork}, indent=2) + "\n"
    )


def pre_start(test_dir: Path, env: dict, pytest_args: list[str]):
    global _mock

    e2e_root = str(Path(__file__).resolve().parent.parent.parent)
    if e2e_root not in sys.path:
        sys.path.insert(0, e2e_root)
    from gravity_e2e.utils.mock_anvil import MockAnvil, DEFAULT_PORTAL_ADDRESS

    amount = 1_000_000_000_000_000_000
    recipient = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
    sender = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
    _mock = MockAnvil(port=28546)
    _mock.start()
    nonces = _mock.preload_events(
        count=2,
        amount=amount,
        recipient=recipient,
        sender_address=sender,
        events_per_block=1,
    )
    _mock.set_finalized(0)

    metadata_path = test_dir / "artifacts" / METADATA_FILE
    metadata = json.loads(metadata_path.read_text())
    metadata["bridge"] = {
        "rpcUrl": _mock.rpc_url,
        "amount": amount,
        "recipient": recipient,
        "sender": sender,
        "portal": DEFAULT_PORTAL_ADDRESS,
        "nonces": nonces,
        "sourceBlocks": [1, 2],
    }
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
    LOG.info("Staged two bridge events; finalized source block remains 0")


def post_stop(test_dir: Path, env: dict):
    global _mock
    if _mock is not None:
        _mock.stop()
        _mock = None
    (test_dir / "artifacts" / METADATA_FILE).unlink(missing_ok=True)
