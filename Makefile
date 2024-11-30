BINARY ?= reth
FEATURE ?= grevm
MODE ?= release

BIN_DIRS := reth bench kvstore
BIN_PATHS := $(addprefix bin/, $(BIN_DIRS))

ifeq ($(MODE),release)
    CARGO_FLAGS := --release
else
    CARGO_FLAGS :=
endif

ifeq ($(FEATURE),grevm)
    CARGO_FEATURES := --no-default-features --features grevm
else ifeq ($(FEATURE),preth)
    CARGO_FEATURES := --features preth
else
    $(error Invalid FEATURE selected. Use "preth" or "grevm")
endif

.PHONY: all $(BIN_DIRS) clean

all: $(BINARY)

reth:
	cd bin/reth && cargo build $(CARGO_FLAGS) $(CARGO_FEATURES)

bench:
	cd bin/bench && cargo build $(CARGO_FLAGS)

kvstore:
	cd bin/kvstore && cargo build $(CARGO_FLAGS)

clean:
	for dir in $(BIN_PATHS); do \
	    (cd $$dir && cargo clean); \
	done
