# gravity_node Docker Deployment

Container images containing `gravity_node`, `gravity_cli`, and `curl` for
running a validator or VFN, managing node identity during container startup,
and executing container health checks.
Build the image once, ship it everywhere, mount configuration and chain data
from the host. Upgrades are a single `docker compose up -d` against a new
image tag — configuration and chain state persist across restarts.

## Contents

| File | Purpose |
|---|---|
| `Dockerfile` | Multi-stage build (`rust:1.93-slim` → `ubuntu:24.04`). Includes normal runtime targets plus explicit storage and Byzantine test targets. Non-root (uid `10001`). `tini` as PID 1. |
| `entrypoint.sh` | Reads `reth_config.json` (same schema as `cluster/templates/reth_config.json.tpl`) and `exec`s `gravity_node node` in the foreground. |
| `bft-node-supervisor.sh` | Test-target-only child-process supervisor that keeps the container available while a corrupted node process is stopped. |
| `bft-storage` | Test-target-only WAL/database backup, truncation, evidence, and restoration hook for bft-jepsen. |
| `bft-byzantine` | Test-target-only protocol-aware equivocation control and evidence hook for bft-jepsen. |
| `test-storage-fixture.sh` | Isolated image-level acceptance for storage corruption, container restart, exact restoration, and recovery. |
| `test-byzantine-fixture.sh` | Isolated image-level acceptance for Byzantine hook authorization, capabilities, evidence, and recovery. |
| `docker-compose.yaml` | Single-node deployment. Intended for one host running one validator. |
| `docker-compose.cluster.yaml` | 4 validators + 1 VFN on one host. For end-to-end image verification against `cluster/` artifacts. |
| `render-cluster-config.sh` | Renders the 5-node config set from `cluster/output/` + `cluster/templates/`. |
| `.env.example` | Operator-tunable knobs (image tag, log level). Copy to `.env`. |
| `.gitignore` | Prevents `config/` (contains private keys) and `.env` from being committed. |

## Prerequisites

- Docker ≥ 20.10 with BuildKit (`docker buildx`) and Compose v2.
- Either `net.ipv4.ip_forward=1` on the host, **or** pass `--network=host` to
  `docker build` so the builder can resolve DNS and reach package mirrors.
- Roughly 20 GB free on the Docker root filesystem for build cache.

## Build

From the repository root:

```bash
DOCKER_BUILDKIT=1 docker build --network=host \
    -f docker/gravity_node/Dockerfile \
    -t gravity_node:$(git rev-parse --short HEAD) \
    --build-arg CARGO_PROFILE=release \
    .
```

Build arguments:

- `CARGO_PROFILE` — `release` (default) or `performance` (LTO, slower build, faster runtime).

The image always builds and installs both `gravity_node` and `gravity_cli` in
`/usr/local/bin`. It also includes `curl` for container health checks.

The `.dockerignore` at the repository root keeps `target/`, `.git/`, and other
large directories out of the build context.

## Devnet topology verification (4 validators + 1 VFN)

Use this flow to validate that the image can reach consensus end-to-end before
promoting a tag.

1. Generate cluster artifacts once:

   ```bash
   cd cluster
   make init && make genesis
   cd ..
   ```

   This writes `cluster/output/genesis.json`, `waypoint.txt`, and per-node
   `identity.yaml` under `cluster/output/node{1..4}/config/` and
   `cluster/output/vfn1/config/`.

2. Render per-node configuration for the containers:

   ```bash
   cd docker/gravity_node
   ./render-cluster-config.sh
   ```

   Output lands in `docker/gravity_node/config/{node1..node4,vfn1}/`. All
   paths in the rendered files point at the container-internal locations
   (`/gravity/config`, `/gravity/data`).

3. Start the topology:

   ```bash
   IMAGE_TAG=<your-tag> docker compose -f docker-compose.cluster.yaml up -d
   ```

   All five services use `network_mode: host`. The `cluster`-generated
   genesis hard-codes validator peer addresses as `127.0.0.1:6180..6183`, so
   NAT + port mapping would break on-chain discovery.

