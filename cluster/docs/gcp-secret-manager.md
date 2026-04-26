# Deploying with GCP Secret Manager Identity

This runbook walks through deploying a `gravity_node` whose `IdentityBlob`
(consensus / network / account private keys) is **fetched from Google
Cloud Secret Manager at startup** instead of being read from a local
file.

End-to-end validated on a single-validator GCE VM
(`gravity-testnet-node-oregon-1`, project `gravity-infrastructure`).
The validated lifecycle has private keys pass through process RAM only —
operator workstation generates them straight into Secret Manager, and
the runtime VM fetches them straight into `gravity_node`'s heap. No
disk on either side ever holds an `identity.yaml`.

---

## 1. When to use Secret Manager

Set `identity = { source = "gcp_secret", secret = "..." }` on a per-node
basis when:

- Private keys must not live on the node's disk (compromised filesystem
  snapshot, container image leak, or backup tape no longer hands the
  attacker a usable validator key).
- IAM should be the single point of grant/revoke for who can read the
  key.
- You want to rotate keys by adding a new version without redeploying
  binaries.

Otherwise leave `source = "file"` (or omit `identity` entirely). Mode is
per-node, so a cluster can mix.

---

## 2. Workflow at a glance

```
admin workstation                                    runtime GCE VM
(human, secretmanager.admin)                         (SA: secretAccessor)
─────────────────────────────                        ────────────────────
                                                       
make init                                              make deploy
  per cluster.toml node:                                 ├─ file:        cp identity.yaml → data_dir
  ├─ source = file                                       └─ gcp_secret:  validator.yaml renders from_gcp_secret
  │   gravity_cli generate-key                                            (no identity.yaml on data_dir)
  │     --output-file        → identity.yaml         make start
  │     --public-output-file → identity.public.yaml    └─ gravity_node:
  │                                                       reads validator.yaml
  └─ source = gcp_secret                                  fetches identity (file or SM)
      gravity_cli generate-key                            seeds consensus + networks
        --secret <resource>  → SM HTTPS POST :addVersion
        --public-output-file → identity.public.yaml
                                                       
make genesis (only when YOU produce genesis.json)
  aggregate_genesis.py reads identity.public.yaml     (genesis.json + waypoint.txt
  for every validator → validator_genesis.json         shipped from admin to VM
  → forge → genesis.json → waypoint.txt                under cluster/output/)
```

Two operational modes diverge only in **whether `make genesis` runs**:

- **Mode A — Join an existing chain.** Chain operator hands you
  `genesis.json` + `waypoint.txt`. You skip `make genesis` entirely.
- **Mode B — Bootstrap a new chain.** You ARE the chain operator. You
  run `make genesis` to construct the validator set from the public-key
  sidecars produced by `make init`.

The runtime VM behavior is identical in both modes. The init step
behaves identically too — the `identity.source` per-node setting is the
only knob.

---

## 3. Architecture & trust boundaries

**Where the private key lives over its lifetime (`source = gcp_secret`):**

| Stage | Location | Who can read |
|-------|----------|--------------|
| Generation | `gravity_cli` process RAM | Process only |
| Upload | TLS in transit to `secretmanager.googleapis.com` | Nobody (TLS terminated by Google) |
| At rest | Secret Manager (Google-managed encryption) | IAM principals on the resource |
| In transit to node | TLS from Google to GCE VM | Nobody |
| In node memory | `gravity_node` process heap | OS user running gravity_node |
| **Never** | Anyone's disk — admin workstation, runtime VM, or backup | — |

