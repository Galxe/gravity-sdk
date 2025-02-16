#!/bin/bash

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration definitions
declare -A VALID_VERSIONS=(
    ["release"]="Release Version"
    ["debug"]="Debug Version"
    ["quick-release"]="Quick Relase Version"
)

declare -A VALID_MODES=(
    ["cluster"]="Cluster Mode"
    ["single"]="Single Node Mode"
)

# Default settings
bin_name="gravity_node"
node_arg=""
bin_version="release"
mode="cluster"
recover="false"

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Show help information
show_help() {
    echo "Usage: $0 [options]"
    echo
    echo "Options:"
    echo "  -b, --bin_name NAME     Specify binary name (default: gravity_node)"
    echo "  -n, --node NODE         Specify node name (required)"
    echo "  -v, --bin_version VER   Specify version:"
    for ver in "${!VALID_VERSIONS[@]}"; do
        echo "                      - $ver (${VALID_VERSIONS[$ver]})"
    done
    echo "  -m, --mode MODE         Specify run mode:"
    for m in "${!VALID_MODES[@]}"; do
        echo "                      - $m (${VALID_MODES[$m]})"
    done
    echo "  -r, --recover          Preserve existing data (default: false)"
    echo "  -h, --help             Show this help message"
    echo
    echo "Examples:"
    echo "  $0 -n node1 -v release -m cluster"
    echo "  $0 --node node1 --bin_version debug --mode single --recover"
}

# Check prerequisites
check_prerequisites() {
    local missing_tools=()
    for tool in mkdir cp rm; do
        if ! command -v $tool &> /dev/null; then
            missing_tools+=($tool)
        fi
    done

    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        exit 1
    fi
}

# Validate parameters
validate_params() {
    if [[ -z "${VALID_VERSIONS[$bin_version]}" ]]; then
        log_error "Invalid version: '$bin_version'"
        echo "Available versions:"
        for ver in "${!VALID_VERSIONS[@]}"; do
            echo "  - $ver (${VALID_VERSIONS[$ver]})"
        done
        exit 1
    fi

    if [[ -z "$node_arg" ]]; then
        log_error "--node parameter is required"
        show_help
        exit 1
    fi

    if [[ -z "${VALID_MODES[$mode]}" ]]; then
        log_error "Invalid mode: '$mode'"
        echo "Available modes:"
        for m in "${!VALID_MODES[@]}"; do
            echo "  - $m (${VALID_MODES[$m]})"
        done
        exit 1
    fi

    if [[ "$mode" == "single" && "$node_arg" != "node1" ]]; then
        log_error "Single mode only supports 'node1'"
        exit 1
    fi
}

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
    -b|--bin_name)
        bin_name="$2"
        shift
        ;;
    -n|--node)
        node_arg="$2"
        shift
        ;;
    -v|--bin_version)
        bin_version="$2"
        shift
        ;;
    -m|--mode)
        mode="$2"
        shift
        ;;
    -r|--recover)
        recover="true"
        ;;
    -h|--help)
        show_help
        exit 0
        ;;
    *)
        log_error "Unknown parameter: $1"
        echo "Use --help or -h to show usage information"
        exit 1
        ;;
    esac
    shift
done

# Main execution logic
main() {
    # Check environment
    check_prerequisites

    # Validate parameters
    validate_params

    # Check binary file
    if [[ ! -f "target/$bin_version/$bin_name" ]]; then
        log_error "Binary not found: target/$bin_version/$bin_name"
        exit 1
    fi

    # Prepare directories
    if [[ "$recover" != "true" ]]; then
        log_info "Cleaning directory /tmp/$node_arg"
        rm -rf "/tmp/$node_arg"
    else
        log_warn "Preserving existing data"
    fi

    log_info "Creating required directories"
    mkdir -p "/tmp/$node_arg"/{genesis,bin,data,logs,script}

    # Copy files
    log_info "Copying configuration files"
    cp -r "$node_arg/genesis" "/tmp/$node_arg"

    if [[ "$mode" == "cluster" ]]; then
        log_info "Setting up cluster mode"
        cp -r "deploy_utils/four_nodes_config.json" "/tmp/$node_arg/genesis/nodes_config.json"
        cp -r "deploy_utils/four_nodes_discovery" "/tmp/$node_arg/discovery"
    else
        log_info "Setting up single node mode"
        cp -r "deploy_utils/single_node_config.json" "/tmp/$node_arg/genesis/nodes_config.json"
        cp -r "deploy_utils/single_node_discovery" "/tmp/$node_arg/discovery"
    fi

    log_info "Copying program files"
    cp "target/$bin_version/$bin_name" "/tmp/$node_arg/bin"
    cp "deploy_utils/start.sh" "/tmp/$node_arg/script"
    cp "deploy_utils/stop.sh" "/tmp/$node_arg/script"

    log_info "Deployment completed!"
    log_info "Configuration summary:"
    echo "  - Node: $node_arg"
    echo "  - Version: $bin_version (${VALID_VERSIONS[$bin_version]})"
    echo "  - Mode: $mode (${VALID_MODES[$mode]})"
    echo "  - Path: /tmp/$node_arg"
}

# Execute main program
main