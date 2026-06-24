#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLUSTER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="${CLUSTER_TOML:-$CLUSTER_DIR/cluster.toml}"
CHAOS_BACKEND="${CHAOS_BACKEND:-local}"

# shellcheck source=cluster/chaos/lib/net.sh
source "$SCRIPT_DIR/lib/net.sh"
# shellcheck source=cluster/chaos/lib/docker.sh
source "$SCRIPT_DIR/lib/docker.sh"

usage() {
    cat <<'EOF'
Usage:
  chaos.sh [--config <cluster.toml>] [--backend local|docker] <command> [args]

Commands:
  list                         List all configured nodes
  validators                   List validator nodes
  kill <node>                  Stop a node via cluster/stop.sh
  hard-kill <node>             SIGKILL a node PID directly
  start <node>                 Start a node via cluster/start.sh
  restart <node> [wait_s]      Stop, wait, then start a node
  random-kill                  Stop a stake-weighted majority-failure victim candidate
  majority-victims             Print nodes whose stake is > 1/3 of validator stake
  snapshot [--validators]      Print a JSON RPC/process snapshot
  partition <node>             Drop target peer traffic using GRAVITY_CHAOS chain
  partition-asym <node> in|out Drop only inbound or outbound peer traffic
  partition-split <left> <right>
                               docker: split comma-separated node groups
  delay <lat> [jitter]         local: add host-level tc netem delay
  delay <node> <lat> [jitter]  docker: add container-level tc netem delay
  loss <pct>                   local: add host-level tc netem packet loss
  loss <node> <pct>            docker: add container-level packet loss
  throttle <rate>              local: add host-level tc bandwidth limit
  throttle <node> <rate>       docker: add container-level bandwidth limit
  heal [node]                  local: flush chain/tc; docker: reconnect network/tc
  net-status [node]            local: show iptables/tc; docker: show one/all node network/tc

Notes:
  - start/kill/restart never deploy or clean data directories.
  - CHAOS_BACKEND defaults to local. Docker backend maps node ids to container
    names directly, or via CHAOS_DOCKER_PREFIX / fuzzy compose-name matching.
EOF
}

log() {
    printf '[chaos] %s\n' "$*"
}

die() {
    printf '[chaos][error] %s\n' "$*" >&2
    exit 1
}

cluster_py() {
    python3 "$SCRIPT_DIR/lib/cluster.py" "$@"
}

node_field() {
    cluster_py node-field --config "$CONFIG_FILE" "$1" "$2"
}

parse_common_args() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --config)
                CONFIG_FILE="$2"
                shift 2
                ;;
            --backend)
                CHAOS_BACKEND="$2"
                shift 2
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                break
                ;;
        esac
    done
    REMAINING_ARGS=("$@")
}

stop_node() {
    local node="$1"
    if [ "$CHAOS_BACKEND" = "docker" ]; then
        log "stopping $node via docker"
        docker_node_stop "$node"
    else
        log "stopping $node via cluster/stop.sh"
        bash "$CLUSTER_DIR/stop.sh" --config "$CONFIG_FILE" --nodes "$node"
    fi
}

hard_kill_node() {
    local node="$1"
    if [ "$CHAOS_BACKEND" = "docker" ]; then
        log "SIGKILL $node via docker kill"
        docker_node_kill "$node"
    else
        local data_dir pid_file pid
        data_dir="$(node_field "$node" data_dir)"
        pid_file="$data_dir/script/node.pid"
        [ -f "$pid_file" ] || die "$node pid file not found: $pid_file"
        pid="$(cat "$pid_file")"
        log "SIGKILL $node pid=$pid"
        kill -9 "$pid" 2>/dev/null || true
        rm -f "$pid_file"
    fi
}

start_node() {
    local node="$1"
    if [ "$CHAOS_BACKEND" = "docker" ]; then
        log "starting $node via docker"
        docker_node_start "$node"
    else
        log "starting $node via cluster/start.sh"
        bash "$CLUSTER_DIR/start.sh" --config "$CONFIG_FILE" --nodes "$node"
    fi
}

restart_node() {
    local node="$1"
    local wait_s="${2:-5}"
    if [ "$CHAOS_BACKEND" = "docker" ]; then
        log "restarting $node via docker"
        docker_node_restart "$node"
    else
        stop_node "$node"
        log "waiting ${wait_s}s before restart"
        sleep "$wait_s"
        start_node "$node"
    fi
}

partition_node() {
    local node="$1"
    if [ "$CHAOS_BACKEND" = "docker" ]; then
        docker_partition_node "$node"
        return
    fi
    local peer_hosts ports
    peer_hosts="$(cluster_py peer-hosts --config "$CONFIG_FILE" "$node")"
    ports="$(cluster_py node-ports --config "$CONFIG_FILE" "$node")"

    if [ -n "$peer_hosts" ]; then
        log "partitioning $node from peer hosts: $peer_hosts"
        for host in $peer_hosts; do
            chaos_drop_host "$host"
        done
    fi

    if [ -n "$ports" ]; then
        log "dropping $node local listener ports: $ports"
        for port in $ports; do
            chaos_drop_port "$port"
        done
    fi

    if [ -z "$peer_hosts" ] && [ -z "$ports" ]; then
        die "no peer hosts or p2p ports found for $node"
    fi
}

