#!/usr/bin/env python3
"""
Render cluster.toml and genesis.toml from templates and test parameters.

Usage:
    python render_config.py                     # uses test_params.toml
    python render_config.py my_params.toml      # uses custom params file
"""

import sys
from pathlib import Path

try:
    import tomllib
except ImportError:
    import tomli as tomllib

SCRIPT_DIR = Path(__file__).resolve().parent


def source_to_inline_toml(source: dict) -> str:
    """Convert source dict to TOML inline table string."""
    parts = []
    for k, v in source.items():
        parts.append(f'{k} = "{v}"')
    return "{ " + ", ".join(parts) + " }"


def hardforks_to_toml(hardforks: dict) -> str:
    """Convert hardforks dict to TOML key-value lines."""
    return "\n".join(f"{k} = {v}" for k, v in hardforks.items())


def main():
    params_file = SCRIPT_DIR / (sys.argv[1] if len(sys.argv) > 1 else "test_params.toml")
    if not params_file.exists():
        print(f"Error: params file not found: {params_file}")
        print(f"Copy test_params.toml.example to test_params.toml and edit it.")
        sys.exit(1)

    with open(params_file, "rb") as f:
        params = tomllib.load(f)

    source_inline = source_to_inline_toml(params["source"])
    hardforks_text = hardforks_to_toml(params.get("hardforks", {}))
    contracts = params.get("genesis_contracts", {})

    cluster = (SCRIPT_DIR / "cluster.toml.tpl").read_text()
    cluster = cluster.replace("{{SOURCE}}", source_inline)
    (SCRIPT_DIR / "cluster.toml").write_text(cluster)

    genesis = (SCRIPT_DIR / "genesis.toml.tpl").read_text()
    genesis = genesis.replace("{{HARDFORKS}}", hardforks_text)
    genesis = genesis.replace("{{GENESIS_CONTRACTS_REPO}}", contracts.get("repo", ""))
    genesis = genesis.replace("{{GENESIS_CONTRACTS_REF}}", contracts.get("ref", ""))
    (SCRIPT_DIR / "genesis.toml").write_text(genesis)

    print(f"Rendered from {params_file.name}:")
    print(f"  source:     {source_inline}")
    print(f"  hardforks:  {params.get('hardforks', {})}")
    print(f"  contracts:  {contracts.get('ref', 'N/A')}")


if __name__ == "__main__":
    main()
