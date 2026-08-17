# OracleV1 Bridge Hardfork E2E

This manual four-validator suite proves that the live `sourceType=0` bridge
survives the OracleV1 code-only hardfork. It is intentionally separate from
`oracle_v1_rolling_upgrade`: the rolling suite proves binary replacement, while
this suite proves bridge state and execution across the activation block.

The test starts every validator on the candidate binary but installs the exact
signed testnet pre-fork runtimes for `NativeOracle` and `OracleTaskConfig` in
genesis. A deterministic source RPC exposes canonical `MessageSent` logs in two
stages:

1. release nonce `N` before `oracleV1Block` and verify the legacy record, mint,
   balance, and nonce on all validators;
2. stop node4 with a pre-fork database;
3. capture the exact code and account proofs at `H-1` and `H` on the remaining
   validators;
4. restart node4 after `H` and require it to replay the migration and converge;
5. release nonce `N+1` after `H+1` and verify one mint, sequential source
   progress, and identical state on all validators.

The summary records whether `getRecord(N+1)` remains populated. Set
`ORACLE_V1_REQUIRE_SOURCE0_RECORDS=1` to turn preservation of the legacy pull
API into a hard release gate. This defaults to `0` until the compatibility
decision tracked in `gravity_chain_core_contracts#112` is resolved.

Run the suite with the candidate binary already built:

```bash
./gravity_e2e/run_test.sh \
  oracle_v1_bridge_hardfork \
  --force-init \
  --log-cli-level=INFO
```

For a shorter local smoke, the activation block may be overridden while still
leaving enough time for the first bridge delivery and node4 shutdown:

```bash
ORACLE_V1_BRIDGE_ACTIVATION_BLOCK=180 \
  ./gravity_e2e/run_test.sh \
    oracle_v1_bridge_hardfork \
    --force-init \
    --log-cli-level=INFO
```

The ignored `artifacts/oracle_v1_bridge_hardfork_summary.json` contains the
pre/post contract proofs, bridge observations, replay timing, canonical
checkpoints, and `getRecord` compatibility result.
