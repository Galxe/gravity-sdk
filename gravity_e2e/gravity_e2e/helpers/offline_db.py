"""
Offline ``gravity_node db`` command helper.

Runs reth-style database subcommands against a *stopped* node's datadir
(gravity_node forwards ``db`` to reth's ``Commands::Db``, see worktree
``bin/gravity_node/src/cli.rs:180-182`` and greth
``crates/ethereum/cli/src/interface.rs:281-283``). This is the common
foundation for the storage-v2 rolling-upgrade cases:

- TC4: run ``db migrate-changesets`` to move legacy changeset tables into
  static-file segments, then verify the flip.
- TC1/TC2: probe the ``gravity_storage_settings`` entry in the Metadata
  table (missing / present-legacy / present-static-files).
- TC1/TC4: check the on-disk changeset segment files and their ``.csoff``
  sidecars without running any binary.

Everything is case-agnostic: binary path, datadir, chain spec and the
optional static-files dir are always passed in explicitly via
:class:`OfflineDbEnv`. The helper never asserts — every call returns a
structured result object and the pass/fail semantics stay in the case.

This module is synchronous by design, matching
``helpers/storage_anchors.py``: offline db commands run while the node is
stopped, callers are pytest cases that can call blocking code directly,
and async tests can wrap calls in ``asyncio.to_thread`` when needed.

Typical TC4 usage (node already stopped; three steps)::

    from gravity_e2e.helpers.offline_db import (
        ACCOUNT_CHANGESETS_TABLE, STORAGE_CHANGESETS_TABLE,
        OfflineDbEnv, SettingsState, count_table_entries,
        inspect_changeset_static_files, migrate_changesets,
        read_storage_settings,
    )

    env = OfflineDbEnv(
        binary=cluster_root / "bin" / "gravity_node",
        datadir=node_datadir,             # the reth datadir of the node
        chain=node_dir / "genesis.json",  # chain spec file used by the node
    )

    # 1. Migrate (idempotent in the binary: rerunning after any crash
    #    window recovers, see greth db/migrate_changesets.rs:7-12).
    result = migrate_changesets(env)
    assert result.ok, result.summary()

    # 2. Settings entry flipped to static files.
    probe = read_storage_settings(env)
    assert probe.state is SettingsState.PRESENT_STATIC_FILES, probe.summary()

    # 3. Database tables cleared + segment files and sidecars on disk.
    for table in (ACCOUNT_CHANGESETS_TABLE, STORAGE_CHANGESETS_TABLE):
        count = count_table_entries(env, table)
        assert count.count == 0, count.summary()
    layout = inspect_changeset_static_files(env.datadir)
    assert layout.has_segment_files and layout.has_sidecar_files

Known limitations (documented on purpose):

- Unit tests exercise this module against a shell stub only. Validation of
  the real binary's argument surface is deferred to the TC1 integration
  stage — if greth renames a flag, TC1 is where it surfaces.
- On the gravity RocksDB backend, ``db stats`` reports a hardcoded ``0``
  for every table's "# Entries" (greth db/stats.rs:92-107, "Placeholder
  for RocksDB"), so :func:`db_stats` is only useful for table presence /
  static-file shapes. For the TC4 "changeset tables cleared" criterion use
  :func:`count_table_entries`, which walks the table via
  ``db list <TABLE> --count`` (greth db/list.rs:117-121) and prints an
  exact row count.

greth source references below are file:line into the local checkout
``/home/neko/gravity/gravity-reth-merge-v2.3.0``.
"""

from __future__ import annotations

import json
import logging
import re
import subprocess
import time
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Union

LOG = logging.getLogger(__name__)

# Default timeout for quick read-only commands (stats / get / list).
DEFAULT_COMMAND_TIMEOUT_S = 120.0
# Migration walks and rewrites both changeset tables; minutes-scale on big
# datadirs. Callers with huge histories should raise this explicitly.
DEFAULT_MIGRATE_TIMEOUT_S = 900.0