4. Confirm consensus is advancing:

   ```bash
   for p in 8545 8546 8547 8548 8550; do
       printf 'port %s block=' "$p"
       curl -s --noproxy '*' -X POST \
           -H 'Content-Type: application/json' \
           --data '{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}' \
           "http://127.0.0.1:$p" | jq -r .result
   done
   ```

   Each endpoint should return a hex block number that increments across
   successive calls. A skew of one block between the current proposer and
   the other nodes is normal.

5. Tear down:

   ```bash
   docker compose -f docker-compose.cluster.yaml down -v
   ```

   The `-v` flag removes the per-node named volumes along with the
   containers.

Default port range used by this topology: `6180–6183`, `6190–6195`,
`8545–8554`, `8566`, `9001–9006`, `10000–10005`, `12024–12029`, `1024–1029`.
Ensure nothing else on the host — including `cluster`'s host-mode deployment
— is holding these ports before starting.

## Disposable BFT storage-fault image

The normal `runtime` and `runtime-host-binary` images do not contain the
destructive storage hook. Build one of the explicit test targets for a cluster
whose data volumes can be discarded:

```bash
# Build gravity_node from source.
docker build --target runtime-storage-test \
  -t gravity_node:storage-test \
  -f docker/gravity_node/Dockerfile .

# Or package binaries already built on the host.
docker build --target runtime-host-binary-storage-test \
  --build-arg HOST_BINARY=target/quick-release/gravity_node \
  --build-arg HOST_CLI_BINARY=target/quick-release/gravity_cli \
  -t gravity_node:storage-test \
  -f docker/gravity_node/Dockerfile .
```

Each target container must use a fresh disposable data volume and receive all
of these environment variables:

```yaml
environment:
  BFT_STORAGE_FIXTURE_ENABLED: "1"
  BFT_STORAGE_DISPOSABLE_DATA: I_UNDERSTAND_THIS_DATA_WILL_BE_DESTROYED
  BFT_STORAGE_DATA_ROOT: /gravity/data
  BFT_STORAGE_WAL_PATH: /gravity/data/data/consensus_db
  BFT_STORAGE_DATABASE_PATH: /gravity/data/data/reth/db
  BFT_STORAGE_DATABASE_MUTATION_FILE: state/CURRENT
```

For `wal`, the hook selects the newest non-empty `*.log` below the configured
component path. For `database`, the mutation file is relative to the component
path. Before truncation, the stopped component is archived in full and its
SHA-256 is recorded. Healing verifies the archive, replaces the mutated
component, checks the original file size and hash, and requires the node child
process to remain stable. A successful heal removes the large backup archive
but retains the small JSON evidence under `/gravity/data/.bft-storage/states`.
An active fault also leaves `/gravity/data/.bft-storage-active`, so replacing
or restarting the container cannot turn a corrupted child into a restart
storm; the new supervisor waits for `heal` while keeping `docker exec` usable.

The backup is stored on the same disposable volume. The hook refuses injection
unless free space is at least the component size plus 256 MiB; adjust only with
`BFT_STORAGE_BACKUP_RESERVE_MIB`. Never enable this target against an existing
operator or long-running test volume.

The bft-jepsen controller invokes the hook as root through `docker exec`:

```text
/usr/local/bin/bft-storage inject <wal|database> <fault-id> <node-id>
/usr/local/bin/bft-storage heal <fault-id>
/usr/local/bin/bft-storage read <fault-id> <wal|database> <node-id>
```

Run the isolated image-level acceptance test with fake node binaries and a
temporary named volume:

```bash
bash docker/gravity_node/test-storage-fixture.sh
```

## Protocol-aware Byzantine test image

The normal runtime images do not contain the Byzantine hook, and the normal
binary does not compile the conflicting-message path. Build the explicit
source target for a disposable BFT test cluster:

```bash
docker build --target runtime-byzantine-test \
  -t gravity_node:byzantine-test \
  -f docker/gravity_node/Dockerfile .
```

To package host binaries, compile `gravity_node` with the test feature first,
then select the matching host-binary target:

```bash
RUSTFLAGS="--cfg tokio_unstable" \
  cargo build --bin gravity_node --profile quick-release \
    --features byzantine-test

docker build --target runtime-host-binary-byzantine-test \
  --build-arg HOST_BINARY=target/quick-release/gravity_node \
  --build-arg HOST_CLI_BINARY=target/quick-release/gravity_cli \
  -t gravity_node:byzantine-test \
  -f docker/gravity_node/Dockerfile .
```

