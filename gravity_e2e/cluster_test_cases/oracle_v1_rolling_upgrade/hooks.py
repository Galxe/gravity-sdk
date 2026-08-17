"""Prepare old/new binaries and the OracleV1 pre-fork genesis state."""

import hashlib
import json
import logging
import os
from pathlib import Path
import shutil

from web3 import Web3

try:
    import tomllib
except ImportError:
    import tomli as tomllib


LOG = logging.getLogger(__name__)
PROJECT_ROOT = Path(__file__).resolve().parents[3]
SIGNED_TESTNET_GENESIS = PROJECT_ROOT / "genesis" / "testnet" / "genesis.json"
METADATA_FILE = "oracle_v1_rolling_upgrade_metadata.json"
SUMMARY_FILE = "oracle_v1_rolling_upgrade_summary.json"

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


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _required_binary(env: dict, name: str) -> Path:
    value = env.get(name)
    if not value:
        raise RuntimeError(f"{name} must point to an executable gravity_node")
    path = Path(value).expanduser().resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise RuntimeError(f"{name} is not an executable file")
    return path


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


def _replace_binary(source: Path, destination: Path) -> None:
    temporary = destination.with_name(f".{destination.name}.rolling.tmp")
    temporary.unlink(missing_ok=True)
    shutil.copy2(source, temporary)
    temporary.chmod(0o755)
    os.replace(temporary, destination)


def _prepare_genesis(test_dir: Path, activation_override: str | None) -> dict:
    genesis_path = test_dir / "artifacts" / "genesis.json"
    generated = json.loads(genesis_path.read_text())
    signed = json.loads(SIGNED_TESTNET_GENESIS.read_text())

    if activation_override is not None:
        activation_block = int(activation_override)
        generated.setdefault("config", {})["oracleV1Block"] = activation_block
    else:
        activation_block = generated.get("config", {}).get("oracleV1Block")
    if not isinstance(activation_block, int) or activation_block <= 0:
        raise RuntimeError("config.oracleV1Block must be a positive integer")

    generated_alloc = generated.get("alloc", {})
    signed_alloc = signed.get("alloc", {})
    contracts = []
    for contract in ORACLE_V1_CONTRACTS:
        generated_key = _alloc_key(generated_alloc, contract["address"])
        signed_key = _alloc_key(signed_alloc, contract["address"])
        pre_fork_code = signed_alloc[signed_key].get("code", "")
        if _runtime_hash(pre_fork_code) != contract["preForkCodeHash"]:
            raise RuntimeError(
                f"signed testnet {contract['name']} pre-fork hash changed"
            )
        generated_hash = _runtime_hash(
            generated_alloc[generated_key].get("code", "")
        )
        if generated_hash not in {
            contract["preForkCodeHash"],
            contract["postForkCodeHash"],
        }:
            raise RuntimeError(
                f"generated {contract['name']} is not a frozen OracleV1 state"
            )
        generated_alloc[generated_key]["code"] = pre_fork_code
        contracts.append(dict(contract))

    temporary = genesis_path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(generated, indent=2) + "\n")
    os.replace(temporary, genesis_path)
    return {"activationBlock": activation_block, "contracts": contracts}


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    old_binary = _required_binary(env, "GRAVITY_OLD_BINARY")
    new_binary = _required_binary(env, "GRAVITY_NEW_BINARY")
    old_hash = _sha256(old_binary)
    new_hash = _sha256(new_binary)
    if old_hash == new_hash:
        raise RuntimeError("old and new gravity_node binaries must differ")

    hardfork = _prepare_genesis(
        test_dir, env.get("ORACLE_V1_ROLLING_ACTIVATION_BLOCK")
    )
    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    (artifacts / SUMMARY_FILE).unlink(missing_ok=True)
    (artifacts / METADATA_FILE).write_text(
        json.dumps(
            {
                "oldBinarySha256": old_hash,
                "newBinarySha256": new_hash,
                "oracleV1Hardfork": hardfork,
            },
            indent=2,
        )
        + "\n"
    )
    env["GRAVITY_OLD_BINARY"] = str(old_binary)
    env["GRAVITY_NEW_BINARY"] = str(new_binary)
    LOG.info(
        "Prepared OracleV1 rolling upgrade old=%s new=%s activation=%d",
        old_hash[:12],
        new_hash[:12],
        hardfork["activationBlock"],
    )


def pre_start(test_dir: Path, env: dict, pytest_args: list[str]):
    old_binary = _required_binary(env, "GRAVITY_OLD_BINARY")
    with (test_dir / "cluster.toml").open("rb") as source:
        config = tomllib.load(source)
    base_dir = Path(config["cluster"]["base_dir"])
    for node in config["nodes"]:
        destination = base_dir / node["id"] / "bin" / "gravity_node"
        _replace_binary(old_binary, destination)
        LOG.info("Installed old binary for %s", node["id"])


def post_stop(test_dir: Path, env: dict):
    (test_dir / "artifacts" / METADATA_FILE).unlink(missing_ok=True)
