"""Deterministic local Binance index-kline fixture shared by oracle E2E suites."""

from contextlib import contextmanager
import json
import logging
import os
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

LOG = logging.getLogger(__name__)

BUCKET_START_MS = 1_783_252_500_000
INTERVAL_MS = 60_000
DECIMALS = 8
MOCK_BINANCE_PORT = 18547
SUPPORTED_PAIRS = {"NVDAUSDT", "TSLAUSDT"}


def format_price(scaled_price: int) -> str:
    whole = scaled_price // 10**DECIMALS
    fraction = scaled_price % 10**DECIMALS
    return f"{whole}.{fraction:0{DECIMALS}d}"


def mock_close_price(pair: str, bucket_index: int) -> str:
    base_prices = {"NVDAUSDT": 19_592_645_000, "TSLAUSDT": 40_067_545_000}
    increments = {"NVDAUSDT": 10_000_000, "TSLAUSDT": 25_000_000}
    return format_price(base_prices[pair] + bucket_index * increments[pair])


class MockBinanceIndexKlineHandler(BaseHTTPRequestHandler):
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
        if (
            params.get("interval", [""])[0] != "1m"
            or params.get("limit", [""])[0] != "1"
            or start_time < BUCKET_START_MS
            or (start_time - BUCKET_START_MS) % INTERVAL_MS != 0
            or end_time != start_time + INTERVAL_MS - 1
            or pair not in SUPPORTED_PAIRS
        ):
            self.send_error(400)
            return

        bucket_index = (start_time - BUCKET_START_MS) // INTERVAL_MS
        close_px = mock_close_price(pair, bucket_index)
        response = [
            [
                start_time,
                close_px,
                close_px,
                close_px,
                close_px,
                "0",
                end_time,
                "0",
                60,
                "0",
                "0",
                "0",
            ]
        ]
        payload = json.dumps(response).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, fmt: str, *args):
        LOG.debug("mock Binance index kline: " + fmt, *args)


class ReusableThreadingHTTPServer(ThreadingHTTPServer):
    allow_reuse_address = True


@contextmanager
def mock_binance_index_kline_server(port: int = MOCK_BINANCE_PORT):
    server = ReusableThreadingHTTPServer(("127.0.0.1", port), MockBinanceIndexKlineHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def serve_forever(port: int, pid_file: Path | None = None):
    if pid_file is not None:
        pid_file.parent.mkdir(parents=True, exist_ok=True)
        pid_file.write_text(f"{os.getpid()}\n")
    server = ReusableThreadingHTTPServer(("127.0.0.1", port), MockBinanceIndexKlineHandler)
    LOG.info("Mock Binance index kline server listening on 127.0.0.1:%s", port)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        if pid_file is not None:
            pid_file.unlink(missing_ok=True)