# Metadata table name and the storage-settings key:
# - table Metadata { Key = String, Value = Vec<u8> }:
#   crates/storage/db-api/src/tables/mod.rs:541-545; ``db get`` parses the
#   table argument by exact table name (tables/mod.rs:239-268).
# - key literal: crates/storage/storage-api/src/metadata.rs:10.
METADATA_TABLE = "Metadata"
GRAVITY_STORAGE_SETTINGS_KEY = "gravity_storage_settings"

# Changeset table names: crates/storage/db-api/src/tables/mod.rs:445,454.
ACCOUNT_CHANGESETS_TABLE = "AccountChangeSets"
STORAGE_CHANGESETS_TABLE = "StorageChangeSets"

# Changeset static-file segment name strings ("account-change-sets" /
# "storage-change-sets"): crates/static-file/types/src/segment.rs:78-79.
# Data file name pattern "static_file_{segment}_{start}_{end}":
# segment.rs:129-133 (parse: segment.rs:166-180).
_CHANGESET_DATA_RE = re.compile(
    r"^static_file_(account-change-sets|storage-change-sets)_(\d+)_(\d+)$"
)
# Changeset-offsets sidecar = data file name + ".csoff":
# crates/storage/nippy-jar/src/lib.rs:63,246-248 and
# crates/storage/provider/src/providers/static_file/writer.rs:309.
_CHANGESET_SIDECAR_RE = re.compile(
    r"^static_file_(account-change-sets|storage-change-sets)_(\d+)_(\d+)\.csoff$"
)

# ``db get ... --raw`` prints the value as one hex::encode_prefixed line:
# crates/cli/commands/src/db/get.rs:227-238.
_RAW_HEX_LINE_RE = re.compile(r"^0x(?:[0-9a-fA-F]{2})+$")
# ``db get`` miss marker (tracing error!, exit code stays 0):
# crates/cli/commands/src/db/get.rs:236-241.
_NO_CONTENT_MARKER = "No content for the given table key"
# ``db list <TABLE> --count`` output: crates/cli/commands/src/db/list.rs:121.
_ENTRY_COUNT_RE = re.compile(r"^(\d+) entries found\.$")
# reth's stdout tracing layer may emit ANSI colors (LogArgs default is
# ``--color always``, crates/node/core/src/args/log.rs:387); we both pass
# ``--color never`` and strip defensively.
_ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def _strip_ansi(text: str) -> str:
    return _ANSI_RE.sub("", text)


# ---------------------------------------------------------------------------
# Command execution layer
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class OfflineDbEnv:
    """Where and how to run offline db commands. All paths are explicit.

    Attributes:
        binary: path to the gravity_node executable.
        datadir: the node's reth datadir (must contain the ``db`` dir;
            the binary itself enforces existence, greth db/mod.rs:96-104).
        chain: chain spec — in e2e always the path to the node's
            ``genesis.json`` (a built-in chain name also parses,
            greth common.rs:48-59).
        static_files_dir: optional ``--datadir.static-files`` override;
            when None the binary uses ``<datadir>/static_files``
            (greth dirs.rs:293-299).
    """

    binary: Union[str, Path]
    datadir: Union[str, Path]
    chain: Union[str, Path]
    static_files_dir: Optional[Union[str, Path]] = None

    def db_argv(self, subcommand_args: Sequence[str]) -> List[str]:
        """Full argv for ``gravity_node db <...>``.

        Argument surface (greth sources):
        - ``db`` subcommand: crates/ethereum/cli/src/interface.rs:281-283.
        - The db command flattens EnvironmentArgs before its subcommand
          (crates/cli/commands/src/db/mod.rs:28-34), so ``--datadir`` /
          ``--chain`` go between ``db`` and the subcommand — same shape as
          greth's own parse test (db/mod.rs:229-240).
        - ``--datadir``: crates/node/core/src/args/datadir_args.rs:19-20.
        - ``--chain``: crates/cli/commands/src/common.rs:48-59.
        - ``--color never`` (global LogArgs flag; default "always" would
          ANSI-color the log lines): crates/node/core/src/args/log.rs:
          124-132,394-411.
        - ``--datadir.static-files``: datadir_args.rs:22-29.
        """
        argv = [
            str(self.binary),
            "db",
            "--datadir",
            str(self.datadir),
            "--chain",
            str(self.chain),
            "--color",
            "never",
        ]
        if self.static_files_dir is not None:
            argv += ["--datadir.static-files", str(self.static_files_dir)]
        argv += list(subcommand_args)
        return argv


