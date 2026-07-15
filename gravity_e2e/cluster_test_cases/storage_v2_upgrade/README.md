# storage_v2_upgrade (storage-v2 TC2 + TC3)

Rolling upgrade of a 6-node cluster from **gravity-sdk v1.7.5** to the
**greth v2.3.0 integration binary**, verifying merge-level disk
compatibility: history written by v1.7.5 must be read back bit-identically
by the new binary (anchor replay on every node), service must stay
continuous throughout (sustained tx load, height-gap ceilings, hardfork
timeline), and the upgraded cluster must survive graceful and crash
restarts (TC3 tail on the same cluster).

Orchestration skeleton: `rolling_upgrade/` (vfn-first order, height-gap
prechecks, tx failover, `[source]` old-binary pinning, hardfork wait).
Storage verification: the storage-v2 helpers (`helpers/storage_anchors.py`,
`helpers/offline_db.py`, `helpers/storage_case_lib.py`) in the same
consumption pattern as `storage_v2_baseline/` (TC1).

## Fork timeline — the operational rule this case encodes

Mainnet activates **no** gravity hardforks (`genesis/mainnet/genesis.json`
has no `alphaTime` / `betaBlock` / `gammaBlock` / `deltaBlock`). greth
v2.3.0 gates its behavior changes — **system-tx gas exemption** and the
one-shot **SYSTEM_CALLER balance migration** — on the Gravity **Alpha**
fork. That gives the upgrade its one hard rule:

> **Alpha must NOT activate before every node runs the new binary.**

If `alphaTime` is placed before existing v1.7.5 history (e.g. the legacy
`alphaTime = 0` some older suites still carry), the new binary re-executes
those blocks under exempt semantics — SYSTEM_CALLER stops being debited
`gas_used × basefee` per block — and computes a different state root.
Symptom fingerprint (observed live 2026-07-15): the upgraded node aborts
~2 s after start with

```
panicked at aptos-core/consensus/src/block_storage/block_store.rs:773:
assertion `left == right` failed    (two 32-byte block hashes)
```

The case enforces the rule twice: phase 1 rejects any `alphaTime` less
than 25 min away, and phase 3 refuses to start another node's upgrade
within 5 min of activation. `test_params.toml` schedules `alphaTime` as a
render-relative offset (`"+45m"`), which both keeps the upgrade window
safe and lets phase 11 exercise the production sequence "upgrade the
fleet, then Alpha activates". Beta (`betaBlock = 100`, active while
v1.7.5 still runs) is safe because neither binary attaches semantics to
it; `gammaBlock` follows rolling_upgrade's rule of activating only after
the whole fleet is upgraded.

## Test flow

1. Cluster (node1–node4 genesis, node5 validator, vfn1) starts on the
   **old** binary; a guard asserts the deployed binaries differ from the
   upgrade target.
2. History on v1.7.5: two faucet transfers, `AnchorTarget` deploy, two
   `set()` storage writes; H1 anchors collected over all six kinds
   (balances/slots at historical blocks, txs, receipts, logs, block
   hashes), positive-controlled, persisted to
   `<output-dir>/storage_v2_upgrade/anchors.json`.
3. Rolling upgrade under continuous tx load (vfn1 first; the sender fails
   over to node1 for that window). Every node: height-gap precheck →
   graceful stop → wait for the real process exit → binary swap
   (hardlink, copy fallback) → start. 120 s between nodes.
4. Anchor replay against **every** node — the core "old data read by the
   new binary" assertion. A mismatch here is a product-bug finding, not a
   case tolerance.
5. Wait until all nodes pass the max hardfork block (`gammaBlock`
   activates only after the whole cluster is upgraded — rolling_upgrade's
   timeline rule), then stabilize + monitor height gaps.
6. Tx stats floors (≥ 100 confirmed, ≥ 50 % success) and a log scan of
   every node's execution logs (see below).
7. H2 offline on the stopped vfn1 (upgraded datadir):
   - `gravity_storage_settings` **MISSING** — v1.7.5 predates the entry
     and only a fresh v2.3.0 `init_genesis` writes it (TC1 asserts
     PRESENT_LEGACY on a fresh datadir); the upgrade path must not write
     it behind our back;
   - no changeset static-file segments and no `.csoff` sidecars;
   - `AccountChangeSets` / `StorageChangeSets` non-empty
     (`db list <TABLE> --count`);
   then restart vfn1 and let it catch up.
8. TC3a: graceful stop → start for every node (rejoin + catch-up + gap
   ceiling each time).
9. TC3b: `kill -9` node3 (via the shared `Node.force_kill()`), restart —
   crash recovery / pipe consistency checks must do real work and the
   node must rejoin.
10. Anchor replay on every node again, log scan (including an unwind-loop
    ceiling), gap check.
    *TC4 extension point:* `db migrate-changesets` + static-file layout
    assertions slot in here as an additional phase.
11. **Alpha activation tail** (conditional — runs when `alphaTime` is
    scheduled within ~25 min at phase start; skipped for absent or
    far-future schedules): wait for the fully upgraded chain to cross
    `alphaTime`, then assert continued block production and closed height
    gaps, assert the gas-exempt semantic took effect (SYSTEM_CALLER
    balance constant across post-activation blocks — pre-Alpha it
    decreases every block), replay the pre-upgrade anchors once more, and
    collect + replay a fresh round of post-activation anchors
    (`anchors_post_alpha.json`).

