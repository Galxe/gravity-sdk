"""
Unit tests for gravity_e2e.helpers.offline_db

No real gravity_node binary and no cluster: the "binary" is a shell stub
written into tmp_path that records the argv it receives, emits preset
stdout/stderr, and exits with a preset code (optionally sleeping to
exercise the timeout path). Output samples mirror the exact formats
produced by the greth CLI sources; each sample cites its origin
(file:line in /home/neko/gravity/gravity-reth-merge-v2.3.0).

Real-binary argument-surface validation is deliberately out of scope here
and belongs to the TC1 integration stage (see module docstring of
gravity_e2e.helpers.offline_db).
"""

import time
from pathlib import Path

import pytest

from gravity_e2e.helpers.offline_db import (
    ACCOUNT_CHANGESETS_TABLE,
    STORAGE_CHANGESETS_TABLE,
    OfflineDbEnv,
    SettingsState,
    count_table_entries,
    db_stats,
    decode_scale_bytes,
    inspect_changeset_static_files,
    migrate_changesets,
    parse_db_stats,
    parse_entry_count,
    read_storage_settings,
    run_db_command,
    segment_block_range,
)

# ---------------------------------------------------------------------------
# Stub binary helpers
# ---------------------------------------------------------------------------


def make_stub(
    directory: Path,
    *,
    stdout: str = "",
    stderr: str = "",
    exit_code: int = 0,
    sleep_s: float = 0.0,
) -> Path:
    """Write an executable shell stub standing in for gravity_node.

    The stub dumps its argv (one arg per line) to ``argv.txt`` next to
    itself, then emits the preset stdout/stderr and exits with the preset
    code. ``sleep_s`` runs a sleep with all stdio detached so that killing
    the stub on timeout does not leave a pipe-holding child behind.
    """
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub_stdout.txt").write_text(stdout)
    (directory / "stub_stderr.txt").write_text(stderr)
    script = directory / "gravity_node"
    script.write_text(
        "#!/usr/bin/env bash\n"
        f'printf \'%s\\n\' "$@" > "{directory}/argv.txt"\n'
        + (
            f"sleep {sleep_s} </dev/null >/dev/null 2>&1\n"
            if sleep_s
            else ""
        )
        + f'cat "{directory}/stub_stdout.txt"\n'
        + f'cat "{directory}/stub_stderr.txt" >&2\n'
        + f"exit {exit_code}\n"
    )
    script.chmod(0o755)
    return script


def read_argv(directory: Path) -> list:
    """Argv recorded by the stub (excluding argv[0])."""
    return (directory / "argv.txt").read_text().splitlines()


def make_env(tmp_path: Path, stub: Path, **overrides) -> OfflineDbEnv:
    datadir = tmp_path / "datadir"
    datadir.mkdir(exist_ok=True)
    chain = tmp_path / "genesis.json"
    chain.write_text("{}")
    kwargs = {"binary": stub, "datadir": datadir, "chain": chain}
    kwargs.update(overrides)
    return OfflineDbEnv(**kwargs)


def expected_prefix(env: OfflineDbEnv) -> list:
    """The argv prefix every db invocation must start with.

    Derived from greth sources:
    - "db" subcommand: crates/ethereum/cli/src/interface.rs:281-283
    - --datadir / --datadir.static-files:
      crates/node/core/src/args/datadir_args.rs:19-29
    - --chain (global on the db command's EnvironmentArgs):
      crates/cli/commands/src/common.rs:48-59
    - --color never (global LogArgs flag, default is "always"):
      crates/node/core/src/args/log.rs:124-132,394-411
    """
    prefix = [
        "db",
        "--datadir",
        str(env.datadir),
        "--chain",
        str(env.chain),
        "--color",
        "never",
    ]
    if env.static_files_dir is not None:
        prefix += ["--datadir.static-files", str(env.static_files_dir)]
    return prefix


# ---------------------------------------------------------------------------
# Realistic output samples
# ---------------------------------------------------------------------------

# Log-line shape produced by reth's tracing stdout layer (LogArgs
# init_tracing; the db commands run with INFO verbosity by default,
# crates/node/core/src/args/log.rs:414-424). Content varies; parsers must
# treat these lines as noise.
LOG_NOISE = (
    "2026-07-15T03:12:01.123456Z  INFO Opening storage db_path=... sf_path=...\n"
)

