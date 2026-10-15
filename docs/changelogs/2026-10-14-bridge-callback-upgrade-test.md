# Pin the callback failure upgrade for Bridge verification

Primary record: [Mono result and Dev evidence](https://github.com/Galxe/mono-grav/blob/codex/issue131-oracle-install-e2e/docs/changelogs/2026-10-14-bridge-callback-upgrade-test.md).

## Background and approved behavior

The user approved this temporary SDK pin for the PR119 NativeOracle upgrade at
Issue131's existing timestamp. SDK retains its parser and execution integration.

## Integration and verification

Reviewed and committed SDK `2213742d5847a9834bcf19835e82f203673b52c1` pins
Reth `0a52b0c24c007fe1745c7f0f26ac7abf9012a0a6` in the manifest, workspace
patch and lock sources. Gaptos `a64f8adc274bf2681df796766ef9a5b195fee44b`,
package versions and unrelated lock edges remain unchanged.

Gravity Dev passed the 55 fullnode tests,
`cargo clippy --locked --all-targets --all-features -- -D warnings`, and the
locked `gravity_node` release build. The primary Mono record owns the exact
lock/ELF identity, build inputs and cross-repository acceptance evidence.

## Execution boundaries and outcome

All builds, tests and formatting ran on Gravity Dev; local work was editing and
static review. This final edit is documentation only: the SDK/Reth revisions
above remain the tested code heads, with no repin, lock regeneration or rebuild.
The primary Mono record owns final case dispositions and original harness issues.
No push, merge or deployment follows from this record.
