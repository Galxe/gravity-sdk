# syntax=docker/dockerfile:1.7

# ─── Stage 1: builder ────────────────────────────────────────────────
FROM rust:1.93-slim-bookworm AS builder

ARG CARGO_PROFILE=release
ARG CARGO_BIN=gravity_node

RUN apt-get update && apt-get install -y --no-install-recommends \
        clang \
        llvm \
        libclang-dev \
        build-essential \
        pkg-config \
        libssl-dev \
        libudev-dev \
        libzstd-dev \
        cmake \
        git \
        ca-certificates \
        protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

ENV RUSTFLAGS="--cfg tokio_unstable"
RUN --mount=type=cache,target=/build/target,id=gravity-target \
    --mount=type=cache,target=/usr/local/cargo/registry,id=gravity-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=gravity-cargo-git \
    cargo build --bin "${CARGO_BIN}" --profile "${CARGO_PROFILE}" \
    && mkdir -p /out \
    && cp "target/${CARGO_PROFILE}/${CARGO_BIN}" /out/gravity_node

# ─── Stage 2: runtime ────────────────────────────────────────────────
FROM debian:12-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        libudev1 \
        libzstd1 \
        jq \
        tini \
        curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 gravity \
    && useradd  --system --uid 10001 --gid gravity --home-dir /gravity --shell /usr/sbin/nologin gravity \
    && mkdir -p /gravity/config /gravity/data /gravity/logs \
    && chown -R gravity:gravity /gravity

COPY --from=builder /out/gravity_node /usr/local/bin/gravity_node
COPY docker/gravity_node/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/gravity_node /usr/local/bin/entrypoint.sh

USER gravity
WORKDIR /gravity

ENV GRAVITY_CONFIG_DIR=/gravity/config \
    GRAVITY_DATA_DIR=/gravity/data \
    RUST_BACKTRACE=1

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
CMD ["node"]