# `db get mdbx Metadata gravity_storage_settings --raw` hit: a single
# hex::encode_prefixed line of the raw value bytes
# (crates/cli/commands/src/db/get.rs:227-233). The raw value is the
# SCALE-compressed Vec<u8> (compact length prefix + bytes, `impl
# ScaleValue for Vec<u8>` in crates/storage/db-api/src/scale.rs:48-49);
# the inner bytes are the serde_json encoding of GravityStorageSettings
# (crates/storage/db-api/src/models/metadata.rs:39-41).
SETTINGS_FALSE_JSON = '{"changesets_in_static_files":false}'
SETTINGS_TRUE_JSON = '{"changesets_in_static_files":true}'

# Exact raw line captured live (TC1, 2026-07-15) from greth v2.3.0
# `db get mdbx Metadata gravity_storage_settings --raw` on a freshly
# initialized datadir: 0x90 == SCALE compact length 36 (36 << 2), then the
# 36 serde_json bytes of {"changesets_in_static_files":false}.
LIVE_SETTINGS_FALSE_RAW = (
    "0x907b226368616e6765736574735f696e5f7374617469635f66696c6573223a66616c73657d"
)


def scale_compact_len(length: int) -> bytes:
    """SCALE compact-length prefix (independent re-implementation so the
    fixtures do not mirror the helper under test)."""
    if length < 64:
        return bytes([length << 2])
    if length < 2**14:
        return ((length << 2) | 0b01).to_bytes(2, "little")
    if length < 2**30:
        return ((length << 2) | 0b10).to_bytes(4, "little")
    raise NotImplementedError("big-integer mode not needed for fixtures")


def raw_hex_line(payload: str) -> str:
    """One --raw output line: SCALE(Vec<u8>) of the JSON payload, hex."""
    data = payload.encode()
    return "0x" + (scale_compact_len(len(data)) + data).hex()


# `db get` miss: tracing error! line, exit code still 0
# (crates/cli/commands/src/db/get.rs:236-241).
NO_CONTENT_LINE = (
    "2026-07-15T03:12:02.000000Z ERROR No content for the given table key.\n"
)

# Same line as emitted with the default `--color always` ANSI formatting.
NO_CONTENT_LINE_ANSI = (
    "\x1b[2m2026-07-15T03:12:02.000000Z\x1b[0m \x1b[31mERROR\x1b[0m "
    "No content for the given table key.\n"
)

# `db stats` output: a static-files table, a blank separator, then the
# database table (print order: crates/cli/commands/src/db/stats.rs:55-61).
# Both are comfy_table ASCII_MARKDOWN tables (stats.rs:71,154). The db
# table header is stats.rs:72-79; per-row "# Entries" is a hardcoded 0
# placeholder on the RocksDB backend (stats.rs:92-107, "Placeholder for
# RocksDB") — mirrored here on purpose. Non-zero counts are still covered
# below via PARSE_ONLY_STATS so the parser is proven against the mdbx-style
# output too. The trailing dash row and "Tables"/"Freelist" summary rows
# are stats.rs:110-141. Segment/range rendering in the static-files table:
# StaticFileSegment derive_more::Display (variant name,
# crates/static-file/types/src/segment.rs:11-27) and SegmentRangeInclusive
# "start..=end" (segment.rs:650-654).
DB_STATS_SAMPLE = """\
2026-07-15T03:12:01.123456Z  INFO Opening storage db_path=... sf_path=...
| Segment            | Block Range | Transaction Range | Shape (columns x rows) | Size    |
|--------------------|-------------|-------------------|------------------------|---------|
| Headers            | 0..=1233    | N/A               | 3 x 1234               | 42.1 KB |
| AccountChangeSets  | 0..=1233    | N/A               | 1 x 1234               | 10.5 KB |
| StorageChangeSets  | 0..=1233    | N/A               | 1 x 1234               | 11.2 KB |
| ------------------ | ----------- | ----------------- | ---------------------- | ------- |
| Total              |             |                   |                        | 63.8 KB |


| Table Name         | # Entries | Branch Pages | Leaf Pages | Overflow Pages | Total Size |
|--------------------|-----------|--------------|------------|----------------|------------|
| AccountChangeSets  | 0         | 0            | 0          | 0              | 0 B        |
| Bytecodes          | 0         | 0            | 0          | 0              | 0 B        |
| StorageChangeSets  | 0         | 0            | 0          | 0              | 0 B        |
| ------------------ | --------- | ------------ | ---------- | -------------- | ---------- |
| Tables             |           |              |            |                | 0 B        |
| Freelist           | 0         |              |            |                | 0 B        |
"""

