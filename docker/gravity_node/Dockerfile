# syntax=docker/dockerfile:1.7
#
# Runtime build targets:
#
#   1. `runtime` (default) — compile gravity_node and gravity_cli from source
#      inside the container's `builder` stage:
#        docker build -t gravity_node:src \
#                     -f docker/gravity_node/Dockerfile .
#
#   2. `runtime-host-binary` — skip the cargo build; copy pre-built
#      binaries from the host build context. Used by the release pipeline so
#      we don't recompile after the runner has already produced a native
#      binaries, and by regression/pfn_chain_stress for the same reason:
#        docker build --target runtime-host-binary \
#                     --build-arg HOST_BINARY=target/quick-release/gravity_node \
#                     --build-arg HOST_CLI_BINARY=target/quick-release/gravity_cli \
#                     -t gravity_node:rel \
#                     -f docker/gravity_node/Dockerfile .
#
# The opt-in `runtime-storage-test` and
# `runtime-host-binary-storage-test` targets add the destructive BFT storage
# fixture and its child-process supervisor. They are never selected by the
# default or release build.
#
# The opt-in `runtime-byzantine-test` and
# `runtime-host-binary-byzantine-test` targets add the protocol-aware
# equivocation hook. The source target compiles gravity_node with the
# `byzantine-test` feature; neither target is used by default or release builds.
#
# Stage pruning means host-binary targets do not enter the `builder` stage;
# the cargo build is genuinely skipped, not just cached.

# ─── Stage 1: builder (used only by the `runtime` target) ────────────
FROM rust:1.93-slim-bookworm AS builder

ARG CARGO_PROFILE=release

RUN apt-get update && apt-get install -y --no-install-recommends \
        clang \
        llvm \
        libclang-dev \
        build-essential \
        pkg-config \
        libudev-dev \
        libzstd-dev \
        cmake \
        git \
        ca-certificates \
        protobuf-compiler \
        perl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

ENV RUSTFLAGS="--cfg tokio_unstable"
RUN --mount=type=cache,target=/build/target,id=gravity-target \
    --mount=type=cache,target=/usr/local/cargo/registry,id=gravity-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=gravity-cargo-git \
    cargo build --bin gravity_node --bin gravity_cli --profile "${CARGO_PROFILE}" \
    && mkdir -p /out \
    && cp "target/${CARGO_PROFILE}/gravity_node" /out/gravity_node \
    && cp "target/${CARGO_PROFILE}/gravity_cli" /out/gravity_cli

# Reuse the normal build cache, then replace only gravity_node with a binary
# that contains the test-only Byzantine control path.
FROM builder AS builder-byzantine-test

ARG CARGO_PROFILE=release

RUN --mount=type=cache,target=/build/target,id=gravity-target \
    --mount=type=cache,target=/usr/local/cargo/registry,id=gravity-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=gravity-cargo-git \
    cargo build --bin gravity_node --profile "${CARGO_PROFILE}" \
        --features byzantine-test \
    && cp "target/${CARGO_PROFILE}/gravity_node" /out/gravity_node

# ─── Stage 2: runtime-base (shared apt + user setup + entrypoint) ────
# Common to both leaf targets. The binaries are added in the leaves
# so we can pick the source (builder vs build context) per target.
FROM ubuntu:24.04 AS runtime-base

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        jq \
        tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 gravity \
    && useradd  --system --uid 10001 --gid gravity --home-dir /gravity --shell /usr/sbin/nologin gravity \
    && mkdir -p /gravity/config /gravity/data /gravity/logs \
    && chown -R gravity:gravity /gravity

COPY docker/gravity_node/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

WORKDIR /gravity

ENV GRAVITY_CONFIG_DIR=/gravity/config \
    GRAVITY_DATA_DIR=/gravity/data \
    RUST_BACKTRACE=1

# Note: USER and ENTRYPOINT/CMD are set in each leaf so that the binary
# COPY can happen as root (default user in runtime-base) before dropping
# to `gravity`.

