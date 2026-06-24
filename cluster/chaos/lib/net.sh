#!/usr/bin/env bash

CHAOS_CHAIN="${CHAOS_CHAIN:-GRAVITY_CHAOS}"
CHAOS_TC_HANDLE="${CHAOS_TC_HANDLE:-77:}"
CHAOS_NET_IFACE="${CHAOS_NET_IFACE:-}"
SUDO="${SUDO:-sudo}"

run_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        "$SUDO" "$@"
    fi
}

detect_iface() {
    if [ -n "$CHAOS_NET_IFACE" ]; then
        printf '%s\n' "$CHAOS_NET_IFACE"
        return
    fi
    ip route show default 2>/dev/null | awk '/default/ {print $5; exit}'
}

iptables_bin() {
    if command -v iptables >/dev/null 2>&1; then
        printf 'iptables\n'
    elif command -v iptables-legacy >/dev/null 2>&1; then
        printf 'iptables-legacy\n'
    else
        return 1
    fi
}

ensure_chain() {
    local ipt
    ipt="$(iptables_bin)"
    run_root "$ipt" -N "$CHAOS_CHAIN" 2>/dev/null || true
    run_root "$ipt" -C INPUT -j "$CHAOS_CHAIN" 2>/dev/null || \
        run_root "$ipt" -I INPUT -j "$CHAOS_CHAIN"
    run_root "$ipt" -C OUTPUT -j "$CHAOS_CHAIN" 2>/dev/null || \
        run_root "$ipt" -I OUTPUT -j "$CHAOS_CHAIN"
}

chaos_drop_host() {
    local host="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -s "$host" -j DROP
    run_root "$ipt" -A "$CHAOS_CHAIN" -d "$host" -j DROP
}

chaos_drop_host_in() {
    local host="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -s "$host" -j DROP
}

chaos_drop_host_out() {
    local host="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -d "$host" -j DROP
}

chaos_drop_port() {
    local port="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -p tcp --sport "$port" -j DROP
    run_root "$ipt" -A "$CHAOS_CHAIN" -p tcp --dport "$port" -j DROP
}

chaos_drop_port_in() {
    local port="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -p tcp --dport "$port" -j DROP
}

chaos_drop_port_out() {
    local port="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -p tcp --sport "$port" -j DROP
}

chaos_drop_dport_out() {
    local port="$1"
    local ipt
    ipt="$(iptables_bin)"
    ensure_chain
    run_root "$ipt" -A "$CHAOS_CHAIN" -p tcp --dport "$port" -j DROP
}

chaos_heal_iptables() {
    local ipt
    ipt="$(iptables_bin)"
    run_root "$ipt" -F "$CHAOS_CHAIN" 2>/dev/null || true
}

chaos_net_status() {
    local ipt iface
    ipt="$(iptables_bin)"
    iface="$(detect_iface)"
    echo "iptables chain: $CHAOS_CHAIN"
    run_root "$ipt" -S "$CHAOS_CHAIN" 2>/dev/null || true
    if [ -n "$iface" ] && command -v tc >/dev/null 2>&1; then
        echo
        echo "tc qdisc dev $iface:"
        run_root tc qdisc show dev "$iface" 2>/dev/null || true
    fi
}

chaos_delay() {
    local delay="$1"
    local jitter="${2:-}"
    local iface
    iface="$(detect_iface)"
    [ -n "$iface" ] || { echo "cannot detect network interface" >&2; return 1; }
    if [ -n "$jitter" ]; then
        run_root tc qdisc replace dev "$iface" root handle "$CHAOS_TC_HANDLE" netem delay "$delay" "$jitter"
    else
        run_root tc qdisc replace dev "$iface" root handle "$CHAOS_TC_HANDLE" netem delay "$delay"
    fi
}

chaos_loss() {
    local pct="$1"
    local iface
    iface="$(detect_iface)"
    [ -n "$iface" ] || { echo "cannot detect network interface" >&2; return 1; }
    run_root tc qdisc replace dev "$iface" root handle "$CHAOS_TC_HANDLE" netem loss "$pct"
}

chaos_throttle() {
    local rate="$1"
    local iface
    iface="$(detect_iface)"
    [ -n "$iface" ] || { echo "cannot detect network interface" >&2; return 1; }
    run_root tc qdisc replace dev "$iface" root handle "$CHAOS_TC_HANDLE" tbf rate "$rate" burst 32kbit latency 400ms
}

chaos_heal_tc() {
    local iface
    iface="$(detect_iface)"
    [ -n "$iface" ] || return 0
    if run_root tc qdisc show dev "$iface" 2>/dev/null | grep -q " ${CHAOS_TC_HANDLE}"; then
        run_root tc qdisc del dev "$iface" root 2>/dev/null || true
    fi
}
