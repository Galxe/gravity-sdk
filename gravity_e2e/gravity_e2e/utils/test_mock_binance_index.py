import json
from urllib.error import HTTPError
from urllib.request import urlopen

import pytest

from gravity_e2e.utils.mock_binance_index import (
    BUCKET_START_MS,
    INTERVAL_MS,
    format_price,
    mock_binance_index_kline_server,
    mock_scaled_price,
)


def _url(base_url: str, pair: str, start_time: int) -> str:
    return (
        f"{base_url}/fapi/v1/indexPriceKlines?pair={pair}&interval=1m&"
        f"startTime={start_time}&endTime={start_time + INTERVAL_MS - 1}&limit=1"
    )


def test_returns_exact_closed_bucket_and_tracks_requests():
    with mock_binance_index_kline_server(port=0) as server:
        with urlopen(_url(server.base_url, "TSLAUSDT", BUCKET_START_MS), timeout=2) as response:
            rows = json.loads(response.read())

        assert len(rows) == 1
        assert rows[0][0] == BUCKET_START_MS
        assert rows[0][4] == format_price(mock_scaled_price("TSLAUSDT", 0))
        assert rows[0][6] == BUCKET_START_MS + INTERVAL_MS - 1
        assert server.request_count("TSLAUSDT") == 1


def test_price_is_deterministic_per_pair_and_bucket():
    with mock_binance_index_kline_server(port=0) as server:
        start_time = BUCKET_START_MS + 2 * INTERVAL_MS
        with urlopen(_url(server.base_url, "NVDAUSDT", start_time), timeout=2) as response:
            rows = json.loads(response.read())

        assert rows[0][4] == format_price(mock_scaled_price("NVDAUSDT", 2))


def test_rejects_unaligned_bucket():
    with mock_binance_index_kline_server(port=0) as server:
        with pytest.raises(HTTPError) as error:
            urlopen(_url(server.base_url, "TSLAUSDT", BUCKET_START_MS + 1), timeout=2)
        assert error.value.code == 400
