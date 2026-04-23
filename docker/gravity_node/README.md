# gravity_node Docker Deployment

Binary-only container images for running a `gravity_node` validator or VFN.
Build the image once, ship it everywhere, mount configuration and chain data
from the host. Upgrades are a single `docker compose up -d` against a new
image tag — configuration and chain state persist across restarts.

## Contents

| File | Purpose |
|---|---|
| `Dockerfile` | Multi-stage build (`rust:1.93-slim` → `debian:12-slim`). Binary only. Non-root (uid `10001`). `tini` as PID 1. |
| `entrypoint.sh` | Reads `reth_config.json` (same schema as `cluster/templates/reth_config.json.tpl`) and `exec`s `gravity_node node` in the foreground. |
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
- `CARGO_BIN` — binary name; defaults to `gravity_node`.

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
