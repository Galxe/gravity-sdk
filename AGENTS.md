# Repository Guidance

## Review guidelines

- Focus on correctness, security, consensus safety, rollback behavior, and compatibility regressions.
- Verify claims against the actual execution path before reporting a finding.
- Treat consensus divergence, state corruption, unsafe key handling, authentication bypass, authorization bypass, fund loss, permanent DoS, and data-loss risks as high-priority findings.
- Pay special attention to consensus recovery, block synchronization, quorum store, epoch changes, validator-set changes, DKG/JWK data, randomness, genesis assets, and operator-facing configuration.
- For cross-repository changes involving gravity-reth, grevm, contracts, or docsite behavior, identify which side owns the broken invariant before assigning severity.
- For docs-only changes, verify commands, paths, release tags, package links, network names, and source-of-truth URLs.
- Avoid reporting a finding only because code looks unusual. Check whether the behavior is intentional, already covered by tests, or constrained by an upstream/downstream invariant.
- When reporting a finding, include the concrete call path, affected files, severity rationale, and a minimal reproduction or validation path when possible.

