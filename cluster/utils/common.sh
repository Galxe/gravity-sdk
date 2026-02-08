#!/bin/bash

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Parse TOML using Python
parse_toml() {
    python3 << 'PYTHON_SCRIPT'
import json
import sys
import os

# Try tomllib (Python 3.11+) first, then fall back to toml
try:
    import tomllib
    def load_toml(f):
        return tomllib.load(f)
    open_mode = 'rb'
except ImportError:
    try:
        import toml
        def load_toml(f):
            return toml.load(f)
        open_mode = 'r'
    except ImportError:
        print("Error: Neither tomllib (Python 3.11+) nor toml package available.", file=sys.stderr)
        print("Install toml: pip3 install toml", file=sys.stderr)
        sys.exit(1)

config_file = os.environ.get('CONFIG_FILE', 'cluster.toml')

try:
    with open(config_file, open_mode) as f:
        config = load_toml(f)
    print(json.dumps(config))
except FileNotFoundError:
    print(f"Error: Config file not found: {config_file}", file=sys.stderr)
    sys.exit(1)
except Exception as e:
    print(f"Error parsing TOML: {e}", file=sys.stderr)
    sys.exit(1)
PYTHON_SCRIPT
}

# Find local binary
find_binary() {
    local bin_name="$1"
    local project_root="$2"
    
    # Check configured path if passed (handled by caller usually, but generic check here)
    if [ -f "$project_root/target/quick-release/$bin_name" ]; then
        echo "$project_root/target/quick-release/$bin_name"
        return 0
    fi
    if [ -x "$project_root/target/release/$bin_name" ]; then
        echo "$project_root/target/release/$bin_name"
        return 0
    fi
    if [ -x "$project_root/target/debug/$bin_name" ]; then
        echo "$project_root/target/debug/$bin_name"
        return 0
    fi
    
    if command -v "$bin_name" &> /dev/null; then
        command -v "$bin_name"
        return 0
    fi
    
    return 1
}
