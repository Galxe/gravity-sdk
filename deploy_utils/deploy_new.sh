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
    ["quick-release"]="Quick Release Version"
    ["pprof"]="PProf Version"
)

# Default settings
bin_name="gravity_node"
bin_version="debug"
config_dir=""
deploy_dir=""

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
    echo "  -v, --bin_version VER   Specify version:"
    for ver in "${!VALID_VERSIONS[@]}"; do
        echo "                      - $ver (${VALID_VERSIONS[$ver]})"
    done
    echo "  -c, --config_dir DIR    Specify config directory (required)"
    echo "                          Must contain: identity.yaml, validator.yaml, reth_config.json"
    echo "  -d, --deploy_dir DIR    Specify deployment directory (required)"
    echo "                          Should match '/tmp/gravity_node' in config files"
    echo "  -h, --help             Show this help message"
    echo
    echo "Examples:"
    echo "  $0 -c ./my_config -d /tmp/gravity_node -v release"
    echo "  $0 --config_dir ./configs/node1 --deploy_dir /tmp/gravity_node --bin_version debug"
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

# Validate config directory
validate_config_dir() {
    if [[ -z "$config_dir" ]]; then
        log_error "--config_dir parameter is required"
        show_help
        exit 1
    fi

    if [[ ! -d "$config_dir" ]]; then
        log_error "Config directory does not exist: $config_dir"
        exit 1
    fi

    # Check required config files
    local missing_files=()
    for file in "identity.yaml" "validator.yaml" "reth_config.json"; do
        if [[ ! -f "$config_dir/$file" ]]; then
            missing_files+=($file)
        fi
    done

    if [ ${#missing_files[@]} -ne 0 ]; then
        log_error "Missing required config files in $config_dir: ${missing_files[*]}"
        exit 1
    fi

    log_info "Config directory validation passed: $config_dir"
}

# Validate deploy directory
validate_deploy_dir() {
    if [[ -z "$deploy_dir" ]]; then
        log_error "--deploy_dir parameter is required"
        show_help
        exit 1
    fi

    # Check if deploy_dir exists or can be created
    if [[ ! -d "$deploy_dir" ]]; then
        if ! mkdir -p "$deploy_dir" 2>/dev/null; then
            log_error "Cannot create directory: $deploy_dir (Permission denied)"
            exit 1
        fi
    else
        # Check write permission by attempting to create a temp file
        if ! touch "$deploy_dir/.write_test" 2>/dev/null; then
            log_error "No write permission in directory: $deploy_dir"
            exit 1
        fi
        rm "$deploy_dir/.write_test"
    fi

    log_info "Deploy directory validation passed: $deploy_dir"
}

# Validate parameters
validate_params() {
    validate_config_dir
    validate_deploy_dir

    if [[ -z "${VALID_VERSIONS[$bin_version]}" ]]; then
        log_error "Invalid version: '$bin_version'"
        echo "Available versions:"
        for ver in "${!VALID_VERSIONS[@]}"; do
            echo "  - $ver (${VALID_VERSIONS[$ver]})"
        done
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
    -v|--bin_version)
        bin_version="$2"
        shift
        ;;
    -c|--config_dir)
        config_dir="$2"
        shift
        ;;
    -d|--deploy_dir)
        deploy_dir="$2"
        shift
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

    SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
    TARGET_DIR="$SCRIPT_DIR/../target"

    # Check binary file
    if [[ ! -f "$TARGET_DIR/$bin_version/$bin_name" ]]; then
        log_error "Binary not found: $TARGET_DIR/$bin_version/$bin_name"
        exit 1
    fi

    # Clean and prepare directories
    log_info "Cleaning directory $deploy_dir"
    rm -rf "$deploy_dir"
    
    # Prepare directories
    log_info "Creating required directories"
    mkdir -p "$deploy_dir"/{config,bin,data,logs,script}

    # Copy configuration files
    log_info "Copying configuration files from $config_dir"
    cp "$config_dir/identity.yaml" "$deploy_dir/config/"
    cp "$config_dir/validator.yaml" "$deploy_dir/config/"
    cp "$config_dir/reth_config.json" "$deploy_dir/config/"
    cp "$config_dir/waypoint.txt" "$deploy_dir/config/"
    cp "$config_dir/network_config.json" "$deploy_dir/config/config.json"
    cp "$config_dir/discovery" "$deploy_dir/"

    # Copy program files
    log_info "Copying program files"
    cp "$TARGET_DIR/$bin_version/$bin_name" "$deploy_dir/bin"
    cp "$SCRIPT_DIR/start.sh" "$deploy_dir/script"
    cp "$SCRIPT_DIR/stop.sh" "$deploy_dir/script"

    log_info "Deployment completed!"
    log_info "Configuration summary:"
    echo "  - Config source: $config_dir"
    echo "  - Deploy target: $deploy_dir"
    echo "  - Version: $bin_version (${VALID_VERSIONS[$bin_version]})"
    echo "  - Binary: $bin_name"
}

# Execute main program
main 
