# Combined Oracle Demo

The deterministic review gate for Binance price rounds and a binary
Polymarket settlement in one local Gravity cluster. It also writes the
combined frontend runtime config when `--demo-config-out` is supplied.

Run:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo --force-init
```

See [`../../docs/ORACLE_E2E.md`](../../docs/ORACLE_E2E.md) for the coverage
matrix, frontend mode, stop command, pinned revisions, and scope.
