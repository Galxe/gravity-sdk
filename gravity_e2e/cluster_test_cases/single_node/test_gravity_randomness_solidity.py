import json
import logging
from pathlib import Path
from typing import Optional

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import run_sync

LOG = logging.getLogger(__name__)

RANDOMNESS_OUT_DIR = (
    Path(__file__).resolve().parents[2]
    / "tests/contracts/randomness/out/GravityPrevRandao.sol"
)
TEST_DOMAIN = Web3.keccak(text="GRAVITY_PREVRANDAO_E2E")
DICE_DOMAIN = Web3.keccak(text="GRAVITY_RANDOM_DICE_ROLL")
PREVRANDAO_LOOKUP_FAILED_SELECTOR = Web3.keccak(
    text="PrevRandaoLookupFailed(uint256)"
)[:4].hex()
EMPTY_RANDOMNESS_RANGE_SELECTOR = Web3.keccak(text="EmptyRandomnessRange()")[:4].hex()


async def _send_raw_tx(w3: Web3, account, tx):
    tx.pop("type", None)
    tx.pop("maxFeePerGas", None)
    tx.pop("maxPriorityFeePerGas", None)
    tx.setdefault("chainId", await run_sync(lambda: w3.eth.chain_id))
    tx.setdefault("nonce", await run_sync(w3.eth.get_transaction_count, account.address, "pending"))
    tx.setdefault("gasPrice", await run_sync(lambda: w3.eth.gas_price))

    signed = account.sign_transaction(tx)
    raw_tx = getattr(signed, "raw_transaction", None) or getattr(signed, "rawTransaction")
    tx_hash = await run_sync(w3.eth.send_raw_transaction, raw_tx)
    return await run_sync(w3.eth.wait_for_transaction_receipt, tx_hash, 120)


def _artifact_path(contract_name: str) -> Path:
    return RANDOMNESS_OUT_DIR / f"{contract_name}.json"


def _load_contract_artifact(contract_name: str) -> tuple[list, str]:
    artifact_path = _artifact_path(contract_name)
    if not artifact_path.exists():
        raise FileNotFoundError(
            f"{contract_name} artifact not found. Compile the randomness contracts first:\n"
            "  cd gravity_e2e/tests/contracts/randomness\n"
            "  forge build"
        )

    artifact = json.loads(artifact_path.read_text())
    bytecode = artifact.get("bytecode", {}).get("object", "")
    if not bytecode:
        raise ValueError(f"{contract_name} artifact has no bytecode: {artifact_path}")

    return artifact["abi"], "0x" + bytecode.removeprefix("0x")


async def _deploy_contract(w3: Web3, account, contract_name: str, gas: int = 1_000_000):
    abi, bytecode = _load_contract_artifact(contract_name)
    contract = w3.eth.contract(abi=abi, bytecode=bytecode)

    tx = contract.constructor().build_transaction(
        {
            "from": account.address,
            "value": 0,
            "gas": gas,
        }
    )
    receipt = await _send_raw_tx(w3, account, tx)
    assert receipt["status"] == 1, f"{contract_name} deployment failed"
    assert receipt["contractAddress"], f"{contract_name} deployment returned no contractAddress"

    address = Web3.to_checksum_address(receipt["contractAddress"])
    return w3.eth.contract(address=address, abi=abi)


async def _call_roll(w3: Web3, account, dice, height: Optional[int] = None, mode: str = "default"):
    if mode == "default":
        fn = dice.functions.rollDice()
    elif mode == "current":
        fn = dice.functions.rollDiceAtCurrentBlock()
    elif mode == "parent":
        fn = dice.functions.rollDiceAtParentBlock()
    elif mode == "history":
        assert height is not None, "explicit height is required for mode='height'"
        fn = dice.functions.rollDiceFromHistory(height)
    else:
        raise ValueError(f"unknown roll mode: {mode}")

    tx = fn.build_transaction(
        {
            "from": account.address,
            "value": 0,
            "gas": 2_000_000,
        }
    )
    receipt = await _send_raw_tx(w3, account, tx)
    LOG.info("roll receipt for mode=%s height=%s: %s", mode, height, dict(receipt))
    assert receipt["status"] == 1, (
        f"GravityRandomDice roll failed for mode={mode} height={height}"
    )
    return receipt


def _assert_valid_roll(
    roll,
    signer,
    expected_randomness_height: int,
    expected_execution_height: int,
    expected_nonce: int,
):
    assert roll[0].lower() == signer.address.lower()
    assert 1 <= roll[1] <= 6, f"Dice result out of range: {roll[1]}"
    assert roll[2] == expected_randomness_height
    assert roll[3] == expected_execution_height
    assert roll[4] == expected_nonce
    assert roll[5] == DICE_DOMAIN
    assert int(roll[6].hex(), 16) != 0, "Derived randomness should be non-zero"


