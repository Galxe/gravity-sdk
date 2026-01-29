# Gravity SDK CI Runner - Pre-compiled Rust Build Environment
# This image pre-compiles dependencies to speed up CI builds significantly
#
# Build strategy:
# 1. Install system dependencies
# 2. Copy Cargo.toml files and create dummy sources
# 3. Pre-compile all dependencies (cached in image)
# 4. In CI, real source replaces dummies, only project code compiles

FROM rust:1.88.0-bookworm

LABEL maintainer="Gravity Team"
LABEL description="Pre-compiled CI environment for Gravity SDK"

# Install system build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    llvm \
    libudev-dev \
    libssl-dev \
    pkg-config \
    git \
    && rm -rf /var/lib/apt/lists/*

# Set environment variables
ENV CARGO_TERM_COLOR=always
ENV CARGO_INCREMENTAL=0
ENV RUSTFLAGS="--cfg tokio_unstable"

# Install rustfmt nightly for formatting checks
RUN rustup toolchain install nightly --component rustfmt

# Create workspace directory for pre-compilation
WORKDIR /prebuild

# Copy workspace Cargo files for dependency pre-compilation
COPY Cargo.toml Cargo.lock ./
COPY aptos-core/consensus/Cargo.toml aptos-core/consensus/
COPY aptos-core/consensus/consensus-types/Cargo.toml aptos-core/consensus/consensus-types/
COPY aptos-core/consensus/safety-rules/Cargo.toml aptos-core/consensus/safety-rules/
COPY aptos-core/mempool/Cargo.toml aptos-core/mempool/
COPY bin/bench/Cargo.toml bin/bench/
COPY bin/gravity_cli/Cargo.toml bin/gravity_cli/
COPY bin/gravity_node/Cargo.toml bin/gravity_node/
COPY crates/api/Cargo.toml crates/api/
COPY crates/block-buffer-manager/Cargo.toml crates/block-buffer-manager/
COPY crates/build-info/Cargo.toml crates/build-info/
COPY crates/gravity-sdk/Cargo.toml crates/gravity-sdk/
COPY crates/txn_metrics/Cargo.toml crates/txn_metrics/
COPY dependencies/aptos-executor/Cargo.toml dependencies/aptos-executor/
COPY dependencies/aptos-executor-types/Cargo.toml dependencies/aptos-executor-types/
COPY external/gravity_chain_core_contracts/genesis-tool/Cargo.toml external/gravity_chain_core_contracts/genesis-tool/

# Create dummy source files to allow cargo to resolve and compile dependencies
RUN mkdir -p aptos-core/consensus/src && echo "pub fn _dummy() {}" > aptos-core/consensus/src/lib.rs && \
    mkdir -p aptos-core/consensus/consensus-types/src && echo "pub fn _dummy() {}" > aptos-core/consensus/consensus-types/src/lib.rs && \
    mkdir -p aptos-core/consensus/safety-rules/src && echo "pub fn _dummy() {}" > aptos-core/consensus/safety-rules/src/lib.rs && \
    mkdir -p aptos-core/mempool/src && echo "pub fn _dummy() {}" > aptos-core/mempool/src/lib.rs && \
    mkdir -p bin/bench/src && echo "fn main() {}" > bin/bench/src/main.rs && \
    mkdir -p bin/gravity_cli/src && echo "fn main() {}" > bin/gravity_cli/src/main.rs && \
    mkdir -p bin/gravity_node/src && echo "fn main() {}" > bin/gravity_node/src/main.rs && \
    mkdir -p crates/api/src && echo "pub fn _dummy() {}" > crates/api/src/lib.rs && \
    mkdir -p crates/block-buffer-manager/src && echo "pub fn _dummy() {}" > crates/block-buffer-manager/src/lib.rs && \
    mkdir -p crates/build-info/src && echo "pub fn _dummy() {}" > crates/build-info/src/lib.rs && \
    mkdir -p crates/gravity-sdk/src && echo "pub fn _dummy() {}" > crates/gravity-sdk/src/lib.rs && \
    mkdir -p crates/txn_metrics/src && echo "pub fn _dummy() {}" > crates/txn_metrics/src/lib.rs && \
    mkdir -p dependencies/aptos-executor/src && echo "pub fn _dummy() {}" > dependencies/aptos-executor/src/lib.rs && \
    mkdir -p dependencies/aptos-executor-types/src && echo "pub fn _dummy() {}" > dependencies/aptos-executor-types/src/lib.rs && \
    mkdir -p external/gravity_chain_core_contracts/genesis-tool/src && echo "fn main() {}" > external/gravity_chain_core_contracts/genesis-tool/src/main.rs

# Fetch all dependencies
RUN cargo fetch

# Pre-compile dependencies in debug mode (for tests)
# The build will "fail" on our dummy files but dependencies get compiled and cached
RUN cargo build --tests 2>/dev/null || true

# Move compiled dependencies to a known location
# These will be copied to the actual workspace in CI
RUN mkdir -p /cargo-cache && \
    cp -r /usr/local/cargo/registry /cargo-cache/ && \
    cp -r /usr/local/cargo/git /cargo-cache/ && \
    cp -r target /cargo-cache/

# Clean up prebuild directory
RUN rm -rf /prebuild/*

# Set working directory for CI
WORKDIR /github/workspace

# Default command
CMD ["/bin/bash"]
