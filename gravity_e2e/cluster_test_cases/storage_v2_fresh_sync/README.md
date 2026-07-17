# storage_v2_fresh_sync (storage-v2 TC9)

SF-enabled **fresh nodes sync from block 0** on a rolling-upgraded
network, and an **SF validator votes** — with the SF × non-SF
upstream/downstream matrix fully covered. Design doc:
`_local/drafts/storage-v2-e2e/sf-fresh-sync-design.md` (repo-external).

## Topology (9 nodes)

| node | role | layout | upstream | purpose |
|---|---|---|---|---|
| node1, node2 | genesis validator | legacy (upgraded) | — | consensus core, born v1.7.5 |
| vfn1 | vfn | legacy (upgraded) | node1 | control + sf_pfn2/pfn1 upstream |
| pfn1 | pfn | legacy (upgraded) | vfn1 | **tx entry** + control |
| sf_val1 | validator | **SF** | — (joins) | G3: SF validator votes |
| sf_vfn1 | vfn | **SF** | node2 | SF vfn ← legacy validator |
| sf_vfn2 | vfn | **SF** | sf_val1 | SF vfn ← SF validator |
| sf_pfn1 | pfn | **SF** | sf_vfn1 | SF pfn ← SF vfn |
| sf_pfn2 | pfn | **SF** | vfn1 | SF pfn ← legacy vfn |

## Phases

1. legacy core live on v1.7.5; SF nodes stopped + data-wiped back to
   fresh (the runner auto-starts everything);
2. v1.7.5-era history + anchor batch A via pfn1; cross ≥1 epoch;
3. rolling upgrade → merge v2.3.0 (sender paused per swap window — with
   2 validators every swap freezes the chain, expected), Alpha crossing,
   anchor batch B;
4. sf_vfn1 / sf_pfn2 / sf_pfn1 from-0 sync (SF-enable hook at first
   start);
5. offline layout probes: SF mirror of TC1 (settings
   PRESENT_STATIC_FILES, segments+sidecars, tables == 0) vs upgraded
   legacy controls (settings MISSING, no SF files, tables > 0);
6. A+B anchor replays on the SF fullnodes (v1.7.5 history served by the
   SF read path — the core from-0 evidence);
7. sf_val1 from-0 sync → governance-enabled permissionless join with
   **equal stake** (2 ETH = genesis stake → 3-validator quorum = ALL
   votes) → L1 active, L2 healthy epoch;
8. sf_vfn2 (SF ← SF validator) closes the matrix;
9. **L3 necessity probe**: stop sf_val1 → chain MUST freeze → restart →
   chain resumes, still active (its offline SF probe rides the window);
10. load floors, final A+B replay on all nodes, log scan.

## SF enable (until greth wires a fresh-init switch)

`[sf] mode = "migrate"` (form D): first start (fresh init) → stop →
`db migrate-changesets` → restart. **Requires a binary with the #391
preflight fix** (block-0 genesis reverts must be accepted and migrated
as entity rows). `mode = "flag"` (form B, `--storage.v2`) is reserved
until greth wires the flag into init_genesis.

## Fund flow (three accounts — do not "just use the faucet")

| account | role | why |
|---|---|---|
| genesis faucet | **gas-only** governance wallet (owner + pool[0] voter) | `[faucet_init]` **sweeps** its on-chain balance at suite init: `cluster/faucet.sh:44-56` writes `eth_balance` into the bench config, but gravity_bench overrides it — `main.rs:384-411` scales the cascade up to (on-chain − 1%) and `src/txn_plan/constructor/faucet.rs:64-104` splits it all among the bench accounts. Leftover ≈ 0.5 ETH (live attempt3 died funding 1 ETH from 0.488). |
| bench[0] (accounts.csv) | sf_val1's join/staking signer | `manager._ensure_evm_account` assigns csv rows to VALIDATOR-role nodes in order; the 2 ETH equal-power stake comes from its init balance. |
| bench[1] (accounts.csv) | the **bank**: all foreground value transfers (A/B history, bg-sender funding) | disjoint from the background TxSender's dedicated account (funded once by the bank in phase 2 — nonce-race lock from live attempt2). |

Phase 1 fail-fasts when the post-sweep faucet gas budget or the bank
balance is missing. Unit locks: `TxSender` may never be fed
`cluster.faucet`; `TransactionBuilder` may never be fed the faucet;
`[faucet_init] num_accounts` must equal the bench partition.

## Run

```
cp test_params.toml.example test_params.toml   # edit binaries if needed
python render_config.py                        # re-render before EVERY run
cd ../.. && python3 gravity_e2e/runner.py --force-init storage_v2_fresh_sync
```

Old binary: v1.7.5 (see storage_v2_upgrade/README for the release-asset
channel and the source-build fallback). New binary: `[sf_source]` and
`GRAVITY_NEW_BINARY` must point at the same merge v2.3.0 build. Expected
duration ~50-70 min. Without a rendered cluster.toml the runner skips
the suite (binary opt-in, CI-neutral).
