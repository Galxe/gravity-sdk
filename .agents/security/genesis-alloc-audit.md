---
description: How to audit a Reth/geth-style genesis.json for dev/test credential leaks (Anvil mnemonic addresses in alloc, dev chainId, suspicious "infinite money" balances) before publishing it as a mainnet artifact
---

# Genesis Alloc Audit

Run a deterministic, blocklist-based audit of an EL `genesis.json` before publishing it as a mainnet artifact. The goal is to catch the most common ways a genesis leaks dev credentials:

- An Anvil/Hardhat-derived account in `alloc` (those private keys are globally known).
- `chainId` left at the dev default (`1337`, `31337`).
- The public test mnemonic or privkey embedded literally as a string in the file.
- An "infinite money" balance preset that is implausible for any real allocation.

This skill is the EL-side check only. CL waypoint, validator BLS keys, PoP, and onchain config (`epoch_interval_micros`, etc.) live in different files and are out of scope — call those out as not-covered when reporting.

The companion script is `audit_genesis.py` in this directory.

## How to run

```bash
python3 .agents/security/audit_genesis.py <path-to-genesis.json> [--expected-chain-id N]
```

Exit code is `1` if any **FAIL** finding is produced (CI-friendly), `0` otherwise (PASS or WARN-only).

The script writes a Markdown report to stdout with:

1. **Header** — genesis path, chainId, alloc entry count.
2. **alloc table** — every address + balance, sorted by lower-case address. The script cannot know which custom addresses are legitimate; the human reviewer does. List them so the reviewer can scan.
3. **Findings** — every blocklist hit, sorted FAIL-then-WARN, plus a final verdict line.

## What it checks (and why)

| Check | Why it matters |
|---|---|
| `chainId in {1337, 31337}` | Defaults from the `genesis-tool` template and Anvil. A "mainnet" artifact with a dev chainId is an immediate sign that the ceremony input was wrong. |
| `chainId == --expected-chain-id` (optional) | Belt-and-suspenders if the team has agreed on a specific mainnet chainId in advance. Pass it on the CLI to enforce. |
| Anvil/Hardhat default first-10 addresses present in `alloc` | The seed mnemonic `test test test ... junk` is public. Any address derived from it has a globally known privkey — funding it on mainnet is equivalent to setting fire to the tokens AND letting the burner spam transactions from a pre-funded account. |
| Anvil default privkey or test mnemonic appearing as a raw string anywhere in the file | Catches sneaky cases like leftover comments, metadata fields, or stray test fixtures stuck inside the JSON blob. |
| Balance >= 2^160 wei | Legitimate per-account allocations top out near total token supply (typically << 2^100 wei). Beyond 2^160 wei (~10^30 ETH) almost always indicates a dev "infinite money" preset. WARN, not FAIL — a system contract that acts as the token mint may be intentionally large, and a human reviewer should sign off rather than the script auto-rejecting. |

## Updating the blocklist

Constants live at the top of `audit_genesis.py`:

- `DEV_ADDRESSES` — dict of `address -> source-tag`. To add a new dev mnemonic family, derive its first ~10 accounts and append, with a `source` tag pointing at where the mnemonic originates (e.g. `foundry-default-#3`, `hardhat-mnemonic-#0`). The `source` tag shows up in finding output and dramatically speeds up triage.
- `DEV_LITERAL_STRINGS` — list of `(string, source-tag)`. Any literal that should never appear in a mainnet genesis (mnemonics, privkeys, well-known test-vector strings).
- `DEV_CHAIN_IDS` — set of integer chain IDs known to be dev/test.
- `SUSPICIOUS_BALANCE_THRESHOLD_BITS` — bitsize cutoff for the oversized-balance WARN.

Inline constants (rather than a separate JSON data file) keep this a single self-contained script with no extra moving parts. If the blocklist ever grows past a couple of dozen entries, splitting it out is reasonable.

## What this skill does NOT do

- Not a static analyzer for predeployed bytecode. If `alloc[X].code` is custom contract code, this script won't audit the contract; that needs a separate review.
- Not a CL audit. Validator BLS keys, PoP, onchain configs are out of scope.
- Not a tokenomics / balance-sum checker. Only flags absurd *individual* balances.
- Doesn't validate EIP-55 checksumming (alloc keys often arrive lower-cased after `jq`-style processing, and EIP-55 is not a security concern here).