All timeouts are case-internal; the case does not use pytest-timeout.

## Getting the v1.7.5 binary

The canonical channel is the GitHub release asset:

```bash
mkdir -p target/quick-release/v1.7.5 && cd target/quick-release/v1.7.5
gh release download v1.7.5 --repo Galxe/gravity-sdk
sha256sum -c gravity_node.sha256 gravity_cli.sha256   # must both pass
chmod +x gravity_node gravity_cli
./gravity_node --version                              # glibc preflight
```

**glibc preflight:** the release assets are built on Ubuntu 24.04 runners
and require glibc ≥ 2.38 (`objdump -T gravity_node | grep GLIBC_2.3` shows
up to `GLIBC_2.39`). On such hosts (CI included) the asset is used as-is.
On older hosts (e.g. Debian 12 / glibc 2.36) `--version` fails with
``version `GLIBC_2.38' not found`` — then build the same tag from source
instead (same flags the tag's own release workflow uses) and drop the
binaries into the same directory:

```bash
git -C /path/to/gravity-sdk worktree add --detach /tmp/gravity-sdk-v175 v1.7.5
cd /tmp/gravity-sdk-v175
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile quick-release \
    --bin gravity_node --bin gravity_cli
cp target/quick-release/{gravity_node,gravity_cli} \
    <this-checkout>/target/quick-release/v1.7.5/
./target/quick-release/v1.7.5/gravity_node --version  # reports build_tag: v1.7.5
git -C /path/to/gravity-sdk worktree remove --force /tmp/gravity-sdk-v175
```

Nothing under `target/` is tracked; never commit binaries.

## How to run

`cluster.toml` and `genesis.toml` are **not tracked** — they are rendered
from the `.tpl` templates + your local `test_params.toml`. Until you
render them, `runner.py` skips this suite (including full runs).

```bash
cd gravity_e2e/cluster_test_cases/storage_v2_upgrade
cp test_params.toml.example test_params.toml
# edit test_params.toml if your v1.7.5 binary lives elsewhere
python render_config.py    # re-render right before running: the relative
                           # alphaTime ("+45m") is anchored to render time

# from the repo root; the upgrade target defaults to
# target/quick-release/gravity_node — override with GRAVITY_NEW_BINARY:
export GRAVITY_NEW_BINARY=/path/to/v2.3.0/gravity_node
python gravity_e2e/runner.py storage_v2_upgrade
```

Expected duration: **~50–65 min** (dominated by 6 × 120 s inter-node
waits, the `gammaBlock=8000` hardfork wait, 5 min health monitoring, the
TC3 restart cycle, and the Alpha activation tail).

### gravity_cli versions

`deploy.sh`/`init.sh`/`genesis.sh` resolve a single `gravity_cli` via
`find_binary` (this checkout's `target/{quick-release,release,debug}/`,
then `$PATH`) and copy it to `<base_dir>/gravity_cli`; the node binary is
the only thing the rolling upgrade swaps. This case follows
rolling_upgrade and does not switch gravity_cli mid-run: cluster tooling
runs whatever `find_binary` resolves (in the storage-v2 worktree that is
the v2.3.0 CLI via the `target/quick-release/gravity_cli` symlink; the
manager's validator/stake subcommands are not exercised by this case).

## Log scan

Case-local (`upgrade_lib.py::scan_log_lines`), over each node's
`logs/debug.log` (stdout/stderr: reth ERROR-level lines and rust panics)
and `execution_logs/` (reth file logs at INFO). Patterns are deliberately
conservative, each tied to a storage-corruption failure family:
`panicked at`, `corrupt*` (RocksDB "Corruption:"), `failed to decode` /
`decode error` / `DecodeError` (reth-db value decode), `DatabaseError`.
Generic ERROR lines (consensus timeouts, peer churn) do not match.
`unwind` mentions are counted separately and only asserted against a loop
ceiling (100/file) — bounded unwinding can be legitimate crash recovery
after the TC3 `kill -9`, an unbounded stream means the consistency check
is looping.

## Files

- `cluster.toml.tpl` — 6 nodes, rolling_upgrade topology, dedicated port
  block (rpc 18845–18850, validator 6580+, vfn 6590+, …) and base_dir
  `/tmp/gravity-cluster-storage-v2-upgrade` so a live run cannot collide
  with other suites.
- `genesis.toml.tpl` — rolling_upgrade's genesis with this case's ports
  and `{{HARDFORKS}}` / contracts-pin placeholders (consistency with the
  cluster template is enforced by a unit test).
- `render_config.py` — renders both files; supports render-relative
  timestamp forks (`"+45m"`); pure logic unit-tested.
- `upgrade_lib.py` — pure helpers (upgrade order, log scan, Alpha
  timeline guards); unit-tested in
  `gravity_e2e/tests/unit/test_storage_upgrade_case.py`.
- `test_params.toml.example` — old-binary pin, contracts ref (`main`,
  what a v1.7.5-born chain initializes with), and the fork timeline
  (mainnet posture, Alpha-after-fleet-upgrade rule, gamma timeline math).
- `contracts/` — `AnchorTarget.sol` + artifact (same as TC1; solc 0.8.21).
