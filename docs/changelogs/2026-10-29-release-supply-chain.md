# ci: harden the official release supply chain

## 1. Background and Current State

Primary cross-repository record: [mono-grav #165 design](https://github.com/Galxe/mono-grav/blob/codex/issue165-supply-chain-hardening/docs/changelogs/2026-10-29-supply-chain-hardening.md).
SDK base: `337e0ea9a60d8e18f3e7a0319960e0ecf3008e3c`. The former release workflow uses the shared self-hosted Linux runner, job-wide write permissions, native builds, published releases and `--clobber`. It has no signed build provenance.
Authorized read-only GitHub API observations on 2026-10-29: immutable releases `enabled=false`; rulesets `[]`; only environment `default`, with no protection rules. These are rollout blockers, not claims of exploitation.

## 2. Problem Model and End-to-End Behavior

B1: Build node/CLI and their image in a fresh GitHub-hosted VM with read-only source access, no writable shared caches, no publication token or signing capability.
B2: Release only an existing version tag selected as the workflow ref, approved through a protected environment. Use short-lived identity to attest binaries, manifest and OCI digest. Stage a new draft, verify all uploaded bytes, then publish immutable.
F1: Existing version, missing policy, missing review controls, wrong tag, changed bytes or missing attestations fail closed. Content corrections require a new version; OCI consumers use the signed digest.

## 3. Research, Findings, and Architecture Decision

D1: Use GitHub artifact attestations (OIDC, no artifact-signing private key). Consumer checks bind repository/workflow/source tag/source commit/signer commit and reject self-hosted attestations.
D2: GitHub-hosted Ubuntu 22.04 gives a fresh host, HOME, filesystem and Docker daemon per job. Build Docker access/root is confined to the disposable VM; untrusted build execution gets no publication or OIDC permission. Cargo inputs are locked, no shared cache is restored.
D3: GitHub immutable releases and active non-bypass tag rules must be checked before publishing. The immutable-settings API requires Administration read: use a dedicated GitHub App installation token with read-only platform access, automatically revoked at job completion. The App key authenticates the policy reader, never signs artifacts; it is not passed to build/publish.
Alternatives: retaining the shared runner keeps its contamination boundary; checking only hashes does not bind origin; a policy receipt variable can drift from live settings.
References: [immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases), [artifact attestations](https://docs.github.com/en/actions/concepts/security/artifact-attestations), [strict verification flags](https://cli.github.com/manual/gh_attestation_verify), [repository immutable-settings API](https://docs.github.com/en/rest/repos/repos#check-if-immutable-releases-are-enabled-for-a-repository).

## 4. Implementation Design

`release.yml` owns isolated policy/build/publish jobs and pinned third-party actions. `scripts/release/release.py` owns read-only policy audit, manifest generation and draft upload/download/publish verification. `scripts/release/test_release.py` executes behavioral fault cases with a fake GitHub command backend. `docs/release-security.md` owns administrator prerequisites, exact consumer policy and legacy migration.
Manifest v1 binds `repository`, `tag`, `source_commit`, `workflow`, `run_id`, `run_attempt`, node/CLI SHA256 and OCI name/digest. Manifest and binaries are independently attested. Direct workflow signer commit equals the source commit.

- [x] Read current workflow, children #167/#168/#169 and #164 scope.
- [x] Implement policy enforcement and release helpers.
- [x] Replace workflow and document prerequisites/migration.
- [x] Run behavioral tests and complete diff/secret review.
- [ ] Publish reviewed PR; retain actual rollout as explicit remaining work.

## 5. Verification and E2E Design

Run `python3 -m unittest discover -s scripts/release -p 'test_*.py' -v`, workflow YAML parsing and `git diff --check`. No `make lint-fix` target exists; repository lint is unavailable for this Python/workflow-only change. Behavioral tests cover all policy predicates, missing/partial platform responses, existing releases, failed upload/download, content tampering and publication immutability.
E2E Required: yes for operational acceptance. A controlled new version must build on hosted runners, obtain real attestations, publish immutable and reject asset/tag replacement; consumers must accept authentic bytes and reject tampered/wrong-origin bytes. Local mocked transport tests do not establish real GitHub OIDC, immutable enforcement, OS compatibility or full build success.

## 6. Human Decisions and Interaction

The user authorized network access, all child implementation, GPT-6.1 Sol delegation, commit/push/PR. This overrides routine aldev staging gates and single-agent preference. No authorization was given to modify live settings, issue a release, rotate credentials, clean the historical runner or deploy. Parent review precedes agent publication.

## 7. Outcome and Evidence

Candidate implemented: B1/B2/F1 enforced by isolated build/publish jobs and the release helper. `python3 -m unittest discover -s scripts/release -p 'test_*.py' -v` passed 20 tests, including a 20-case live-policy acceptance/rejection matrix, real byte hashes and complete staged/downloaded asset comparisons, interrupted uploads, rerun refusal, wrong source identity, invalid proof rejection and separate policy/publisher identities. `/tmp/jugge-actionlint/actionlint .github/workflows/release.yml` and `git diff --check` passed. PyYAML parsed four jobs. Local `gh version` is `2.74.2`; its verifier help confirms every consumer policy flag used here.
Action references were resolved through GitHub APIs on 2026-10-29 and pinned to concrete commits, including dereferencing the annotated provenance action tag. Live read-only settings confirmed the blockers in section 1. No platform settings were mutated or release/rollout performed.
Full native Cargo/OCI build, real OIDC signing and controlled immutable publication were not run: they require owner-configured platform controls and a protected test version. Tests mock the GitHub/network boundary and do not assert a real signature passed. No `make lint-fix` target exists. The required scanner (`PATH=/tmp/issue845-publish-tools:$PATH /home/jingyue/projects/mono-grav/misc/cc-plugins/plugins/aldev/scripts/secret-scan.sh --base origin/main --repo /home/jingyue/projects/gravity-sdk-issue165`) passed against the staged candidate (~53KB, no leaks). Full diff self-review found and fixed missing runner-context support at job-level env and narrow fail-closed policy behavior. Parent review/publication evidence pending.

## 8. Remaining Work

Configure GitHub App policy-read authority, immutable releases, tag protections and protected environment; verify GitHub plan support for attestations/protection controls. Execute the controlled release and consuming deployment acceptance. Confirm hosted runner build capacity and supported target OS ABI; retire old-tag shared-runner dispatch paths through infrastructure policy before activation. Preserve former runner evidence through the restricted incident process. None of these live steps is implied complete by this PR.