# ─── Stage 3a: runtime-host-binary — copy pre-built host binaries ───
# Used by the release pipeline and regression/pfn_chain_stress.
FROM runtime-base AS runtime-host-binary

ARG HOST_BINARY=docker/gravity_node/bin/gravity_node
ARG HOST_CLI_BINARY=docker/gravity_node/bin/gravity_cli
COPY --chmod=755 ${HOST_BINARY} /usr/local/bin/gravity_node
COPY --chmod=755 ${HOST_CLI_BINARY} /usr/local/bin/gravity_cli

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD ["node"]

# Opt-in destructive storage fixture. This target is never selected by a
# default or release build and must be paired with an explicitly disposable
# data volume at runtime.
FROM runtime-host-binary AS runtime-host-binary-storage-test

USER root
COPY docker/gravity_node/bft-node-supervisor.sh /usr/local/bin/bft-node-supervisor
COPY docker/gravity_node/bft-storage /usr/local/bin/bft-storage
RUN chmod 755 /usr/local/bin/bft-node-supervisor /usr/local/bin/bft-storage \
    && mkdir -p /run/bft-node \
    && chown gravity:gravity /run/bft-node

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/bft-node-supervisor"]
CMD ["node"]

# Opt-in protocol-aware Byzantine fixture for a host-built binary. The caller
# must supply a gravity_node compiled with `--features byzantine-test`.
FROM runtime-host-binary AS runtime-host-binary-byzantine-test

USER root
COPY docker/gravity_node/bft-byzantine /usr/local/bin/bft-byzantine
RUN chmod 755 /usr/local/bin/bft-byzantine \
    && mkdir -p /run/bft-node \
    && chown gravity:gravity /run/bft-node

ENV BFT_BYZANTINE_CONTROL_PATH=/run/bft-node/byzantine-control.json \
    BFT_BYZANTINE_EVIDENCE_PATH=/run/bft-node/byzantine-evidence.json

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD ["node"]

# ─── Stage 3b: source-runtime-base — copy from in-container build ───
FROM runtime-base AS source-runtime-base

COPY --from=builder --chmod=755 /out/gravity_node /usr/local/bin/gravity_node
COPY --from=builder --chmod=755 /out/gravity_cli /usr/local/bin/gravity_cli

# Source-built equivalent of the host-binary storage-test target.
FROM source-runtime-base AS runtime-storage-test

COPY docker/gravity_node/bft-node-supervisor.sh /usr/local/bin/bft-node-supervisor
COPY docker/gravity_node/bft-storage /usr/local/bin/bft-storage
RUN chmod 755 /usr/local/bin/bft-node-supervisor /usr/local/bin/bft-storage \
    && mkdir -p /run/bft-node \
    && chown gravity:gravity /run/bft-node

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/bft-node-supervisor"]
CMD ["node"]

# Source-built protocol-aware Byzantine fixture. The binary comes from the
# dedicated feature-enabled builder stage, not from the normal runtime stage.
FROM runtime-base AS runtime-byzantine-test

COPY --from=builder-byzantine-test --chmod=755 /out/gravity_node /usr/local/bin/gravity_node
COPY --from=builder-byzantine-test --chmod=755 /out/gravity_cli /usr/local/bin/gravity_cli
COPY docker/gravity_node/bft-byzantine /usr/local/bin/bft-byzantine
RUN chmod 755 /usr/local/bin/bft-byzantine \
    && mkdir -p /run/bft-node \
    && chown gravity:gravity /run/bft-node

ENV BFT_BYZANTINE_CONTROL_PATH=/run/bft-node/byzantine-control.json \
    BFT_BYZANTINE_EVIDENCE_PATH=/run/bft-node/byzantine-evidence.json

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD ["node"]

# ─── Stage 3c: runtime (default target) ─────────────────────────────
# Put last so `docker build .` (no --target) picks this and the
# from-source build remains the default behaviour.
FROM source-runtime-base AS runtime

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD ["node"]
