# Rust style and port constraints

Use the BeautifulRust best-practices, review and port skills as guidance, subject to repository rules and observable compatibility. [AGENTS.md](../AGENTS.md) governs the task; [ARCHITECTURE.md](ARCHITECTURE.md) maps the current products. Prefer simple Rust ownership, narrow visibility, explicit domain types, useful errors, and tests of behavior. Avoid speculative abstractions and clones added only to satisfy borrowing. Separate safe policy from unsafe or SDK-facing adapters where the actual ownership boundary permits it.

For a port, read the original implementation, its callers and tests first. Characterize observable ordering, limits, failures and quirks. Map original behavior to Rust tests and keep unexplained behavior until a change is explicitly approved. Review soundness and correctness before style. Cite concrete source and consequences in review findings.

## Compatibility exceptions

These are reasons not to apply a generic BeautifulRust rule mechanically. They do not excuse undocumented unsafe code or a behavioral regression.

- **ABI and packing:** Preserve externally visible layouts, calling conventions, fixed-width fields, alignment, packing, status codes and required out-parameters. `repr(C)` alone does not prove a match. Check size, offsets, ownership and the matching native declarations and tests. Keep unsafe blocks small; state the invariants in `SAFETY` comments and document caller obligations.
- **`TCHAR`:** The engine's platform conventions include UTF-32 `TCHAR` assumptions. Do not replace this boundary with UTF-8 `&str` or `CString` merely for style; preserve encoding, termination, invalid input and length semantics. When comparing historical Apple C++ boundaries, preserve the meaning of its `MACOSX` guards. Rust uses the actual `target_os` and feature conditions; do not copy a C++ guard into Rust.
- **Reentry:** Native callbacks can synchronously reenter Rust. Do not hold an incompatible mutable borrow or lock across a callback; preserve callback ordering and state visibility. Test reentry and failure paths rather than assuming a callback is a leaf.
- **Drop order:** SDK objects, callbacks, guards and engine resources can require a specific teardown order and thread. RAII is useful only if its destruction order preserves that contract. Do not move cleanup across an observable callback or destroy an owner before its dependents.
- **Numerical behavior:** Preserve required integer widths, overflow, rounding, floating-point operation order, wire bytes and deterministic ordering. An idiomatic algorithm or mathematically equivalent expression can change results; characterize against the applicable reference before replacing it.
- **Exact dependency pins:** `Cargo.toml` deliberately uses exact versions. Do not replace them with version ranges solely because the generic skill recommends ranges. Dependency or lockfile changes need their own scope and verification.

Use `Result` for expected failure where the boundary permits it; do not silently alter a legacy error contract or unwind through FFI. Do not assume `catch_unwind` handles aborting panics or makes reentry safe. Preserve allocation/free ownership across a boundary. Use documented, narrow lint exceptions only when necessary, with a reason.

## Verification

Choose tests by the touched domain and production feature set, not by style alone. Use `cargo fmt --check` rather than a workspace-wide formatting rewrite when checking a bounded task. A generic `--all-features` Cargo invocation may require platform SDKs or incompatible linkage; select supported combinations and report what was not checked. CTest registration, data profiles, artifacts and required behavioral evidence are described in [WORKER_CONTRACTS.md](WORKER_CONTRACTS.md). A compile, clippy run or component test alone is not runtime or retail verification.

## Lint backlog

The ported crates carry a backlog of pre-existing `rustc`/clippy warnings (unused imports,
redundant `unsafe` blocks, style lints) inherited from the port. CI runs clippy with
`-D warnings` for `hp2-ui`, `hp2-codecs`, `hp2-file-hash`, `hp2-bootstrap-paths`, and `hp2-ini`;
add another crate to that step once its backlog is cleared and its behavioral checks pass. Do not suppress the backlog with blanket `allow` attributes.
