"""Prepare old/new binaries plus live Oracle sources for the rolling proof."""

import csv
from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
from io import BytesIO, TextIOWrapper
import json
import logging
import os
import shutil
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import parse_qs, urlparse
from urllib.request import Request, urlopen
from zipfile import ZipFile

try:
    import tomllib
except ImportError:
    import tomli as tomllib


LOG = logging.getLogger(__name__)

BRIDGE_RPC_PORT = 28646
BRIDGE_EVENT_COUNT = 6
BRIDGE_AMOUNT = 1_000_000_000_000_000_000
BRIDGE_RECIPIENT = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
BRIDGE_SENDER = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
BRIDGE_URI = (
    "gravity://0/31337/events?"
    "contract=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512&"
    "eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&"
    "fromBlock=0"
)

BINANCE_REPLAY_PORT = 28647
INTERVAL_MS = 60_000
DEFAULT_BINANCE_GRACE_MS = 120_000
DEFAULT_BINANCE_BASE_URL = "https://fapi.binance.com"
DEFAULT_BINANCE_ARCHIVE_BASE_URL = "https://data.binance.vision"
DEFAULT_BINANCE_MODE = "official-archive"
BINANCE_FEEDS = ((1001, "NVDAUSDT"), (1002, "TSLAUSDT"))

_METADATA_FILE = "oracle_rolling_activation_metadata.json"
_RELAYER_FILE = "relayer_config.live.json"
_mock = None
_binance_replay = None


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _required_binary(env: dict, name: str) -> Path:
    value = env.get(name) or os.environ.get(name)
    if not value:
        raise RuntimeError(f"{name} must point to an existing gravity_node binary")
    path = Path(value).expanduser().resolve()
    if not path.is_file():
        raise RuntimeError(f"{name} is not a file: {path}")
    if not os.access(path, os.X_OK):
        raise RuntimeError(f"{name} is not executable: {path}")
    return path


