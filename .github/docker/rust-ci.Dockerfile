# Gravity SDK CI Runner - Minimal Rust Build Environment
# This image contains only what's needed for building and testing Gravity SDK
#
# Image size: ~1.2GB (vs 30GB+ on default GitHub runner)
# Includes: Ubuntu 22.04 + build tools + clang/llvm + Rust 1.88.0

FROM ubuntu:22.04

LABEL maintainer="Gravity Team"
LABEL description="Minimal CI environment for Gravity SDK"

# Prevent interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install essential build dependencies
# - build-essential, pkg-config: basic build tools
# - clang, llvm: required for RocksDB and native dependencies
# - libudev-dev, libssl-dev: required by Gravity SDK crates
# - git: for cargo git dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    clang \
    llvm \
    libudev-dev \
    libssl-dev \
    git \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Install Rust (version must match rust-toolchain.toml)
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.88.0

# Install Rust toolchain with clippy and rustfmt
# Also install nightly for rustfmt (used by cargo +nightly fmt)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain ${RUST_VERSION} \
    --profile default \
    -c clippy \
    -c rustfmt \
    && rustup toolchain install nightly --component rustfmt \
    && chmod -R a+w $RUSTUP_HOME $CARGO_HOME

# Set RUSTFLAGS for tokio_unstable (required by gravity_node)
ENV RUSTFLAGS="--cfg tokio_unstable"

# Set working directory
WORKDIR /workspace

# Default command
CMD ["/bin/bash"]
