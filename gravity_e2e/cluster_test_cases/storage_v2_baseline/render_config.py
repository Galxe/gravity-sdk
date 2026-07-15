#!/usr/bin/env python3
"""
Render cluster.toml from cluster.toml.tpl and test_params.toml.

This is how the case pins the gravity_node binary WITHOUT touching any
tracked file (same pattern as cluster_test_cases/rolling_upgrade): the
tracked template carries a {{SOURCE}} placeholder, the untracked
test_params.toml supplies the [source] table (bin_path / project_path /
github+rev, exactly the forms cluster/deploy.sh resolve_source accepts),
and the rendered cluster.toml is gitignored via the case-local .gitignore.

Self-contained on purpose (no imports beyond stdlib) so it runs as a bare
script from the case directory:

    python render_config.py                 # uses test_params.toml
    python render_config.py my_params.toml  # custom params file

genesis.toml is tracked directly — nothing in it depends on the binary.
"""

import sys
from pathlib import Path

try:
    import tomllib
except ImportError:
    import tomli as tomllib

SCRIPT_DIR = Path(__file__).resolve().parent
PLACEHOLDER = "{{SOURCE}}"


def source_to_inline_toml(source: dict) -> str:
    """Render a [source] table as a TOML inline table string."""
    parts = [f'{key} = "{value}"' for key, value in source.items()]
    return "{ " + ", ".join(parts) + " }"


def render_cluster_toml(template: str, source: dict) -> str:
    """Substitute the node source into the cluster template.

    Raises ValueError when the template has no placeholder or the source
    table is empty — both would otherwise produce a cluster.toml that
    deploy.sh rejects much later with a fuzzier error.
    """
    if PLACEHOLDER not in template:
        raise ValueError(f"template has no {PLACEHOLDER} placeholder")
    if not source:
        raise ValueError(
            "empty [source] table — need bin_path, project_path, or github+rev"
        )
    return template.replace(PLACEHOLDER, source_to_inline_toml(source))


def main() -> None:
    params_file = SCRIPT_DIR / (
        sys.argv[1] if len(sys.argv) > 1 else "test_params.toml"
    )
    if not params_file.exists():
        print(f"Error: params file not found: {params_file}")
        print("Copy test_params.toml.example to test_params.toml and edit it.")
        sys.exit(1)

    with open(params_file, "rb") as f:
        params = tomllib.load(f)

    template = (SCRIPT_DIR / "cluster.toml.tpl").read_text()
    rendered = render_cluster_toml(template, params.get("source", {}))
    (SCRIPT_DIR / "cluster.toml").write_text(rendered)

    print(f"Rendered cluster.toml from {params_file.name}:")
    print(f"  source: {source_to_inline_toml(params['source'])}")


if __name__ == "__main__":
    main()