# Hypothetical non-placeholder counts (shape identical; what an mdbx
# backend would print) to prove the parser reads real numbers and not
# just zeros.
PARSE_ONLY_STATS = """\
| Table Name         | # Entries | Branch Pages | Leaf Pages | Overflow Pages | Total Size |
|--------------------|-----------|--------------|------------|----------------|------------|
| AccountChangeSets  | 1234      | 1            | 10         | 0              | 163.8 KB   |
| StorageChangeSets  | 56789     | 2            | 300        | 4              | 4.9 MB     |
| ------------------ | --------- | ------------ | ---------- | -------------- | ---------- |
| Tables             |           |              |            |                | 5.1 MB     |
| Freelist           | 0         |              |            |                | 0 B        |
"""

# `db list <TABLE> --count` output line:
# crates/cli/commands/src/db/list.rs:121 `println!("{count} entries found.")`.
COUNT_ZERO = LOG_NOISE + "0 entries found.\n"
COUNT_MANY = LOG_NOISE + "1234 entries found.\n"


# ---------------------------------------------------------------------------
# Command execution layer
# ---------------------------------------------------------------------------


class TestRunDbCommand:
    def test_success_captures_output_and_argv(self, tmp_path):
        stub = make_stub(tmp_path, stdout="hello out\n", stderr="hello err\n")
        env = make_env(tmp_path, stub)

        result = run_db_command(env, ["stats"])

        assert result.ok
        assert result.returncode == 0
        assert not result.timed_out
        assert result.stdout == "hello out\n"
        assert result.stderr == "hello err\n"
        assert result.argv[0] == str(stub)
        assert read_argv(tmp_path) == expected_prefix(env) + ["stats"]

    def test_static_files_override_in_argv(self, tmp_path):
        stub = make_stub(tmp_path)
        sf_dir = tmp_path / "custom_sf"
        env = make_env(tmp_path, stub, static_files_dir=sf_dir)

        result = run_db_command(env, ["stats"])

        assert result.ok
        argv = read_argv(tmp_path)
        assert argv == expected_prefix(env) + ["stats"]
        idx = argv.index("--datadir.static-files")
        assert argv[idx + 1] == str(sf_dir)

    def test_nonzero_exit_preserves_full_output(self, tmp_path):
        # Error shape: gravity_node prints `Error: {err:?}` to stderr and
        # exits 1 for failed utility commands
        # (worktree bin/gravity_node/src/main.rs:291-294); the datadir
        # existence check is crates/cli/commands/src/db/mod.rs:96-104.
        stderr = 'Error: Datadir does not exist: "/nonexistent"\n'
        stub = make_stub(tmp_path, stdout="partial\n", stderr=stderr, exit_code=1)
        env = make_env(tmp_path, stub)

        result = run_db_command(env, ["stats"])

        assert not result.ok
        assert result.returncode == 1
        assert result.stdout == "partial\n"
        assert result.stderr == stderr

    def test_timeout_kills_and_reports(self, tmp_path):
        stub = make_stub(tmp_path, stdout="never flushed", sleep_s=30)
        env = make_env(tmp_path, stub)

        start = time.monotonic()
        result = run_db_command(env, ["stats"], timeout=0.5)
        elapsed = time.monotonic() - start

        assert result.timed_out
        assert not result.ok
        assert result.returncode is None
        assert elapsed < 10  # killed, not waited for

    def test_missing_binary_is_reported_not_raised(self, tmp_path):
        env = make_env(tmp_path, tmp_path / "no_such_binary")

        result = run_db_command(env, ["stats"])

        assert not result.ok
        assert result.returncode is None
        assert not result.timed_out
        assert "no_such_binary" in result.summary()


