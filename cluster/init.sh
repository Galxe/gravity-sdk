#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${GRAVITY_ARTIFACTS_DIR:-$SCRIPT_DIR/output}"

source "$SCRIPT_DIR/utils/common.sh"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Pull a node's `identity.source` ("file" by default) and `identity.secret`
# from cluster.toml. genesis.toml-only nodes (genesis_validators[] without a
# matching cluster.toml entry) default to file because there's no place to
# declare a secret resource for them.
resolve_identity_config() {
    local node_id="$1"
    local cluster_json="$2"

    local node
    node=$(echo "$cluster_json" | jq -c ".nodes[]? | select(.id == \"$node_id\")")
    if [ -z "$node" ] || [ "$node" = "null" ]; then
        echo "file|"
        return
    fi
    local source secret
    source=$(echo "$node" | jq -r '.identity.source // "file"')
    secret=$(echo "$node" | jq -r '.identity.secret // empty')
    echo "${source}|${secret}"
}

# Per-node key generation. Public-key sidecar (identity.public.yaml) is
# always written so `make genesis` can read public material without ever
# touching the private blob (file mode) or hitting Secret Manager (gcp_secret
# mode). Idempotency is keyed off the sidecar's presence + required fields,
# so re-running init across mixed source types is safe.
prepare_node_keys() {
    local node_id="$1"
    local node_config_dir="$2"
    local identity_source="$3"
    local identity_secret="$4"

    mkdir -p "$node_config_dir"
    local identity_file="$node_config_dir/identity.yaml"
    local public_file="$node_config_dir/identity.public.yaml"

    log_info "  [$node_id] Checking identity (source=$identity_source)..."

    # Idempotency: if the public sidecar exists with the four required
    # fields, the matching keypair has already been generated. For file
    # mode also confirm the private blob is present (sidecar without blob
    # would mean we lost the disk copy after a previous run).
    if [ -f "$public_file" ]; then
        local has_all=1
        for k in account_address consensus_public_key consensus_pop network_public_key; do
            grep -q "^$k:" "$public_file" || { has_all=0; break; }
        done
        if [ "$has_all" = "1" ]; then
            if [ "$identity_source" = "file" ] && [ ! -f "$identity_file" ]; then
                log_warn "  [$node_id] sidecar present but identity.yaml missing — regenerating"
            else
                log_info "  [$node_id] Identity already initialized."
                return 0
            fi
        else
            log_warn "  [$node_id] sidecar incomplete — regenerating"
        fi
    fi

    case "$identity_source" in
        file)
            log_info "  [$node_id] Generating key (file)..."
            "$GRAVITY_CLI" genesis generate-key \
                --output-file "$identity_file" \
                --public-output-file "$public_file" >/dev/null
            ;;
        gcp_secret)
            if [ -z "$identity_secret" ]; then
                log_error "  [$node_id] identity.source = gcp_secret requires identity.secret in cluster.toml"
                exit 1
            fi
            if [ -z "${GCP_ACCESS_TOKEN:-}" ] && ! curl -sS -m 2 -H "Metadata-Flavor: Google" \
                http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/email \
                >/dev/null 2>&1; then
                log_error "  [$node_id] GCP auth not available."
                log_error "    On a non-GCE host, set: export GCP_ACCESS_TOKEN=\$(gcloud auth print-access-token)"
                exit 1
            fi
            log_info "  [$node_id] Generating key → Secret Manager ($identity_secret)..."
            # Note: stdout is preserved (not redirected to /dev/null) so the
            # operator sees the public-material summary and the resulting
            # version path; useful for staking-tx registration and for
            # pinning cluster.toml away from versions/latest.
            "$GRAVITY_CLI" genesis generate-key \
                --secret "$identity_secret" \
                --public-output-file "$public_file"
            ;;
        *)
            log_error "  [$node_id] unknown identity.source '$identity_source' (expected 'file' or 'gcp_secret')"
            exit 1
            ;;
    esac
}

main() {
    log_info "Initializing node keys..."

    # Create output dir if not exists
    mkdir -p "$OUTPUT_DIR"

    # Find gravity_cli
    GRAVITY_CLI=$(find_binary "gravity_cli" "$PROJECT_ROOT") || true
    if [ -z "$GRAVITY_CLI" ]; then
        log_error "gravity_cli not found! Please build it first."
        exit 1
    fi
    log_info "Using gravity_cli: $GRAVITY_CLI"

    # Accept genesis.toml path as argument, or use defaults
    local input_config="${1:-}"
    local config_dir=""

    if [ -n "$input_config" ] && [ -f "$input_config" ]; then
        config_dir="$(dirname "$input_config")"
    else
        config_dir="$SCRIPT_DIR"
    fi

    GENESIS_CONFIG="$config_dir/genesis.toml"
    CLUSTER_CONFIG="$config_dir/cluster.toml"

    if [ -n "$input_config" ] && [ -f "$input_config" ]; then
        if [[ "$input_config" == *"genesis.toml" ]]; then
            GENESIS_CONFIG="$input_config"
        else
            CLUSTER_CONFIG="$input_config"
        fi
    fi

    nodes_to_process=()

    if [ -f "$GENESIS_CONFIG" ]; then
        log_info "Reading genesis validators from genesis.toml..."
        export CONFIG_FILE="$GENESIS_CONFIG"
        config_json=$(parse_toml)

        genesis_validators=$(echo "$config_json" | jq -c '.genesis_validators // []')
        validator_count=$(echo "$genesis_validators" | jq 'length')

        for i in $(seq 0 $((validator_count - 1))); do
            node_id=$(echo "$genesis_validators" | jq -r ".[$i].id")
            nodes_to_process+=("$node_id")
        done
        log_info "Found $validator_count genesis validators"
    fi

    cluster_config_json="{}"
    if [ -f "$CLUSTER_CONFIG" ]; then
        log_info "Reading nodes from cluster.toml..."
        export CONFIG_FILE="$CLUSTER_CONFIG"
        cluster_config_json=$(parse_toml)

        node_count=$(echo "$cluster_config_json" | jq '.nodes | length')

        for i in $(seq 0 $((node_count - 1))); do
            node=$(echo "$cluster_config_json" | jq ".nodes[$i]")
            node_id=$(echo "$node" | jq -r '.id')

            if [[ ! " ${nodes_to_process[*]} " =~ " ${node_id} " ]]; then
                nodes_to_process+=("$node_id")
            fi
        done
    fi

    if [ ${#nodes_to_process[@]} -eq 0 ]; then
        log_error "No nodes found! Create genesis.toml or cluster.toml first."
        log_info "Copy genesis.toml.example to genesis.toml and/or cluster.toml.example to cluster.toml"
        exit 1
    fi

    log_info "Generating keys for ${#nodes_to_process[@]} nodes..."
    for node_id in "${nodes_to_process[@]}"; do
        node_conf_out="$OUTPUT_DIR/$node_id/config"
        IFS='|' read -r idsrc idsec <<< "$(resolve_identity_config "$node_id" "$cluster_config_json")"
        prepare_node_keys "$node_id" "$node_conf_out" "$idsrc" "$idsec"
    done

    echo ""
    log_info "Init complete!"
    log_info "Public material (sidecar) in: $OUTPUT_DIR/<node>/config/identity.public.yaml"
    log_info "Run 'make genesis' next to generate genesis.json."
}

main "$@"
