#!/usr/bin/env python3
"""Audit a Reth/geth-style genesis.json for dev/test credential leaks.

Outputs a Markdown report to stdout. Exit code 1 on any FAIL, 0 otherwise
(PASS or WARN-only).

Usage:
    python audit_genesis.py <genesis.json> [--expected-chain-id N]

What it checks:
  - chainId is not a known dev value (1337, 31337) and matches the
    optional --expected-chain-id.
  - alloc does not contain any address derived from the public Anvil /
    Hardhat / Foundry test mnemonic ("test test test ... junk").
  - The raw file text does not embed the well-known anvil default
    private key or the test mnemonic anywhere (defends against stray
    comments, metadata fields, leftover fixtures).
  - No individual alloc balance is implausibly large (>= 2^160 wei),
    which usually indicates a dev "infinite money" preset.

Extending: edit the constants at the top of this file.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Anvil / Hardhat / Foundry default mnemonic:
#   "test test test test test test test test test test test junk"
# All ten accounts derived from this mnemonic have publicly-known private
# keys. None of these addresses must ever appear in a mainnet alloc.
DEV_ADDRESSES: dict[str, str] = {
    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266": "anvil-mnemonic-#0",
    "0x70997970C51812dc3A010C7d01b50e0d17dc79C8": "anvil-mnemonic-#1",
    "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC": "anvil-mnemonic-#2",
    "0x90F79bf6EB2c4f870365E785982E1f101E93b906": "anvil-mnemonic-#3",
    "0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65": "anvil-mnemonic-#4",
    "0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc": "anvil-mnemonic-#5",
    "0x976EA74026E726554dB657fA54763abd0C3a0aa9": "anvil-mnemonic-#6",
    "0x14dC79964da2C08b23698B3D3cc7Ca32193d9955": "anvil-mnemonic-#7",
    "0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f": "anvil-mnemonic-#8",
    "0xa0Ee7A142d267C1f36714E4a8F75612F20a79720": "anvil-mnemonic-#9",
}

# Strings that should never appear literally in a mainnet genesis file.
DEV_LITERAL_STRINGS: list[tuple[str, str]] = [
    (
        "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        "anvil-default-privkey-#0",
    ),
    (
        "test test test test test test test test test test test junk",
        "anvil-foundry-test-mnemonic",
    ),
]

DEV_CHAIN_IDS: set[int] = {1337, 31337}

# 2^160 wei ~= 1.46e+30 ETH. Any per-account allocation at or above this
# threshold is implausible for a real mainnet and almost always means a
# dev-style "infinite money" preset. Reported as WARN (not FAIL) because
# a system contract acting as the token mint may be intentionally large
# and should be confirmed by a human, not auto-rejected.
SUSPICIOUS_BALANCE_THRESHOLD_BITS = 160


def parse_balance(b) -> int | None:
    if b is None:
        return None
    if isinstance(b, int):
        return b
    s = str(b).strip()
    try:
        if s.lower().startswith("0x"):
            return int(s, 16)
        return int(s, 10)
    except ValueError:
        return None


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Audit a Reth/geth-style genesis.json for dev/test credential leaks."
    )
    ap.add_argument("genesis_path", type=Path)
    ap.add_argument(
        "--expected-chain-id",
        type=int,
        default=None,
        help="Require chainId to equal this value (in addition to the dev-id blocklist).",
    )
    args = ap.parse_args()

    raw = args.genesis_path.read_text()
    g = json.loads(raw)

    findings: list[tuple[str, str, str]] = []  # (severity, label, detail)

    # --- chainId ---
    chain_id = g.get("config", {}).get("chainId")
    if chain_id in DEV_CHAIN_IDS:
        findings.append(
            ("FAIL", "Dev chainId", f"chainId={chain_id} is a known dev value")
        )
    if args.expected_chain_id is not None and chain_id != args.expected_chain_id:
        findings.append(
            (
                "FAIL",
                "ChainId mismatch",
                f"expected {args.expected_chain_id}, got {chain_id}",
            )
        )

    # --- alloc address blocklist ---
    alloc = g.get("alloc", {}) or {}
    alloc_lc = {a.lower(): a for a in alloc.keys()}
    for dev_addr, source in DEV_ADDRESSES.items():
        if dev_addr.lower() in alloc_lc:
            real_key = alloc_lc[dev_addr.lower()]
            bal = alloc[real_key].get("balance", "0x0")
            findings.append(
                (
                    "FAIL",
                    f"Dev address in alloc ({source})",
                    f"{real_key} balance={bal}",
                )
            )

    # --- literal string blocklist on raw file text ---
    raw_lc = raw.lower()
    for s, source in DEV_LITERAL_STRINGS:
        if s.lower() in raw_lc:
            findings.append(
                (
                    "FAIL",
                    f"Dev secret literal in genesis ({source})",
                    f"matched: {s[:48]}{'...' if len(s) > 48 else ''}",
                )
            )

    # --- suspicious balance heuristic ---
    threshold = 1 << SUSPICIOUS_BALANCE_THRESHOLD_BITS
    for addr, entry in alloc.items():
        bal = parse_balance(entry.get("balance"))
        if bal is None:
            continue
        if bal >= threshold:
            findings.append(
                (
                    "WARN",
                    "Oversized balance (possible dev-style 'infinite money')",
                    f"{addr} balance={entry.get('balance')} (~2^{bal.bit_length()} wei)",
                )
            )

    # ---------- report ----------
    print(f"## Genesis audit: `{args.genesis_path}`")
    print()
    print(f"- **chainId**: `{chain_id}`")
    print(f"- **alloc entries**: {len(alloc)}")
    print()
    print("### alloc entries")
    print()
    print("| Address | Balance |")
    print("|---|---|")
    for addr in sorted(alloc.keys(), key=str.lower):
        bal = alloc[addr].get("balance", "0x0")
        print(f"| `{addr}` | `{bal}` |")
    print()
    print("### Findings")
    print()

    if not findings:
        print("PASS - no blocklist hits, no suspicious balances.")
        return 0

    sev_order = {"FAIL": 0, "WARN": 1}
    findings.sort(key=lambda f: sev_order.get(f[0], 99))

    for sev, label, detail in findings:
        print(f"- **{sev}** - {label}: {detail}")

    fails = sum(1 for s, _, _ in findings if s == "FAIL")
    warns = sum(1 for s, _, _ in findings if s == "WARN")
    if fails:
        verdict = "FAIL"
    elif warns:
        verdict = "WARN"
    else:
        verdict = "PASS"
    print()
    print(f"**Verdict**: {verdict} ({fails} FAIL, {warns} WARN)")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
