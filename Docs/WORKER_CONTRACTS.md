# Bounded worker contracts

Use this contract for delegated implementation, review and verification. It supplements [AGENTS.md](../AGENTS.md); it does not authorize changes outside the task. See [ARCHITECTURE.md](ARCHITECTURE.md) for products and test entry points, and [RUST_STYLE.md](RUST_STYLE.md) for compatibility constraints.

## Task card

Before starting, specify:

1. **Outcome and invariant:** one observable behavior to preserve or change, plus the production call path and owner being addressed.
2. **Boundary:** allowed files and directories, explicit exclusions, dependencies on other workers, and whether the task is read-only. Identify pre-existing changes and do not reset, stash, overwrite or reformat unrelated work.
3. **Reference:** original implementation, callers, relevant tests and any ABI, data-format or SDK constraints. For a port, distinguish policy, adapter and actual state/resource ownership; map old behavior to new tests.
4. **Verification:** exact applicable commands and named tests, required feature combinations, platform/SDK and data profile (`data-none`, `data-prototype` or `data-retail`). Check that required CTest names are registered for the chosen configuration; an absent test is a gap, not a pass. State prerequisites and the intended artifact location.
5. **Stop conditions:** missing prerequisites, conflicts in shared files, unexpected destructive changes, uncertain ownership, baseline failures or incompatible evidence. Escalate rather than expanding scope or suppressing a failure.

For behavioral changes, select tests that exercise the affected path and failure cases. Where relevant, include ABI/layout, reentry, teardown and platform-backed process tests, not just pure policy tests. A changed default or feature-gate promotion needs explicit authorization and matching acceptance evidence. Native-text gate descriptions currently disagree between `Build/feature-gates.json` and historical guidance in `Docs/OPERATIONS.md`; report the conflict rather than choosing a new policy or default.

## Handback

Report:

- The invariant and exact files changed, or state that the task was read-only. Identify any original behavior or quirk retained and any approved difference.
- Exact commands, named tests, observed pass/fail/blocked outcomes, platform, enabled features and data profile. Do not infer behavior from a successful compile, a test listing or another profile. A retail claim must explain data and index provenance. Bootstrap verifies the overlay manifest/checksums when either passes its readable-file check. It accepts legacy roots when neither does; root acceptance alone is not retail qualification.
- Artifact paths under the test's `HP2_ARTIFACT_DIR`, including failure logs where applicable. The CTest wrapper supplies an isolated environment; standalone checks need an explicitly managed artifact directory.
- Unregistered tests, unavailable SDKs or data, untested platforms and features, open risks, and other unresolved conditions. Report the first failure and any flakiness; do not retry merely to obtain a green result.
- For a port, map the original behavior and tests to the Rust implementation and tests, and say whether production ownership and native implementation removal were actually verified. Do not call a policy-only change a completed cutover.

Golden changes require a separate acceptance record. Do not change pinned third-party sources or commit unless the task and repository rules authorize it.
