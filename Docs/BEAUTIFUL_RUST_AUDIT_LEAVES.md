# BeautifulRust leaf audit — partial

**Status: review incomplete.** The requested scope is 5 internal, unpublished application libraries: `hp2-bootstrap-paths`, `hp2-file-hash`, `hp2-ffi-support`, `hp2-ini` and `hp2-launcher`. Workspace edition is 2024; declared Rust version is 1.99. The highest-priority next step is to finish reading the pending files and their production callers. The checks below do not replace that review.

The initial audit changed no runtime, configuration or test files. The audited checkout already had unrelated modifications, including `hp2-launcher/src/model.rs` and `store.rs`. Their existing diff was inspected; both files were read in full. Later separately reviewed fixes and characterization are recorded below and in the execution ledger.

## Current continuation checkpoint

The initial audit and first-continuation sections below are retained historical
evidence. Their pending lists and zero-test observations are not current queues.
The original `inventory.json` is also retained; hashing was never a source review.

Audit reports record full reading of **42/44 original inventoried files**:
5 manifests and 37 Rust files. Complete caller/owner review and runtime
qualification are separate and remain incomplete. Only
`reporters/lifecycle_gate.rs` and `reporters/static_bsp_probe.rs` lack accepted
complete reads. `leaf-reading-integrity-ihydlsj4/` resolves the identified
integrator-output truncation for `boot.rs` (838 lines) and
`presentation_policy.rs` (1133 lines), including the accepted scale correction.
The fresh reads used chunks of at most 150 lines with matching endpoint hashes;
this follow-up does not re-audit the other 40 files or complete caller review.
The lifecycle/static-BSP provider
refusal was retained; requested Sonnet 5.5 dispatch failed before worker creation
with `no_active_sender_terminal`. No fallback model or invented sender was used.
New `crates/hp2-launcher/AGENTS.md` is domain navigation, outside the original
44 manifest/source entries; it does not increase this source-reading count.

Reports under `leaf-audit/` add these full reads and bounded caller reviews:

- `session-continuation.md`: `import_session.rs`, `migrate.rs`, `orchestrate.rs`,
  `saves.rs`, `tree.rs`; `tree-caller-review.md` adds bounded tree ownership review.
- `policy-continuation.md`: `profile.rs`.
- `reporters-continuation.md`: `global_tick.rs`, `trace_services.rs`.
- `launch-round-review.md`, `shadow-admission-review.md`: the named source files.
- `fire-texture-actor-slot-review.md`: `fire_texture.rs`, `actor_slot_dump.rs`.
- `main-loop-policy-review.md`: `main_loop.rs`, `policy.rs`.
- `transition-creature-review.md`: `actor_transition_ledger.rs`, `creature_generator.rs`.
- `fidelity-world-collision-review.md`: `fidelity.rs`, `world_collision_gate.rs`.
- `boot-presentation-integrator-review.md`: `boot.rs`, `presentation_policy.rs`;
  its companion SHA-256 file records current reviewed sources and sampled hosts.

The original 5 first-continuation reads below are also included. Source reports
identify reset allocation leaks, untyped reflection-pointer reads, event-limit
attribution and potential reentry/lifetime problems. They distinguish concrete
source mechanisms from untested reachability and hypotheses; none is an accepted
runtime fix merely because it was listed.

The embedded-NUL discrepancy has since been reproduced through real bootstrap
entry points, with valid and wrong-hash controls. Historical native C++ has the
same truncation mechanism. This is inherited compatibility behavior, not a proven
Rust-port regression or native-binary parity result. The original recommendation
to reject NUL below is **not accepted for implementation**. See
`bootstrap-nul-repro/{EVIDENCE.md,native-reference.md}`. Active handbook guidance
now correctly distinguishes readable indexed overlays from accepted legacy roots.

Hash characterization now has 5 passing tests and an exact `file_hash_contracts`
CTest; production bodies/signatures remain unchanged. Its zero-test result below
is only the initial baseline. The user also approved exposing all 12 valid launcher
render-scale choices. The repair and review are in `launcher-scale-repair/`.
The review caught an unmatched/NaN fallback regression; `fallback-review/` retains
both pre-fix failures and the subsequent correction to 100%. Re-review gives
bounded acceptance: 3 focused policy tests, 13 exact UI tests and 4 exact
settings-parity tests passed. These are callback/egui/store checks, not physical
window, retail or native parity qualification. Concurrent `model.rs` and
`store.rs` changes remain untouched.