@dataclass
class DbCommandResult:
    """Outcome of one db command invocation. Output is kept in full."""

    argv: List[str]
    returncode: Optional[int]  # None: timed out or failed to spawn
    stdout: str
    stderr: str
    duration_s: float
    timed_out: bool = False
    spawn_error: Optional[str] = None  # e.g. binary missing

    @property
    def ok(self) -> bool:
        return self.returncode == 0

    def summary(self) -> str:
        """Multi-line diagnostic dump, suitable for pytest assert messages."""
        status = (
            "timed out"
            if self.timed_out
            else (self.spawn_error or f"exit code {self.returncode}")
        )
        return (
            f"command: {' '.join(self.argv)}\n"
            f"status: {status} (after {self.duration_s:.1f}s)\n"
            f"--- stdout ---\n{self.stdout}\n"
            f"--- stderr ---\n{self.stderr}"
        )


def _decode_stream(raw: Union[str, bytes, None]) -> str:
    if raw is None:
        return ""
    if isinstance(raw, bytes):
        return raw.decode("utf-8", errors="replace")
    return raw


def run_db_command(
    env: OfflineDbEnv,
    subcommand_args: Sequence[str],
    timeout: float = DEFAULT_COMMAND_TIMEOUT_S,
) -> DbCommandResult:
    """Execute ``gravity_node db <subcommand_args...>`` and capture everything.

    Never raises for command failure: nonzero exit, timeout (process gets
    killed) and unspawnable binary all come back as a result object with
    ``ok == False`` and full output preserved for debugging.
    """
    argv = env.db_argv(subcommand_args)
    LOG.info("Running offline db command: %s (timeout=%.0fs)", " ".join(argv), timeout)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        duration = time.monotonic() - start
        result = DbCommandResult(
            argv=argv,
            returncode=None,
            stdout=_decode_stream(exc.stdout),
            stderr=_decode_stream(exc.stderr),
            duration_s=duration,
            timed_out=True,
        )
        LOG.error("Offline db command timed out:\n%s", result.summary())
        return result
    except OSError as exc:
        duration = time.monotonic() - start
        result = DbCommandResult(
            argv=argv,
            returncode=None,
            stdout="",
            stderr="",
            duration_s=duration,
            spawn_error=f"failed to spawn {argv[0]}: {exc}",
        )
        LOG.error("Offline db command failed to spawn:\n%s", result.summary())
        return result

    duration = time.monotonic() - start
    result = DbCommandResult(
        argv=argv,
        returncode=proc.returncode,
        stdout=proc.stdout,
        stderr=proc.stderr,
        duration_s=duration,
    )
    if result.ok:
        LOG.info("Offline db command succeeded in %.1fs", duration)
    else:
        LOG.error("Offline db command failed:\n%s", result.summary())
    return result


# ---------------------------------------------------------------------------
# migrate-changesets (TC4)
# ---------------------------------------------------------------------------


def migrate_changesets(
    env: OfflineDbEnv, timeout: float = DEFAULT_MIGRATE_TIMEOUT_S
) -> DbCommandResult:
    """Run ``db migrate-changesets`` (legacy changeset tables -> static files).

    The subcommand takes no arguments of its own (unit struct ``Command``,
    greth db/migrate_changesets.rs:32-33; mounted as ``migrate-changesets``
    via Subcommands::MigrateChangesets, db/mod.rs:59-60). Rerunning is safe:
    crash-window recovery and idempotence live in the binary
    (migrate_changesets.rs:7-12,49-73); the helper just executes and
    reports, so TC4 can rerun and assert on both results.
    """
    return run_db_command(env, ["migrate-changesets"], timeout=timeout)


# ---------------------------------------------------------------------------
# gravity_storage_settings probe (TC1/TC2/TC4)
# ---------------------------------------------------------------------------


