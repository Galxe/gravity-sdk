# Pin the callback failure upgrade for Bridge verification

## Background and approved behavior

This temporary integration branch tests contracts PR119
`174123adf25395d0490210a3fad46e6018e10366` with Issue131's timestamp installer.
Reth installs the new NativeOracle runtime with its task/callback configuration;
SDK keeps its existing parser and execution integration. Historical Gamma bytes
and the original PR branches remain separate.

The focused source sequence has four messages: a normal Sender deposit of 6,
a direct Portal message with a valid bridge-shaped body, a trusted Sender deposit
of `2**128`, and a normal Sender deposit of 7. The Receiver rejects the direct
publisher with `InvalidSender`; the real mint precompile rejects the oversized
amount with `MintFailed`. Oracle consumes all four nonces and source positions,
while only messages 1 and 4 mint, for a destination balance delta of 13. The
failed mint leaves its source tokens locked; no automatic refund or retry occurs.

Ordinary source accounts may call Sender. Receiver authentication checks the
Sender contract embedded by Portal, and the mint precompile authenticates the
Receiver contract. No recipient address allowlist is implied by this test.

## Integration and verification

Pin one exact reviewed Reth revision in the node manifest, workspace patch, and
all Reth lock sources. Preserve Gaptos `a64f8adc274bf2681df796766ef9a5b195fee44b`,
package versions, and unrelated dependency edges. Cargo lock generation,
formatting, builds, and tests run only on Gravity Dev with cached offline inputs.
The eventual SDK/Reth revisions and actual lock hashes belong to the evidence;
verification remains pending until those exact inputs have run.

The new runtime is compiled from PR119 with solc
`0.8.30+commit.73712a01`, Prague, optimizer 200, viaIR, and metadata bytecode hash
`none`. It has 4010 bytes, Keccak
`0x13c995f14f5add575e656480be4c734a76bf1b4500097e8258f214c65e174a74`, and raw SHA256
`b4140d0eb3140d06dec304938594e8c5568b88412a82c51e132696760dce0444`.
Dev's three Forge Bridge integration tests passed on these inputs; those tests
use a mocked mint and do not establish node E2E acceptance.

A fresh locked release and candidate provenance bind SDK/Reth/Gaptos source,
Cargo.lock, compiler/settings, contracts revision/runtime, and the actual ELF.
The candidate carries the raw NativeOracle artifact and optional
`bridge_callback_upgrade` metadata for independent Mono verification.

After root review of the final code and candidate, run these six cases serially
once: the new focused Bridge case; quiet; source-offline-backlog;
source-online-delayed-finality; empty-task-activation; full oracle-task-install.
Each result remains independent. Preserve failures without automatic retries or
an SDK845 fix. Historical passes do not establish acceptance of this release.

## Execution boundaries and outcome

Use a new owned Dev directory and task lock; preserve old evidence and branches.
The PFN was already offline at this run's preflight. Preserve its observed
absence and existing binary/config/data metadata; do not start or stop it and
do not describe this condition as healthy. Resource guards run before, during,
and after owned jobs. No push, PR publication, merge, or deployment is selected.

Current SDK build/release/E2E outcomes: pending. The primary Mono changelog owns
the cross-repository outcome and source-bound Dev evidence.
