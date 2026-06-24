#!/usr/bin/env bash

CHAOS_DOCKER_PREFIX="${CHAOS_DOCKER_PREFIX:-}"
CHAOS_DOCKER_NETWORK="${CHAOS_DOCKER_NETWORK:-}"
CHAOS_DOCKER_STATE_DIR="${CHAOS_DOCKER_STATE_DIR:-/tmp/gravity-chaos-docker-${USER:-user}}"
CHAOS_DOCKER_IFACE="${CHAOS_DOCKER_IFACE:-eth0}"
CHAOS_DOCKER_PARTITION_PREFIX="${CHAOS_DOCKER_PARTITION_PREFIX:-gravity-chaos-partition}"
CHAOS_DOCKER_IPTABLES_CHAIN="${CHAOS_DOCKER_IPTABLES_CHAIN:-GRAVITY_CHAOS}"

docker_container_for_node() {
    local node="$1"
    local candidate="${CHAOS_DOCKER_PREFIX}${node}"
    if docker inspect "$candidate" >/dev/null 2>&1; then
        printf '%s\n' "$candidate"
        return 0
    fi

    # Common compose names include project_service_1 or project-service-1.
    local match
    match="$(docker ps -a --format '{{.Names}}' | awk -v n="$node" '
        $0 == n { print; exit }
        $0 ~ "(^|[_-])" n "($|[_-])" { print; exit }
    ')"
    if [ -n "$match" ]; then
        printf '%s\n' "$match"
        return 0
    fi

    echo "no docker container found for node '$node' (tried '$candidate')" >&2
    return 1
}

docker_node_stop() {
    docker stop "$(docker_container_for_node "$1")"
}

docker_node_start() {
    docker start "$(docker_container_for_node "$1")"
}

docker_node_restart() {
    docker restart "$(docker_container_for_node "$1")"
}

docker_node_kill() {
    docker kill "$(docker_container_for_node "$1")"
}

docker_node_status() {
    local node="$1"
    local container
    container="$(docker_container_for_node "$node")"
    docker ps -a --filter "name=^/${container}$" \
        --format '{{.ID}} {{.Names}} {{.Image}} {{.Status}} {{.Ports}}'
}

docker_networks_for_container() {
    local container="$1"
    if [ -n "$CHAOS_DOCKER_NETWORK" ]; then
        printf '%s\n' "$CHAOS_DOCKER_NETWORK"
        return
    fi
    docker inspect "$container" \
        --format '{{range $name, $_ := .NetworkSettings.Networks}}{{println $name}}{{end}}' |
        awk '$0 != "none" {print}'
}

docker_container_uses_host_network() {
    local container="$1"
    docker inspect "$container" --format '{{.HostConfig.NetworkMode}}' | grep -qx 'host'
}

docker_partition_state_file() {
    printf '%s/%s.networks\n' "$CHAOS_DOCKER_STATE_DIR" "$1"
}

docker_iptables_state_file() {
    printf '%s/%s.iptables\n' "$CHAOS_DOCKER_STATE_DIR" "$1"
}

docker_tc_state_file() {
    printf '%s/%s.tc\n' "$CHAOS_DOCKER_STATE_DIR" "$1"
}

docker_disconnect_partition_networks() {
    local container="$1"
    docker inspect "$container" \
        --format '{{range $name, $_ := .NetworkSettings.Networks}}{{println $name}}{{end}}' |
        awk -v prefix="${CHAOS_DOCKER_PARTITION_PREFIX}-" 'index($0, prefix) == 1 {print}' |
        while IFS= read -r network; do
            [ -n "$network" ] || continue
            docker network disconnect "$network" "$container" 2>/dev/null || true
        done
}

docker_prune_partition_networks() {
    docker network ls --format '{{.Name}}' |
        awk -v prefix="${CHAOS_DOCKER_PARTITION_PREFIX}-" 'index($0, prefix) == 1 {print}' |
        while IFS= read -r network; do
            [ -n "$network" ] || continue
            docker network rm "$network" >/dev/null 2>&1 || true
        done
}

docker_save_original_networks() {
    local container="$1"
    local state_file="$2"
    local networks
    [ ! -f "$state_file" ] || {
        echo "container $container already has saved network state: $state_file" >&2
        return 1
    }
    networks="$(docker_networks_for_container "$container")"
    [ -n "$networks" ] || {
        echo "container $container has no docker networks to save" >&2
        return 1
    }
    printf '%s\n' "$networks" > "$state_file"
}

docker_partition_node() {
    local node="$1"
    local container state_file networks
    container="$(docker_container_for_node "$node")"
    if docker_container_uses_host_network "$container"; then
        echo "container $container uses network_mode=host; docker network disconnect cannot partition host-network containers" >&2
        return 1
    fi
    mkdir -p "$CHAOS_DOCKER_STATE_DIR"
    state_file="$(docker_partition_state_file "$container")"
    docker_save_original_networks "$container" "$state_file"
    while IFS= read -r network; do
        [ -n "$network" ] || continue
        docker network disconnect "$network" "$container" 2>/dev/null || true
    done < "$state_file"
    echo "partitioned $node ($container); saved networks in $state_file"
}

