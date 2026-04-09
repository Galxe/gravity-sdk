"""
Shared helpers for RandomDice contract tests.

Provides deploy / roll / read helpers that work with Web3 + TransactionBuilder.
"""

import json
import logging
from pathlib import Path

from eth_account import Account
from web3 import Web3

from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions, run_sync

LOG = logging.getLogger(__name__)

# RandomDice function selectors (keccak256 of signatures)
SELECTORS = {
    "rollDice": "0x837e7cc6",
    "lastRollResult": "0xefeb9231",
    "lastSeedUsed": "0xd904baa6",
    "getLatestRoll": "0x3871da26",
}


def load_bytecode() -> str:
    """Load compiled RandomDice bytecode from known paths."""
    paths = [
        Path(__file__).resolve().parent.parent.parent
        / "contracts_data" / "RandomDice.json",
        Path(__file__).resolve().parent.parent.parent
        / "tests" / "contracts" / "randomness" / "out"
        / "RandomDice.sol" / "RandomDice.json",
    ]
    for p in paths:
        if p.exists():
            data = json.loads(p.read_text())
            bc = data.get("bytecode") or data.get("bin")
            if isinstance(bc, dict):
                bc = bc.get("object", "")
            bc = str(bc)
            if not bc.startswith("0x"):
                bc = "0x" + bc
            return bc
    raise FileNotFoundError(
        "RandomDice not compiled. Run: forge build\n"
        + "\n".join(f"  - {p}" for p in paths)
    )


async def deploy(w3: Web3, deployer) -> str:
    """Deploy RandomDice, return contract address."""
    bytecode = load_bytecode()
    tb = TransactionBuilder(w3, deployer, TransactionOptions(gas_limit=500_000))
    result = await tb.build_and_send_tx(to=None, data=bytecode)
    assert result.success, f"Deploy failed: {result.error}"
    addr = result.tx_receipt.contractAddress
    assert addr, "No contract address in receipt"
    LOG.info(f"RandomDice deployed at {addr}")
    return addr


async def roll(w3: Web3, contract: str, caller) -> tuple:
    """Call rollDice(). Returns (block_number, tx_hash, gas_used)."""
    tb = TransactionBuilder(w3, caller, TransactionOptions(gas_limit=100_000))
    result = await tb.build_and_send_tx(to=contract, data=SELECTORS["rollDice"])
    assert result.success, f"rollDice failed: {result.error}"
    return result.block_number, result.tx_hash, result.gas_used


async def last_result(w3: Web3, contract: str) -> int:
    raw = await run_sync(w3.eth.call, {"to": contract, "data": SELECTORS["lastRollResult"]})
    return int(raw.hex(), 16)


async def last_seed(w3: Web3, contract: str) -> int:
    raw = await run_sync(w3.eth.call, {"to": contract, "data": SELECTORS["lastSeedUsed"]})
    return int(raw.hex(), 16)


async def latest_roll(w3: Web3, contract: str) -> tuple:
    """Returns (roller_addr, roll_result, seed)."""
    raw = await run_sync(w3.eth.call, {"to": contract, "data": SELECTORS["getLatestRoll"]})
    h = raw.hex()
    if h.startswith("0x"):
        h = h[2:]
    roller = "0x" + h[24:64]
    roll_result = int(h[64:128], 16) if len(h) >= 128 else 0
    seed = int(h[128:192], 16) if len(h) >= 192 else 0
    return roller, roll_result, seed


def fund_new_account(w3: Web3, faucet, amount_eth: float = 2.0):
    """Create + fund a new account synchronously. Returns LocalAccount."""
    acct = Account.create()
    nonce = w3.eth.get_transaction_count(faucet.address, "pending")
    tx = {
        "to": acct.address,
        "value": Web3.to_wei(amount_eth, "ether"),
        "gas": 21000,
        "gasPrice": w3.eth.gas_price,
        "nonce": nonce,
        "chainId": w3.eth.chain_id,
    }
    signed = faucet.sign_transaction(tx)
    raw = getattr(signed, "raw_transaction", None) or getattr(signed, "rawTransaction")
    tx_hash = w3.eth.send_raw_transaction(raw)
    w3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)
    return acct
