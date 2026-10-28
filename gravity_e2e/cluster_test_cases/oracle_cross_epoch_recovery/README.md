# Oracle cross-epoch recovery E2E

This suite checks that an offline node recovers Oracle deliveries across a
validator-set change. Its four genesis validators have voting power
`5:5:1:1` and a 180-second epoch. Node1 and node2 provide 10 of 12 voting
power while node4 is stopped. After governance removes node3, they provide 10
of the remaining 11, so consensus and Oracle certification can continue on
both sides of the epoch change.

The test registers a deterministic NVDA price feed (`sourceType=3`, feed ID
`1005`) using the local Binance fixture. It first confirms the feed delivers,
then stops node4 and observes another Oracle delivery in the original
four-validator epoch. Governance calls `forceLeaveValidatorSet` for node3;
the test waits for the next epoch and a delivery with the changed validator
set. Finally, it restarts node4 and checks that node1, node2, and node4 agree
on the delivery receipts, block hashes, state roots, source progress, and
resolved prices at both delivery heights. It also checks node4's recovery log
for the Oracle block from the original epoch.

Run from the SDK repository root after building `gravity_node` and
`gravity_cli`:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh \
    oracle_cross_epoch_recovery \
    --force-init \
    --log-cli-level=INFO
```

The fixture uses localhost only; it does not need Binance credentials or
public network access.

The shared local runner globally stops `gravity_node` processes, and genesis
generation resets its contracts checkout. Run the command in a disposable test
environment when other local clusters or contract edits need to be preserved.
