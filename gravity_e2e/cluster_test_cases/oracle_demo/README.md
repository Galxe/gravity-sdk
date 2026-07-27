# Combined Oracle Demo

The deterministic review gate for Binance price rounds and a binary
Polymarket settlement in one local Gravity cluster. It also writes the
combined frontend runtime config when `--demo-config-out` is supplied.

The binary market has no Gravity-local oracle deadline. After the reviewed
Polygon CTF payout reaches the resolver, anyone may call `finalizeMarket`;
missing, pending, or invalid observations leave the market locked.

Run:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo --force-init
```

See [`../../docs/ORACLE_E2E.md`](../../docs/ORACLE_E2E.md) for the coverage
matrix, frontend mode, stop command, pinned revisions, and scope.