The boot caller review exposed a conditional unwind lifetime violation:
automatic BootHost field drop destroyed owner before attached reporters, whereas
explicit finish orders them correctly. The separately accepted fix declares
reporters before owner. Source and real-owner unwind contracts pass; the prior
rebuilt shipping product also passed its named closure and prototype smoke checks.
`boot-shipping-requalification/run-20261109T155258Z-60180/` and
`boot-shipping-inspection-fsi6zj5b/` retain independently inspected copies, JUnit,
product identities and canaries. The potential stderr-failure route remains
unreproduced; no pre-fix UB test was executed. Later importer/pointer shipping
qualification passed 14 selected checks and Main's independent copy/input/product
inspection at `import-platform-shipping/run-hwtnntkv/` and
`import-platform-inspection-c_ttptly/`. The newer picker/P8 checkpoint then passed
17 fast checks and 3 native-free closure checks at `picker-p8-shipping/`; Main's
independent retained-record acceptances are `picker-p8-inspection-lnxm3l07/` and
`picker-p8-inspection-f42vi8fs/`. This qualifies the recorded macOS/synthetic/
prototype bounds, not native modal acceptance. Later renderer characterization,
5 platform lint fixes and exact input-test extraction have their own default-library
behavior/doc evidence in the execution ledger and no new shipping rebuild. Existing
broader reentry, pointer, reset and publication findings remain open.