class SettingsState(Enum):
    """Three-state result of probing the gravity_storage_settings entry."""

    MISSING = "missing"  # no entry: database predates the settings (legacy)
    PRESENT_LEGACY = "present_legacy"  # changesets_in_static_files == false
    PRESENT_STATIC_FILES = "present_static_files"  # == true


@dataclass
class StorageSettingsProbe:
    """Result of :func:`read_storage_settings`.

    ``state`` is None when the probe could not determine a state (command
    failed / output unparseable); ``error`` then says why and ``command``
    keeps the full output.
    """

    command: DbCommandResult
    state: Optional[SettingsState] = None
    settings: Optional[Dict] = None  # decoded JSON when present
    error: Optional[str] = None

    def summary(self) -> str:
        head = f"storage settings probe: state={self.state}, error={self.error}"
        return head + "\n" + self.command.summary()


def read_storage_settings(
    env: OfflineDbEnv, timeout: float = DEFAULT_COMMAND_TIMEOUT_S
) -> StorageSettingsProbe:
    """Read the ``gravity_storage_settings`` Metadata entry, three-state.

    Runs ``db get mdbx Metadata gravity_storage_settings --raw``:

    - ``get mdbx <TABLE> <KEY> [--raw]``: greth db/get.rs:31-56. The bare
      key string is accepted because clap's value parser JSON-wraps
      non-JSON input (get.rs:287-293) and the Metadata key type is String
      (tables/mod.rs:541-545).
    - ``--raw`` prints the raw value bytes as a single 0x-hex line
      (get.rs:227-238); for Metadata the value is the serde_json encoding
      of GravityStorageSettings (models/metadata.rs:39-41), so the hex
      decodes straight to a JSON object. A missing field defaults to
      false, unknown fields are ignored (models/metadata.rs:17-22) —
      mirrored here via ``dict.get``.
    - A missing entry prints "No content for the given table key." and
      still exits 0 (get.rs:236-241) -> :attr:`SettingsState.MISSING`.

    Note: reth itself treats an *undecodable* entry as legacy
    (storage-api/src/metadata.rs:24-28); this probe instead reports it as
    an error so a corrupted entry cannot silently pass a TC1 assertion.
    """
    command = run_db_command(
        env,
        ["get", "mdbx", METADATA_TABLE, GRAVITY_STORAGE_SETTINGS_KEY, "--raw"],
        timeout=timeout,
    )
    if command.timed_out or command.spawn_error is not None:
        return StorageSettingsProbe(
            command=command, error=command.spawn_error or "command timed out"
        )
    if not command.ok:
        return StorageSettingsProbe(
            command=command, error=f"db get exited with code {command.returncode}"
        )

    stdout = _strip_ansi(command.stdout)
    hex_lines = [
        line.strip()
        for line in stdout.splitlines()
        if _RAW_HEX_LINE_RE.match(line.strip())
    ]
    if hex_lines:
        payload = bytes.fromhex(hex_lines[-1][2:])
        try:
            settings = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError):
            return StorageSettingsProbe(
                command=command,
                error=f"undecodable settings payload: {hex_lines[-1]}",
            )
        if not isinstance(settings, dict):
            return StorageSettingsProbe(
                command=command,
                error=f"settings payload is not a JSON object: {settings!r}",
            )
        state = (
            SettingsState.PRESENT_STATIC_FILES
            if settings.get("changesets_in_static_files", False)
            else SettingsState.PRESENT_LEGACY
        )
        return StorageSettingsProbe(command=command, state=state, settings=settings)

    combined = stdout + "\n" + _strip_ansi(command.stderr)
    if _NO_CONTENT_MARKER in combined:
        return StorageSettingsProbe(command=command, state=SettingsState.MISSING)

    return StorageSettingsProbe(
        command=command,
        error="db get output contained neither a raw value nor a no-content marker",
    )


# ---------------------------------------------------------------------------
# db stats (TC1/TC4)
# ---------------------------------------------------------------------------


