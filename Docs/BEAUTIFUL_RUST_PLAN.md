# BeautifulRust adoption and LLM-maintainability plan

**Status: approved; implementation and qualification are in progress.**

The user approved the full project-wide program, not just the pilot. The
[execution ledger](BEAUTIFUL_RUST_STATUS.md) records the complete scope, observed
results, remaining tasks, and blockers. The inspection counts below are historical
planning observations; they are not current coverage or completion claims.

## Goal and invariant

Make the project easy for an LLM worker to navigate, change, and verify without
loading its history. Improve runtime and build performance where measurements
justify changes. Preserve supported gameplay, error behavior, ABI layouts,
package/save/network formats, numerical behavior, and intentional compatibility.

Treat these as four separate outcomes: correctness, agent maintainability,
runtime performance, and development-loop performance. A smaller file or a clean
Clippy run is not proof of a faster or correct game.

## Reference and inspection scope

Reviewed BeautifulRust at commit
[`b9718436d77ee7bcf17cdcb6aa27628feb95d97a`](https://github.com/morganlinton/BeautifulRust/tree/b9718436d77ee7bcf17cdcb6aa27628feb95d97a):

- `rust-best-practices`: writing/refactoring rules, ownership, safety, tests, and measurement.
- `rust-review`: prioritized findings, locations, named rules, and concrete fixes.
- `rust-port`: characterize behavior first and preserve intentional quirks.
- `lints/lints.toml` and `lints/clippy.toml`: machine-enforceable guidance.

Use the skills as reviewed guidance, not as authority to change engine contracts.
This plan does not require BeautifulRust's hosted MCP service or a global plugin.

Repository inspection used HEAD `f5d1befcb8bdd8339eaafaf11acf059c0413bb28`
plus the live working tree. There were 55 status entries before this plan was
added. Existing changes include core, input, menus, platform, rendering, CMake,
fixtures, and third-party metadata. Preserve that work; do not reset, stash,
commit, or overwrite it as part of adopting this plan.

### Observed facts, not performance findings

| Observation | Consequence |
| --- | --- |
| Cargo metadata reports 17 workspace packages; none is `hp2-ffi`. | Repair the navigation and CI package references before dispatching cleanup workers. |
| The live tree contains 836 Rust files and 494,343 text lines. This includes 122,329 integration-test lines and 11,610 lines in `hp2-abi/src/engine/generated.rs`; other files also contain inline tests. | Exclude generated code and separate tests from implementation when judging size. |
| `hp2-wgpu/src/lib.rs` has 8,586 lines, `hp2-render/src/scene_pipeline.rs` has 4,399, and `hp2-engine/src/net_chan.rs` has 4,080, including inline tests. | Review responsibilities and test placement; do not split solely to meet a line limit. |
| `AGENTS.md:9`, `hp2-core/src/lib.rs:4`, and CI still describe the removed `hp2-ffi` bridge. CI invokes that missing package in three jobs. | These are concrete stale references. The CI command was not executed during this audit. |
| `Docs/OPERATIONS.md`, `Docs/RUST_COMPLETION_PLAN.md`, and `Docs/BEHAVIOR_MATRIX.md` have about 10,300 lines combined, with historical checkpoints mixed into current guidance. | Separate current instructions from preserved historical evidence. |
| No workspace lint table, repository BeautifulRust skills, or root Clippy configuration was found. | Adopt a pinned policy deliberately; do not enable every upstream lint as an immediate global error. |
| The existing macOS preset selection lists 219 tests. This is the existing configured tree, not a fresh configure or test run. | Use it as discovery evidence only. Reconfigure and validate expected registrations before qualification. |
| Platform guidance and native-text gate descriptions disagree with current presets and `Build/feature-gates.json`. | Reconcile documentation against measured configuration and policy. Do not change defaults or promote gates to resolve prose conflicts. |

## How to apply BeautifulRust here

### Adopt

- Use ownership, `Result`, descriptive internal types, narrow visibility, and
  simple concrete APIs where the existing contract permits them.
- Require meaningful `SAFETY` explanations and caller obligations at unsafe
  boundaries. Pin reentry, lifetime, alignment, initialization, and teardown rules.
- Keep policy separate from platform/ABI adaptation where that separation is real.
- Add characterization tests before moving poorly covered behavior.
- Review correctness and soundness before idiom, naming, or formatting.
- Measure before optimizing. Keep dependency and abstraction costs visible.

### Adapt rather than copy blindly

- Preserve measured `repr(C)`/packed layouts, discriminants, plain-data public
  fields, serialized sentinels, legacy flags, and required function signatures.
  Typed wrappers belong inside a boundary unless a separate ABI change is approved.
- Do not replace engine TCHAR/FString storage with `CString` or UTF-8 merely
  because general FFI guidance uses those types. Preserve the actual encoding.
- Do not replace callbacks, raw pointers, or owner/service tables without proving
  their role. A safe-looking reference can be invalid across synchronous reentry.
- Preserve callback order, partial-error side effects, allocation ownership,
  RNG draws, floating-point evaluation order, and drop order.
- Keep deliberate exact dependency pins and the pinned toolchain until a separate
  dependency/toolchain change has evidence. Do not unpin to satisfy style advice.
- This project implements a VM and legacy data semantics. BeautifulRust's advice
  against reproducing an incidental source-language runtime does not authorize
  simplifying the game's runtime or parser semantics.
- Do not add builders, traits, new crates, `non_exhaustive`, or wrapper layers only
  to satisfy a generic checklist. Internal application code must stay direct.
- Do not run `--all-features` indiscriminately. Native SDK features and text
  providers need explicit host-specific build/link combinations.

## Rollout and decision gates

### 0. Establish a trustworthy starting point

**Deliverables**

1. Agree on a checkpoint that includes the intended in-progress work. A worktree
   made from HEAD alone does not include these uncommitted changes.
2. Record toolchain, Cargo graph, shipping features, platform, data roots,
   executable identities, expected CTest names/labels, and existing failures.
3. Reconfigure the intended preset. Build verification binaries and run the
   applicable baseline contracts and gameplay scenarios. Keep failures visible.
4. Establish runtime and development-loop measurements before changing code.

**Gate:** no subsystem cleanup starts without a baseline, an owned file list,
and an explicit behavior/test contract. Missing data or missing tests are
blockers for the affected claim, not passes.

### 1. Fix the map that workers read

**Proposed layout; new paths below do not exist yet:**

```text
AGENTS.md                         Short routing, rules, and verification contract
Docs/ARCHITECTURE.md               Current products, crate graph, domain ownership
Docs/OPERATIONS.md                 Current configure/build/test commands only
Docs/BEHAVIOR_MATRIX.md            Current behavior -> tests -> data profiles
Docs/RUST_STYLE.md                 BeautifulRust adoption and engine exceptions
Docs/history/                     Historical checkpoints and evidence narratives
.agents/skills/rust-*/             Pinned upstream skills and relevant references
```

- Retain `CLAUDE.md` as a pointer, not a competing handbook. Configure Claude's
  skill discovery separately where needed; do not assume it reads `.agents/skills`.
- Record BeautifulRust's URL, commit, and MIT attribution beside the vendored
  skills. Keep project exceptions separate from upstream text. Do not vendor
  its benchmarks, MCP service, or evaluation corpus into the normal worker context.
- Give large/high-risk domains short local instructions or crate READMEs. Each
  names entry points, ownership, invariants, applicable tests, and forbidden edits.
- Explain the current relationship between policy modules and `*_owner` adapters
  before renaming either. Group modules by use case, not generic `helpers` buckets.
- Keep current-state rustdoc short. Preserve important reasoning and evidence in
  linked documents; remove stale progress journals from the default read path.
- Preserve historical records, hashes, failures, and acceptance conditions when
  moving them. Add clear historical labels and repair links.
- Document the actual Linux arm64/x64 presets. Keep the frozen C++ browser job
  separate from claims about the current Rust product.

**Gate:** a fresh worker can identify the implementation owner, required test
commands, and compatibility constraints for a sample task without reading history.

### 2. Make the policy executable

**Deliverables**

- Repair CI references to actual packages and shipping feature combinations.
  Derive the matrix from manifests and CMake, not a replacement guess for `hp2-ffi`.
- Add read-only formatting checks and relevant rustdoc checks to CI. Keep
  mechanical formatting repairs separate from behavior changes.
- Add shared workspace lint configuration and member inheritance. Start with
  high-signal safety, debugging-leftover, and idiom checks; stage documentation
  and broad pedantic checks after measuring their signal.
- Establish existing warning debt. Reject new warnings; require `-D warnings`
  for clean packages as they graduate. No crate-wide suppression to manufacture green.
- Require local exceptions to name the invariant and reason. Treat generated ABI
  output separately; fix its producer when appropriate, not thousands of generated lines.
- Add a small proposed `Build/check_agent_contracts.py` plus tests. It must check
  live package/preset references and required CTest registrations for the selected
  platform/profile. Reuse Cargo metadata and CMake/CTest as authorities.
- Keep existing build/test commands canonical. A convenience check command can
  orchestrate them, but must not become a second build system or acceptance ledger.
- Measure targeted-test build time and disk use. Review per-test Cargo target
  directories only after proving that shared caching preserves isolation and
  distinct SDK, feature, toolchain, and sanitizer configurations.

**Gate:** known failures remain visible; required checks cannot pass with zero
matching tests. Worker instructions and CI refer to the same real products.

### 3. Prove the approach with one bounded pilot

**Pilot: extract pure texture conversion from GPU cache/upload code.**

Current candidate: `crates/hp2-wgpu/src/texture.rs:177-310`. It has tested palette,
RGB8, and RGB16 conversion. The file was clean at inspection; `lib.rs` and
`draw.rs` were not. Recheck ownership/status before starting.

**Allowed implementation files:** `texture.rs` and a private child such as
`texture/conversion.rs`. Keep existing public paths through explicit re-exports.
Do not edit the renderer root, draw batching, shaders, ABI types, or upload policy.

1. Preserve existing tests and add missing empty-input/malformed-length cases
   before moving implementation. Preserve current error/panic behavior.
2. Move only conversion code and directly related types. Do not change algorithms,
   buffer allocation strategy, or dependencies in this extraction.
3. Replace historical commentary only with accurate, short contract documentation.
   Preserve RGB565 channel order, palette masking, alpha, and byte-order rationale.
4. Obtain an independent review and run the named tests and real renderer smoke.

**Existing commands to use after the baseline is established:**

```sh
cargo fmt -p hp2-wgpu -- --check
cargo test --locked -p hp2-wgpu --lib texture::
cargo clippy --locked -p hp2-wgpu --all-targets -- -D warnings
cargo doc --locked -p hp2-wgpu --no-deps
cmake --build --preset macos-arm64 --target hp2_verification_binaries
ctest --preset macos-arm64 --no-tests=error --output-on-failure \
  -R '^wgpu_surface_round_trip$'
ctest --preset macos-arm64 --no-tests=error --output-on-failure \
  -R '^texture_owner$'
ctest --preset macos-arm64 --no-tests=error --output-on-failure \
  -R '^canonical_renderer_smoke_maps$'
```

Run the CTest selections separately, as shown. A combined selection can pass
when a required conditional test is absent but another test matches. Verify
all three names in the fresh listing; a missing registration is blocked.
Cargo commands also need the agreed isolated artifact environment. Record any
baseline formatting/lint failure rather than claiming the pilot introduced or
fixed it. Do not reformat unrelated in-progress files to satisfy this check.
`wgpu_surface_round_trip` currently runs the non-ignored wgpu library tests;
its name is not proof of real surface presentation. Ignored GPU upload tests
must be scheduled explicitly with GPU resource serialization if used as evidence.
`texture_owner` and the map smoke are prototype evidence, not retail evidence.

**Gate:** unchanged observable results, no new warnings, a smaller relevant read
set, stable public paths, and an independently understandable module boundary.
This pilot makes no runtime speedup claim.

### 4. Roll out by domain, not by file-size quota

After the pilot, create small worker cards in this order, adjusted for active work:

| Domain | Work boundary and main risk |
| --- | --- |
| Leaf crates: INI, paths, hashes, codecs | Simplify APIs/errors and pin edge cases. Preserve encodings and byte output. |
| Importer, launcher, audio, platform | Separate pure policy from filesystem/UI/SDK adaptation. Preserve cancellation, persistence, resource and thread lifetimes. |
| Core package/configuration | Review `package_flow.rs` and `config_ini.rs` responsibilities. Pin case folding, partial errors, save bytes, allocation, and reentry. |
| Engine networking/world | Review `net_chan.rs` lifecycle/send/receive boundaries. Preserve wire bytes, sequence handling, callback order, and teardown. |
| Renderer and wgpu | Break orchestration into real device/frame/submission/capture responsibilities. Audit `scene_pipeline.rs` unsafe access. Preserve numerical and presentation semantics. |
| Build/test support | Decompose large registration/scripts by domain only after test inventory checks can prove no coverage disappeared. |

Existing examples include `package_flow_contracts` and the six `config_ini_*`
contracts. Each other card must select real tests from a fresh registration
listing; the table above is not permission to invent test names.

Keep the present workspace shape initially. Add a crate only for an independent
responsibility with a demonstrated dependency, testing, or compilation benefit.
Use private modules before introducing another package. Treat large files as
review signals, not automatic violations. Never split generated ABI declarations
by hand. Do not fold active menu/input work into unrelated cleanup.

**Gate per card:** behavior tests, boundary review, explicit before/after API
mapping where signatures change, and no unrelated changes. Fix a newly discovered
product defect in a separate, characterized change rather than hiding it in a move.

### 5. Optimize measured bottlenecks

This measurement track starts in phase 0; it need not wait for every style cleanup.

- Use fixed, recorded scenarios: Entry startup, the existing representative map
  smoke, an exercised gameplay/input route, package load, and save/reload.
- Record executable/data hashes, release settings, resolution, renderer, machine,
  feature set, instrumentation, and warm/cold-cache state.
- Measure frame-time distribution, CPU/GPU cost, allocations, peak memory,
  startup/map/save latency, targeted test time, and build/check time separately.
- Use macOS Instruments/Metal tools or Linux profiling where available. Add
  focused microbenchmarks only for a demonstrated hotspot.
- Test real production performance separately from deterministic verification,
  loopback audio, screenshots, and readback. Verification instrumentation can
  dominate timing; never present its cost as normal gameplay performance.
- Potential investigations include buffer reuse, redundant copies, texture upload
  batching, and repeated reflected-property lookup. These are hypotheses, not
  established bottlenecks or approved implementation changes.
- Use a predefined repeated sample set and retain every result. Repeated benchmark
  samples are not permission to rerun failing behavior checks until green.

**Gate per optimization:** profile evidence explains the bottleneck, the change
improves its declared metric beyond measurement noise, and behavior remains
verified. Record trade-offs. Do not promise an arbitrary percentage improvement.

### 6. Qualify the integrated result

- Run the full applicable named contract/component suite and real-process boot,
  rendering, input, and save/reload scenarios on the integrated candidate.
- Exercise macOS arm64 and Linux arm64/x64 where supported resources exist. An
  unrun platform remains unverified, even when its configuration compiles.
- State `data-none`, `data-prototype`, or `data-retail` for each result. For retail,
  independently verify imported-data provenance; a manifest's presence is insufficient.
- Run applicable native-free and bundle audits. Preserve the approved functional
  acceptance policy; neither invent a new exact-frame parity gate nor relax
  existing diagnostic comparisons or accepted contracts.
- Run applicable sanitizer configurations, but inspect actual Rust and native
  compiler instrumentation. A CMake sanitizer preset does not itself prove
  Rust code was instrumented. Use Miri only on supported isolated code, not real GPU/SDK calls.
- Retain existing feature-gate and human acceptance requirements. An automated
  smoke is not audible-audio, physical-input, visual, or installed-user approval.

**Gate:** evidence supports the declared scope. Unavailable retail data, hardware,
platforms, or human checks remain explicit unresolved conditions.

## Worker execution model

Use one integrator, up to three non-overlapping implementation workers, and a
read-only reviewer. These are roles, not a requirement to run all workers at once.

- The integrator owns shared Cargo, CMake, CI, skill/configuration, and public
  boundary changes. Workers request those edits instead of making conflicting copies.
- Dispatch from the agreed checkpoint, with separate workspaces when appropriate.
  Each card owns a small file set and one behavior invariant, not an entire crate.
- Serialize GPU qualification across workers. CTest resource locks alone do not
  coordinate independent CTest processes in different workspaces.
- Review before integration; rerun affected checks on the combined candidate.
  Independent branch passes do not prove that their combination works.
- Stop a card when it needs a public-layout change, unknown compatibility rule,
  golden update, third-party change, gate/default change, or another worker's files.

### Required worker card

```text
ID / dependencies / exact base checkpoint
Goal and one-sentence invariant
Allowed files; forbidden files and behavior changes
Entry points and required skill/reference files
Current behavior and compatibility constraints
Selected platform, features, data profile, and prerequisites
Exact baseline and post-change commands; expected test registrations
HP2_ARTIFACT_DIR and required reports
Acceptance conditions and escalation conditions
```

### Required worker handback

```text
Changed paths and short reasons
Behavior/API changes, or an explicit statement that there are none
Exact commands and observed pass/fail/blocked results
Data profile, artifact paths, and executable identity for runtime evidence
Known pre-existing failures and unresolved conditions
Independent review findings and their disposition
```

Use BeautifulRust's must-fix / should-fix / consider structure for code reviews.
Each finding needs a location, a named rule, the concrete consequence, and a
minimal fix. Respect deliberate engine exceptions. Include what should be kept.

## Recommended first implementation batch

1. **Truth and baseline:** reconcile the current build map, preserve historical
   evidence, and establish actual test/profile and performance baselines.
2. **Skills and guardrails:** pin the three skills, document engine exceptions,
   repair CI package references, and introduce staged lint/registration checks.
3. **Pilot characterization:** pin texture-conversion edge cases without moving code.
4. **Pilot extraction:** make the bounded module move, review it, and qualify it.
5. **Evaluate before scaling:** give fresh workers representative navigation/change
   tasks. Compare files/context read, correct test selection, first-pass test
   success, and reviewer corrections against the baseline.

Prefer fewer, clearer concepts and predictable change boundaries over clever Rust.
Success means a fresh worker can make a small correct change with a small relevant
context, and measurements support any performance claim. It does not mean every
file is short or every general-purpose lint is enabled.

## Checks actually performed for this planning document

- `cargo metadata --no-deps --locked --offline --format-version 1`: exit 0;
  17 members; `hp2-ffi` absent.
- `ctest --preset macos-arm64 --show-only=json-v1`: exit 0; 219 tests in the
  existing preset selection. No test was executed by this command.
- Inspected current manifests, presets, CI, registration code, source samples,
  navigation documents, git status, and all three upstream skill files.
- Checked this document's whitespace, code fences, workspace facts, and named
  CTest references. An independent read-only review identified a partial-selection
  risk; the pilot now requires separate, fail-closed test invocations.

Discovery JSON is under `/tmp/hp2-beautifulrust.4CEXPB/Artifacts/`.
No behavioral tests, build, formatting check, Clippy run, benchmark, Linux run,
retail run, or human acceptance check was performed for this planning task.
No runtime, build configuration, gate, fixture, or existing instruction file was
changed. The only repository addition from this task is this proposal.