class TestMigrateChangesets:
    def test_argv(self, tmp_path):
        stub = make_stub(tmp_path)
        env = make_env(tmp_path, stub)

        result = migrate_changesets(env)

        assert result.ok
        # `migrate-changesets` takes no arguments of its own: unit struct
        # Command (crates/cli/commands/src/db/migrate_changesets.rs:32-33),
        # subcommand name from Subcommands::MigrateChangesets
        # (crates/cli/commands/src/db/mod.rs:59-60).
        assert read_argv(tmp_path) == expected_prefix(env) + ["migrate-changesets"]

    def test_rerun_is_just_another_invocation(self, tmp_path):
        # Idempotent-rerun semantics live in the binary
        # (migrate_changesets.rs:49-56); the helper only re-executes.
        stub = make_stub(tmp_path)
        env = make_env(tmp_path, stub)

        first = migrate_changesets(env)
        second = migrate_changesets(env)

        assert first.ok and second.ok

    def test_failure_result(self, tmp_path):
        stderr = "Error: changeset history is pruned; migrating a pruned database is not yet supported\n"
        stub = make_stub(tmp_path, stderr=stderr, exit_code=1)
        env = make_env(tmp_path, stub)

        result = migrate_changesets(env)

        assert not result.ok
        assert result.stderr == stderr


# ---------------------------------------------------------------------------
# SCALE Vec<u8> decoding (raw Metadata values)
# ---------------------------------------------------------------------------


class TestDecodeScaleBytes:
    def test_single_byte_mode_live_payload(self):
        payload = bytes.fromhex(LIVE_SETTINGS_FALSE_RAW[2:])
        assert decode_scale_bytes(payload) == SETTINGS_FALSE_JSON.encode()

    def test_single_byte_mode_empty_vec(self):
        assert decode_scale_bytes(b"\x00") == b""

    def test_two_byte_mode(self):
        data = b"a" * 100
        assert decode_scale_bytes(scale_compact_len(100) + data) == data

    def test_four_byte_mode(self):
        data = b"b" * 20000
        assert decode_scale_bytes(scale_compact_len(20000) + data) == data

    def test_length_mismatch_raises(self):
        with pytest.raises(ValueError):
            decode_scale_bytes(b"\x90" + b"short")

    def test_trailing_garbage_raises(self):
        payload = bytes.fromhex(LIVE_SETTINGS_FALSE_RAW[2:]) + b"junk"
        with pytest.raises(ValueError):
            decode_scale_bytes(payload)

    def test_empty_payload_raises(self):
        with pytest.raises(ValueError):
            decode_scale_bytes(b"")

    def test_big_integer_mode_mismatch_raises(self):
        # 0b11 mode: first byte says 4 length bytes follow; declared length
        # (2**30) can never match a real settings payload.
        prefix = bytes([0b11]) + (2**30).to_bytes(4, "little")
        with pytest.raises(ValueError):
            decode_scale_bytes(prefix + b"tiny")


# ---------------------------------------------------------------------------
# gravity_storage_settings three-state probe
# ---------------------------------------------------------------------------