def _bucket_start_ms(env: dict) -> int:
    configured = env.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    if configured:
        return int(configured)

    grace_ms = int(
        env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_BINANCE_GRACE_MS))
    )
    minimum_lag = (grace_ms + INTERVAL_MS - 1) // INTERVAL_MS + 1
    lag_minutes = int(
        env.get("BINANCE_PRICE_FEED_LAG_MINUTES", str(minimum_lag))
    )
    return ((int(time.time() * 1000) // INTERVAL_MS) - lag_minutes) * INTERVAL_MS


def _archive_row(raw: list[str]) -> list:
    if len(raw) != 12:
        raise RuntimeError(f"unexpected Binance archive row width: {len(raw)}")
    row = list(raw)
    for index in (0, 6, 8):
        row[index] = int(row[index])
    return row


def _download_archive(
    archive_base_url: str, pair: str, archive_date: str
) -> tuple[dict[int, list], dict]:
    filename = f"{pair}-1m-{archive_date}.zip"
    source_url = (
        f"{archive_base_url}/data/futures/um/daily/indexPriceKlines/"
        f"{pair}/1m/{filename}"
    )
    request = Request(source_url, headers={"User-Agent": "gravity-e2e/1"})
    with urlopen(request, timeout=45) as response:
        payload = response.read()

    archive_sha256 = hashlib.sha256(payload).hexdigest()
    with ZipFile(BytesIO(payload)) as archive:
        csv_names = [
            name for name in archive.namelist() if name.lower().endswith(".csv")
        ]
        if len(csv_names) != 1:
            raise RuntimeError(
                f"{source_url} contained {len(csv_names)} CSV files"
            )
        with archive.open(csv_names[0]) as binary:
            reader = csv.reader(TextIOWrapper(binary, encoding="utf-8"))
            header = next(reader)
            if header[:7] != [
                "open_time",
                "open",
                "high",
                "low",
                "close",
                "volume",
                "close_time",
            ]:
                raise RuntimeError(
                    f"unexpected Binance archive header for {pair}: {header}"
                )
            rows = {
                int(row[0]): _archive_row(row)
                for row in reader
                if row
            }
    if not rows:
        raise RuntimeError(f"Binance archive contained no rows: {source_url}")
    return rows, {
        "pair": pair,
        "sourceUrl": source_url,
        "sha256": archive_sha256,
    }


def _archive_dates(env: dict) -> list[str]:
    configured = env.get("BINANCE_PRICE_FEED_ARCHIVE_DATE")
    if configured:
        datetime.strptime(configured, "%Y-%m-%d")
        return [configured]

    today = datetime.now(timezone.utc).date()
    return [
        (today - timedelta(days=offset)).isoformat()
        for offset in range(1, 8)
    ]


def _official_archive_data(env: dict) -> dict:
    archive_base_url = env.get(
        "BINANCE_PRICE_FEED_ARCHIVE_BASE_URL",
        DEFAULT_BINANCE_ARCHIVE_BASE_URL,
    ).rstrip("/")
    configured_bucket = env.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    last_error = None

    for archive_date in _archive_dates(env):
        try:
            rows_by_pair = {}
            provenance = []
            for _, pair in BINANCE_FEEDS:
                rows, source = _download_archive(
                    archive_base_url, pair, archive_date
                )
                rows_by_pair[pair] = rows
                provenance.append(source)
        except (HTTPError, OSError, RuntimeError) as error:
            last_error = error
            if env.get("BINANCE_PRICE_FEED_ARCHIVE_DATE"):
                raise
            continue

        common_buckets = set.intersection(
            *(set(rows) for rows in rows_by_pair.values())
        )
        if not common_buckets:
            last_error = RuntimeError(
                f"no common Binance minute bucket on {archive_date}"
            )
            continue
        bucket_start_ms = (
            int(configured_bucket)
            if configured_bucket
            else max(common_buckets)
        )
        if bucket_start_ms not in common_buckets:
            raise RuntimeError(
                f"bucket {bucket_start_ms} is absent from {archive_date}"
            )
        return {
            "archiveBaseUrl": archive_base_url,
            "archiveDate": archive_date,
            "bucketStartMs": bucket_start_ms,
            "provenance": provenance,
            "rows": {
                pair: rows_by_pair[pair][bucket_start_ms]
                for _, pair in BINANCE_FEEDS
            },
        }

    raise RuntimeError(
        "could not load a common Binance production archive for all feeds"
    ) from last_error


def _price_uri(
    feed_id: int, pair: str, bucket_start_ms: int, grace_ms: int
) -> str:
    return (
        f"gravity://3/{feed_id}/price_feed?"
        f"provider=binance_index_kline_v1&pair={pair}&interval=1m&"
        f"bucketStartMs={bucket_start_ms}&decimals=8&graceMs={grace_ms}"
    )


def _polymarket_hooks(test_dir: Path):
    hook_path = test_dir.parent / "polymarket_live_dynamic_mirror" / "hooks.py"
    spec = importlib.util.spec_from_file_location(
        "gravity_e2e_live_polymarket_hooks", hook_path
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load live Polymarket helpers from {hook_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _replace_binary(source: Path, destination: Path) -> None:
    destination.unlink(missing_ok=True)
    try:
        os.link(source, destination)
    except OSError:
        shutil.copy2(source, destination)
    destination.chmod(0o755)
    if _sha256(destination) != _sha256(source):
        raise RuntimeError(f"binary replacement verification failed: {destination}")


def _start_binance_replay(binance: dict):
    rows = binance["rows"]

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self):
            parsed = urlparse(self.path)
            if parsed.path != "/fapi/v1/indexPriceKlines":
                self.send_error(404)
                return

            params = parse_qs(parsed.query)
            pair = params.get("pair", [""])[0]
            try:
                start_time = int(params.get("startTime", ["-1"])[0])
                end_time = int(params.get("endTime", ["-1"])[0])
            except ValueError:
                self.send_error(400)
                return
            row = rows.get(pair)
            if (
                row is None
                or params.get("interval", [""])[0] != "1m"
                or params.get("limit", [""])[0] != "1"
                or start_time != row[0]
                or end_time != row[6]
            ):
                self.send_error(400)
                return

            payload = json.dumps([row]).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, fmt: str, *args):
            LOG.debug("Binance archive replay: " + fmt, *args)

    server = ThreadingHTTPServer(("127.0.0.1", BINANCE_REPLAY_PORT), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    old_binary = _required_binary(env, "GRAVITY_OLD_BINARY")
    new_binary = _required_binary(env, "GRAVITY_NEW_BINARY")
    old_hash = _sha256(old_binary)
    new_hash = _sha256(new_binary)
    if old_hash == new_hash:
        raise RuntimeError("old and new gravity_node binaries must differ")

    rpc_url = (
        env.get("POLYGON_RPC_URL")
        or env.get("POLYGON_QUICKNONE_HTTP_URL")
        or os.environ.get("POLYGON_RPC_URL")
        or os.environ.get("POLYGON_QUICKNONE_HTTP_URL")
    )
    if not rpc_url:
        raise RuntimeError("POLYGON_RPC_URL is required for this live suite")

    polymarket = _polymarket_hooks(test_dir)
    gamma_url = env.get("POLYMARKET_GAMMA_URL", polymarket.DEFAULT_GAMMA_URL)
    market = polymarket._discover_market(rpc_url, gamma_url)

    binance_mode = env.get(
        "ORACLE_ROLLING_BINANCE_MODE", DEFAULT_BINANCE_MODE
    )
    if binance_mode == "official-archive":
        archive_data = _official_archive_data(env)
        binance_base_url = f"http://127.0.0.1:{BINANCE_REPLAY_PORT}"
        bucket_start_ms = archive_data["bucketStartMs"]
    elif binance_mode == "production-rest":
        archive_data = None
        binance_base_url = env.get(
            "BINANCE_PRICE_FEED_BASE_URL", DEFAULT_BINANCE_BASE_URL
        ).rstrip("/")
        bucket_start_ms = _bucket_start_ms(env)
    else:
        raise RuntimeError(
            "ORACLE_ROLLING_BINANCE_MODE must be "
            "'official-archive' or 'production-rest'"
        )
    grace_ms = int(
        env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_BINANCE_GRACE_MS))
    )
    price_uris = {
        str(feed_id): _price_uri(feed_id, pair, bucket_start_ms, grace_ms)
        for feed_id, pair in BINANCE_FEEDS
    }

    mappings = {BRIDGE_URI: f"http://127.0.0.1:{BRIDGE_RPC_PORT}"}
    mappings.update({uri: binance_base_url for uri in price_uris.values()})
    mappings[market["taskUri"]] = rpc_url

    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    relayer_path = artifacts / _RELAYER_FILE
    relayer_path.write_text(
        json.dumps({"uri_mappings": mappings}, indent=2) + "\n"
    )
    metadata_path = artifacts / _METADATA_FILE
    binance_metadata = {
        "mode": binance_mode,
        "baseUrl": binance_base_url,
        "bucketStartMs": bucket_start_ms,
        "graceMs": grace_ms,
        "feeds": [
            {
                "feedId": feed_id,
                "pair": pair,
                "taskUri": price_uris[str(feed_id)],
            }
            for feed_id, pair in BINANCE_FEEDS
        ],
    }
    if archive_data is not None:
        binance_metadata.update(archive_data)

    metadata_path.write_text(
        json.dumps(
            {
                "oldBinarySha256": old_hash,
                "newBinarySha256": new_hash,
                "bridge": {
                    "rpcUrl": f"http://127.0.0.1:{BRIDGE_RPC_PORT}",
                    "eventCount": BRIDGE_EVENT_COUNT,
                    "amount": BRIDGE_AMOUNT,
                    "recipient": BRIDGE_RECIPIENT,
                    "sender": BRIDGE_SENDER,
                    "taskUri": BRIDGE_URI,
                },
                "binance": binance_metadata,
                "polymarket": market,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n"
    )

    env["GRAVITY_OLD_BINARY"] = str(old_binary)
    env["GRAVITY_NEW_BINARY"] = str(new_binary)
    env["BINANCE_PRICE_FEED_MODE"] = "live"
    env["BINANCE_PRICE_FEED_BASE_URL"] = binance_base_url
    env["BINANCE_PRICE_FEED_BUCKET_START_MS"] = str(bucket_start_ms)
    env["BINANCE_PRICE_FEED_GRACE_MS"] = str(grace_ms)
    env["RELAYER_CONFIG_TPL"] = str(relayer_path)
    LOG.info(
        "[hook] Prepared old=%s new=%s Binance mode=%s bucket=%s "
        "Polymarket mirror=%s",
        old_hash[:12],
        new_hash[:12],
        binance_mode,
        bucket_start_ms,
        market["mirrorId"],
    )


def pre_start(test_dir: Path, env: dict, pytest_args: list[str]):
    global _binance_replay, _mock

    old_binary = _required_binary(env, "GRAVITY_OLD_BINARY")
    cluster = tomllib.loads((test_dir / "cluster.toml").read_text())
    base_dir = Path(cluster["cluster"]["base_dir"])
    for node in cluster["nodes"]:
        destination = base_dir / node["id"] / "bin" / "gravity_node"
        _replace_binary(old_binary, destination)
        LOG.info("[hook] Installed old binary for %s", node["id"])

    e2e_root = str(test_dir.parents[1])
    if e2e_root not in sys.path:
        sys.path.insert(0, e2e_root)
    from gravity_e2e.utils.mock_anvil import MockAnvil

    _mock = MockAnvil(port=BRIDGE_RPC_PORT)
    _mock.start()
    _mock.preload_events(
        count=BRIDGE_EVENT_COUNT,
        amount=BRIDGE_AMOUNT,
        recipient=BRIDGE_RECIPIENT,
        sender_address=BRIDGE_SENDER,
        events_per_block=1,
    )
    _mock.set_finalized(0)
    LOG.info(
        "[hook] Mock bridge source ready on localhost with %s hidden events",
        BRIDGE_EVENT_COUNT,
    )

    metadata = json.loads(
        (test_dir / "artifacts" / _METADATA_FILE).read_text()
    )
    if metadata["binance"]["mode"] == "official-archive":
        _binance_replay = _start_binance_replay(metadata["binance"])
        LOG.info(
            "[hook] Binance official archive replay ready for %s at bucket %s",
            metadata["binance"]["archiveDate"],
            metadata["binance"]["bucketStartMs"],
        )


def post_stop(test_dir: Path, env: dict):
    global _binance_replay, _mock

    if _binance_replay is not None:
        server, thread = _binance_replay
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        _binance_replay = None

    if _mock is not None:
        _mock.stop()
        _mock = None

    artifacts = test_dir / "artifacts"
    for name in (_METADATA_FILE, _RELAYER_FILE):
        (artifacts / name).unlink(missing_ok=True)
