"""
Pytest configuration and fixtures for Hardfork + Bridge E2E tests.

Provides:
  - mock_anvil_metadata: parsed metadata from hooks.py's pre-loaded MockAnvil
  - bridge_verify_timeout: configurable timeout for NativeMinted polling
"""

import json
import logging
import os
import sys
from pathlib import Path

_current_dir = Path(__file__).resolve().parent
# Add gravity_e2e parent to path
_gravity_e2e_parent = _current_dir
while _gravity_e2e_parent.name != "gravity_e2e" or not (_gravity_e2e_parent / "gravity_e2e").is_dir():
    _gravity_e2e_parent = _gravity_e2e_parent.parent
    if _gravity_e2e_parent == _gravity_e2e_parent.parent:
        break
if str(_gravity_e2e_parent) not in sys.path:
    sys.path.insert(0, str(_gravity_e2e_parent))

import pytest

LOG = logging.getLogger(__name__)


def pytest_addoption(parser):
    """Add bridge-specific command line options."""
    parser.addoption(
        "--bridge-verify-timeout",
        action="store",
        default="120",
        help="Timeout in seconds for verifying NativeMinted events (default: 120)",
    )


@pytest.fixture(scope="session")
def bridge_verify_timeout(request) -> int:
    """Timeout for verifying all NativeMinted events."""
    return int(request.config.getoption("--bridge-verify-timeout"))


@pytest.fixture(scope="module")
def mock_anvil_metadata() -> dict:
    """
    Read MockAnvil metadata written by hooks.py.

    Returns dict with: port, rpc_url, bridge_count, amount,
    recipient, sender_address, portal_address, nonces, finalized_block.
    """
    metadata_file = _current_dir / "mock_anvil_metadata.json"
    if not metadata_file.exists():
        pytest.skip(
            "mock_anvil_metadata.json not found — "
            "MockAnvil was not started by hooks.py"
        )

    metadata = json.loads(metadata_file.read_text())
    LOG.info(
        f"[fixture] Read MockAnvil metadata: "
        f"{metadata['bridge_count']} events, "
        f"finalized_block={metadata['finalized_block']}"
    )
    return metadata
