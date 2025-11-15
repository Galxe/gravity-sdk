from setuptools import setup, find_packages

setup(
    name="gravity-e2e",
    version="0.1.0",
    description="E2E Test Framework for Gravity Node",
    packages=find_packages(),
    install_requires=[
        "web3>=6.0.0",
        "eth-account>=0.8.0",
        "aiohttp>=3.8.0",
        "pyyaml>=6.0",
        "pytest>=7.0.0",
        "pytest-asyncio>=0.21.0",
    ],
    python_requires=">=3.8",
    entry_points={
        "console_scripts": [
            "gravity-e2e=gravity_e2e.main:main",
        ],
    },
)