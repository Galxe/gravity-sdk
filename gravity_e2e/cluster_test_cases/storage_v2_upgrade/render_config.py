#!/usr/bin/env python3
"""
Render cluster.toml and genesis.toml from templates and test parameters.

Same mechanism as cluster_test_cases/rolling_upgrade, hardened like
storage_v2_baseline's renderer: the tracked templates carry placeholders,
the untracked test_params.toml supplies

- [source]            -> {{SOURCE}} on every node (the OLD, pre-upgrade
                         binary; bin_path / project_path / github+rev,
                         exactly the forms cluster/deploy.sh
                         resolve_source accepts),
- [hardforks]         -> {{HARDFORKS}} (merged into genesis.json .config
                         by cluster/genesis.sh),
- [genesis_contracts] -> {{GENESIS_CONTRACTS_REPO}} / {{GENESIS_CONTRACTS_REF}},

and the rendered files are gitignored via the case-local .gitignore.

Self-contained on purpose (no imports beyond stdlib) so it runs as a bare
script from the case directory:

    python render_config.py                 # uses test_params.toml
    python render_config.py my_params.toml  # custom params file
"""

import re
import sys
import time
from pathlib import Path

try:
    import tomllib
except ImportError:
    import tomli as tomllib

SCRIPT_DIR = Path(__file__).resolve().parent
SOURCE_PLACEHOLDER = "{{SOURCE}}"
HARDFORKS_PLACEHOLDER = "{{HARDFORKS}}"
REPO_PLACEHOLDER = "{{GENESIS_CONTRACTS_REPO}}"
REF_PLACEHOLDER = "{{GENESIS_CONTRACTS_REF}}"

# Relative timestamp-fork offsets: "+45m", "+3600s", "+2h" (anchored to
# render time). Lets test_params schedule a timestamp fork (e.g. alphaTime)
# a fixed distance into the future without hand-computing unix epochs —
# rolling_upgrade's mechanism (fork point computed once before the run,
# config never touched mid-test), parameterized. Re-render shortly before
# each run so the anchor stays fresh.
_RELATIVE_TIME_RE = re.compile(r"^\+(\d+)([smh])$")
_UNIT_SECONDS = {"s": 1, "m": 60, "h": 3600}


def _relative_offset_s(value):
    """Seconds encoded by a "+NN[smh]" string, or None when not that form."""
    if not isinstance(value, str):
        return None
    match = _RELATIVE_TIME_RE.match(value.strip())
    if not match:
        return None
    return int(match.group(1)) * _UNIT_SECONDS[match.group(2)]


def resolve_hardfork_value(value, now=None):
    """Resolve one [hardforks] value: ints pass through, "+NN[smh]" strings
    become ``now + offset``, and a table of named "+NN[smh]" components
    becomes ``now + sum(components)`` (unix seconds) — the schedule formula
    ``alphaTime = render_time + upgrade_budget + stability_window + margin``
    with each component tunable in test_params. Anything else raises."""
    if isinstance(value, int):
        return value
    offset = _relative_offset_s(value)
    if offset is not None:
        return int(now if now is not None else time.time()) + offset
    if isinstance(value, dict) and value:
        total = 0
        for name, part in value.items():
            part_offset = _relative_offset_s(part)
            if part_offset is None:
                raise ValueError(
                    f"unsupported schedule component {name} = {part!r} — "
                    f"each component must be a '+NN[smh]' string"
                )
            total += part_offset
        return int(now if now is not None else time.time()) + total
    raise ValueError(
        f"unsupported hardfork value {value!r} — use an int, '+NN[smh]', or "
        f"a table of '+NN[smh]' components"
    )


def source_to_inline_toml(source: dict) -> str:
    """Render a [source] table as a TOML inline table string."""
    parts = [f'{key} = "{value}"' for key, value in source.items()]
    return "{ " + ", ".join(parts) + " }"


def hardforks_to_toml(hardforks: dict, now=None) -> str:
    """Render the [hardforks] table as TOML key-value lines, resolving
    relative "+NN[smh]" values against ``now`` (see resolve_hardfork_value)."""
    return "\n".join(
        f"{key} = {resolve_hardfork_value(value, now)}"
        for key, value in hardforks.items()
    )


def render_cluster_toml(template: str, source: dict) -> str:
    """Substitute the OLD-binary source into every node of the template.

    Raises ValueError when the template has no placeholder or the source
    table is empty — both would otherwise produce a cluster.toml that
    deploy.sh rejects much later with a fuzzier error.
    """
    if SOURCE_PLACEHOLDER not in template:
        raise ValueError(f"template has no {SOURCE_PLACEHOLDER} placeholder")
    if not source:
        raise ValueError(
            "empty [source] table — need bin_path, project_path, or github+rev"
        )
    return template.replace(SOURCE_PLACEHOLDER, source_to_inline_toml(source))


def render_genesis_toml(
    template: str, hardforks: dict, contracts: dict, now=None
) -> str:
    """Substitute hardforks and genesis-contract pin into the template.

    hardforks may be empty (no extra forks -> the case skips the hardfork
    wait); values may use the relative "+NN[smh]" form. The contracts
    repo+ref are required — an empty ref would make cluster/genesis.sh
    check out garbage much later.
    """
    for placeholder in (HARDFORKS_PLACEHOLDER, REPO_PLACEHOLDER, REF_PLACEHOLDER):
        if placeholder not in template:
            raise ValueError(f"template has no {placeholder} placeholder")
    repo = contracts.get("repo")
    ref = contracts.get("ref")
    if not repo or not ref:
        raise ValueError(
            "[genesis_contracts] must set both repo and ref "
            f"(got repo={repo!r}, ref={ref!r})"
        )
    rendered = template.replace(
        HARDFORKS_PLACEHOLDER, hardforks_to_toml(hardforks, now)
    )
    rendered = rendered.replace(REPO_PLACEHOLDER, repo)
    return rendered.replace(REF_PLACEHOLDER, ref)


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

    cluster_tpl = (SCRIPT_DIR / "cluster.toml.tpl").read_text()
    cluster = render_cluster_toml(cluster_tpl, params.get("source", {}))
    (SCRIPT_DIR / "cluster.toml").write_text(cluster)

    now = time.time()
    genesis_tpl = (SCRIPT_DIR / "genesis.toml.tpl").read_text()
    genesis = render_genesis_toml(
        genesis_tpl,
        params.get("hardforks", {}),
        params.get("genesis_contracts", {}),
        now=now,
    )
    (SCRIPT_DIR / "genesis.toml").write_text(genesis)

    resolved_hardforks = {
        key: resolve_hardfork_value(value, now)
        for key, value in params.get("hardforks", {}).items()
    }
    print(f"Rendered from {params_file.name}:")
    print(f"  source:     {source_to_inline_toml(params['source'])}")
    print(f"  hardforks:  {resolved_hardforks}")
    print(f"  contracts:  {params['genesis_contracts'].get('ref')}")


if __name__ == "__main__":
    main()