partition_node_asym() {
    local node="$1"
    local direction="$2"
    local peer_hosts target_ports peer_ports port host
    case "$direction" in
        in|out) ;;
        *) die "partition-asym direction must be 'in' or 'out'" ;;
    esac

    target_ports="$(cluster_py node-ports --config "$CONFIG_FILE" "$node")"
    peer_ports="$(cluster_py peer-ports --config "$CONFIG_FILE" "$node")"
    if [ "$CHAOS_BACKEND" = "docker" ]; then
        docker_partition_asym_node "$node" "$direction" "$target_ports" "$peer_ports"
        return
    fi

    peer_hosts="$(cluster_py peer-hosts --config "$CONFIG_FILE" "$node")"
    if [ "$direction" = "in" ]; then
        if [ -n "$peer_hosts" ]; then
            log "dropping inbound traffic from peer hosts for $node: $peer_hosts"
            for host in $peer_hosts; do
                chaos_drop_host_in "$host"
            done
        fi
        if [ -n "$target_ports" ]; then
            log "dropping inbound listener ports for $node: $target_ports"
            for port in $target_ports; do
                chaos_drop_port_in "$port"
            done
        fi
    else
        if [ -n "$peer_hosts" ]; then
            log "dropping outbound traffic to peer hosts for $node: $peer_hosts"
            for host in $peer_hosts; do
                chaos_drop_host_out "$host"
            done
        fi
        if [ -n "$target_ports" ]; then
            log "dropping outbound traffic from $node listener ports: $target_ports"
            for port in $target_ports; do
                chaos_drop_port_out "$port"
            done
        fi
        if [ -n "$peer_ports" ]; then
            log "dropping outbound traffic to peer listener ports for $node: $peer_ports"
            for port in $peer_ports; do
                chaos_drop_dport_out "$port"
            done
        fi
    fi

    if [ -z "$peer_hosts" ] && [ -z "$target_ports" ] && [ -z "$peer_ports" ]; then
        die "no peer hosts or p2p ports found for $node"
    fi
}

partition_split() {
    local left="$1"
    local right="$2"
    if [ "$CHAOS_BACKEND" != "docker" ]; then
        die "partition-split is currently supported only with --backend docker"
    fi
    docker_partition_split "$left" "$right"
}

parse_common_args "$@"
set -- "${REMAINING_ARGS[@]}"
[ "$#" -gt 0 ] || { usage; exit 1; }

cmd="$1"
shift

case "$cmd" in
    list)
        cluster_py nodes --config "$CONFIG_FILE"
        ;;
    validators)
        cluster_py nodes --config "$CONFIG_FILE" --validators
        ;;
    kill)
        [ "$#" -eq 1 ] || die "kill requires <node>"
        stop_node "$1"
        ;;
    hard-kill)
        [ "$#" -eq 1 ] || die "hard-kill requires <node>"
        hard_kill_node "$1"
        ;;
    start)
        [ "$#" -eq 1 ] || die "start requires <node>"
        start_node "$1"
        ;;
    restart)
        [ "$#" -ge 1 ] || die "restart requires <node> [wait_s]"
        restart_node "$1" "${2:-5}"
        ;;
    random-kill)
        victim="$(cluster_py majority-victims --config "$CONFIG_FILE" | awk '{print $1}')"
        [ -n "$victim" ] || die "no victim selected"
        stop_node "$victim"
        ;;
    majority-victims)
        cluster_py majority-victims --config "$CONFIG_FILE"
        ;;
    snapshot)
        validators_flag=()
        if [ "${1:-}" = "--validators" ]; then
            validators_flag=(--validators)
        fi
        cluster_py snapshot --config "$CONFIG_FILE" "${validators_flag[@]}"
        ;;
    partition)
        [ "$#" -eq 1 ] || die "partition requires <node>"
        partition_node "$1"
        ;;
    partition-asym)
        [ "$#" -eq 2 ] || die "partition-asym requires <node> in|out"
        partition_node_asym "$1" "$2"
        ;;
    partition-split)
        [ "$#" -eq 2 ] || die "partition-split requires <left_csv> <right_csv>"
        partition_split "$1" "$2"
        ;;
    delay)
        if [ "$CHAOS_BACKEND" = "docker" ]; then
            [ "$#" -ge 2 ] || die "docker delay requires <node> <lat> [jitter]"
            docker_delay_node "$1" "$2" "${3:-}"
        else
            [ "$#" -ge 1 ] || die "delay requires <lat> [jitter]"
            chaos_delay "$1" "${2:-}"
        fi
        ;;
    loss)
        if [ "$CHAOS_BACKEND" = "docker" ]; then
            [ "$#" -eq 2 ] || die "docker loss requires <node> <pct>"
            docker_loss_node "$1" "$2"
        else
            [ "$#" -eq 1 ] || die "loss requires <pct>"
            chaos_loss "$1"
        fi
        ;;
    throttle)
        if [ "$CHAOS_BACKEND" = "docker" ]; then
            [ "$#" -eq 2 ] || die "docker throttle requires <node> <rate>"
            docker_throttle_node "$1" "$2"
        else
            [ "$#" -eq 1 ] || die "throttle requires <rate>"
            chaos_throttle "$1"
        fi
        ;;
    heal)
        if [ "$CHAOS_BACKEND" = "docker" ]; then
            if [ "$#" -ge 1 ]; then
                docker_heal_node "$1"
                docker_tc_heal_node "$1"
            else
                docker_heal_all
            fi
        else
            chaos_heal_iptables
            chaos_heal_tc
        fi
        ;;
    net-status)
        if [ "$CHAOS_BACKEND" = "docker" ]; then
            if [ "$#" -ge 1 ]; then
                docker_net_status_node "$1"
            else
                while IFS= read -r node; do
                    [ -n "$node" ] || continue
                    docker_net_status_node "$node"
                    printf '\n'
                done < <(cluster_py nodes --config "$CONFIG_FILE")
            fi
        else
            chaos_net_status
        fi
        ;;
    -h|--help)
        usage
        ;;
    *)
        die "unknown command: $cmd"
        ;;
esac
