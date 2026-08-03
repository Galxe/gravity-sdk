"""Deterministic local Binance index-kline server for oracle tests."""

from contextlib import contextmanager
import json
import logging
import threading
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

LOG = logging.getLogger(__name__)

BUCKET_START_MS = 1_783_252_500_000
INTERVAL_MS = 60_000
DECIMALS = 8
MOCK_BINANCE_PORT = 18_547
SUPPORTED_PAIRS = {"NVDAUSDT", "TSLAUSDT"}

_BASE_PRICES = {"NVDAUSDT": 19_592_645_000, "TSLAUSDT": 40_067_545_000}
_INCREMENTS = {"NVDAUSDT": 10_000_000, "TSLAUSDT": 25_000_000}


def format_price(scaled_price: int) -> str:
    whole = scaled_price // 10**DECIMALS
    fraction = scaled_price % 10**DECIMALS
    return f"{whole}.{fraction:0{DECIMALS}d}"


def mock_scaled_price(pair: str, bucket_index: int) -> int:
    return _BASE_PRICES[pair] + bucket_index * _INCREMENTS[pair]


class _MockBinanceServer(ThreadingHTTPServer):
    allow_reuse_address = True

    def __init__(self, address):
        super().__init__(address, _MockBinanceHandler)
        self._request_counts = Counter()
        self._request_lock = threading.Lock()

    @property
    def base_url(self) -> str:
        host, port = self.server_address
        return f"http://{host}:{port}"

    def record_request(self, pair: str) -> None:
        with self._request_lock:
            self._request_counts[pair] += 1

    def request_count(self, pair: str) -> int:
        with self._request_lock:
            return self._request_counts[pair]


class _MockBinanceHandler(BaseHTTPRequestHandler):
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

        valid = (
            params.get("interval", [""])[0] == "1m"
            and params.get("limit", [""])[0] == "1"
            and start_time >= BUCKET_START_MS
            and (start_time - BUCKET_START_MS) % INTERVAL_MS == 0
            and end_time == start_time + INTERVAL_MS - 1
            and pair in SUPPORTED_PAIRS
        )
        if not valid:
            self.send_error(400)
            return

        bucket_index = (start_time - BUCKET_START_MS) // INTERVAL_MS
        close_price = format_price(mock_scaled_price(pair, bucket_index))
        response = [[
            start_time,
            close_price,
            close_price,
            close_price,
            close_price,
            "0",
            end_time,
            "0",
            60,
            "0",
            "0",
            "0",
        ]]
        payload = json.dumps(response).encode()
        self.server.record_request(pair)
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, fmt: str, *args):
        LOG.debug("mock Binance index kline: " + fmt, *args)


@contextmanager
def mock_binance_index_kline_server(port: int = MOCK_BINANCE_PORT):
    server = _MockBinanceServer(("127.0.0.1", port))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