Each instrumented validator must opt in at runtime:

```yaml
environment:
  BFT_BYZANTINE_FIXTURE_ENABLED: "1"
```

The current Gravity target advertises only `equivocation`. While the selected
validator is proposer, it signs two different proposals for the same
epoch/round and sends them to two non-overlapping validator groups. The node
writes typed protocol evidence under `/run/bft-node`; the hook exposes only
normalized counters to bft-jepsen:

```text
/usr/local/bin/bft-byzantine inject equivocation <fault-id> <node-id>
/usr/local/bin/bft-byzantine read <fault-id>
/usr/local/bin/bft-byzantine heal <fault-id>
```

`double-sign` and `twin` are rejected until they have independent, real
protocol implementations. Run the isolated image/hook contract test with:

```bash
bash docker/gravity_node/test-byzantine-fixture.sh
```

## Single-node deployment

```bash
cd docker/gravity_node
cp .env.example .env                       # set IMAGE_TAG, RUST_LOG
mkdir -p config/
# Place the following files under ./config/:
#   genesis.json
#   waypoint.txt
#   identity.yaml
#   validator.yaml
#   reth_config.json
#   relayer_config.json
docker compose up -d
docker compose logs -f
```

The configuration directory is mounted read-only. Chain data and logs live on
the named volumes `gravity-data` and `gravity-logs`; these survive container
recreation on image upgrade.

## Image distribution without a registry

When no image registry is available, ship the image tarball directly:

```bash
TAG=$(git rev-parse --short HEAD)

# On the build host:
docker save gravity_node:$TAG | gzip > gravity_node-$TAG.tar.gz
scp gravity_node-$TAG.tar.gz <target>:/tmp/

# On the target host:
gunzip -c /tmp/gravity_node-$TAG.tar.gz | docker load
```

## Upgrade flow

1. Build the new tag on the build host.
2. Distribute it (registry push, or `docker save` + `scp` + `docker load`).
3. On each target host:

   ```bash
   cd /path/to/docker/gravity_node
   sed -i 's/^IMAGE_TAG=.*/IMAGE_TAG=<new-tag>/' .env
   docker compose up -d
   ```

Compose recreates the container from the new image. The `config/` mount and
the `gravity-data` / `gravity-logs` volumes are not touched, so chain state
and operator configuration persist.

## Security

- `identity.yaml` contains the node's consensus private key. On production
  hosts, `chown` it to the container uid (`10001`) and `chmod 600`.
- For mainnet validators, integrate `gravity_cli --kms` (see the
  `feat/cli-kms-signer` branch) and remove the on-disk key entirely.
- The container runs as non-root (`10001:10001`). Named volumes inherit this
  ownership; bind-mounted host directories must be readable by uid `10001`.
- `network_mode: host` exposes every port that the node binds. Restrict RPC,
  metrics, and inspection endpoints to the loopback interface or an internal
  VLAN at the host firewall.

## Operational notes

- Container stdout is sparse by design (`reth_config.json` filters stdout to
  errors). Full logs live inside the container:
  - `/gravity/data/execution_logs/dev/reth.log` — execution layer
  - `/gravity/data/consensus_log/validator.log` — consensus layer
  - `/gravity/data/data/` — chain state (RocksDB, reth, quorum store,
    secure storage)
- Container logs managed by Docker are rotated at 100 MB × 10 files
  (`json-file` driver). Adjust in the compose file if your retention policy
  differs.
- The process handles `SIGTERM` for graceful shutdown. `stop_grace_period`
  is set to 60 seconds; increase for larger chain state if shutdown truncates.

## Troubleshooting

- **`curl` returns `502 Bad Gateway`** — an `http_proxy` environment variable
  is intercepting the request. Pass `--noproxy '*'` or `unset http_proxy`.
- **Build fails with DNS errors in `apt-get`** — host has IPv4 forwarding
  disabled. Build with `--network=host`, or enable forwarding on the host.
- **Container restarts immediately** — `docker logs <container>` shows only
  the reth boot preamble. The real error is typically in
  `/gravity/data/execution_logs/dev/reth.log`. `docker exec` into the
  container to read it.
- **Port already in use** — another `gravity_node` process (likely a
  `cluster/` host-mode deployment) is holding the same port range. Stop it
  before starting the container topology.