docker_connect_partition_member() {
    local node="$1"
    local target_network="$2"
    local container state_file
    container="$(docker_container_for_node "$node")"
    if docker_container_uses_host_network "$container"; then
        echo "container $container uses network_mode=host; docker split partition requires bridge networks" >&2
        return 1
    fi
    state_file="$(docker_partition_state_file "$container")"
    docker_save_original_networks "$container" "$state_file"
    docker network connect --alias "$node" --alias "$container" "$target_network" "$container"
    while IFS= read -r network; do
        [ -n "$network" ] || continue
        docker network disconnect "$network" "$container" 2>/dev/null || true
    done < "$state_file"
}

docker_partition_split() {
    local left_csv="$1"
    local right_csv="$2"
    local split_id="${CHAOS_DOCKER_SPLIT_ID:-$(date +%s)-$$}"
    local left_network="${CHAOS_DOCKER_PARTITION_PREFIX}-${split_id}-a"
    local right_network="${CHAOS_DOCKER_PARTITION_PREFIX}-${split_id}-b"
    local node

    mkdir -p "$CHAOS_DOCKER_STATE_DIR"
    docker network create --driver bridge "$left_network" >/dev/null
    docker network create --driver bridge "$right_network" >/dev/null

    for node in ${left_csv//,/ }; do
        [ -n "$node" ] || continue
        docker_connect_partition_member "$node" "$left_network"
    done
    for node in ${right_csv//,/ }; do
        [ -n "$node" ] || continue
        docker_connect_partition_member "$node" "$right_network"
    done

    echo "split partition created: [$left_csv] on $left_network ; [$right_csv] on $right_network"
}

docker_heal_container() {
    local container="$1"
    local state_file
    state_file="$(docker_partition_state_file "$container")"
    [ -f "$state_file" ] || {
        echo "no saved docker network state for $container; skipping"
        return 0
    }
    docker_disconnect_partition_networks "$container"
    while IFS= read -r network; do
        [ -n "$network" ] || continue
        docker network connect --alias "$container" "$network" "$container" 2>/dev/null || true
    done < "$state_file"
    rm -f "$state_file"
    echo "healed $container"
}

docker_iptables_heal_container() {
    local container="$1"
    local state_file
    state_file="$(docker_iptables_state_file "$container")"
    [ -f "$state_file" ] || return 0
    docker exec --user 0 "$container" sh -lc "
        set +e
        if command -v iptables >/dev/null 2>&1; then ipt=iptables;
        elif command -v iptables-legacy >/dev/null 2>&1; then ipt=iptables-legacy;
        else exit 0; fi
        while \$ipt -D INPUT -j '$CHAOS_DOCKER_IPTABLES_CHAIN' 2>/dev/null; do :; done
        while \$ipt -D OUTPUT -j '$CHAOS_DOCKER_IPTABLES_CHAIN' 2>/dev/null; do :; done
        \$ipt -F '$CHAOS_DOCKER_IPTABLES_CHAIN' 2>/dev/null || true
        \$ipt -X '$CHAOS_DOCKER_IPTABLES_CHAIN' 2>/dev/null || true
    " >/dev/null 2>&1 || true
    rm -f "$state_file"
    echo "healed docker iptables for $container"
}

docker_heal_node() {
    local node="$1"
    local container
    container="$(docker_container_for_node "$node")"
    docker_heal_container "$container"
    docker_iptables_heal_container "$container"
    docker_prune_partition_networks
}

docker_heal_all() {
    local state_file container network
    shopt -s nullglob
    for state_file in "$CHAOS_DOCKER_STATE_DIR"/*.networks; do
        container="$(basename "$state_file" .networks)"
        docker_heal_container "$container"
    done
    for state_file in "$CHAOS_DOCKER_STATE_DIR"/*.iptables; do
        container="$(basename "$state_file" .iptables)"
        docker_iptables_heal_container "$container"
    done
    for state_file in "$CHAOS_DOCKER_STATE_DIR"/*.tc; do
        container="$(basename "$state_file" .tc)"
        docker_tc_heal_container "$container"
    done
    docker_prune_partition_networks
}

docker_iptables() {
    local node="$1"
    shift
    local container
    container="$(docker_container_for_node "$node")"
    docker exec --user 0 "$container" sh -lc "
        set -e
        if command -v iptables >/dev/null 2>&1; then ipt=iptables;
        elif command -v iptables-legacy >/dev/null 2>&1; then ipt=iptables-legacy;
        else echo 'iptables is not installed in container $container' >&2; exit 127; fi
        \"\$ipt\" \"\$@\"
    " sh "$@"
}

docker_ensure_iptables_chain() {
    local node="$1"
    local container state_file
    container="$(docker_container_for_node "$node")"
    mkdir -p "$CHAOS_DOCKER_STATE_DIR"
    state_file="$(docker_iptables_state_file "$container")"
    touch "$state_file"
    docker_iptables "$node" -N "$CHAOS_DOCKER_IPTABLES_CHAIN" 2>/dev/null || true
    docker_iptables "$node" -C INPUT -j "$CHAOS_DOCKER_IPTABLES_CHAIN" 2>/dev/null || \
        docker_iptables "$node" -I INPUT -j "$CHAOS_DOCKER_IPTABLES_CHAIN"
    docker_iptables "$node" -C OUTPUT -j "$CHAOS_DOCKER_IPTABLES_CHAIN" 2>/dev/null || \
        docker_iptables "$node" -I OUTPUT -j "$CHAOS_DOCKER_IPTABLES_CHAIN"
}

docker_partition_asym_node() {
    local node="$1"
    local direction="$2"
    local target_ports="$3"
    local peer_ports="$4"
    local port
    case "$direction" in
        in|out) ;;
        *) echo "direction must be 'in' or 'out': $direction" >&2; return 1 ;;
    esac
    docker_ensure_iptables_chain "$node"
    if [ "$direction" = "in" ]; then
        for port in $target_ports; do
            [ -n "$port" ] || continue
            docker_iptables "$node" -A "$CHAOS_DOCKER_IPTABLES_CHAIN" -p tcp --dport "$port" -j DROP
        done
    else
        for port in $target_ports; do
            [ -n "$port" ] || continue
            docker_iptables "$node" -A "$CHAOS_DOCKER_IPTABLES_CHAIN" -p tcp --sport "$port" -j DROP
        done
        for port in $peer_ports; do
            [ -n "$port" ] || continue
            docker_iptables "$node" -A "$CHAOS_DOCKER_IPTABLES_CHAIN" -p tcp --dport "$port" -j DROP
        done
    fi
    echo "asymmetric docker partition: node=$node direction=$direction target_ports=[$target_ports] peer_ports=[$peer_ports]"
}

docker_tc() {
    local node="$1"
    shift
    local container
    container="$(docker_container_for_node "$node")"
    docker exec --user 0 "$container" "$@"
}

docker_mark_tc_node() {
    local node="$1"
    local container state_file
    container="$(docker_container_for_node "$node")"
    mkdir -p "$CHAOS_DOCKER_STATE_DIR"
    state_file="$(docker_tc_state_file "$container")"
    touch "$state_file"
}

docker_delay_node() {
    local node="$1"
    local delay="$2"
    local jitter="${3:-}"
    docker_mark_tc_node "$node"
    if [ -n "$jitter" ]; then
        docker_tc "$node" tc qdisc replace dev "$CHAOS_DOCKER_IFACE" root netem delay "$delay" "$jitter"
    else
        docker_tc "$node" tc qdisc replace dev "$CHAOS_DOCKER_IFACE" root netem delay "$delay"
    fi
}

docker_loss_node() {
    docker_mark_tc_node "$1"
    docker_tc "$1" tc qdisc replace dev "$CHAOS_DOCKER_IFACE" root netem loss "$2"
}

docker_throttle_node() {
    docker_mark_tc_node "$1"
    docker_tc "$1" tc qdisc replace dev "$CHAOS_DOCKER_IFACE" root tbf rate "$2" burst 32kbit latency 400ms
}

docker_tc_heal_container() {
    local container="$1"
    local state_file
    state_file="$(docker_tc_state_file "$container")"
    docker exec --user 0 "$container" sh -lc "command -v tc >/dev/null 2>&1 && tc qdisc del dev '$CHAOS_DOCKER_IFACE' root 2>/dev/null || true" >/dev/null 2>&1 || true
    rm -f "$state_file"
    echo "healed docker tc for $container"
}

docker_tc_heal_node() {
    local container
    container="$(docker_container_for_node "$1")"
    docker_tc_heal_container "$container"
}

docker_net_status_node() {
    local node="$1"
    local container
    container="$(docker_container_for_node "$node")"
    echo "container: $container"
    docker inspect "$container" --format '{{json .NetworkSettings.Networks}}'
    echo
    echo "iptables chain $CHAOS_DOCKER_IPTABLES_CHAIN:"
    docker exec --user 0 "$container" sh -lc "
        if command -v iptables >/dev/null 2>&1; then ipt=iptables;
        elif command -v iptables-legacy >/dev/null 2>&1; then ipt=iptables-legacy;
        else exit 0; fi
        \$ipt -S '$CHAOS_DOCKER_IPTABLES_CHAIN' 2>/dev/null || true
    "
    echo
    docker exec --user 0 "$container" sh -lc "command -v tc >/dev/null 2>&1 && tc qdisc show dev '$CHAOS_DOCKER_IFACE' || true"
}
