# Polymarket Match Oracle E2E

The focused local test for a three-outcome sports market. It retains coverage
that the binary combined demo does not provide: random winner mapping, winner
claim, and loser zero-claim checks.

Run:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh polymarket_mock --force-init
```

See [`../../docs/ORACLE_E2E.md`](../../docs/ORACLE_E2E.md) for the complete
workflow, deterministic debugging option, revisions, and product boundary.
