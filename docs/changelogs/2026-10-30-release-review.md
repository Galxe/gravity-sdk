# SDK PR #851 review follow-up

## 1. Background and Current State

AshinGau's [2026-10-29 review](https://github.com/Galxe/gravity-sdk/pull/851#issuecomment-6495903031) raises hosted-build glibc drift, unapplied settings/templates, long-lived policy-App credentials and duplicate-dispatch queueing. The environment concern's arbitrary-tag-publication conclusion is contradicted by the earlier fail-closed audit-policy job and final recheck.

## 2. Problem Model and End-to-End Behavior

B3: inspect actual node and CLI ELF requirements before artifact transfer, rejecting GLIBC symbols newer than 2.35. F2: missing version data or failed ELF inspection rejects the build. Preserve existing independent environment-review and non-overwrite publication gates.

## 3. Research, Findings, and Architecture Decision

D4: use readelf --version-info without executing binaries; 2.35 preserves the existing selected Ubuntu 22.04 build ceiling and does not assert target-host acceptance. Required target OS/runtime-image smoke tests remain outstanding. The post-push Codex review correctly identified that Administration-read cannot see ruleset bypass_actors; GitHub returns that field only with ruleset-write access. The current read-limited design cannot complete its audit. This is a new external-authority decision, not grounds to interpret an omitted list as empty. A policy-reader App has no equivalent ordinary GITHUB_TOKEN Administration-read permission; its long-lived key remains an owner-managed operational credential, not an artifact-signing key. Same-tag serial execution intentionally prevents concurrent publication.

## 4. Implementation Design

- [x] Add release.py check-abi and invoke after both builds before upload.
- [x] Cover accepted/rejected ABI versions and unknown inspection results.
- [x] Clarify template pending application/readback, credential scope/rotation, fail-closed missing environment and intentional concurrency.
- [x] Report hidden bypass-list visibility as an explicit permission blocker and correct setup documentation; preserve fail-closed rejection and current App permissions.

## 5. Verification and E2E Design

Run python3 -m unittest discover -s scripts/release -p 'test_*.py' -v, /tmp/jugge-actionlint/actionlint .github/workflows/release.yml, git diff --check and the required secret scanner. No make lint-fix target exists; lint unavailable. Full Cargo/image build, real OIDC/immutable release acceptance and target-host smoke tests remain owner-controlled operational gates, not implied by PR CI.

## 6. Human Decisions and Interaction

The user authorized valid review fixes and existing PR updates. Main agent approved an ELF GLIBC ceiling of 2.35 preserving Ubuntu 22.04, documentation of operational App/environment/ruleset duties, and unchanged reviewer/concurrency behavior. No settings, secrets, release or deployment changes are authorized.

## 7. Outcome and Evidence

24 release-helper tests passed with `python3 -m unittest discover -s scripts/release -p 'test_*.py' -v`; `/tmp/jugge-actionlint/actionlint .github/workflows/release.yml` and `git diff --check` passed. A temporary-directory CLI run against actual local `/bin/sleep` and `/bin/true` ELF files also passed. Full candidate self-review covered the existing producer flow plus the ABI guard; no additional blocker found. Parent independently read the complete follow-up diff and approved publication. `PATH=/tmp/issue845-publish-tools:$PATH /home/jingyue/projects/mono-grav/misc/cc-plugins/plugins/aldev/scripts/secret-scan.sh --base origin/main --repo /home/jingyue/projects/gravity-sdk-issue165 --pr-body /tmp/sdk-pr851-body-current.md` passed committed history, staged candidate and PR text with no leaks. The prior Codex prerelease-classification finding was already fixed before this follow-up; its thread is resolved.

## 8. Remaining Work

The [new P1 review](https://github.com/Galxe/gravity-sdk/pull/851#discussion_r4415308970) remains unresolved pending an owner-approved policy-reader authority design. See the [official ruleset response visibility contract](https://docs.github.com/en/rest/repos/rules#get-a-repository-ruleset). Current-head checks and review monitoring continue. No App permission elevation or platform mutation was performed. Owner still must configure and read back immutable/tag/environment/App controls and execute supported-host release acceptance. Required SDK E2E and human approval are merge gates; no self-hosted dispatch is triggered here.
