BINARY ?= gravity_node
FEATURE ?=
MODE ?= release

BIN_DIRS := gravity_node bench kvstore
BIN_PATHS := $(addprefix bin/, $(BIN_DIRS))

ifeq ($(MODE),release)
    CARGO_FLAGS := --release
else ifeq ($(MODE),quick-release)
    CARGO_FLAGS := --profile quick-release
else
    CARGO_FLAGS :=
endif

CARGO_FEATURES := $(if $(FEATURE),--features $(FEATURE),)

.PHONY: all $(BIN_DIRS) clean

all: $(BINARY)

gravity_node:
	RUSTFLAGS="--cfg tokio_unstable" cargo build -p gravity_node $(CARGO_FLAGS) $(CARGO_FEATURES)

bench:
	cargo build -p bench $(CARGO_FLAGS) $(CARGO_FEATURES)

kvstore:
	cargo build -p kvstore $(CARGO_FLAGS) $(CARGO_FEATURES)

clean:
	for dir in $(BIN_PATHS); do \
		(cd $$dir && cargo clean); \
	done