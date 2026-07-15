# storage_v2_baseline (storage-v2 TC1)

Fresh-node storage baseline for the storage-v2 track: asserts that a
freshly initialized gravity_node uses the **legacy changeset layout**
(changesets in database tables, none in static files) and persists a
`gravity_storage_settings` Metadata entry saying so, and that a
collected set of historical anchors (balances, storage slots, txs,
receipts, logs, block hashes) replays identically across a graceful
restart. Control group for the later migration cases (TC2+); first
real-binary exercise of `helpers/storage_anchors.py` and
`helpers/offline_db.py`.

> **STATUS: live-verified.** First live run passed on 2026-07-15 against
> the greth v2.3.0 pre-built binary (`1 passed in 23.37s`; settings
> PRESENT_LEGACY, AccountChangeSets=99 / StorageChangeSets=440, no
> changeset segments/sidecars, anchor replay 21/21 matched). That run
> also validated the offline `gravity_node db get/stats/list --count`
> argument surface for the first time and exposed one helper fix along
> the way: raw Metadata values are SCALE-compressed `Vec<u8>`, now
> handled by `helpers/offline_db.py::decode_scale_bytes`.

## Test flow

1. Runner brings up the fresh single-node cluster; block production checked.
2. History: two faucet transfers to a fresh account, `AnchorTarget`
   deploy (`contracts/`, solc 0.8.21, same convention as
   `prague/contracts/`), two `set()` storage writes each emitting an event.
3. H1: collect anchors covering all six kinds (balance/slot sampled at
   historical blocks), positive-control the expected values, persist JSON
   to `<output-dir>/storage_v2_baseline/anchors.json`.
4. Graceful stop (SIGTERM via the node's `stop.sh`).
5. H2 offline (read-only db command smoke; `migrate-changesets` belongs
   to TC4 and is not run):
   - `gravity_storage_settings` present with
     `changesets_in_static_files == false` (fresh init writes the legacy
     settings — *not* MISSING);
   - no changeset segment files and no `.csoff` sidecars on disk;
   - `AccountChangeSets` / `StorageChangeSets` non-empty via
     `db list <TABLE> --count`;
   - `db stats` runs and parses (entry counts logged only — placeholder
     zeros on the RocksDB backend).
6. Restart the node, wait until it serves past the anchored history.
7. H1: replay the anchor set — `report.ok` must hold for every anchor.

All timeouts are case-internal; the case does not use pytest-timeout.

## How to run

The case follows the `rolling_upgrade/` config-rendering pattern:
`cluster.toml` is **not tracked** — it is rendered from
`cluster.toml.tpl` + your local `test_params.toml`. Until you render it,
`runner.py` skips this suite (including full runs of all suites).

```bash
cd gravity_e2e/cluster_test_cases/storage_v2_baseline
cp test_params.toml.example test_params.toml
# edit test_params.toml — see "Choosing the binary" below
python render_config.py

# from the repo root
python gravity_e2e/runner.py storage_v2_baseline
```

## Choosing the binary (no tracked file changes needed)

`cluster/deploy.sh` resolves each node's binary from the `source` field
in cluster.toml (`resolve_source`: `bin_path` | `project_path` |
`github`+`rev`). There is no env-var override for it, so this case pins
the binary through the untracked `test_params.toml` instead — the
rendered `cluster.toml` and your `test_params.toml` are both gitignored
(case-local `.gitignore`).

To run against an external pre-built binary (e.g. a greth v2.3.0
integration build):

```toml
[source]
bin_path = "/home/neko/gravity/gravity-sdk-greth-v2.3.0/target/quick-release/gravity_node"
```

With `project_path = "../"` (the default other suites use) deploy.sh
will cargo-build the workspace if `target/quick-release/gravity_node`
does not exist yet.

The offline step (5) always uses `<base_dir>/node1/bin/gravity_node` —
the hardlink deploy.sh created from whatever source you configured — so
the db commands run the exact binary the node ran.

### gravity_cli prerequisite

Independently of the node binary, `deploy.sh`/`init.sh`/`genesis.sh`
require a `gravity_cli` found via `find_binary`: they look in this
checkout's `target/{quick-release,release,debug}/` and then `$PATH`, and
abort if absent. When running against an external build from a checkout
that has no local `target/`, symlink it in (untracked; `/target` is
gitignored):

```bash
mkdir -p target/quick-release
ln -sf /home/neko/gravity/gravity-sdk-greth-v2.3.0/target/quick-release/gravity_cli \
    target/quick-release/gravity_cli
```

## Files

- `cluster.toml.tpl` — single node, dedicated port block (rpc 18745,
  validator 6480, vfn 6490, ...) and base_dir
  `/tmp/gravity-cluster-storage-v2-baseline`, so a live run cannot
  collide with other suites or a manually started cluster.
- `genesis.toml` — tracked copy of `single_node/genesis.toml` with only
  the validator ports adjusted (consistency with the template is
  enforced by a unit test).
- `render_config.py` — renders `cluster.toml`; pure logic unit-tested.
- `storage_baseline_lib.py` — pure helpers (anchor-spec construction,
  offline-env path derivation, `set(uint256)` calldata); unit-tested in
  `gravity_e2e/tests/unit/test_storage_baseline_case.py`.
- `contracts/` — `AnchorTarget.sol` + compiled artifact (solc 0.8.21,
  `--optimize --combined-json abi,bin`).
