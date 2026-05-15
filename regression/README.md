# Regression Suite

Performance & stability regression scenarios for the Gravity node. Each
subdirectory is one scenario — its own cluster topology, stress shape,
and pass/fail criteria. Distinct from `docker/`, which only holds
production image recipes.

| Scenario | What it covers |
|----------|----------------|
| [`pfn_chain_stress/`](pfn_chain_stress/) | Sustained-TPS / stability bench. Supports 5-node chain or 3-node simple topology, single or parallel clusters, optional cpuset isolation — all through one `run.sh`. |

## Adding a scenario

Drop a new subdirectory with at minimum:

- `run.sh` — one-click entry: build → render → up → exercise → measure
- `stop.sh` — tear down
- `README.md` — topology, baselines, caveats

Reuse `docker/gravity_node/Dockerfile` with `--target runtime-host-binary`
for the node image — no good reason to in-container-compile for a regression
test that needs fast iteration.
