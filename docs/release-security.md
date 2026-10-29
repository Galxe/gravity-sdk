# Official release security

This implements the SDK portion of [mono-grav #165](https://github.com/Galxe/mono-grav/issues/165), children [#167](https://github.com/Galxe/mono-grav/issues/167), [#168](https://github.com/Galxe/mono-grav/issues/168) and [#169](https://github.com/Galxe/mono-grav/issues/169). It changes release tooling; platform enablement and controlled acceptance below remain necessary. It does not issue the [#164 emergency hotfix](https://github.com/Galxe/mono-grav/issues/164).

## Required platform setup

A repository administrator must review and apply these settings separately. The workflow deliberately fails when they are absent or unreadable. This PR does not change live settings.

1. Confirm the GitHub plan supports artifact attestations, protected environments with required reviewers, tag rulesets and immutable releases. The SDK is currently public. Private/internal repository attestations require GitHub Enterprise Cloud; do not silently downgrade verification when plan or visibility changes.
2. Enable immutable releases in repository/organization settings. The policy reader checks `GET /repos/Galxe/gravity-sdk/immutable-releases` for `enabled=true`. This API requires **Administration read**, which the ordinary `GITHUB_TOKEN` cannot request.
3. **Policy-reader authority is an unresolved rollout blocker.** The current workflow requests a dedicated App token with **Administration read** and **Environments read**, scoped to only gravity-sdk. That is sufficient for the immutable-settings query, but **cannot establish an empty ruleset bypass list**: [GitHub's ruleset API](https://docs.github.com/en/rest/repos/rules#get-a-repository-ruleset) returns `bypass_actors` only to callers with ruleset-write access. The checker rejects missing visibility with an explicit error, and this PR must not be presented as operationally ready. An owner must approve an isolated reader design with sufficient visibility, or an independently verifiable alternative; do not silently elevate the App, assume an omitted list is empty, or supply an unverified policy receipt. The existing `RELEASE_POLICY_APP_ID` / `RELEASE_POLICY_APP_PRIVATE_KEY` inputs remain for the read-limited candidate; actual installation, credential scope/rotation and any permission redesign require owner review. The App key is not an artifact-signing key; artifact signatures use GitHub OIDC.
4. Add the [version tag ruleset](../scripts/release/tag-ruleset.json): active target `tag`, include `refs/tags/v*`, empty excludes and bypass actors, rules `update` and `deletion`. The checker intentionally accepts this narrow pattern rather than approximating arbitrary fnmatch policy. Limit initial version-tag creation to authorized release maintainers through a separate organization/repository policy; keep update/deletion rules independent so creator exceptions cannot move existing tags.
5. Create environment **release**. Require at least one designated release owner/team, enable **Prevent self-review**, disable administrator bypass, and use custom deployment policies with **only tag `v*`**. Do not permit branch refs or general `*`. Environment owners verify the exact source SHA, reviewed code/dependencies and expected version before approving. Branch protection/review of release source and workflow changes remains an administrator responsibility.
6. Permit the job token only the requested capabilities: policy/test/build `contents:read`; publish `contents:write`, `packages:write`, `attestations:write`, `id-token:write`. Organization Actions policy must allow the exact pinned upstream action commits. Ensure GHCR `gravity_node` permits this repository's job token to publish.
7. Provision consumers with a compatible trusted `gh` CLI and authenticated API/registry access as required. Check `gh attestation verify --help` for `--signer-digest`, `--source-ref`, `--source-digest` and `--deny-self-hosted-runners`. Missing support is a hard error. Local development verified CLI **2.74.2** with these flags; runner availability must be confirmed in controlled acceptance.

`tag-ruleset.json` is a template pending administrator application, not evidence of active policy. Retain the applied ruleset ID and read back `gh api repos/Galxe/gravity-sdk/rulesets?includes_parents=true` plus its detailed rules and the release environment before accepting rollout. The App key remains a long-lived read-only policy credential; owner must confirm actual repository/environment secret scope and rotation. This change does not claim it is already environment-scoped or eliminate the key.

Diagnostic audit using the configured policy App installation token as `GH_TOKEN` (the current read-only candidate cannot complete bypass verification; a successful fixture is not a live permission proof):

```bash
python3 scripts/release/release.py audit-policy
```

The checker rejects malformed/truncated responses, disabled/evaluate rulesets, bypass actors, missing update/delete rules, broad environment refs, self-review or administrator bypass. Network/API permission failures stop the workflow. No setting is mutated by this command.

## Build, approval and publication

Create a new reviewed version tag pointing to the intended source commit. A tag push triggers the workflow; manual dispatch must select that same tag as its workflow ref and input (`gh workflow run release.yml --repo Galxe/gravity-sdk --ref vX.Y.Z -f tag=vX.Y.Z`). Branch dispatch cannot publish. Missing environment/reviewers/tag controls cause the earlier policy job and final publication recheck to fail closed; merging this workflow does not permit arbitrary tag pushes to publish before setup. Same-tag push and manual dispatch intentionally share a serialized concurrency group: a duplicate waits, then `assert-new` rejects any draft/release. This avoids concurrent publication races; duplicate fast-failure is not the chosen contract. The tag must already be protected; existing releases **including drafts** are refused. Tags with prerelease suffixes (for example `v1.2.3-rc.1`) are marked as GitHub prereleases; stable tags become ordinary releases.

Each policy/build/publish job runs in its own fresh GitHub-hosted Ubuntu 22.04 VM. PR validation runs the offline helper tests and never enters policy/build/publish. The build job has no release App key, publication token, package write or OIDC permission. It installs the pinned Rust toolchain from `rust-toolchain.toml`, uses `cargo build --locked` with ordinary production features, fresh Cargo/Rustup directories and no restored caches. Root/package installation and Docker access are confined to the disposable hosted VM; Docker does not connect to the historical self-hosted runner or its socket. The source build must still be trusted and reviewed.

The build logs OS/glibc/toolchain, rejects either ELF binary requiring a GLIBC symbol newer than 2.35 (the selected Ubuntu 22.04 build ceiling), and checks runtime image CLI/curl. `readelf --version-info` inspects actual artifact requirements without executing them; missing/unreadable version information fails closed. This guard limits build-image/sysroot drift and does not prove target-host compatibility. Ubuntu 22.04's glibc baseline must be tested on supported deployment OSes; lower build glibc alone is not full ABI acceptance. Toolchain/lockfile are fixed, but hosted-image and apt/runtime-image inputs still evolve: this provides recorded provenance and isolation, **not bit-for-bit reproducible builds**. Do not add shared writable caches as an optimization without reviewing the isolation boundary.

Build output transfer uses the exact immutable Actions artifact ID from this run. The approved publish job loads the image without executing it, pushes a unique `sha-<commit>-run-<run>-attempt-<attempt>` reference and records its canonical OCI digest. It creates no version/latest image tag. Consumers pin `ghcr.io/galxe/gravity_node@sha256:...`; GHCR tags themselves are not protected by GitHub release immutability.

The publish job generates GitHub artifact attestations for both binary files, the OCI digest and `release-manifest.json`. It rechecks live policy with a fresh read-only App token after approval and immediately before release creation. It verifies all four subjects against strict provenance, creates a new draft, uploads without replacement, downloads **every complete asset**, compares the exact inventory and SHA256 for every file, then publishes. A final API check must report `immutable=true` and `draft=false`. Registry credentials are logged out; App installation tokens are revoked by the pinned action and the hosted VMs are discarded.

An interruption leaves a draft or orphan OCI content for an owner to investigate. Reruns cannot modify drafts or published versions. Do not automatically remove the draft, overwrite bytes or reuse a version. Publish corrected content under a new version and record its source/change reason. A policy change raced by a privileged administrator remains within the platform administrator trust boundary; final `immutable=true` is checked and any failure must be escalated.

## Provenance and manifest contract

The release contains exactly these files:

- `gravity_node`, `gravity_cli` and their `.sha256` sidecars.
- `release-manifest.json`.
- `binary-provenance.jsonl`, `manifest-provenance.jsonl`, `image-provenance.jsonl` (Sigstore bundles, also discoverable through GitHub attestations; image proof is pushed to GHCR).

Manifest version 1:

```json
{
  "schema_version": 1,
  "repository": "Galxe/gravity-sdk",
  "tag": "vX.Y.Z",
  "source_commit": "<full lowercase 40-character commit SHA>",
  "workflow": "Galxe/gravity-sdk/.github/workflows/release.yml",
  "run_id": "<decimal workflow run ID>",
  "run_attempt": "<decimal attempt>",
  "artifacts": {
    "gravity_node": {"sha256": "<64 lowercase hex>"},
    "gravity_cli": {"sha256": "<64 lowercase hex>"}
  },
  "image": {"name": "ghcr.io/galxe/gravity_node", "digest": "sha256:<64 lowercase hex>"}
}
```

Consumers obtain the expected tag/source SHA and binary hashes from reviewed deployment configuration; never trust a same-origin checksum/manifest alone. The manifest is itself attested. For this direct workflow the signer commit and source commit are the same approved SHA. Verify each downloaded binary before installation, retaining the full expected SHA256 check:

```bash
gh attestation verify ./gravity_node \
  --repo Galxe/gravity-sdk \
  --signer-workflow Galxe/gravity-sdk/.github/workflows/release.yml \
  --signer-digest "$APPROVED_SOURCE_SHA" \
  --source-ref "refs/tags/$APPROVED_TAG" \
  --source-digest "$APPROVED_SOURCE_SHA" \
  --deny-self-hosted-runners
```

Repeat for CLI and the manifest, or append `--bundle ./binary-provenance.jsonl` for a saved binary bundle. Use the identical policy for `oci://ghcr.io/galxe/gravity_node@sha256:...` and the image bundle. Verify content/digest and the expected origin; an arbitrary valid signature or repository-only match is insufficient. Do not invent `--source-repo` (not a CLI verification flag). Fail closed for absent proof, unknown source/workflow/ref, tampered bytes, unsupported verifier or network errors. Source claims/predicate metadata do not replace certificate-bound identity policy. An attestation identifies the approved workflow/source; it does not prove harmless source or uncompromised dependencies.

After installation, consumers must rehash the installed and running executable content and confirm health/version/convergence according to their deployment contract. A verified build and PR CI do not establish deployed runtime state.

## Legacy versions and controlled acceptance

Existing mutable/unsigned versions (including v1.10.1) do not become signed or immutable retroactively. Keep exact independently reviewed repository/tag/asset/SHA pins in a **consumer-owned, narrowly scoped, expiring migration allowlist** if continued use is necessary. Record owner, trust basis, fixed removal date and replacement version; no generic missing-proof skip flag. This SDK producer provides no bypass. New releases use this workflow and require proofs. Mainnet/Testnet consumers own their actual legacy entries and sunset policies.

Before enabling this path for an official release, the release owner must preserve the old runner's incident evidence, configure the platform prerequisites and execute a disposable protected version through the entire path. Record run URLs, source SHA, full binary hashes, OCI digest, attestation verification results, OS smoke test and environment approval. Demonstrate:

1. Another PR/E2E job's marker/cache/HOME/credentials are absent in the hosted release VM; hosts and Docker daemons are distinct. Do not populate secret probe files with actual credentials.
2. Authentic node/CLI and OCI content pass the consuming policy; tampered, unsigned, wrong-repository/workflow/tag/commit content is rejected.
3. After publication, asset replacement, version-tag movement/deletion and rerun cannot change the published content. Perform these destructive rejection checks only against the controlled test version, with separate owner authorization.
4. A supported target OS starts the exact verified binaries/image, and deployment consumers retain their pre/post-install byte checks.

Historical tags still contain their historical workflow definitions: manual dispatch against an old tag can execute that old path. Before activation, the infrastructure owner must prevent old shared-runner release execution and old ref dispatch through runner access/Actions policy while preserving incident evidence; new workflow code cannot retroactively replace tagged workflows. Confirm ordinary hosted VM CPU/memory/disk capacity for the full Rust build, or provision a fresh hosted larger runner after review; no candidate build capacity has yet been demonstrated.

No release, historical-runner cleanup, platform settings change or production rollout was executed for this implementation. Do not mark #167/#168/#169 operationally complete until those acceptance records exist. If setup fails, stop publication; keep historical release paths disabled and fix the configuration or publish a new reviewed workflow version. Do not roll back by restoring shared-runner publication or overwrite behavior.

References: [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases), [GitHub artifact attestations](https://docs.github.com/en/actions/concepts/security/artifact-attestations), [GitHub CLI verification policy](https://cli.github.com/manual/gh_attestation_verify), [GitHub App installation-token action](https://github.com/actions/create-github-app-token).