@dataclass
class DbStatsResult:
    """Result of :func:`db_stats`: database table name -> "# Entries"."""

    command: DbCommandResult
    entries: Dict[str, int] = field(default_factory=dict)
    error: Optional[str] = None

    def summary(self) -> str:
        head = f"db stats: {len(self.entries)} tables parsed, error={self.error}"
        return head + "\n" + self.command.summary()


def parse_db_stats(text: str) -> Dict[str, int]:
    """Parse ``db stats`` output into {table name: entry count}.

    Only the database table (header "Table Name" / "# Entries",
    greth db/stats.rs:72-79) is read; the static-files "Segment" table
    printed above it (stats.rs:55-61) has no entry counts and is skipped.
    Tables render in comfy_table ASCII_MARKDOWN (stats.rs:71): one
    ``| cell | cell |`` row per line. Separator rows (all dashes) and the
    "Tables" total row (empty entries cell) are skipped; the "Freelist"
    row parses like any other and is left in for the caller to ignore.

    WARNING: on the gravity RocksDB backend every count is a hardcoded 0
    placeholder (stats.rs:92-107) — use :func:`count_table_entries` for
    real emptiness checks.
    """
    entries: Dict[str, int] = {}
    name_idx: Optional[int] = None
    entries_idx: Optional[int] = None
    for line in _strip_ansi(text).splitlines():
        stripped = line.strip()
        if not (stripped.startswith("|") and stripped.endswith("|")):
            # Blank line / log noise ends any table in progress.
            name_idx = entries_idx = None
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if name_idx is None:
            if "Table Name" in cells and "# Entries" in cells:
                name_idx = cells.index("Table Name")
                entries_idx = cells.index("# Entries")
            continue
        if all(not cell or set(cell) == {"-"} for cell in cells):
            continue  # markdown header separator or the pre-total dash row
        try:
            count = int(cells[entries_idx])
        except (IndexError, ValueError):
            continue  # e.g. the "Tables" total row (empty entries cell)
        name = cells[name_idx]
        if name:
            entries[name] = count
    return entries


def db_stats(
    env: OfflineDbEnv,
    timeout: float = DEFAULT_COMMAND_TIMEOUT_S,
    skip_consistency_checks: bool = False,
) -> DbStatsResult:
    """Run ``db stats`` (subcommand: greth db/mod.rs:39-40) and parse it.

    ``skip_consistency_checks`` appends ``--skip-consistency-checks``
    (stats.rs:20-22), useful when probing a datadir that was not shut down
    cleanly. See the class docstring / module docstring for the RocksDB
    "# Entries" placeholder caveat.
    """
    args = ["stats"]
    if skip_consistency_checks:
        args.append("--skip-consistency-checks")
    command = run_db_command(env, args, timeout=timeout)
    if command.timed_out or command.spawn_error is not None:
        return DbStatsResult(
            command=command, error=command.spawn_error or "command timed out"
        )
    if not command.ok:
        return DbStatsResult(
            command=command, error=f"db stats exited with code {command.returncode}"
        )
    entries = parse_db_stats(command.stdout)
    if not entries:
        return DbStatsResult(
            command=command, error="no 'Table Name' table found in db stats output"
        )
    return DbStatsResult(command=command, entries=entries)


# ---------------------------------------------------------------------------
# db list --count: exact table entry count (TC4 "tables cleared")
# ---------------------------------------------------------------------------


@dataclass
class TableCountResult:
    """Result of :func:`count_table_entries`."""

    command: DbCommandResult
    table: str
    count: Optional[int] = None
    error: Optional[str] = None

    def summary(self) -> str:
        head = f"count[{self.table}]: {self.count}, error={self.error}"
        return head + "\n" + self.command.summary()


def parse_entry_count(text: str) -> Optional[int]:
    """Extract N from the ``db list --count`` output line "N entries found."

    Line format: greth db/list.rs:121. Returns None when no such line
    exists (log noise is ignored).
    """
    count = None
    for line in _strip_ansi(text).splitlines():
        match = _ENTRY_COUNT_RE.match(line.strip())
        if match:
            count = int(match.group(1))
    return count


