# Polymarket Binary Oracle E2E

The focused local test for a binary market. Unlike the combined demo's fixed
outcome, this suite randomizes the winning slot and checks winner claimability,
loser zero-claimability, and exact source progress.

Set `POLYMARKET_MOCK_WINNING_SLOT` to `0` or `1` for a reproducible run.

Run:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh polymarket_mock --force-init
```

See [`../../docs/ORACLE_E2E.md`](../../docs/ORACLE_E2E.md) for the complete
workflow, deterministic debugging option, revisions, and product boundary.
