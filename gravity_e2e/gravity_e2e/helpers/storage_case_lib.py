"""
Pure helpers shared by the storage-v2 cluster cases
(storage_v2_baseline / storage_v2_upgrade).

Everything here is deterministic and unit-tested in
gravity_e2e/tests/unit/test_storage_case_lib.py. The pytest cases keep
orchestration and assertions; this module keeps the derivable facts:

- OnChainHistory / build_anchor_spec: turn the tx receipts recorded while
  building history into the H1 AnchorSpec, covering all six anchor kinds
  (balance, storage, transaction, receipt, logs, block_hash).
- derive_offline_env: encode the on-disk layout that cluster/deploy.sh
  materializes for a node, so the H2 offline db commands look at exactly
  what the node ran with.
- encode_set_call: calldata for AnchorTarget.set(uint256) without going
  through web3 contract ABI codecs (stable across web3 v6/v7).

History: extracted verbatim from storage_v2_baseline's case-local
storage_baseline_lib.py when storage_v2_upgrade needed the same logic;
``BaselineHistory`` kept as an alias for the recorded name.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import List, Union

from gravity_e2e.helpers.offline_db import OfflineDbEnv
from gravity_e2e.helpers.storage_anchors import AnchorSpec

# keccak256("set(uint256)")[:4]; also visible in the tracked
# prague/contracts/Counter.json bytecode (same signature).
SET_SELECTOR = "60fe47b1"

# AnchorTarget.value lives in storage slot 0 (first declared variable).
VALUE_SLOT = 0


@dataclass(frozen=True)
class TxPoint:
    """One confirmed transaction: its hash and inclusion block."""

    tx_hash: str
    block_number: int


@dataclass(frozen=True)
class OnChainHistory:
    """What a case did on-chain, as recorded from receipts.

    Attributes:
        faucet: sender address funding the transfers.
        recipient: fresh account receiving the transfers.
        contract: deployed AnchorTarget address.
        transfers: confirmed faucet->recipient transfers (>= 1 required;
            the recipient balance differs at each inclusion block).
        deploy: the AnchorTarget deployment transaction.
        sets: confirmed AnchorTarget.set() calls (>= 1 required; slot 0
            differs at each inclusion block).
    """

    faucet: str
    recipient: str
    contract: str
    transfers: List[TxPoint]
    deploy: TxPoint
    sets: List[TxPoint]


# Name under which storage_v2_baseline originally recorded this dataclass.
BaselineHistory = OnChainHistory


def encode_set_call(value: int) -> str:
    """Calldata for AnchorTarget.set(value) as a 0x-hex string."""
    if not 0 <= value < 2**256:
        raise ValueError(f"set() argument out of uint256 range: {value}")
    return "0x" + SET_SELECTOR + format(value, "064x")


def build_anchor_spec(history: OnChainHistory) -> AnchorSpec:
    """Build the H1 collection spec from the recorded history.

    Covers all six anchor kinds:
    - balance: recipient at every transfer block (different value each
      time) plus the faucet at the last transfer block;
    - storage: contract slot 0 at every set() block (different value each
      time) — the changeset-backed historical reads these cases exist for;
    - transaction + receipt: every recorded tx hash;
    - block_hash: every block touched by the history;
    - logs: one address-filtered range spanning the contract's lifetime
      (deploy through last set), expected to hold one ValueSet per set().
    """
    if not history.transfers:
        raise ValueError("history has no transfers — balance anchors need one")
    if not history.sets:
        raise ValueError("history has no set() calls — storage anchors need one")

    balances = [(history.recipient, t.block_number) for t in history.transfers]
    balances.append((history.faucet, history.transfers[-1].block_number))

    storage_slots = [
        (history.contract, VALUE_SLOT, s.block_number) for s in history.sets
    ]

    all_points = [*history.transfers, history.deploy, *history.sets]
    tx_hashes: List[str] = []
    for point in all_points:
        if point.tx_hash not in tx_hashes:
            tx_hashes.append(point.tx_hash)

    block_numbers = sorted({point.block_number for point in all_points})

    contract_blocks = [history.deploy.block_number] + [
        s.block_number for s in history.sets
    ]
    log_ranges = [(min(contract_blocks), max(contract_blocks), history.contract)]

    return AnchorSpec(
        balances=balances,
        storage_slots=storage_slots,
        tx_hashes=tx_hashes,
        block_numbers=block_numbers,
        log_ranges=log_ranges,
    )


def derive_offline_env(
    base_dir: Union[str, Path], node_id: str = "node1"
) -> OfflineDbEnv:
    """OfflineDbEnv for a node deployed by cluster/deploy.sh.

    Layout facts (all from cluster/deploy.sh and
    cluster/templates/reth_config.json.tpl):
    - node dir: <base_dir>/<node_id> with bin/ config/ data/ logs/ script/;
    - node binary hardlinked to <node dir>/bin/gravity_node — using it here
      guarantees the offline commands run the exact binary the node ran
      (in storage_v2_upgrade: the post-upgrade binary after the swap);
    - STORAGE_DIR = <node dir>/data, reth datadir = ${STORAGE_DIR}/reth;
    - --datadir.static-files is ALSO ${STORAGE_DIR}/reth (the datadir root,
      not the reth default <datadir>/static_files), so it must be passed
      explicitly or the offline commands would look in the wrong place;
    - chain spec: the node runs --chain <base_dir>/genesis.json (deploy.sh
      copies the suite genesis there).
    """
    base = Path(base_dir)
    datadir = base / node_id / "data" / "reth"
    return OfflineDbEnv(
        binary=base / node_id / "bin" / "gravity_node",
        datadir=datadir,
        chain=base / "genesis.json",
        static_files_dir=datadir,
    )
