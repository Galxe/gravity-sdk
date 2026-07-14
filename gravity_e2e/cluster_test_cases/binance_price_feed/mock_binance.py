"""CLI wrapper for the shared deterministic Binance index-kline fixture."""

import argparse
import logging
from pathlib import Path
import sys

E2E_ROOT = Path(__file__).resolve().parents[2]
if str(E2E_ROOT) not in sys.path:
    sys.path.insert(0, str(E2E_ROOT))

from gravity_e2e.utils.mock_binance_index import MOCK_BINANCE_PORT, serve_forever


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=MOCK_BINANCE_PORT)
    parser.add_argument("--pid-file", type=Path)
    args = parser.parse_args()

    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    serve_forever(args.port, args.pid_file)


if __name__ == "__main__":
    main()
