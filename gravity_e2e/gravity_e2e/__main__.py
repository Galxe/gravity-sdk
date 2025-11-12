import asyncio
from .main import main
import sys

if __name__ == "__main__":
    sys.exit(asyncio.run(main()))