def _as_bytes32_hex(value) -> str:
    if isinstance(value, str):
        hex_value = value
    else:
        hex_value = Web3.to_hex(value)
    return "0x" + hex_value.removeprefix("0x").zfill(64).lower()


def _block_randomness(block) -> str:
    value = (
        block.get("mixHash")
        or block.get("mix_hash")
        or block.get("prevRandao")
        or block.get("prev_randao")
    )
    assert value is not None, f"Block {block.get('number')} has no mixHash/prevRandao"
    return _as_bytes32_hex(value)


async def _send_harness_tx(w3: Web3, account, harness, fn_name: str, *args):
    fn = getattr(harness.functions, fn_name)(*args)
    tx = fn.build_transaction(
        {
            "from": account.address,
            "value": 0,
            "gas": 2_000_000,
        }
    )
    receipt = await _send_raw_tx(w3, account, tx)
    assert receipt["status"] == 1, f"{fn_name} transaction failed"
    return receipt


async def _assert_call_reverts(callable_fn, expected_fragment: str):
    try:
        await run_sync(callable_fn)
    except Exception as exc:
        error_text = str(exc)
        assert expected_fragment in error_text, (
            f"Expected revert containing {expected_fragment!r}, got {exc!r}"
        )
        return

    raise AssertionError(f"Expected call to revert with {expected_fragment!r}")


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_gravity_randomness_solidity_wrapper(cluster: Cluster):
    """
    Deploy the Solidity wrapper inspired by Aptos randomness APIs and verify it
    can consume the prevRandao-by-height precompile from an application contract.
    """
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become live"

    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    assert node.w3.is_connected(), "node1 web3 not connected"

    signer = cluster.faucet
    assert signer, "Faucet account is required to deploy GravityRandomDice"

    assert await node.wait_for_block_increase(timeout=30, delta=3), (
        "Node did not produce enough blocks for Solidity randomness lookup"
    )

    dice = await _deploy_contract(node.w3, signer, "GravityRandomDice", gas=1_000_000)
    LOG.info("Deployed GravityRandomDice at %s", dice.address)
    assert dice.functions.nextRollId().call() == 0

    current_receipt = await _call_roll(node.w3, signer, dice, mode="current")
    current_block = int(current_receipt["blockNumber"])
    current_roll = dice.functions.getLatestRoll().call()
    next_roll_id = dice.functions.nextRollId().call()
    LOG.info(
        "rollDiceAtCurrentBlock(): block=%s latest=%s next_roll_id=%s",
        current_block,
        current_roll,
        next_roll_id,
    )
    _assert_valid_roll(current_roll, signer, current_block, current_block, 0)
    assert next_roll_id == 1, "Dice roll id should increment after current-block roll"

    parent_receipt = await _call_roll(node.w3, signer, dice, mode="parent")
    parent_block = int(parent_receipt["blockNumber"])
    parent_roll = dice.functions.getLatestRoll().call()
    next_roll_id = dice.functions.nextRollId().call()
    LOG.info(
        "rollDiceAtParentBlock(): block=%s latest=%s next_roll_id=%s",
        parent_block,
        parent_roll,
        next_roll_id,
    )
    _assert_valid_roll(parent_roll, signer, parent_block - 1, parent_block, 1)
    assert next_roll_id == 2, "Dice roll id should increment after parent-block roll"

    default_receipt = await _call_roll(node.w3, signer, dice, mode="default")
    default_block = int(default_receipt["blockNumber"])
    default_roll = dice.functions.getLatestRoll().call()
    next_roll_id = dice.functions.nextRollId().call()
    LOG.info(
        "rollDice(): block=%s latest=%s next_roll_id=%s",
        default_block,
        default_roll,
        next_roll_id,
    )
    _assert_valid_roll(default_roll, signer, default_block, default_block, 2)
    assert next_roll_id == 3, "Dice roll id should increment after default roll"

    fixed_height = max(1, current_block - 24)
    historical_receipt = await _call_roll(node.w3, signer, dice, fixed_height, mode="history")
    historical_roll = dice.functions.getLatestRoll().call()
    next_roll_id = dice.functions.nextRollId().call()
    LOG.info(
        "rollDiceAt(%s): latest=%s next_roll_id=%s",
        fixed_height,
        historical_roll,
        next_roll_id,
    )

    historical_block = int(historical_receipt["blockNumber"])
    _assert_valid_roll(historical_roll, signer, fixed_height, historical_block, 3)
    assert next_roll_id == 4, "Dice roll id should increment after historical roll"

    repeated_history_receipt = await _call_roll(node.w3, signer, dice, fixed_height, mode="history")
    repeated_roll = dice.functions.getLatestRoll().call()
    next_roll_id = dice.functions.nextRollId().call()
    LOG.info(
        "second rollDiceAt(%s): latest=%s next_roll_id=%s",
        fixed_height,
        repeated_roll,
        next_roll_id,
    )

    repeated_history_block = int(repeated_history_receipt["blockNumber"])
    _assert_valid_roll(repeated_roll, signer, fixed_height, repeated_history_block, 4)
    assert next_roll_id == 5, "Dice roll id should increment on every roll"
    assert repeated_roll[6] != historical_roll[6], (
        "Two rolls at the same height should derive different samples via the contract nonce"
    )
    assert default_receipt["gasUsed"] < historical_receipt["gasUsed"], (
        "Default roll should use the cheaper current-block path"
    )
    assert default_receipt["gasUsed"] < repeated_history_receipt["gasUsed"], (
        "Default roll should remain cheaper than repeated historical lookup"
    )

    assert dice.functions.lastRoller().call().lower() == signer.address.lower()
    assert dice.functions.lastRollResult().call() == repeated_roll[1]
    assert dice.functions.lastRandomnessHeight().call() == fixed_height
    assert dice.functions.lastExecutionHeight().call() == repeated_history_block
    assert dice.functions.lastNonceUsed().call() == 4
    assert dice.functions.lastDomain().call() == DICE_DOMAIN
    assert dice.functions.lastRandomness().call() == repeated_roll[6]


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_gravity_prevrandao_solidity_interfaces(cluster: Cluster):
    """
    Exercise every external/public Solidity wrapper interface through a harness:
    raw lookup, reverting lookup, derivation, bounded ranges, consumer helpers,
    default height selection, nonce accounting, and error paths.
    """
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become live"

    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    assert node.w3.is_connected(), "node1 web3 not connected"

    signer = cluster.faucet
    assert signer, "Faucet account is required to deploy GravityPrevRandaoHarness"

    assert await node.wait_for_block_increase(timeout=30, delta=5), (
        "Node did not produce enough blocks for Solidity prevRandao interface coverage"
    )

    harness = await _deploy_contract(
        node.w3,
        signer,
        "GravityPrevRandaoHarness",
        gas=2_400_000,
    )
    LOG.info("Deployed GravityPrevRandaoHarness at %s", harness.address)

    latest_height = node.get_block_number()
    historical_height = max(1, latest_height - 16)
    historical_block = node.w3.eth.get_block(historical_height, full_transactions=False)
    expected_randomness = _block_randomness(historical_block)
    future_height = latest_height + 1_000

    current_height_from_call, current_prevrandao = harness.functions.prevRandaoWithHeightExternal().call()
    latest_block = node.w3.eth.get_block(current_height_from_call, full_transactions=False)
    expected_current_prevrandao = _block_randomness(latest_block)
    assert (
        _as_bytes32_hex(harness.functions.prevRandaoExternal().call(block_identifier=current_height_from_call))
        == expected_current_prevrandao
    )
    assert _as_bytes32_hex(current_prevrandao) == expected_current_prevrandao

    found, raw_prevrandao = harness.functions.tryHistoricalPrevRandaoAtExternal(historical_height).call()
    assert found is True
    assert _as_bytes32_hex(raw_prevrandao) == expected_randomness
    historical_estimate = harness.functions.tryHistoricalPrevRandaoAtExternal(
        historical_height
    ).estimate_gas({"from": signer.address})
    LOG.info(
        "estimateGas tryHistoricalPrevRandaoAtExternal(%s): %s",
        historical_height,
        historical_estimate,
    )
    assert historical_estimate > 0

    future_found, future_randomness = harness.functions.tryHistoricalPrevRandaoAtExternal(future_height).call()
    assert future_found is False
    assert _as_bytes32_hex(future_randomness) == "0x" + "00" * 32
    future_estimate = harness.functions.tryHistoricalPrevRandaoAtExternal(
        future_height
    ).estimate_gas({"from": signer.address})
    LOG.info(
        "estimateGas tryHistoricalPrevRandaoAtExternal(%s): %s",
        future_height,
        future_estimate,
    )
    assert future_estimate > 0

    assert (
        _as_bytes32_hex(harness.functions.historicalPrevRandaoAtExternal(historical_height).call())
        == expected_randomness
    )
    reverting_estimate = harness.functions.historicalPrevRandaoAtExternal(
        historical_height
    ).estimate_gas({"from": signer.address})
    LOG.info(
        "estimateGas historicalPrevRandaoAtExternal(%s): %s",
        historical_height,
        reverting_estimate,
    )
    assert reverting_estimate > 0
    await _assert_call_reverts(
        lambda: harness.functions.historicalPrevRandaoAtExternal(future_height).call(),
        PREVRANDAO_LOOKUP_FAILED_SELECTOR,
    )

    derived = harness.functions.deriveExternal(
        raw_prevrandao,
        TEST_DOMAIN,
        harness.address,
        signer.address,
        42,
    ).call()
    assert int(derived.hex(), 16) != 0

    current_uint = harness.functions.uint256External(
        TEST_DOMAIN,
        harness.address,
        signer.address,
        41,
    ).call()
    assert current_uint > 0

    derived_uint = harness.functions.historicalUint256AtExternal(
        historical_height,
        TEST_DOMAIN,
        harness.address,
        signer.address,
        43,
    ).call()
    assert derived_uint > 0

    current_range = harness.functions.rangeExternal(
        1,
        10,
        TEST_DOMAIN,
        harness.address,
        signer.address,
        40,
    ).call()
    assert 1 <= current_range < 10

    ranged = harness.functions.historicalRangeAtExternal(
        historical_height,
        10,
        20,
        TEST_DOMAIN,
        harness.address,
        signer.address,
        44,
    ).call()
    assert 10 <= ranged < 20

    current_range_with_sample = harness.functions.rangeWithSampleExternal(
        30,
        40,
        TEST_DOMAIN,
        harness.address,
        signer.address,
        39,
    ).call()
    assert 30 <= current_range_with_sample[0] < 40
    assert int(current_range_with_sample[1].hex(), 16) != 0

    ranged_with_sample = harness.functions.historicalRangeAtWithSampleExternal(
        historical_height,
        20,
        30,
        TEST_DOMAIN,
        harness.address,
        signer.address,
        45,
    ).call()
    assert 20 <= ranged_with_sample[0] < 30
    assert int(ranged_with_sample[1].hex(), 16) != 0

    await _assert_call_reverts(
        lambda: harness.functions.historicalRangeAtExternal(
            historical_height,
            7,
            7,
            TEST_DOMAIN,
            harness.address,
            signer.address,
            46,
        ).call(),
        EMPTY_RANDOMNESS_RANGE_SELECTOR,
    )

    random_bytes = harness.functions.randomBytes32External(1).call({"from": signer.address})
    assert int(random_bytes.hex(), 16) != 0
    assert (
        harness.functions.randomBytes32External(1).call({"from": signer.address})
        == random_bytes
    )
    assert harness.functions.randomBytes32External(2).call({"from": signer.address}) != random_bytes

    historical_random_bytes = harness.functions.historicalRandomBytes32AtExternal(
        historical_height,
        3,
    ).call({"from": signer.address})
    assert int(historical_random_bytes.hex(), 16) != 0

    random_uint = harness.functions.randomUint256External(4).call({"from": signer.address})
    assert random_uint > 0

    historical_random_uint = harness.functions.historicalRandomUint256AtExternal(
        historical_height,
        5,
    ).call({"from": signer.address})
    assert historical_random_uint > 0

    random_range = harness.functions.randomRangeExternal(500, 511, 6).call(
        {"from": signer.address}
    )
    assert 500 <= random_range < 511

    random_range_sample = harness.functions.randomRangeWithSampleExternal(
        520,
        531,
        7,
    ).call({"from": signer.address})
    assert 520 <= random_range_sample[0] < 531
    assert int(random_range_sample[1].hex(), 16) != 0

    historical_random_range = harness.functions.historicalRandomRangeAtExternal(
        historical_height,
        600,
        611,
        8,
    ).call({"from": signer.address})
    assert 600 <= historical_random_range < 611

    historical_random_range_sample = harness.functions.historicalRandomRangeAtWithSampleExternal(
        historical_height,
        620,
        631,
        9,
    ).call({"from": signer.address})
    assert 620 <= historical_random_range_sample[0] < 631
    assert int(historical_random_range_sample[1].hex(), 16) != 0

    random_bool = harness.functions.randomBoolExternal(10).call({"from": signer.address})
    assert isinstance(random_bool, bool)

    historical_random_bool = harness.functions.historicalRandomBoolAtExternal(
        historical_height,
        11,
    ).call({"from": signer.address})
    assert isinstance(historical_random_bool, bool)

    random_index = harness.functions.randomIndexExternal(17, 12).call({"from": signer.address})
    assert 0 <= random_index < 17

    historical_random_index = harness.functions.historicalRandomIndexAtExternal(
        historical_height,
        19,
        13,
    ).call({"from": signer.address})
    assert 0 <= historical_random_index < 19