The continuing Core, importer, platform and UCC work has its own bounded scope in
[the execution ledger](BEAUTIFUL_RUST_STATUS.md#active-macos-continuation). It does
not increase this original 44-file leaf reading count. UCC default discovery now
uses `hp2_ucc_rust`; 2 original failures and 13 passing post-fix contracts are
retained. Importer lexical traversal is repaired in source, with 5 path tests and
149 default-library cases after separate formatting/doc-link stages. The new SDL
pointer compile-only contract passes but does not establish all public UI thread
and lifetime assumptions. Broader crate audits and the 2 Sonnet reads remain open.

## Initial inventory and coverage (historical)

The complete file-by-file inventory, classifications, review status and SHA-256 snapshots are in `out/macos-arm64/Testing/HP2/beautiful_rust/Artifacts/leaf-audit/inventory.json`. It contains 44 files: 5 manifests and 39 Rust source files. There are no separate test files or generated assets in these 5 crate directories in this snapshot; some source files contain inline tests. Hashing a file for the inventory is not a review.

Read completely: all 5 manifests; `crates/hp2-bootstrap-paths/src/{lib.rs,bootstrap_paths.rs}`, `crates/hp2-file-hash/src/lib.rs`, `crates/hp2-ffi-support/src/lib.rs`, `crates/hp2-ini/src/{lib.rs,launcher.rs}`, and `crates/hp2-launcher/src/{lib.rs,capabilities.rs,display_modes.rs,model.rs,store.rs,reporters/mod.rs}`. This is 17 of 44 files. Relevant bootstrap, settings and importer callers were sampled in `crates/hp2-game/src/{boot_services.rs,launcher_ui.rs}`, `crates/hp2-launcher/src/orchestrate.rs` and `crates/hp2-import/src/collect.rs`; complete caller review remains pending.

**Pending fresh dispatch:** these 27 paths under `crates/hp2-launcher/src/`:

- `atomic.rs`, `boot.rs`, `boot_config.rs`, `command_policy.rs`, `feedback.rs`, `import_session.rs`, `launch_round.rs`, `main_loop.rs`, `migrate.rs`, `orchestrate.rs`, `policy.rs`, `presentation_policy.rs`, `profile.rs`, `saves.rs`, `tree.rs`.
- `reporters/actor_slot_dump.rs`, `actor_transition_ledger.rs`, `creature_generator.rs`, `fidelity.rs`, `fire_texture.rs`, `global_tick.rs`, `json.rs`, `lifecycle_gate.rs`, `shadow_admission.rs`, `static_bsp_probe.rs`, `trace_services.rs`, `world_collision_gate.rs` (all relative to `reporters/`).

## Continuation handoff — still incomplete

The continuation read the entire contents of `atomic.rs`, `boot_config.rs`, `command_policy.rs`, `feedback.rs`, and `reporters/json.rs`. These remain pending review in the inventory: their relevant production caller bodies have not yet been traced and assessed. The other 22 paths in the pending list above have not been read in this continuation. The available continuation context was insufficient to finish the remaining source and caller analysis; no additional paths are marked reviewed. The earlier findings and first check logs remain unchanged. No tests, lint, formatting, documentation build, packaging, or GPU scenarios were rerun.

Two compatibility decisions to retain during the remaining review: `atomic::stage` explicitly checks close errors and uses `fsync` rather than the different Apple `File::sync_all` behavior; `command_policy::forward` fetches live callback state in argument order and immediately returns callback failures. The comments in several files describe historical C++ adapters; those comments alone do not establish a current production call path. The reporter JSON scanner describes trusted ASCII fixtures and native-compatible byte handling; its callers and actual input assumptions still require review.

## Initial findings (historical; see current checkpoint)

### Must investigate: embedded NUL changes the path being verified

**Rule:** BeautifulRust best practices, correctness and untrusted-input validation; repository data identity contract. `crates/hp2-bootstrap-paths/src/bootstrap_paths.rs:447` permits NUL in an entry path. JSON `\\u0000` produces a NUL byte and the checksum parser accepts it; both indexes can therefore agree on `System/Default.ini\0suffix`. At line 665, `cbytes` truncates the name before filesystem lookup, so the verifier hashes `System/Default.ini` instead of the indexed name. The game calls this validation from `crates/hp2-game/src/boot_services.rs:339` and the launcher from `crates/hp2-game/src/launcher_ui.rs:1403`. This is a source-level finding, not a reproduced test. Add an adversarial manifest/checksum regression, then reject embedded NUL in `safe` before any filesystem lookup. Check native compatibility and intended rejection details before changing behavior.

### Should fix: strict clippy does not complete

**Rule:** `clippy::octal_escapes`; BeautifulRust best practices, lint checks and behavior-preserving refactoring. `crates/hp2-launcher/src/presentation_policy.rs:473` contains NUL-separated option bytes such as `b"50%\067%..."`. Rust interprets `\0` as NUL followed by digits, but clippy rejects the ambiguous spelling. Replace each intended separator with `\x00`, preserving every following digit and byte; do not use the suggested octal interpretation. Characterize the existing byte sequence before changing it. The first strict run also reported other launcher lints; see `clippy.log`. Clippy stopped at compilation failure, so this is not a complete lint inventory. The affected file remains pending full review.

### Should fix: rustdoc warnings obscure links

**Rule:** BeautifulRust best practices, documentation and `cargo doc`. For example, `crates/hp2-file-hash/src/lib.rs:15` links to private `HashingSink`; `crates/hp2-ini/src/lib.rs:10` links to private `lower`. Render them as code, or link public documentation instead. `cargo doc` also reports broken links and HTML-like text in launcher sources, including `import_session.rs:123` and `reporters/world_collision_gate.rs:15`. Quote source expressions as code and correct or remove broken links after reviewing their targets. The command exited successfully with warnings, not cleanly.

### Consider: add focused hash behavior tests

**Rule:** BeautifulRust best practices, tests of observable behavior. `crates/hp2-file-hash/src/lib.rs:62–87` provides file and in-memory SHA-256 used by `crates/hp2-import/src/collect.rs:100`; its own test binary ran 0 tests. Add known empty and non-empty byte vectors, a streamed multi-chunk file comparison, and missing-file error coverage. Preserve the current size, lowercase digest and error strings. Importer tests may cover some calls, but were not part of this selection.

## Reviewed without a code change

- Bootstrap deliberately accepts legacy roots without either index; indexed overlays follow manifest, checksum and file-hash checks (`crates/hp2-bootstrap-paths/src/bootstrap_paths.rs:694–721`). This differs from the blanket statement in `AGENTS.md` that runtime never validates the manifest. Do not assume either route qualifies retail data without checking the actual path and provenance.
- `hp2-ini` intentionally has two different codecs: importer duplicate keys are errors, while launcher reads the last match. The launcher preserves encoding and line endings; merging them for stylistic simplicity risks observable bytes and ordering. The documented non-ASCII lowercase/casefold difference remains an explicit limitation, not a reason to silently change behavior.
- `hp2-ffi-support/src/lib.rs:29–39` copies the sink out of its read lock before invoking it, avoiding that lock being held during a synchronous callback. Its panic guard is an FFI containment boundary, not a general recommendation to recover from panics; aborts and callback lifetime requirements still need separate analysis.
- `hp2-launcher/src/display_modes.rs:45–90` preserves first-seen mode order and propagates callback errors. The sampled game host owns the SDK interaction; a policy test alone cannot prove platform behavior.
- Preserve the repository's ABI/packing, UTF-32 boundaries, reentry, resource destruction order, numerical behavior and exact dependency pins over generic style recommendations. No new trait, builder or crate is proposed solely for a checklist.

## Checks and evidence

Host: macOS arm64, `rustc 1.99.1`, data-none, selected packages with default features; no SDK feature override. `HOME`, `TMPDIR`, XDG directories and `CARGO_TARGET_DIR` were isolated under the artifact directory. Existing `/Users/mike/.cargo` and `/Users/mike/.rustup` were used. Commands and environment are retained in `commands.txt` and `provenance.txt` in that directory.

Let `P` be `-p hp2-bootstrap-paths -p hp2-file-hash -p hp2-ffi-support -p hp2-ini -p hp2-launcher`:

- `cargo test --locked --offline $P --all-targets -- --test-threads=1`: exit 0; 229 tests passed, 0 failed, 0 ignored across 5 binaries. The hash binary had 0 tests. See `test.log`.
- `cargo clippy --locked --offline $P --all-targets -- -D warnings`: exit 101. The first failure is retained in `clippy.log`; no rerun.
- `cargo fmt --check $P`: exit 1, formatting differences in `fmt.log`. No formatting was applied. Some files already contained unrelated changes.
- `cargo doc --locked --offline $P --no-deps`: exit 0 with warnings in `doc.log`.

These results cover the selected Cargo targets only. They do not establish SDK-backed behavior, GPU rendering, CTest, prototype or retail behavior, production parity, Miri safety, all feature combinations or other platforms. No CMake, packaging, golden updates or commits were performed. Further findings, including safety, drop-order and caller analysis, remain pending until the unread files are reviewed.
