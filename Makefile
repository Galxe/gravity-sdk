BINARY ?= peth
FEATURE ?=
MODE ?= release

BIN_DIRS := peth bench kvstore
BIN_PATHS := $(addprefix bin/, $(BIN_DIRS))

ifeq ($(MODE),release)
    CARGO_FLAGS := --release
else
    CARGO_FLAGS :=
endif

CARGO_FEATURES := $(if $(FEATURE),--features $(FEATURE),)

.PHONY: all $(BIN_DIRS) clean

all: $(BINARY)

peth:
	cd bin/peth && cargo build $(CARGO_FLAGS) $(CARGO_FEATURES)

bench:
	cd bin/bench && cargo build $(CARGO_FLAGS) $(CARGO_FEATURES)

kvstore:
	cd bin/kvstore && cargo build $(CARGO_FLAGS) $(CARGO_FEATURES)

clean:
	for dir in $(BIN_PATHS); do \
	    (cd $$dir && cargo clean); \
	done
