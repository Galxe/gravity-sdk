#!/bin/bash
# ============================================================
# Inject identity.yaml into GCP Secret Manager
#
# Reads cluster.toml, picks every node whose identity.source =
# "gcp_secret", and uploads output/<id>/config/identity.yaml to the
# resource named in identity.secret.
#
# Run order: make init -> make inject_gcp_identity -> make deploy
#
# Usage:
#   ./inject_gcp_identity.sh [--config cluster.toml]
#                            [--nodes id1,id2,...]
#                            [--update]
#                            [--dry-run]
#
# --update    add a new version when the secret already exists
#             (default: error out, to avoid silent rotation)
# --dry-run   print the gcloud commands without running them
#
# Auth: relies on the caller's `gcloud` login. Permission needed:
#   roles/secretmanager.admin (to create / add-version).
# Runtime nodes only need roles/secretmanager.secretAccessor.
# ============================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${GRAVITY_ARTIFACTS_DIR:-$SCRIPT_DIR/output}"

source "$SCRIPT_DIR/utils/common.sh"

CONFIG_FILE="$SCRIPT_DIR/cluster.toml"
SPECIFIC_NODES=""
UPDATE_EXISTING=0
DRY_RUN=0

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        --nodes)
            SPECIFIC_NODES="$2"
            shift 2
            ;;
        --update)
            UPDATE_EXISTING=1
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            sed -n '2,/^set -e/p' "$0" | sed 's/^# \{0,1\}//;s/^#$//' | head -n -1
            exit 0
            ;;
        *)
            log_error "Unknown parameter: $1"
            exit 1
            ;;
    esac
done

if ! command -v gcloud >/dev/null 2>&1; then
    log_error "gcloud CLI not found. Install Google Cloud SDK and run 'gcloud auth login'."
    exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
    log_error "Config file not found: $CONFIG_FILE"
    exit 1
fi

# Returns 0 if $1 is in the comma-separated $SPECIFIC_NODES list (or list empty).
node_selected() {
    local node_id="$1"
    [ -z "$SPECIFIC_NODES" ] && return 0
    local IFS=','
    for n in $SPECIFIC_NODES; do
        [ "$n" = "$node_id" ] && return 0
    done
    return 1
}

# Parse "projects/<P>/secrets/<S>[/versions/<V>]" into PROJECT and SECRET globals.
# Version segment is ignored — uploads always create a new version.
parse_resource() {
    local resource="$1"
    PROJECT=""
    SECRET=""
    if [[ ! "$resource" =~ ^projects/([^/]+)/secrets/([^/]+)(/versions/[^/]+)?$ ]]; then
        return 1
    fi
    PROJECT="${BASH_REMATCH[1]}"
    SECRET="${BASH_REMATCH[2]}"
    return 0
}

run_or_echo() {
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "    [dry-run] $*"
    else
        "$@"
    fi
}

main() {
    export CONFIG_FILE
    config_json=$(parse_toml)

    node_count=$(echo "$config_json" | jq '.nodes | length')
    if [ "$node_count" -eq 0 ]; then
        log_error "No [[nodes]] entries in $CONFIG_FILE"
        exit 1
    fi

    log_info "Scanning $CONFIG_FILE for gcp_secret nodes..."

    local processed=0
    local skipped_file=0
    local skipped_unselected=0

    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq -c ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        source=$(echo "$node" | jq -r '.identity.source // "file"')
        secret_resource=$(echo "$node" | jq -r '.identity.secret // empty')

        if ! node_selected "$node_id"; then
            skipped_unselected=$((skipped_unselected + 1))
            continue
        fi

        case "$source" in
            file)
                log_info "  [$node_id] skip (identity.source = file)"
                skipped_file=$((skipped_file + 1))
                continue
                ;;
            gcp_secret)
                ;;
            *)
                log_error "  [$node_id] unknown identity.source '$source'"
                exit 1
                ;;
        esac

        if [ -z "$secret_resource" ]; then
            log_error "  [$node_id] identity.source = gcp_secret but identity.secret is empty"
            exit 1
        fi

        if ! parse_resource "$secret_resource"; then
            log_error "  [$node_id] malformed identity.secret '$secret_resource'"
            log_error "             expected projects/<P>/secrets/<S>[/versions/<V>]"
            exit 1
        fi

        local identity_file="$OUTPUT_DIR/$node_id/config/identity.yaml"
        if [ ! -f "$identity_file" ]; then
            log_error "  [$node_id] $identity_file not found — run 'make init' first"
            exit 1
        fi

        log_info "  [$node_id] uploading $identity_file -> projects/$PROJECT/secrets/$SECRET"

        if gcloud secrets describe "$SECRET" --project="$PROJECT" >/dev/null 2>&1; then
            if [ "$UPDATE_EXISTING" -ne 1 ]; then
                log_error "  [$node_id] secret '$SECRET' already exists in project '$PROJECT'"
                log_error "             pass --update to add a new version, or delete it manually"
                exit 1
            fi
            log_info "  [$node_id] secret exists; adding new version"
            new_version=$(run_or_echo gcloud secrets versions add "$SECRET" \
                --project="$PROJECT" \
                --data-file="$identity_file" \
                --format='value(name)')
        else
            log_info "  [$node_id] creating secret with first version"
            run_or_echo gcloud secrets create "$SECRET" \
                --project="$PROJECT" \
                --replication-policy=automatic \
                --data-file="$identity_file"
            new_version=$(run_or_echo gcloud secrets versions list "$SECRET" \
                --project="$PROJECT" \
                --limit=1 \
                --format='value(name)')
        fi

        if [ "$DRY_RUN" -ne 1 ] && [ -n "$new_version" ]; then
            log_info "  [$node_id] uploaded as projects/$PROJECT/secrets/$SECRET/versions/$new_version"
            log_info "  [$node_id] consider pinning cluster.toml to versions/$new_version (currently '$secret_resource')"
        fi

        processed=$((processed + 1))
    done

    log_info "Done. uploaded=$processed file-skipped=$skipped_file unselected-skipped=$skipped_unselected"
    if [ "$processed" -eq 0 ] && [ "$skipped_file" -gt 0 ] && [ -z "$SPECIFIC_NODES" ]; then
        log_warn "No gcp_secret nodes found in cluster.toml — nothing to upload."
    fi
}

main "$@"