def count_table_entries(
    env: OfflineDbEnv,
    table: str,
    timeout: float = DEFAULT_COMMAND_TIMEOUT_S,
) -> TableCountResult:
    """Exact entry count of a database table via ``db list <TABLE> --count``.

    Argument surface: positional table name then ``--count``
    (greth db/list.rs:15-52; count printout db/list.rs:117-121). Unlike
    ``db stats`` this walks the table with a cursor, so it is exact on the
    RocksDB backend — TC4 uses it to assert both changeset tables are
    empty after migration (see ACCOUNT_CHANGESETS_TABLE /
    STORAGE_CHANGESETS_TABLE, tables/mod.rs:445,454).
    """
    command = run_db_command(env, ["list", table, "--count"], timeout=timeout)
    if command.timed_out or command.spawn_error is not None:
        return TableCountResult(
            command=command,
            table=table,
            error=command.spawn_error or "command timed out",
        )
    if not command.ok:
        return TableCountResult(
            command=command,
            table=table,
            error=f"db list exited with code {command.returncode}",
        )
    count = parse_entry_count(command.stdout)
    if count is None:
        return TableCountResult(
            command=command,
            table=table,
            error="no 'N entries found.' line in db list output",
        )
    return TableCountResult(command=command, table=table, count=count)


# ---------------------------------------------------------------------------
# Static-file layout check: pure fs, no binary (TC1/TC4)
# ---------------------------------------------------------------------------


@dataclass
class ChangesetStaticFiles:
    """Changeset-related files found under a node's static_files dir.

    ``exists`` is False when the directory itself is missing (all lists
    are then empty). Data files match
    ``static_file_{account,storage}-change-sets_{start}_{end}`` exactly
    (segment.rs:129-133,78-79); sidecars are the same name + ``.csoff``
    (nippy-jar lib.rs:63,246-248). The ``.idx``/``.off``/``.conf``
    satellites (nippy-jar lib.rs:57-61) and other segments are ignored.
    """

    static_files_dir: Path
    exists: bool
    account_segments: List[Path] = field(default_factory=list)
    storage_segments: List[Path] = field(default_factory=list)
    account_sidecars: List[Path] = field(default_factory=list)
    storage_sidecars: List[Path] = field(default_factory=list)

    @property
    def has_segment_files(self) -> bool:
        """Any changeset segment data file present (TC1 asserts none)."""
        return bool(self.account_segments or self.storage_segments)

    @property
    def has_sidecar_files(self) -> bool:
        """Any ``.csoff`` sidecar present (TC4 asserts present)."""
        return bool(self.account_sidecars or self.storage_sidecars)


def inspect_changeset_static_files(
    datadir: Union[str, Path],
    static_files_dir: Optional[Union[str, Path]] = None,
) -> ChangesetStaticFiles:
    """Scan the static_files dir for changeset segments and sidecars.

    Pure filesystem check — no binary involved, safe on live or stopped
    nodes. ``static_files_dir`` overrides the default
    ``<datadir>/static_files`` location (greth dirs.rs:293-299), matching
    a node started with ``--datadir.static-files``.
    """
    sf_dir = (
        Path(static_files_dir)
        if static_files_dir is not None
        else Path(datadir) / "static_files"
    )
    result = ChangesetStaticFiles(static_files_dir=sf_dir, exists=sf_dir.is_dir())
    if not result.exists:
        LOG.info("static_files dir does not exist: %s", sf_dir)
        return result

    for path in sorted(sf_dir.iterdir()):
        data_match = _CHANGESET_DATA_RE.match(path.name)
        if data_match:
            if data_match.group(1) == "account-change-sets":
                result.account_segments.append(path)
            else:
                result.storage_segments.append(path)
            continue
        sidecar_match = _CHANGESET_SIDECAR_RE.match(path.name)
        if sidecar_match:
            if sidecar_match.group(1) == "account-change-sets":
                result.account_sidecars.append(path)
            else:
                result.storage_sidecars.append(path)

    LOG.info(
        "Changeset static files in %s: %d account / %d storage segments, "
        "%d / %d .csoff sidecars",
        sf_dir,
        len(result.account_segments),
        len(result.storage_segments),
        len(result.account_sidecars),
        len(result.storage_sidecars),
    )
    return result