class TestReadStorageSettings:
    def test_argv(self, tmp_path):
        stub = make_stub(tmp_path, stdout=LOG_NOISE + raw_hex_line(SETTINGS_TRUE_JSON) + "\n")
        env = make_env(tmp_path, stub)

        read_storage_settings(env)

        # get mdbx <TABLE> <KEY> --raw: crates/cli/commands/src/db/get.rs:31-56;
        # table name "Metadata": crates/storage/db-api/src/tables/mod.rs:541-545;
        # key literal: crates/storage/storage-api/src/metadata.rs:10.
        assert read_argv(tmp_path) == expected_prefix(env) + [
            "get",
            "mdbx",
            "Metadata",
            "gravity_storage_settings",
            "--raw",
        ]

    def test_present_true(self, tmp_path):
        stub = make_stub(tmp_path, stdout=LOG_NOISE + raw_hex_line(SETTINGS_TRUE_JSON) + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.PRESENT_STATIC_FILES
        assert probe.settings == {"changesets_in_static_files": True}
        assert probe.error is None

    def test_present_false(self, tmp_path):
        stub = make_stub(tmp_path, stdout=LOG_NOISE + raw_hex_line(SETTINGS_FALSE_JSON) + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.PRESENT_LEGACY
        assert probe.settings == {"changesets_in_static_files": False}

    def test_present_empty_object_defaults_to_legacy(self, tmp_path):
        # serde `#[serde(default)]` on changesets_in_static_files: an entry
        # written before the field existed decodes to false
        # (crates/storage/db-api/src/models/metadata.rs:17-22,64-70).
        stub = make_stub(tmp_path, stdout=raw_hex_line("{}") + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.PRESENT_LEGACY

    def test_missing_entry(self, tmp_path):
        stub = make_stub(tmp_path, stdout=LOG_NOISE + NO_CONTENT_LINE)
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.MISSING
        assert probe.settings is None
        assert probe.error is None

    def test_missing_entry_with_ansi_colors(self, tmp_path):
        stub = make_stub(tmp_path, stdout=NO_CONTENT_LINE_ANSI)
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.MISSING

    def test_live_captured_payload_parses_legacy(self, tmp_path):
        # Golden fixture: the exact bytes greth v2.3.0 printed live (TC1).
        stub = make_stub(tmp_path, stdout=LOG_NOISE + LIVE_SETTINGS_FALSE_RAW + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.PRESENT_LEGACY
        assert probe.settings == {"changesets_in_static_files": False}
        assert probe.error is None

    def test_scale_two_byte_length_mode(self, tmp_path):
        # A future settings JSON > 63 bytes flips SCALE compact into the
        # two-byte length mode; the probe must still decode it.
        long_json = (
            '{"changesets_in_static_files":true,"future_field":"'
            + "x" * 40
            + '"}'
        )
        assert len(long_json.encode()) >= 64
        stub = make_stub(tmp_path, stdout=raw_hex_line(long_json) + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is SettingsState.PRESENT_STATIC_FILES

    def test_scale_length_mismatch_is_error(self, tmp_path):
        # Prefix says 36 bytes but the payload is truncated: report an
        # error, never a state.
        truncated = "0x90" + SETTINGS_FALSE_JSON.encode()[:10].hex()
        stub = make_stub(tmp_path, stdout=truncated + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is None
        assert probe.error is not None

    def test_unprefixed_json_payload_is_error_not_a_state(self, tmp_path):
        # A bare (non-SCALE) JSON payload is not what greth writes; it must
        # surface as an error instead of being guessed at.
        bare = "0x" + SETTINGS_FALSE_JSON.encode().hex()
        stub = make_stub(tmp_path, stdout=bare + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is None
        assert probe.error is not None

    def test_undecodable_payload_is_error_not_a_state(self, tmp_path):
        stub = make_stub(tmp_path, stdout=raw_hex_line("not json") + "\n")
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is None
        assert probe.error is not None

    def test_nonzero_exit_is_error(self, tmp_path):
        stub = make_stub(tmp_path, stderr="Error: boom\n", exit_code=1)
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env)

        assert probe.state is None
        assert probe.error is not None
        assert "boom" in probe.command.stderr

    def test_timeout_is_error(self, tmp_path):
        stub = make_stub(tmp_path, sleep_s=30)
        env = make_env(tmp_path, stub)

        probe = read_storage_settings(env, timeout=0.5)

        assert probe.state is None
        assert probe.command.timed_out


# ---------------------------------------------------------------------------
# db stats
# ---------------------------------------------------------------------------


class TestParseDbStats:
    def test_parses_placeholder_rocksdb_output(self):
        entries = parse_db_stats(DB_STATS_SAMPLE)

        assert entries[ACCOUNT_CHANGESETS_TABLE] == 0
        assert entries[STORAGE_CHANGESETS_TABLE] == 0
        assert entries["Bytecodes"] == 0
        # summary rows: "Tables" has an empty entries cell and is skipped
        assert "Tables" not in entries

    def test_parses_real_counts(self):
        entries = parse_db_stats(PARSE_ONLY_STATS)

        assert entries[ACCOUNT_CHANGESETS_TABLE] == 1234
        assert entries[STORAGE_CHANGESETS_TABLE] == 56789

    def test_does_not_read_static_files_table(self):
        entries = parse_db_stats(DB_STATS_SAMPLE)

        # "Headers" appears only in the static-files (Segment) table, which
        # has no "# Entries" column and must not be picked up.
        assert "Headers" not in entries

    def test_no_table_header_gives_empty(self):
        assert parse_db_stats("no tables here\n") == {}


class TestDbStats:
    def test_wrapper(self, tmp_path):
        stub = make_stub(tmp_path, stdout=DB_STATS_SAMPLE)
        env = make_env(tmp_path, stub)

        result = db_stats(env)

        assert result.error is None
        assert result.entries[ACCOUNT_CHANGESETS_TABLE] == 0
        # subcommand name: crates/cli/commands/src/db/mod.rs:39-40
        assert read_argv(tmp_path) == expected_prefix(env) + ["stats"]

    def test_skip_consistency_checks_flag(self, tmp_path):
        # flag: crates/cli/commands/src/db/stats.rs:20-22
        stub = make_stub(tmp_path, stdout=DB_STATS_SAMPLE)
        env = make_env(tmp_path, stub)

        db_stats(env, skip_consistency_checks=True)

        assert read_argv(tmp_path) == expected_prefix(env) + [
            "stats",
            "--skip-consistency-checks",
        ]

    def test_failure_is_error(self, tmp_path):
        stub = make_stub(tmp_path, stderr="Error: boom\n", exit_code=1)
        env = make_env(tmp_path, stub)

        result = db_stats(env)

        assert result.error is not None
        assert result.entries == {}

    def test_unparseable_output_is_error(self, tmp_path):
        stub = make_stub(tmp_path, stdout="garbage\n")
        env = make_env(tmp_path, stub)

        result = db_stats(env)

        assert result.error is not None


# ---------------------------------------------------------------------------
# db list --count (reliable emptiness check)
# ---------------------------------------------------------------------------


class TestCountTableEntries:
    def test_argv(self, tmp_path):
        stub = make_stub(tmp_path, stdout=COUNT_ZERO)
        env = make_env(tmp_path, stub)

        count_table_entries(env, ACCOUNT_CHANGESETS_TABLE)

        # list <TABLE> --count: crates/cli/commands/src/db/list.rs:15-52
        # (positional table :16-17, --count flag :43-45); table names from
        # crates/storage/db-api/src/tables/mod.rs:445,454.
        assert read_argv(tmp_path) == expected_prefix(env) + [
            "list",
            "AccountChangeSets",
            "--count",
        ]

    def test_zero(self, tmp_path):
        stub = make_stub(tmp_path, stdout=COUNT_ZERO)
        env = make_env(tmp_path, stub)

        result = count_table_entries(env, ACCOUNT_CHANGESETS_TABLE)

        assert result.count == 0
        assert result.error is None

    def test_many(self, tmp_path):
        stub = make_stub(tmp_path, stdout=COUNT_MANY)
        env = make_env(tmp_path, stub)

        result = count_table_entries(env, STORAGE_CHANGESETS_TABLE)

        assert result.count == 1234

    def test_missing_marker_is_error(self, tmp_path):
        stub = make_stub(tmp_path, stdout=LOG_NOISE)
        env = make_env(tmp_path, stub)

        result = count_table_entries(env, ACCOUNT_CHANGESETS_TABLE)

        assert result.count is None
        assert result.error is not None

    def test_nonzero_exit_is_error(self, tmp_path):
        stub = make_stub(tmp_path, stdout=COUNT_ZERO, exit_code=2)
        env = make_env(tmp_path, stub)

        result = count_table_entries(env, ACCOUNT_CHANGESETS_TABLE)

        assert result.count is None
        assert result.error is not None

    def test_parse_entry_count(self):
        assert parse_entry_count("0 entries found.\n") == 0
        assert parse_entry_count(LOG_NOISE + "77 entries found.\n") == 77
        assert parse_entry_count("nothing\n") is None


# ---------------------------------------------------------------------------
# Static-file layout check (pure fs, no binary)
# ---------------------------------------------------------------------------

# File name patterns:
# - data file "static_file_{segment}_{start}_{end}":
#   crates/static-file/types/src/segment.rs:129-133, with segment strings
#   "account-change-sets" / "storage-change-sets" (segment.rs:78-79)
# - .csoff sidecar = data path + extension "csoff":
#   crates/storage/nippy-jar/src/lib.rs:63,246-248 (also
#   crates/storage/provider/src/providers/static_file/writer.rs:309)
# - satellite extensions idx/off/conf: nippy-jar/src/lib.rs:57-61
# - default location <datadir>/static_files:
#   crates/node/core/src/dirs.rs:293-299
ACC_SEG = "static_file_account-change-sets_0_499999"
STO_SEG = "static_file_storage-change-sets_0_499999"


def build_static_files(sf_dir: Path, names: list) -> None:
    sf_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        (sf_dir / name).write_bytes(b"\x00")


class TestInspectChangesetStaticFiles:
    def test_full_layout(self, tmp_path):
        datadir = tmp_path / "datadir"
        build_static_files(
            datadir / "static_files",
            [
                ACC_SEG,
                ACC_SEG + ".csoff",
                ACC_SEG + ".idx",
                ACC_SEG + ".off",
                ACC_SEG + ".conf",
                STO_SEG,
                STO_SEG + ".csoff",
                "static_file_headers_0_499999",  # other segment: ignored
                "static_file_headers_0_499999.idx",
            ],
        )

        layout = inspect_changeset_static_files(datadir)

        assert layout.has_segment_files
        assert layout.has_sidecar_files
        assert [p.name for p in layout.account_segments] == [ACC_SEG]
        assert [p.name for p in layout.storage_segments] == [STO_SEG]
        assert [p.name for p in layout.account_sidecars] == [ACC_SEG + ".csoff"]
        assert [p.name for p in layout.storage_sidecars] == [STO_SEG + ".csoff"]

    def test_segments_without_sidecars(self, tmp_path):
        datadir = tmp_path / "datadir"
        build_static_files(datadir / "static_files", [ACC_SEG, STO_SEG])

        layout = inspect_changeset_static_files(datadir)

        assert layout.has_segment_files
        assert not layout.has_sidecar_files

    def test_empty_dir(self, tmp_path):
        datadir = tmp_path / "datadir"
        build_static_files(datadir / "static_files", [])

        layout = inspect_changeset_static_files(datadir)

        assert layout.exists
        assert not layout.has_segment_files
        assert not layout.has_sidecar_files

    def test_missing_static_files_dir(self, tmp_path):
        datadir = tmp_path / "datadir"
        datadir.mkdir()

        layout = inspect_changeset_static_files(datadir)

        assert not layout.exists
        assert not layout.has_segment_files
        assert not layout.has_sidecar_files

    def test_explicit_static_files_dir_override(self, tmp_path):
        override = tmp_path / "elsewhere"
        build_static_files(override, [ACC_SEG, ACC_SEG + ".csoff"])

        layout = inspect_changeset_static_files(
            tmp_path / "unused_datadir", static_files_dir=override
        )

        assert layout.static_files_dir == override
        assert layout.has_segment_files
        assert layout.has_sidecar_files

    def test_noise_names_ignored(self, tmp_path):
        datadir = tmp_path / "datadir"
        build_static_files(
            datadir / "static_files",
            [
                "static_file_account-change-sets_0_499999.lock",  # wrong ext
                "static_file_account-change-sets_0",  # malformed range
                "account-change-sets_0_499999",  # missing prefix
                "static_file_account-change-sets_0_499999.csoff.bak",
            ],
        )

        layout = inspect_changeset_static_files(datadir)

        assert not layout.has_segment_files
        assert not layout.has_sidecar_files

    def test_multiple_ranges(self, tmp_path):
        # Segments are split into fixed block ranges; a long chain has
        # several files per segment (segment.rs:129-133 name embeds range).
        datadir = tmp_path / "datadir"
        second = "static_file_account-change-sets_500000_999999"
        build_static_files(
            datadir / "static_files",
            [ACC_SEG, ACC_SEG + ".csoff", second, second + ".csoff"],
        )

        layout = inspect_changeset_static_files(datadir)

        assert [p.name for p in layout.account_segments] == [ACC_SEG, second]
        assert [p.name for p in layout.account_sidecars] == [
            ACC_SEG + ".csoff",
            second + ".csoff",
        ]

    def test_segments_sorted_numerically_not_lexicographically(self, tmp_path):
        # Regression: lexicographic sort puts "1000000" before "500000", so
        # segments[-1] (what cases treat as "the highest-block segment")
        # silently pointed at the wrong file once ranges crossed a digit
        # boundary.
        datadir = tmp_path / "datadir"
        first = "static_file_account-change-sets_0_499999"
        middle = "static_file_account-change-sets_500000_999999"
        last = "static_file_account-change-sets_1000000_1499999"
        build_static_files(
            datadir / "static_files",
            [last, first, middle]
            + [name + ".csoff" for name in (last, first, middle)],
        )

        layout = inspect_changeset_static_files(datadir)

        assert [p.name for p in layout.account_segments] == [first, middle, last]
        assert layout.account_segments[-1].name == last
        assert [p.name for p in layout.account_sidecars] == [
            name + ".csoff" for name in (first, middle, last)
        ]


class TestSegmentBlockRange:
    def test_parses_data_and_sidecar_names(self):
        assert segment_block_range(
            "static_file_storage-change-sets_500000_999999"
        ) == (500000, 999999)
        assert segment_block_range(
            Path("/x/static_file_account-change-sets_0_499999.csoff")
        ) == (0, 499999)

    def test_rejects_foreign_names(self):
        with pytest.raises(ValueError, match="not a changeset segment"):
            segment_block_range("static_file_headers_0_499999")