(With `source = file`, the private blob does live at
`output/<id>/config/identity.yaml` on the operator workstation between
`make init` and `make deploy`, then under the runtime data_dir. Use that
mode when the security trade-off doesn't justify the SM dependency.)

---

## 4. Prerequisites

### 4.1 Admin workstation

| Tool | Why | Verified version |
|------|-----|------------------|
| `gcloud` (logged in as a human with `secretmanager.admin`) | Auth + Secret Manager admin | any recent |
| `gravity_cli` (built with `--secret` + `--public-output-file` support) | Generate identity | `feat/identity-from-gcp-secret-manager` or later |

For Mode B *additionally*:

| Tool | Why |
|------|-----|
| Rust toolchain matching `rust-toolchain.toml` | Builds `gravity_cli` |
| `forge` (Foundry) ≥ 1.5 | Compiles genesis contracts |
| `node` + `npm` | Installs contract JS deps |
| `python3-toml` (or Python ≥ 3.11 with `tomllib`) | TOML parsing |
| `jq`, `envsubst` (gettext), `git`, `curl` | Cluster scripts |

Build `gravity_cli`:

```bash
RUSTFLAGS='--cfg tokio_unstable' cargo build -p gravity_cli --profile=quick-release
```

`gravity_cli` is built with `openssl = { features = ["vendored"] }`, so
the resulting binary has no `libssl.so.*` runtime dependency and ships
as a single self-contained artifact across Debian 11 (libssl1.1) and
Debian 12+ (libssl3) hosts. If your build host's glibc is newer than
the runtime VM's (e.g. Debian 12 build → Debian 11 VM), build inside a
Debian 11 container:

```bash
docker run --rm \
  -v $(pwd):/work -v $HOME/.cargo/registry:/root/.cargo/registry \
  -w /work rust:1.93-slim-bullseye bash -c '
    apt-get update -qq && apt-get install -y -qq \
      pkg-config libssl-dev libclang-dev cmake build-essential \
      protobuf-compiler perl libudev-dev
    RUSTFLAGS="--cfg tokio_unstable" cargo build -p gravity_cli --profile=quick-release
  '
```

### 4.2 Runtime VM

| Item | Value |
|------|-------|
| OS | Debian 11+ |
| `gravity_node` binary | Built with `--features gcp-secret-manager` |
| Service account attached | Yes — verify via `gcloud compute instances describe <vm> --format='value(serviceAccounts)'` |
| SA scopes | Includes `cloud-platform` |
| SA IAM | `roles/secretmanager.secretAccessor` on **the specific secret** (not project-wide) |
| Outbound HTTPS to `secretmanager.googleapis.com` | Required |

Verify on the VM, before deploy:

```bash
# Token from metadata server
curl -sS -H "Metadata-Flavor: Google" \
  http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token \
  | head -c 80; echo

# End-to-end fetch
gcloud secrets versions access latest --secret=<secret-name> --project=<project> | head -3
```

> **No SA attached?** Either attach one (VM stop → `gcloud compute
> instances set-service-account` → start), or fall back to
> `export GCP_ACCESS_TOKEN=$(gcloud auth print-access-token)` before
> launching `gravity_node` (~1h token TTL; useful for local testing).

---

## 5. Build `gravity_node` with the feature

The `gcp-secret-manager` feature is **off by default**:

```bash
RUSTFLAGS='--cfg tokio_unstable' \
cargo build -p gravity_node --features gcp-secret-manager --profile=quick-release
```

Sanity-check the resulting binary contains the GCP code paths:

```bash
strings target/quick-release/gravity_node \
  | grep -E 'from_gcp_secret|secretmanager.googleapis.com'
# Must include: from_gcp_secret, FromGcpSecret, IdentityFromGcpSecret,
#               https://secretmanager.googleapis.com,
#               http://metadata.google.internal/computeMetadata/v1/...
```

Without these the binary will reject `validator.yaml` at startup with
`unknown variant 'from_gcp_secret'`.

---

## 6. Configure `cluster.toml`

Per-node identity source declared inline:

```toml
[[nodes]]
id             = "node1"
role           = "validator"           # or "genesis", "vfn", "pfn"
host           = "127.0.0.1"
source         = { bin_path = "/path/to/gravity_node" }
identity       = { source = "gcp_secret",
                   secret = "projects/<P>/secrets/node1-identity/versions/latest" }
validator_port = 6181                  # `p2p_port` is a deprecated alias
vfn_port       = 6195
rpc_port       = 8552
# ...other ports per cluster.toml.example role-port schema...

[genesis_source]
genesis_path  = "./output/genesis.json"
waypoint_path = "./output/waypoint.txt"
```

Notes:

- Resource path uses GCP standard form `projects/<P>/secrets/<S>/versions/<V>`.
  `versions/latest` works for testing but pin to a specific version in
  production — see §10.
- The secret container does **not** need to exist when you write this —
  `make init` will create it on first use.
- Mixing identity sources across nodes is supported. Setting
  `source = "file"` (or omitting `identity`) on a node falls back to
  the on-disk flow with byte-identical rendered output.

---

## 7. Mode A — Join an existing chain (recommended)

Chain operator handed you `genesis.json` + `waypoint.txt`; either your
validator pubkey was pre-registered in genesis or the chain supports
post-genesis registration via staking tx.

```bash
# ─── Admin workstation (one-time) ─────────────────────────────
export GCP_ACCESS_TOKEN=$(gcloud auth print-access-token)

# Generate keypair → directly into Secret Manager (zero disk persistence).
# Sidecar with public-only fields is also written, useful for
# downstream registration; safe to ignore / delete if unneeded.
mkdir -p output/node1/config
gravity_cli genesis generate-key \
  --secret              projects/<P>/secrets/node1-identity/versions/latest \
  --public-output-file  output/node1/config/identity.public.yaml
# stdout prints account_address, consensus_public_key, consensus_pop,
# network_public_key — share with chain operator if pre-registration is
# required, or feed into a staking tx.

# Grant runtime SA access to the secret.
gcloud secrets add-iam-policy-binding node1-identity \
  --project=<P> \
  --member=serviceAccount:gravity-val-0@<P>.iam.gserviceaccount.com \
  --role=roles/secretmanager.secretAccessor

# ─── Runtime VM ───────────────────────────────────────────────
mkdir -p cluster/output
curl -o cluster/output/genesis.json   <release-url>/genesis.json
curl -o cluster/output/waypoint.txt   <release-url>/waypoint.txt
# (cluster.toml already configured per §6 with the secret resource)

make deploy   # renders validator.yaml with from_gcp_secret variant
              # copies genesis.json + waypoint.txt into runtime data_dir
              # does NOT write identity.yaml
make start
```

`make init` and `make genesis` are not invoked. `cluster/output/`
contains only the two external public artifacts.

What the runtime does on `make start`:

1. Reads `validator.yaml` → sees `from_gcp_secret` for safety_rules and
   each network's identity.
2. Tries `GCP_ACCESS_TOKEN` env first, otherwise hits the GCE metadata
   server with `Metadata-Flavor: Google`.
3. With the token, `GET https://secretmanager.googleapis.com/v1/<resource>:access`.
4. base64-decodes the payload, parses as `IdentityBlob`, hands it to
   safety_rules and to each network's identity loader.

---

## 8. Mode B — Bootstrap a new chain

You're producing `genesis.json` + `waypoint.txt` for a fresh chain. The
new flow handles file-source and gcp_secret-source nodes uniformly: a
single `make init` dispatches per `cluster.toml` and always writes a
`identity.public.yaml` sidecar that `make genesis` consumes.

```bash
# Admin workstation
export GCP_ACCESS_TOKEN=$(gcloud auth print-access-token)   # needed iff any node uses gcp_secret

# 1. Generate per-node keys; private storage is per-source.
make init
#    For file-source nodes:
#      output/<id>/config/identity.yaml         (private)
#      output/<id>/config/identity.public.yaml  (public sidecar)
#    For gcp_secret-source nodes:
#      output/<id>/config/identity.public.yaml  (only file written)
#      private blob → projects/<P>/secrets/<S>/versions/<N>

# 2. Aggregate public sidecars into genesis (forge + node required).
make genesis

# 3. Grant runtime SAs access to their respective secrets (one per node
#    that uses gcp_secret).
gcloud secrets add-iam-policy-binding <secret-name> \
  --project=<P> \
  --member=serviceAccount:<vm-sa>@<P>.iam.gserviceaccount.com \
  --role=roles/secretmanager.secretAccessor
```

Then ship `output/genesis.json` + `output/waypoint.txt` (and
`cluster.toml`, gravity_node binary) to each runtime VM and run
`make deploy && make start`.

Result: the only persistent home for any node's private key is
either the operator's `output/<id>/config/identity.yaml` (file mode) or
Secret Manager (gcp_secret mode). The public sidecar is harmless to
distribute or commit.

If you want zero plaintext on the operator workstation even for
file-source nodes, after `make deploy` ships those identities to the
runtime VMs:

```bash
shred -u output/*/config/identity.yaml
```

`identity.public.yaml` files can stay; they're public.

---

## 9. The rendered `validator.yaml`

For comparison — `make deploy` renders one of two variants per node:

```yaml
# from_file (default — identity.yaml must be on disk)
consensus:
  safety_rules:
    initial_safety_rules_config:
      from_file:
        identity_blob_path: <data_dir>/<id>/config/identity.yaml
validator_network:
  identity:
    type: "from_file"
    path: <data_dir>/<id>/config/identity.yaml
```

```yaml
# from_gcp_secret
consensus:
  safety_rules:
    initial_safety_rules_config:
      from_gcp_secret:
        identity_blob_secret: projects/<P>/secrets/<S>/versions/<V>
validator_network:
  identity:
    type: "from_gcp_secret"
    resource: projects/<P>/secrets/<S>/versions/<V>
```

Per-process cache: the runtime fetches the IdentityBlob once per
distinct resource and caches it for the lifetime of the process —
startup is at most one token fetch + N secret accesses (where N = the
number of distinct resources in `validator.yaml`), not N-per-network.

---

## 10. Verification

### 10.1 Process is up

```bash
ps -fu <runtime-user> | grep gravity_node
```

### 10.2 Zero `identity.yaml` on disk for gcp_secret nodes

```bash
find <data_dir> -name 'identity.yaml' | wc -l
# expected: 0 for any gcp_secret node
```

### 10.3 Consensus log shows the expected validator

```bash
grep -m1 'validator: ValidatorSet' <data_dir>/<id>/consensus_log/validator.log
# the address must match `account_address` in the secret payload —
# i.e. `gcloud secrets versions access latest --secret=<S>` shows the
# same first line. It also matches output/<id>/config/identity.public.yaml.
```

### 10.4 Blocks are advancing

```bash
NODE_RPC=http://127.0.0.1:8552
for i in 1 2 3; do
  curl -sS -X POST -H 'Content-Type: application/json' \
    --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    $NODE_RPC; echo
  sleep 4
done
```

Three strictly-increasing responses → consensus is producing blocks
signed with the correctly-sourced key.

### 10.5 If startup fails

| Symptom | Cause |
|---------|-------|
| `unknown variant 'from_gcp_secret'` | gravity_node not built with `--features gcp-secret-manager`. |
| `GCE metadata server unreachable (...) — set GCP_ACCESS_TOKEN for non-GCE environments` | No SA attached **and** `GCP_ACCESS_TOKEN` env not set. Attach an SA, or set the env. |
| `Secret Manager access failed (403)` / `Permission 'secretmanager.versions.access' denied` | SA lacks `roles/secretmanager.secretAccessor` on this specific secret. |
| `Secret Manager access failed (404)` for the resource | Resource path typo; check `cluster.toml` against `gcloud secrets list --project=<P>`. |
| `decode secret payload base64` | Secret payload is not raw YAML bytes (e.g. someone uploaded JSON-wrapped content via the API directly). Re-upload by re-running `make init` (delete `output/<id>/config/identity.public.yaml` first to bypass idempotency), or call `gravity_cli genesis generate-key --secret ...` directly. |
| `Public sidecar not found: …/identity.public.yaml` (during `make genesis`) | Re-run `make init`. Older clusters predating the sidecar split: regenerate via init or run `gravity_cli genesis generate-key --public-output-file …` against existing keys. |

---

## 11. Rotation

> **Don't leave `versions/latest` in production `cluster.toml`.** It
> works, but it makes rotation a side effect of the next restart instead
> of an explicit cluster.toml change. Pin to a specific version once you
> know it.

To rotate a `gcp_secret` validator:

```bash
# Admin workstation — generate fresh keypair, push as new version
export GCP_ACCESS_TOKEN=$(gcloud auth print-access-token)
gravity_cli genesis generate-key \
  --secret             projects/<P>/secrets/<S>/versions/latest \
  --public-output-file output/<id>/config/identity.public.yaml
# stdout: "Uploaded as projects/.../versions/N+1"
# (the version segment in --secret is ignored; addVersion always
#  creates a new version)

# Update cluster.toml to pin versions/<N+1>, then:
ssh <vm> 'cd cluster && make deploy && make restart'
```

Old versions remain accessible until explicitly destroyed. Once the
node is healthy on `<N+1>`:

```bash
gcloud secrets versions destroy <N> --secret=<S> --project=<P>
```

---

## 12. Cleanup

For a test cluster:

```bash
# Runtime VM
cd cluster && make stop && make clean

# Admin workstation — purge the test secret
gcloud secrets delete <secret-name> --project=<P> --quiet
```

---

## 13. Reference: end-to-end commands from the validated test run

Mode B (validated on `gravity-testnet-node-oregon-1`, single
`gcp_secret`-source validator):

```bash
# ─── Admin workstation ─────────────────────────────────────────
export GCP_ACCESS_TOKEN=$(gcloud auth print-access-token)
make init
# → keypair generated in RAM → projects/.../secrets/.../versions/1
# → output/node1/config/identity.public.yaml written
make genesis
# → reads identity.public.yaml only; produces output/genesis.json + waypoint.txt
# Validator address:    6a14b60560935f201d8a1ab3a424e8f1bc0dd33fc80641b8a04b128cfd762938

gcloud secrets add-iam-policy-binding <secret-name> \
  --project=gravity-infrastructure \
  --member=serviceAccount:gravity-val-0@gravity-infrastructure.iam.gserviceaccount.com \
  --role=roles/secretmanager.secretAccessor

# Ship genesis.json + waypoint.txt + cluster.toml + binary to VM
gcloud compute scp ... gravity-testnet-node-oregon-1:~/gravity-test-gcp/...

# ─── Runtime VM ────────────────────────────────────────────────
ssh ... 'cd ~/gravity-test-gcp/cluster && make deploy && make start'

# Verify
curl -sS -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://127.0.0.1:8552
# {"jsonrpc":"2.0","id":1,"result":"0x47"}    ← block number advancing

# Disk audit on VM
ssh ... 'find ~/gravity-test-gcp -name identity.yaml | wc -l'
# 0
```
