"""
Test: Batch RPC Metrics Verification.

Verifies that `reth_rpc_server_batch_calls_started_total` counter is correctly
incremented when batch JSON-RPC requests are received, and NOT incremented
for single (non-batch) requests.

Reference: .agent/workflows/verify-batch-rpc-metrics.md
"""

import json
import logging
import re
import urllib.request
from typing import Dict

import pytest
from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# Number of batch requests to send
BATCH_COUNT = 5

# Methods included in each batch request
BATCH_METHODS = ["eth_blockNumber", "eth_chainId"]


def fetch_metric(metrics_url: str, metric_name: str) -> Dict[str, int]:
    """
    Fetch a specific metric from the Prometheus text endpoint.

    Parses lines like:
        reth_rpc_server_batch_calls_started_total{method="eth_blockNumber"} 5

    Returns:
        dict mapping label values to integer counts, e.g.
        {"eth_blockNumber": 5, "eth_chainId": 5}
        Empty dict if metric not found.
    """
    resp = urllib.request.urlopen(metrics_url, timeout=5)
    text = resp.read().decode("utf-8")

    results: Dict[str, int] = {}
    pattern = re.compile(
        rf'^{re.escape(metric_name)}\{{method="([^"]+)"\}}\s+(\d+)',
    )
    for line in text.splitlines():
        m = pattern.match(line)
        if m:
            method, value = m.group(1), int(m.group(2))
            results[method] = value

    return results


def _post_json(url: str, payload) -> dict:
    """Send a JSON POST request and return the parsed response."""
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    resp = urllib.request.urlopen(req, timeout=5)
    return json.loads(resp.read().decode("utf-8"))


def send_batch_rpc(rpc_url: str, methods: list) -> list:
    """Send a single batch JSON-RPC request containing multiple methods."""
    payload = [
        {"jsonrpc": "2.0", "method": m, "params": [], "id": i + 1}
        for i, m in enumerate(methods)
    ]
    return _post_json(rpc_url, payload)


def send_single_rpc(rpc_url: str, method: str) -> dict:
    """Send a single (non-batch) JSON-RPC request."""
    payload = {"jsonrpc": "2.0", "method": method, "params": [], "id": 1}
    return _post_json(rpc_url, payload)


@pytest.mark.asyncio
async def test_batch_rpc_metrics(cluster: Cluster):
    """
    Verify batch RPC metrics follow the specification:

    1. batch_calls_started_total exists after batch requests
    2. Counter increments per-method per batch entry
    3. Single (non-batch) requests do NOT increment batch_calls counters
    4. Regular calls_started_total still works independently
    """
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    assert node.metrics_url, "node1 has no metrics_port configured"

    # ── Step 1: Ensure node is running ──────────────────────────────
    LOG.info("Step 1: Ensuring node is running...")
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become live"

    # ── Step 2: Confirm metrics endpoint is reachable ───────────────
    LOG.info("Step 2: Confirming metrics endpoint is reachable...")
    resp = urllib.request.urlopen(node.metrics_url, timeout=5)
    assert resp.status == 200, f"Metrics endpoint returned {resp.status}"

    # ── Step 3: Record baseline batch metrics ───────────────────────
    LOG.info("Step 3: Recording baseline batch metrics...")
    baseline = fetch_metric(
        node.metrics_url, "reth_rpc_server_batch_calls_started_total"
    )
    LOG.info(f"Baseline batch metrics: {baseline}")

    baseline_values = {method: baseline.get(method, 0) for method in BATCH_METHODS}

    # ── Step 4: Send N batch requests ───────────────────────────────
    LOG.info(f"Step 4: Sending {BATCH_COUNT} batch requests...")
    for i in range(BATCH_COUNT):
        results = send_batch_rpc(node.url, BATCH_METHODS)
        assert isinstance(
            results, list
        ), f"Batch response should be a list, got {type(results)}"
        assert len(results) == len(BATCH_METHODS), (
            f"Batch response should contain {len(BATCH_METHODS)} results, "
            f"got {len(results)}"
        )
    LOG.info(f"Sent {BATCH_COUNT} batch requests successfully")

    # ── Step 5: Verify batch_calls incremented ──────────────────────
    LOG.info("Step 5: Verifying batch_calls metrics incremented...")
    after_batch = fetch_metric(
        node.metrics_url, "reth_rpc_server_batch_calls_started_total"
    )
    LOG.info(f"After batch metrics: {after_batch}")

    for method in BATCH_METHODS:
        expected = baseline_values[method] + BATCH_COUNT
        actual = after_batch.get(method, 0)
        assert (
            actual >= expected
        ), f"batch_calls for {method}: expected >= {expected}, got {actual}"
    LOG.info("✅ Batch call counters incremented correctly")

    # ── Step 6: Send single (non-batch) request ─────────────────────
    LOG.info("Step 6: Sending single (non-batch) request as control...")
    result = send_single_rpc(node.url, "eth_blockNumber")
    assert "result" in result, f"Single RPC failed: {result}"

    # ── Step 7: Verify batch_calls did NOT increment ────────────────
    LOG.info("Step 7: Verifying batch_calls did NOT increment from single request...")
    after_single = fetch_metric(
        node.metrics_url, "reth_rpc_server_batch_calls_started_total"
    )
    LOG.info(f"After single request metrics: {after_single}")

    for method in BATCH_METHODS:
        assert after_single.get(method, 0) == after_batch.get(method, 0), (
            f"batch_calls for {method} changed after single request: "
            f"{after_batch.get(method, 0)} -> {after_single.get(method, 0)}"
        )
    LOG.info("✅ Single request did NOT affect batch counters")

    # ── Step 8: Verify regular call metrics exist ───────────────────
    LOG.info("Step 8: Verifying regular call metrics exist...")
    regular_calls = fetch_metric(
        node.metrics_url, "reth_rpc_server_calls_started_total"
    )
    assert (
        len(regular_calls) > 0
    ), "reth_rpc_server_calls_started_total metrics not found"
    LOG.info(f"✅ Regular call metrics present: {regular_calls}")

    LOG.info("Batch RPC metrics test PASSED!")
