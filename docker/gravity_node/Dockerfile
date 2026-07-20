# syntax=docker/dockerfile:1.7
#
# Two build targets:
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
# Stage pruning means `--target runtime-host-binary` does not enter the
# `builder` stage — the cargo build is genuinely skipped, not just cached.

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

# ─── Stage 3b: runtime (default target) — copy from in-container build ─
# Put last so `docker build .` (no --target) picks this and the
# from-source build is the default behaviour, matching prior versions.
FROM runtime-base AS runtime

COPY --from=builder --chmod=755 /out/gravity_node /usr/local/bin/gravity_node
COPY --from=builder --chmod=755 /out/gravity_cli /usr/local/bin/gravity_cli

USER gravity
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD ["node"]
