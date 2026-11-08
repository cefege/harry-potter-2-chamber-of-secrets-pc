# Full Rust completion plan

Status: execution started. Shipping-source membership and a deduplicated
ownership backlog are recorded; acceptance-baseline repair and the Rust
object/reflection/VM foundation remain open. No policy-only cutover counts
as native-free completion.

## Active execution goal: finish the macOS Rust port

The active deliverable is a working installed macOS arm64 Harry Potter 2 app
with first-party engine/runtime ownership migrated to Rust, not another
policy-only checkpoint. Linux remains in the overall finish line below;
macOS completion is the current execution priority.

Execution continues across subsystem checkpoints. A passing build, one moved
function, or an appended checkpoint is not a stopping condition or completion
claim. Resume the earliest unfinished ownership family using its recorded
dependencies; reuse existing Rust archive/object bridges rather than creating
parallel abstractions. Delete replaced native implementation in the same
cutover. Narrow native ABI adapters must remain explicitly inventoried until
the final native-free shipping criterion below is met.

MacOS acceptance requires all of:

- Core object allocation/lifetime, reflection, VM and GC ownership in Rust.
- Engine/world/network/resource ownership in Rust.
- Renderer/platform/audio/launcher/tool ownership in Rust.
- A rebuilt and installed app, passing applicable CTests, and a passing bundle
  integrity check.
- Actual-process evidence for gameplay, rendering, keyboard/mouse input,
  audible audio, save creation and reload. Record the exact data profile;
  prototype or silent smoke is not retail or audible-audio evidence.
- A fresh shipping dependency/ownership inventory proving the native-free
  criterion, not merely the absence of policy branches in C++ shells.
- Recorded commands, observed results and artifact paths. Any genuinely
  unavailable external prerequisite is named explicitly, with reachable work
  completed; it must not be reported as a pass.

Current entry point: finish the production Engine object-graph cutover from the
canonical `EngineOwner` checkpoints below. Canonical GameEngine/base-engine
initialization, actor spawning, movement, destruction, primitive collision and
moving-brush ownership have component evidence; the installed game has not been
switched to that graph. Canonical network adapters now build, but full demo
receive/playback and production audio/observer publication remain open. The
shipping native graph still retains its class/object shells, registration,
superclass dispatch, allocator resources and GUndo. Component evidence is not
credited as production native-owner removal.
The earlier locked-desktop presentation failure is retained as evidence. A later
installed `Adv1Willow` run presented 60/60 frames, and `renderer_smoke_wgpu`
passed in 27.66 seconds; presentation is no longer treated as currently blocked.

### Current resume: canonical graph integration

The first fresh `cargo check --locked -p hp2-engine` failed with 715 diagnostics
in the unfinished canonical graph. That failure is retained in
`out/macos-arm64/Testing/HP2/Manual/Artifacts/port-resume/integrated-canonical-engine-check.log`.
That initial failure is now repaired for the canonical Engine shipping-feature
check. Production object-graph cutover and native-owner removal remain unearned.

Verified during this resume, under **data-none**:

- `cargo test --locked -p hp2-core --test network_archive_contracts --test property_value_contracts --test property_text_contracts -p hp2-render --test particle_tube_reference`:
  37 tests passed, including the shared tube kernels through the same
  `RenderMath` seam used by canonical and native hosts.
- The extended `network_archive_contracts` test passed after adding the saving
  path: canonical Int/Bool/Float properties plus an unaligned prefix, raw bytes
  and bounded integer reproduced the independently packed 95-bit wire exactly.
  A finite-capacity writer published its overflow without advancing its cursor.
- A temporary standalone Cargo consumer called the real canonical Core
  reader/writer exercise outside the test runner and exited 0. Its source was
  removed after verification. Evidence:
  `port-resume/canonical-network-reader-writer-runtime-smoke.log` and
  `port-resume/network-archive-54908/report.txt`, relative to the artifact parent
  above.
- `ctest --preset macos-arm64 -R '^(network_archive_contracts|property_value_contracts|property_text_contracts|bit_archive_contracts|scene_lighting_contracts)$'`:
  **5/5 passed**, 101.47 seconds. Log:
  `port-resume/ctest-shared-core-contracts.log`, relative to the same artifact
  parent.
- `cargo check --locked -p hp2-engine --features sdl,openal,renderer-wgpu,text-coretext`:
  passed. This is the Rust Engine crate, not a linked shipping game.
- `cargo test --locked -p hp2-engine --test font_owner_contracts -p hp2-core --test cpp_property_layout_contracts`:
  both passed. Font coverage checks independent version-68/69 native-field
  wire bytes, TCHAR32 map collision rehash, bitmap glyphs and texture references,
  native-font fallback exclusion, and partial-page shutdown after checked errors.
  Dynamic native-property linking failed before the fix, then passed constructor-
  offset restoration and real property-text writes without overwriting neighbors.
- Real standalone Cargo consumers of the canonical Engine graph exited 0:
  `canonical-particle-runtime-smoke-native-cursor-and-bounds.log`,
  `canonical-file-cache-runtime-smoke-after-consumer-api-fix.log`, and
  `canonical-font-archive-standalone-runtime-smoke.log`, all under `port-resume`.
  Reports: `particle-runtime-54289/report.txt`, `cache-runtime-64549/report.txt`,
  `font-owner-66553/report.txt`, under the same directory. Particle expectations
  preserve the original cursor's repeated first survivor and FBox minimum-corner/
  full-edge basis; the initial smoke's mathematical-center assumptions were
  wrong, not a reason to change native behavior. No captured golden was changed.
- `cargo test --locked -p hp2-engine --test particle_list --test particle_update --test particle_fx_replay`:
  four tests passed against the existing original-C++ captures and checked-error
  regression. The original cursor source was also retained in
  `port-resume/original-native-particle-list-source.log`.

Additional integrated evidence from this resume (artifact names below are relative
to `out/macos-arm64/Testing/HP2/Manual/Artifacts/port-resume/`):

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries`:
  passed, including `hp2_game`, `hp2_ucc`, the Rust staticlib and generated public
  headers. Latest log:
  `build-integrated-verification-binaries-after-eight-iterator-and-Canvas-error-sequencing.log`.
  This remains a native-graph executable, not the Rust-owned shipping cutover.
- `ctest --preset macos-arm64 -R '^(font_owner_contracts|startup_defaults_contracts|cpp_property_layout_contracts)$'`:
  **3/3 passed**, data-none; `ctest-canonical-font-startup-and-native-layout.log`.
- `ctest --preset macos-arm64 -R '^(canvas_compatibility_contracts|audio_lifecycle|abi_widths|rust_core_runtime|spawn_owner|model_owner)$'`:
  **6/6 passed**, two data-none and four data-prototype tests;
  `ctest-native-boundary-font-audio-and-core-ABI.log`. The audio oracle checks PCM
  behavior, not audible hardware output.
- A debugger identified the Actor script-end failure as the source serializer's
  speculative DebugInfo lookahead crossing logical Script.Num. Only that
  lookahead now uses initialized native slack; ordinary operand bounds remain
  unchanged. `canonical-actor-script-debugger.txt` retains the stack and offsets.
  The actual-Core regression failed before the fix and passed afterward.
  `ctest --preset macos-arm64 -R '^(metadata_script_tail_contracts|expression_archive_contracts|class_metadata_contracts)$'`:
  **3/3 passed**, data-none; `ctest-expression-and-metadata-script-tail.log`.
- The canonical foreach implementation read its end word before the iterator
  expression and reevaluated arguments. An actual-Core consumer hung before the
  fix; it was terminated after the 300-second deadline, not retried unchanged.
  Canonical and SDK hosts now share the source native loop policy. The regression
  passes once-only argument effects, empty/exhausted iteration, break, nested
  iteration, nested Return with the caller's result, and Stop.
  `canonical-foreach-control-regression-before-marker-fix.log` and
  `canonical-foreach-control-regression-after-shared-native-loop-fix.log`.
- `ctest --preset macos-arm64 -R '^(rust_core_runtime|actor_iterator_contracts|vm_frame_contracts|vm_flow_contracts|vm_call_contracts)$'`:
  **5/5 passed**, four data-none and one data-prototype;
  `ctest-canonical-foreach-and-actor-selection-contracts.log`.
- A standalone canonical Engine consumer, linked with the real SDL2, OpenAL Soft
  and Apple SDK dependencies and all shipping Engine features, exited 0 under
  data-none. It exercised eight actor iterator natives over actual owned levels,
  actors, reflected out arguments and bytecode assignment bodies: default-zone
  membership, class filtering, transitive children, immediate bases, touching
  slots, expanded radius, hidden/visible BSP checks, collision-hash radius chains
  and real cylinder ray hits. Successful trace marks passed the scratch Tick
  invariant. Canvas VM checks exercised 5,000-character arguments, null-font
  TextSize output preservation, null actor handling, and omitted/explicit CR
  cursor behavior with no-draw style.
  `canonical-actor-and-canvas-VM-real-runtime-smoke.log`; actor report:
  `actor-foreach-11459/report.txt`. This is not graphical DrawActor/DrawPortal
  presentation or hardware input/audio evidence.
  Exercised consumer sources and exact SDK link commands are retained as
  `port_resume_actor_foreach_smoke.rs`, `port_resume_engine_package_smoke.rs` and
  `standalone-consumer-commands.json`; temporary Cargo examples were removed.
- `ctest --preset macos-arm64 -R '^(actor_iterator_contracts|canvas_compatibility_contracts)$'`:
  **2/2 passed** after final host integration;
  `ctest-actor-iterator-and-bitmap-canvas-after-final-host-integration.log`.

The ninth iterator now has real producers too: `AllActors` uses an Engine-owned
optional observer and a direct Core patrol recorder over the same process trace
state/writer as the SDK path. Its actual-world consumer verifies live observer
replacement, nested scans, slot versus yielded-order identity, break without a
terminal event, checked errors stopping before body/terminal calls, null-class
resolution and tag filtering. Actual patrol JSON is parsed independently with
Python; reflected property export is not replaced by a default or copied field.
The shipping patrol trace's already-offset Data/Delta is retained even though
full `ExportText` applies Offset again. A mistaken expected Tag value exposed this
quirk; the incidental byte expectation was removed, not re-pinned. The full
canonical export operation independently verifies default suppression versus
Data/Delta alias-forced export on actual owned string properties.

Additional real canonical Pawn natives are now implemented: `PickTarget` (531),
`PickAnyTarget` (534), `AddPawn` (529), `RemovePawn` (530), `WaitForLanding` (527),
its poll (528), named `StopWaiting`, and `FindStairRotation` (524). Their algorithms
are shared with the native host, not copied. Real-world consumers exercise BSP
sight, live health/team filters, strict aim precedence and unchanged out values
when no target improves; duplicate/head/interior list transitions and retained
removed NextPawn; actual execution frames, Sleep-only cancellation, zero/negative
landing timer boundaries and grounded polls with no delta read; and actual
visibility traces for source stair pitch normalization/easing. Missing LongFall
metadata remains a checked error after the timer write, not a successful no-op.

`PickWallAdjust` (526), `LineOfSightTo` (514) and `CanSee` (533) now have canonical
producers too. Wall adjustment uses the real canonical cylinder extent, movement
and visibility traces. CanSee retains the source's non-pawn bLOSFlag alternation;
it is not replaced with an always-visible result in an empty world.

`pointReachable` (521) now composes all ten reach-service leaves over canonical
movement, BSP regions/traces, real class identities and the existing physics
water-line policy. Actor ticking also uses this real low-level actor reachability
instead of its previous missing-producer error. `EAdjustJump` (523) shares the
existing callback-free source policy with the SDK host, with its obsolete
published-table dependency removed from every caller. The source jump guard,
field-read order, compute-before-P_FINISH behavior and unchanged Pawn.Velocity
are preserved.

The actual-world reach consumer verifies flying movement simulation, pose and
velocity restoration, the non-editor 800-unit planar limit, null actor handling,
jump speed caps, gravity handling and vertical-velocity preservation. It owns a
valid BSP partition and explicitly pairs moving-brush initialize/destroy before
Core shutdown. An earlier empty-model fixture hung in PointRegion after the
tracker expanded Nodes.Num to 512 and left root child links at zero; the live
debugger stack and values are in `canonical-reach-BSP-hang-debugger.txt`.
Leaving the manually initialized tracker to reverse-order Core shutdown also
failed with `Invalid moving-brush model removal range`; this consumer now obeys
the tracker API's model-outlives-tracker lifetime contract. Automatic world/model
teardown ordering is not established by this explicit-lifetime component run.

- `ctest --preset macos-arm64 -R '^(actor_iterator_owner_contracts|pawn_native_ai_owner_contracts|pawn_reach_owner_contracts|canonical_property_export_contracts|rust_core_runtime)$'`:
  **5/5 passed** in 52.82 seconds: four data-none contracts and the data-prototype
  Core runtime contract. Log:
  `ctest-canonical-iterator-AI-reach-export-and-Core-runtime.log`.
- The real-feature standalone reach/jump consumer exited **0**:
  `canonical-reach-and-jump-real-features-runtime.log`;
  `pawn-reach-owner-16383/report.txt`. The real-feature AI consumer including
  wall/sight paths also exited **0**:
  `canonical-pawn-AI-real-features-runtime-smoke-with-wall-and-sight-paths.log`;
  `pawn-ai-owner-76074/report.txt`, `pawn-landing-owner-76074/report.txt`.
  These remain data-none component runs, not a map boot or hardware evidence.
- The unchanged native reach capture passed after the table-independent jump
  extraction: `source-pawn-reach-capture-after-table-independent-jump-policy.log`.
  No golden was updated. Exact commands and archived temporary entry points are
  in `standalone-consumer-commands.json`; permanent consumer contracts are under
  `crates/hp2-engine/tests/`.


- `ctest --preset macos-arm64 -R '^(actor_iterator_owner_contracts|pawn_native_ai_owner_contracts|canonical_property_export_contracts)$'`:
  **3/3 passed**, data-none, after latent/stair integration;
  `ctest-AllActors-pawn-AI-and-full-export-after-latent-and-stair-integration.log`.
- Real-feature standalone consumers exited 0:
  `canonical-AllActors-real-features-standalone-runtime-smoke.log` and
  `canonical-pawn-AI-real-features-runtime-smoke-with-stair-traces.log`.
  Reports include `all-actors-owner-35590/report.txt`,
  `pawn-ai-owner-62090/report.txt` and `pawn-landing-owner-62090/report.txt`.
  These are data-none component runs, not a playable map or audible output.
- `cargo test --locked -p hp2-core --test ansi_trace_unansi` and
  `cargo test --locked -p hp2-engine --test pawn_native_ai_fixtures` passed after
  shared-host extraction. Existing native captures were not changed. Logs:
  `shared-patrol-recorder-native-oracle-after-direct-Core-host-extraction.log`,
  `native-AI-original-capture-replay-after-stair-query-host-extraction.log`.
- `cmake --build --preset macos-arm64 --target hp2_verification_binaries` passed
  after canonical reach/jump integration, including the actual native-linked game
  and tools: `build-integrated-verification-after-canonical-reach-and-jump.log`.

The strict, unchanged prototype `Engine.u` probe now passes earlier missing
bindings 308, 534, 529, 527, 524, 526, 521 and 523, but still fails on native
**522** (`Pawn.ClearPaths`). Evidence:
`canonical-full-prototype-after-real-point-and-jump.log`.
No deferred binding, metadata-only load flag or native callback was substituted.
The input-enum and post-load GameEngine checks were not reached. Complete
package loading and a Rust-owned map boot remain unearned; no native-owner
retirement is credited.


Compiler-probed ABI generation also passed after adding the particle texture
array/light color, Actor travel/LOD fields, LevelInfo title and viewport capture/
resource fields. This proves layout generation, not runtime ownership.
The shipping game remains native-graph based; no production-removal gate is
credited. The expected local retail overlay manifest is absent. No retail,
Linux, audible-audio, input, save/reload or new installed-game claim follows
from these component checks and explicitly profiled boundary tests.

## Finish line

The supported macOS arm64 and Linux arm64 game and launcher run with all
first-party engine/runtime code implemented in Rust. No first-party C, C++, or
Objective-C runtime translation unit, native UObject implementation, or C++
service shim is required to build or run either shipping product.

First-party runtime tools are included: the importer CLI and UCC commandlets
must also stop depending on native implementations. Native test/differential
oracles may remain, but cannot be dependencies of the Rust products or tools.

External dependencies and operating-system frameworks may still contain native
code and be called through Rust FFI. This does not require rewriting SDL,
OpenAL, FreeType, system frameworks, or pinned codec/compression dependencies.
Python build/import tools, CMake orchestration, and shipped game scripts/assets
are not C++ engine code and need not become Rust. Do not change package79, save,
network, or gameplay semantics to make the migration easier.

Dormant code is not automatically finished or authorized for deletion. Inventory
it explicitly: migrate supported paths; obtain an explicit scope decision before
retiring unrelated unsupported features. Do not silently drop editor/diagnostic
behavior to improve the completion figures.

## Evidence at this checkpoint

| Native-file category at the accepted checkpoint | Files | Meaning |
| --- | ---: | --- |
| Contains residual native policy | 97 | 96 mixed files and one native crash reporter |
| Native ABI / typed leaves only | 229 | Native storage, construction, dispatch, declarations, or OS leaves still require removal |
| Dormant / declarative only | 47 | Requires explicit final disposition, not automatic migration credit |
| Total first-party native runtime/tool files | 373 | 165 unique compiled source files and 208 included headers |

The configured macOS shipping graph was measured with **164 successful fresh
compiler dependency scans**, including the Objective-C++ launcher source.
The old 323-file aggregate omitted 45 files at the reconciled 368-file
baseline; its residual-policy arithmetic also double-counted `UnDownload.cpp`
and treated already-leaf-only `AStatLog.cpp` and `UnCamMgr.cpp` as new
transitions. Original audit rows and unique accepted deltas yield
93 policy / 182 leaf / 47 dormant; 46 newly covered files add two policy
headers and 44 ABI/leaf files. The first execution batch then added one
typed expression-archive header and moved the speech source from mixed
to leaf-only. **No native translation unit was removed by that batch.**

The shipping game/launcher subset remains **369 files: 164 unique compiled
source files and 205 headers**, classified as 94 policy / 228 leaf / 47 dormant. Two
additional successful compiler scans initially covered the non-shipping
importer and UCC entrypoints, adding two sources and three previously
uncounted headers: four policy-bearing files and one platform file-manager
alias. That reconciled baseline was 374 files, with 166/166 successful scans.
The subsequent importer CLI cutover deleted its C++ entrypoint and calls the
existing Rust importer directly. The current inventory remains **373 files**.
The frame-ownership checkpoint passed **167/167 configured compiler scans**,
covering **165 unique native sources and 208 included headers**; `HP2Paths.cpp`
and `HP2StaticPackages.cpp` each compile for both game and UCC. Count those files
once in the ownership ledger. The CLI removed one native source; the frame
checkpoint added or removed none.

These are ownership and coverage counts, **not effort or completion
percentages**. The original 322 semantic classifications are inherited from
the full-body audit and accepted deltas, not a new all-file body review.
Fresh preprocessing proves build membership, not behavior. Linux membership
still needs its own configured scan. Generated Rust C headers are tracked
separately; upstream dependency sources are outside first-party scope.

Evidence root:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/`.
`RustCompletionBaseline/native-ownership-ledger.json` assigns all 373 files
exactly once to 14 unequal migration families, with dependencies, Rust reuse,
remaining ownership and acceptance scenarios.
`RustCompletionBaseline/{fresh-compiler-dependencies,post-batch-compiler-dependencies}.json`
retain the selected compiler commands and both sets of 164 successful scans.
`pre-batch-native-ownership-ledger.json` preserves the 368-file baseline.
`shipping-game-launcher-ownership-ledger.json` preserves the accepted 369-file
subset; `runtime-tool-compiler-dependencies.json` records both additional
tool commands and successful scans. Neither scope is silently excluded.
`cli-native-removal/pre-cli-native-ownership-ledger.json` preserves the
374-file runtime/tool baseline; `cli-native-removal/post-cli-compiler-dependencies.json`
records the CLI checkpoint's 165 selected unique-source scans.
`rust-text-conversions-and-identity/final-compiler-membership.json`
records the latest 167 configured commands and scans, with generated Rust headers separate.
The older Ninja dependency database had missing/stale records and is not
the source of truth.

Previous game/launcher checkpoint: rebuilt signed macOS app; five named CTests
passed, including six expression-archive and four speech edge/error contracts.
A genuine SDK consumer passed **33 original-versus-Rust archive cases**.
A silent actual-process consumer passed speech replacement/reaping and
failure scenarios without running `/usr/bin/say`. Installed WGPU
`Adv8Forest` smoke passed 1/1 maps for 120 ticks, **data-prototype**.
Commands, logs and remaining dependencies:
`RustCompletionBaseline/execution-batch-acceptance.json`.
The smoke captured zero frames; it does not prove foreground visuals,
retail, Linux, or audible speech-backend behavior.

Latest runtime-tool checkpoint: the Rust importer built and installed;
`import_cli_contracts` and `importer_parity` passed (nine and two exercised
tests respectively). All 26 captured original-native argv cases matched
exit/stdout/stderr bytes. Original-native, Rust build-tree and installed Rust
imports produced the same 427 output files, byte for byte; Rust check-only
also passed. This is **data-none synthetic fixture** acceptance, not actual
retail-data/gameplay qualification. The binary dynamically links only system
libraries, not the first-party engine/renderer. Exact commands and evidence:
`RustCompletionBaseline/cli-native-removal/acceptance.json`.

Frame-backing foundation checkpoint: Rust owns `ProcessEvent` backing storage
and its synchronous lifetime. The compiler-probed plain frame is 40 bytes
instead of the former 48-byte virtual frame; Rust reads cursor/parent/object
state directly. The genuine before/after SDK consumer matched 64 reflected
script calls plus diagnostics and opcode recovery. Fourteen focused CTests
passed; the final installed `Adv8Forest` WGPU smoke passed 120 ticks under
**data-prototype**, with zero captured frames. This is a partial M1 ownership
cut, not a native-free object/VM slice. Evidence:
`RustCompletionBaseline/frame-owned-state/acceptance.json`.

Native-handler foundation checkpoint: Rust directly dispatches checked plain native
handlers; the opaque native-method member-pointer record is removed. The
unchanged genuine SDK consumer matched five native cases and 64 script calls;
16 focused CTests and final installed prototype gameplay passed. M1 stays open.
Evidence: `RustCompletionBaseline/plain-native-dispatch/acceptance.json`.

Class-registration foundation checkpoint: Rust owns the complete native class-registration
sequence and directly invokes checked plain static constructors. Inherited
methods retain their declaring-method identity, so child registration does not
repeat parent initialization. Six unchanged genuine SDK cases matched the
original; 16 focused CTests and final installed prototype gameplay passed.
Static-constructor storage shrank **16 -> 8 bytes**; `UClass` **2424 -> 2416**.
Typed native constructor bodies and object/class/property ownership remain.
Evidence: `RustCompletionBaseline/checked-class-construction/acceptance.json`.

The frame-construction checkpoint moved complete plain `FFrame` initialization
and both `CallFunction`/`ProcessEvent` frame backing/lifetime to Rust, removing
native placement construction. Evidence:
`RustCompletionBaseline/rust-frame-construction/acceptance.json`.

The call/event local-memory checkpoint moved local bytes, event
parameter copying/zeroing and direct checked event dispatch. Per-thread,
per-depth aligned buffers preserve suspended frames and retain only high-water
capacity; 3,600 warmed scopes, including 600 error returns, performed zero
allocations/reallocations/deallocations in the isolated owner probe. Three
unchanged genuine SDK consumers matched native output, all 17 focused CTests
passed and installed prototype gameplay passed. Native object/class/property
ownership, other frame producers and native unwind scopes remain; M1 stays open.
Evidence: `RustCompletionBaseline/rust-vm-locals/acceptance.json`.

Latest foundation checkpoint: seventeen workers completed eight ownership
batches, with one integration owner. Rust now owns the sole 192-entry checked
opcode/named-token catalog, including three name/object/string conversions and
31 scalar/vector/rotator reference mutations; 34 native methods, registrations
and declarations are deleted. All 22 unchanged mixed SDK consumers still match
exactly. 696 of 726 native mutation/conversion records match; 30 reflected
vector/rotator rows lack native evidence (fixture defect, below). Production
Rust-only execution matches 192 native records. All 44 focused CTests
pass; installed prototype gameplay completes 120 ticks. Fresh 167/167 compiler
scans retain 165 native sources and 208 headers: no file/family closure.
Authored source footprint is 70.9% Rust, not an effort/completion estimate.
Native class identities, typed lifetime, remaining resource hooks and GC
integration still keep M1 open.
Evidence: `RustCompletionBaseline/rust-name-conversions-and-reference-mutations/acceptance.json`.

Geometry conversion checkpoint: nine more Core VM natives (string/vector/rotator
conversions, `<<`/`>>` vector-rotator, `OrthoRotation`, `Invert`) moved into the
checked catalog, now 201 entries. Their C++ methods, registrations, declarations
and the `VmConvertFormat` service are deleted, plus the dead Rust `convert()`
dispatcher. A genuine pre-cutover consumer (126 cases + 1 native exception)
replays byte-identically. 53 native VM registrations remain in `UnCorSc.cpp`.
Evidence: `RustCompletionBaseline/rust-geometry-conversions/acceptance.json`.

Type-query and random checkpoint: `DynamicCast`, `MetaCast`, `ClassIsChildOf`,
`IsA`, `Rand`, `FRand`, `VRand`, `RotRand` and `RandRange` moved into the checked
catalog (now 210 entries). C++ publishes the leaf service tables once; Rust
opens the RNG trace scope itself. A genuine pre-cutover consumer (252 cases)
and its `HP2_RNG_TRACE` output (322 draws) replay byte-identically. 44 native VM
registrations remain in `UnCorSc.cpp`.
Evidence: `RustCompletionBaseline/rust-type-queries-and-random/acceptance.json`.

Object-service checkpoint: `New`, `GotoState`, `Enable`, `Disable`, `IsInState`,
`GetStateName`, `Log`, `Warn`, `SaveConfig`, `ResetConfig` and the named natives
`Localize`, `Get/SetPropertyText`, `StaticSaveConfig`, `GetEnum`, `GetLanguage`
and `DynamicLoadObject` now dispatch from Rust. Named natives bind through a
`named_natives!` catalog list. A genuine pre-cutover consumer (153 cases) replays
byte-identically. 27 native VM registrations remain in `UnCorSc.cpp`.
Evidence: `RustCompletionBaseline/rust-object-services/acceptance.json`.

Script-VM checkpoint: the last 27 interpreter natives (variable, context,
array, struct, flow, assignment, debug-info, function-call and iterator
opcodes) now dispatch from Rust, so `UnCorSc.cpp` has no `IMPLEMENT_FUNCTION`
left. A genuine pre-cutover consumer (209 records: 143 real script calls plus 66
hand-assembled tokens) and its RNG trace replay byte-identically. One original
crash (a missing `GotoLabel` label) was fixed before the oracle was frozen.
Evidence: `RustCompletionBaseline/rust-script-vm/acceptance.json`.

Known qualification debt remains visible: the documented earlier full CTest run
had eight failures out of 136; it was not a passing baseline. Subsequent
focused runs passed `importer_parity` and `data_identity_contracts`; they do
not establish the causes of those historical failures. The other six targets
were not rerun here. Foreground visual acceptance remains blocked by
unavailable capture authorization / occluded windows.
Do not retry unchanged permissions or infer visual acceptance from tick counts.
The user reports the earlier 16 GB RAM issue is resolved; do not reopen that
investigation without new evidence.

## Dependency-ordered milestones

### M0 — One executable backlog and a usable acceptance baseline

- Shipping membership and exclusive family assignments are recorded in the
  baseline ledger above. Keep accepted deltas and fresh compiler membership
  synchronized as files are added or removed.
- Assign each remaining family a stable ID, exclusive files/modules, owner,
  dependencies, native responsibility, Rust replacement, callers to migrate,
  acceptance scenario, status, and evidence path.
- Separate portable policy, typed/object ownership, platform bindings,
  generated declarations, and dormant paths. Account for shared headers once;
  do not turn the old overlapping policy-group count into a completion total.
- Triage the eight recorded full-suite failures from existing evidence first.
  Repair causes and configuration gaps; do not skip, re-pin, or retry to green.
- Record which prototype and legally supplied retail data are available, how
  retail identity is verified, and which checks require another platform or an
  authorized foreground session.
- Read-only inspection found all eight currently configured test `TMPDIR`
  aliases resolve to directories. That does not explain or reverse the
  earlier failures. Their checkpoint summary lacks per-failure raw logs;
  retain this evidence limitation instead of claiming all eight share a cause.
  Current commands, environments and prerequisite state are recorded in
  `RustCompletionBaseline/qualification-failure-triage.json`.

Exit: every compiled first-party native file and retained header has a migration
or disposition entry; each runnable acceptance gate has a valid configuration;
known failures and external prerequisites have named owners. No calendar ETA is
published from raw file counts.

### M1 — Prove the Rust object / reflection / VM foundation early

This is the critical path. Do not finish hundreds of wrapper migrations and
only then discover that the C++ object model still owns the program.

- Reuse the existing `hp2-core`, `hp2-engine`, and `hp2-abi` implementations;
  specify Rust ownership of class/property metadata, instance storage, typed
  construction/destruction, native-handler registration, and VM frames.
- Replace C++ virtual/member-pointer dependencies with Rust-owned dispatch,
  while preserving observed object identity, reflection offsets/flags, package
  serialization, callback ordering, and reentrant lifetimes during each cutover.
- Keep compatibility at the existing boundary only while its callers migrate;
  do not introduce a second permanent object model, ECS rewrite, or shadow
  source of truth. Audit raw-pointer aliasing and callback mutation explicitly.
- Deliver one real vertical slice: load an existing package/class, construct an
  instance, access and serialize properties, execute a script/native call with
  parameters/out values, perform GC/destruction, and propagate an error without
  relying on that slice's former C++ object/VM implementation.
- Use existing native behavior as an oracle before deleting it. Exercise nested
  callbacks, error-after-partial-mutation, and destruction ordering, not just
  layout/size assertions or a mock service table.

Exit: the vertical slice passes differential/behavior contracts and a genuine
runtime consumer; the accepted ownership/dispatch contract supports migration
of the remaining classes. The rest of the game may still be mixed at this gate.

#### Foundation ownership contract and current cutover

Existing Rust allocation and VM policy still call native ownership services:
`hp2-core/src/object_construction.rs` receives typed constructor/property
operations. Rust now initializes all plain `FFrame` fields and owns both
`CallFunction` and `ProcessEvent` frame backing, local bytes and synchronous
lifetime; native placement construction and dynamic local-byte continuations
are removed. Event parameter copying/zeroing and plain checked handler dispatch
are Rust-owned. Rust also owns persistent `FStateFrame` initialization and heap
allocation/free across reset, archive load and object destruction. All four
`UObject` base-header data initializers, allocation-local saved constructor
snapshots and checked live instance-constructor dispatch are Rust-owned. Native
class-default vptr installation and existing `FName` checkSlow diagnostics remain
typed leaves. Rust also owns ordinary/superclass `UClass` `ClassWithin`/`ClassCRC`
initialization, class-default byte reset/inheritance and explicit class-array/
property cleanup sequencing through the existing allocator. Typed default
property initialization/exit, binding, default-text clearing and qualified super
destruction remain checked native leaves. Rust additionally owns `UField` /
`UStruct` registration and ordinary scalar initialization, `UState` registration
scalars and native/static `UClass` registration data, preserving unspecified
bytes and the pointer-width config-name/ClassCRC alias. Rust owns default and
C++ property scalar construction, publication/registration sequencing, property
prepend and five initialized subtype fields; typed Category construction remains
native and precedes checked publication. Rust also owns supported-host property
alignment, all active type-specific layout and bool packing, nested callback/
constructor propagation and fixed/dynamic-array inner publication. Other frame/
scratch producers, object/class/property representation, remaining field updates,
typed bodies, method adapters and native unwind scopes remain. Rust owns class-
registration sequencing. The ABI generator mirrors complete plain `FFrame`,
`FStateFrame`, `UObject` and `FDependency` data and required partial metadata/
property fields, including fixed-array, dynamic-array and map descriptor fields.
Rust also owns direct metadata serialization/script/state-hash/handler data and
the shared loading-layout pointer. Twelve earlier constant/flow, 53 by-value
scalar, 40 vector/rotator/extended-prefix and 17 primitive-conversion handlers
now bypass native member invocation. Fixed-argument stepping and `P_FINISH`/
debug policy are Rust-owned where applicable; primitive conversions deliberately
step one expression without finishing. Other handlers and typed reflection/
archive operations stay native.
The existing Rust slot/free-index/hash owner also directly publishes object
indices and maintains intrusive hash links through compiler-probed headers;
native primitive-field service callbacks are no longer involved.
Metadata archive field selection and ordering are Rust-owned too; remaining
native typed value operators preserve reference/name interception and compound
archive lifetimes. `UCommandlet` has a compiler-probed mirror, not a native-free
typed class implementation.
Native vptr/padding and other fields remain opaque.
These are not complete native-free `UObject` / `UClass` implementations or M1
completion.

1. Extend the existing compiler-probed ABI manifest, rather than hand-writing
   guessed layouts, for the object/class/property and frame fields required
   during migration. Preserve alignment, inherited offsets, object indices
   and script pointer/index representation. Probe results alone are not M1.
2. Reuse the existing Rust object-slot/hash/free-index ownership; define one
   authoritative class/property representation and stable instance allocation.
   No second registry, moving object vector, per-field heap allocation or
   emulated C++ vtable as the final design.
3. Replace native typed constructor bodies, method adapters and virtual dispatch
   dependencies with Rust-owned typed operations. Runtime native-handler and
   class-static-constructor member-pointer records are already removed. Migrate
   the selected class family's complete defaults, instance and destruction
   ownership together; name every temporary bridge still required.
4. Finish remaining frame producers, derived state frames and native unwind/error
   scopes. Plain initialization, call/event frame backing and local-byte lifetime,
   event parameter copies and checked native-handler dispatch are accepted.
   Preserve parameter/out-value behavior, callback mutation/reentry, first-error
   and partial-publication semantics without mutable native borrows across callbacks.
5. Prove the package/object/script/GC slice above with a genuine consumer
   whose linked inputs exclude that slice's former native implementations.
   Existing native CTest consumers remain the differential oracle, not a
   substitute for this native-free slice.


### M2 — Migrate complete families in four bounded work lanes

Independent algorithm/OS-binding work can start once its ownership, interfaces
and affected-path acceptance are mapped. Object-layout-dependent changes wait
for the relevant M1 contract. An unresolved broader qualification baseline
does not turn focused evidence into full-suite acceptance. Targets below are
audit-derived families; revalidate residual bodies before assigning them.

| Lane | Complete-family targets | Principal boundaries |
| --- | --- | --- |
| A: Core / VM foundation | Remaining reflection and bytecode archive behavior; foreach/VM frame ownership; generic typed containers, formatting/logging, class registration, object construction and GC leaves | `Core/Inc/UnClass.h`, `UnType.h`, `UnScript.h`, `UnTemplate.h`; `Core/Src/UnClass.cpp`, `UnProp.cpp`, `UnObj.cpp`, `UnCorSc.cpp`; `hp2-core`, relevant `hp2-abi` types |
| B: Engine / world / networking | Moving-brush physics; actor destruction and level lifecycle; pending-client handshake/download; RPC serialization; script iterators; model/mesh archive fixups; engine class/storage ownership | `Engine/Src/UnScript.cpp`, `UnLevAct.cpp`, `UnPenLev.cpp`, `UnModel.cpp`, mesh sources and their `Hp2*Services`; `hp2-engine` |
| C: Rendering / resources | Finish renderer orchestration and dynamic iterator/shadow/light lifetimes as whole families; Canvas/text calls; native render objects, mesh/texture/device adapters | `Render/Src/UnSprite.cpp`, `UnRender.cpp`, `UnLight.cpp`; Engine Canvas and render services; `WgpuDrv`, `NativeText`; `hp2-render`, `hp2-wgpu` |
| D: Platform / audio / launcher / tools | Process/signal/crash-watchdog ownership; file/error output; startup/configuration/feedback; SDL viewport and launcher shell; OpenAL/codec bindings; UCC commandlet/help/error lifecycle; supported opt-in diagnostics | `Core/Src/HP2CrashReporter.cpp` (deleted; now `hp2_platform::crash_reporter`), platform portions of `UnUnix.cpp`; `SDLLaunch`, `SDLDrv`, `ALAudio`, `UCC/Src/UCC.cpp`; `hp2-platform`, `hp2-launcher`, `hp2-audio`, `hp2-import` (CLI entrypoint now Rust) |

Each lane delivers a real replacement plus deletion of the superseded native
implementation. An intermediate policy-only cutover must explicitly name its
remaining native dependency and subsequent removal family. It does not close a
native-removal milestone. Preserve bitmap-font compatibility mode as well as
native text; do not promote feature gates without matching data-profile proof.

Exit per family: migrated callers, removed obsolete implementation/exports,
behavior/error-boundary evidence, focused named CTest acceptance, rebuilt
product scenario, and updated ownership ledger/documentation.

### M3 — Remove the retained native object and service layer

Apply the proven M1 foundation to every remaining engine class and consumer,
including already-Rust-policy families. This overlaps M2 as each type family
becomes ready; it is not an unplanned final rewrite.

- Move class/default-object creation, reflection registration, property storage,
  archive/VM frames, typed temporary lifetimes, and native function invocation
  fully into Rust. Preserve package and save compatibility with real fixtures.
- Replace first-party rendering, audio, platform and launcher adapters with
  direct Rust-owned bindings to their existing external dependencies.
- Remove service callbacks, generated C headers, C++ constructors/destructors,
  member-pointer tables and exception transport when their final callers move.
  Preserve the real error contract in Rust; do not replace it with swallowed
  failures, dummy success, or panics crossing foreign boundaries.
- Resolve every dormant/declarative inventory entry. Preserve supported
  behavior; retire unrelated code only with an explicit scope decision.

Exit: no runtime ownership still depends on first-party native code; no legacy
fallback is needed to run a supported feature. Policy-file and native-leaf
counts both reach zero for shipping runtime and retained first-party tools.

### M4 — Enforce a native-free first-party build

- Make the shipping game, launcher, importer CLI and UCC commandlet runtime
  build from Rust entrypoints through the existing build/install pipeline;
  keeping CMake itself is acceptable.
- Remove obsolete first-party native build targets, source/header dependencies,
  old exports and tests tied only to deleted implementation details.
- Inspect the actual compile graph and linked inputs on both platforms. Add a
  deterministic build-source gate rejecting first-party C/C++/Objective-C
  runtime objects; allow only explicitly identified external dependencies.
- Verify installation, signing, data importing, paths, desktop integration,
  and launch from the installed products, not just a developer binary.

Exit: clean macOS and Linux builds and installed products require zero
first-party native runtime translation units. External native dependencies are
listed explicitly; renaming or excluding a source from the audit is not removal.

### M5 — Qualify the full Rust product

- Run the complete configured CTest suites, matching sanitizer presets, and
  supported map-smoke coverage on macOS arm64 and Linux arm64.
- Exercise actual startup/menu/new game, movement/collision/spells, animation,
  particles/shadows/text, audio, map travel, save/restart/load/continue, and
  repeated resource creation/destruction under the appropriate data profiles.
- Exercise supported networking/demo/diagnostic paths through their dedicated
  consumers when ordinary gameplay does not cover them.
- Verify foreground presented output in an authorized session. Keep this gate
  blocked until that prerequisite exists; do not bypass OS permissions.
- Validate retail data provenance with the importer manifest and actual file
  identity checks; the runtime currently does not validate the overlay manifest
  itself. Prototype success never earns retail acceptance.
- Retain exact commands, results, logs/captures and unresolved conditions under
  `HP2_ARTIFACT_DIR`; update `Docs/BEHAVIOR_MATRIX.md`. Golden changes require
  separate acceptance. No feature-gate promotion by assumption.

Exit: all required behavior/platform/profile gates pass, native-removal build
proof is recorded, and no acceptance criterion is replaced with compile-only
or headless-tick evidence. Only then declare full Rust completion.

## Parallel execution rules

Use up to four implementation workers, one per genuinely independent family,
with one integration owner. Four workers is a concurrency cap, not a promised
fourfold speedup. Assign exclusive files and Rust modules before spawning work.

The integration owner owns shared `hp2-ffi` exports, cbindgen configuration,
`hp2-abi` layout contracts, common generated metadata, CMake registrations,
central docs and aggregate evidence. Workers propose interface changes before
using them. Split ownership of `UnUnix.cpp`, Canvas and platform-render seams
explicitly; two lanes must not edit the same file concurrently.

Workers capture original behavior before replacement and skip builds, tests,
linters and formatters mid-flight. Integrate one compatible batch, then run its
combined build, focused contracts and real product scenarios once. Record any
failure and fix its cause before a justified new run. Do not have every worker
rebuild the entire game or compete for a GUI session/Cargo build lock.

## Progress and estimation

For each accepted batch report:

- Closed family IDs with acceptance evidence, not number of commits or FFI calls.
- Residual-policy files and native-only leaf files separately.
- First-party native runtime translation units actually in the compile graph.
- Remaining object/VM/service ownership boundaries and blocked dependencies.
- Passing platform/data-profile behavior gates, known failures and unmet gates.

M0 establishes the deduplicated backlog. Use elapsed time from accepted,
representative bounded, stateful and object-ownership families to estimate a
range for the remaining families of each kind. Include integration and product
qualification explicitly. Re-estimate after the M1 vertical slice and each
accepted batch; do not extrapolate tiny predicate migrations to object-model
replacement. Until then a calendar finish date or effort-completion percentage
would be invented.

## First execution batch

1. Use the reconciled M0 ledger; finish baseline failure triage and assign the
   M1 ownership/dispatch contract. Retain native behavior as the oracle.
2. Accepted `CORE-EXPRESSION-ARCHIVE`: complete recursive expression
   serialization, including versioned operands and speculative debug reads;
   33 genuine SDK differential cases passed. Native class/object/archive
   ownership remains open.
3. Accepted `PLATFORM-MAC-SPEECH-LIFECYCLE`: process start/reaping and exact
   disabled/failure/replacement behavior; silent real-process consumer passed.
   Native viewport object storage and typed text conversion remain open.
4. Five named CTests and the installed prototype smoke passed; membership was
   refreshed. These two policy-family completions close no native-removal
   family and do not replace the broader failing qualification baseline.
5. Next: the M1 ownership contract and real package/object/script/destruction
   slice. Shared object-layout and dispatch changes have one integration owner.

## Importer CLI native-entrypoint removal

The independent importer CLI cutover is accepted. `crates/hp2-import/src/main.rs`
owns raw argv parsing, deferred UTF-8 conversion, exact error/status output
and direct `run_import` invocation. `HP2ImportMain.cpp` is deleted. Corrosion
links the Rust executable against the same pinned archive/compression targets;
installation handles its imported target without a C++ wrapper or hp2-ffi link.

```sh
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_import
ctest --preset macos-arm64 -R '^(import_cli_contracts|importer_parity)$' --output-on-failure
```

Observed: 2/2 CTests passed in 4.82 seconds; independent original-versus-Rust
argv/import consumers and the installed executable also passed, as recorded
above. This removes one native translation unit, but does not close the
combined UCC/ANSI tool family or the M1 object/VM ownership milestone.


## Rust-owned frame backing checkpoint

`FFrame` no longer inherits the virtual output-device base. Existing logging
uses one explicit temporary output adapter, preserving native suppression,
formatting and exception behavior. Rust owns uninitialized frame storage on
its stack; checked native placement construction starts the C++ object lifetime
in that storage. Compiler-probed size, alignment, field offsets and trivial
destruction are enforced; no fake vtable, heap frame or zero-filled object.

Three frame metadata getters and the separate opcode cursor-slot argument were
removed.
At this checkpoint native dynamic locals, member-pointer calls, other frame
producers, object/class/property ownership and final unwind/error transport
remained. The checked native-handler checkpoint below subsequently removes
the runtime native-method member-pointer record dependency.
This closes **zero entire native-removal families** and removes **zero files**.
M1 stays open until the real package/object/property/script/GC slice excludes
its former native ownership implementation.

The full verification build also exposed the S3TC provider missing from
Core-only FFI consumers. The unchanged compatibility source now compiles once
as an object propagated through `hp2_ffi`, resolving both legacy entrypoints
and the existing pinned libSquish leaf. No upstream source was changed.
Focused codec, config and identity consumers passed alongside the VM checks.
Exact commands, original findings and final evidence are recorded in the
frame checkpoint above and `Docs/BEHAVIOR_MATRIX.md`.

## Checked plain native-handler dispatch checkpoint

Rust now loads and invokes checked plain native handlers directly. Opcode
registration, named native lookup and reflected `UFunction::Func` use the
same callback contract: `(object, frame, result) -> status`. Compiler-typed
C++ adapters preserve receiver adjustment and catch native exceptions before
returning through Rust. The two-word member-pointer record, copy operations,
Itanium-only restriction and redundant C++ step-dispatch callback are deleted.
At this checkpoint class static constructors still used native member pointers;
the next checkpoint removes those records, not the native constructor bodies.

The unchanged genuine SDK consumer matched its original output: five native
cases, including named/indexed calls, a derived actor, latent state polling
and high-native arithmetic; 64 reflected script calls; logging and recovery
after native errors. Compiler-observed handler size changed **16 -> 8 bytes**,
lookup record **24 -> 16**, and `UFunction` **224 -> 216**. All **16 focused
CTests passed in 27.65 seconds**. The final normally configured installed WGPU
app passed **Adv8Forest, 120 ticks, data-prototype**, exit 0. No foreground
visual, Linux, sanitizer or retail claim follows from these runs.

The temporary consumer hook is retired. Fresh **167/167** configured compiler
dependency scans retain **373 unique native files** (shipping game/launcher:
**369**); **zero files removed, zero entire native-removal families closed**.
M1 remains open. Next is real Rust object/class/property construction and
destruction ownership, including dynamic locals and remaining frame producers,
not another collection of algorithm wrappers. Exact commands and findings:
`Docs/BEHAVIOR_MATRIX.md` and
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/plain-native-dispatch/acceptance.json`.

## Checked class construction checkpoint

Rust now owns `UClass::Register`: superclass registration, stashed configuration
name restoration, default storage/initialization, inherited static-constructor
suppression, inherited flags, linking, configuration and localization. Native
callbacks retain typed storage and exception capture. Constructor reentry is
observed before subsequent handler and superclass/flag reads; the first error
preserves partial mutation and prevents later publication.

The unchanged genuine SDK consumer matched **six cases**: parent defaults,
inherited suppression, overriding defaults, real instance defaults, a typed
native failure after partial mutation, and independent successful recovery.
All **16 focused CTests passed in 26.42 seconds**. The final installed WGPU app
completed **Adv8Forest, 120 ticks, data-prototype**, exit 0, with no failure
markers and zero captured frames. The ABI generator check passed.

The static-constructor callback shrank **16 -> 8 bytes** and `UClass`
**2424 -> 2416**; its defaults/callback offsets remain 2368/2392. A canonical
adapter specializes on the actual declared method, not the registering child.
No runtime member-pointer record or compatibility shim remains.

The temporary SDK build hook is retired. Fresh **167/167** compiler dependency
scans still find **373 native files**, including **369** shipping game/launcher
files: **zero native files removed; zero full removal families closed**.
Native constructor bodies, virtual object/class/property representation, typed
destruction, dynamic locals and remaining frame producers keep M1 open.
This is not foreground visual, Linux, sanitizer, retail or full-suite acceptance.
Exact evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/checked-class-construction/acceptance.json`.

## Rust-owned frame construction and call lifetime checkpoint

At this checkpoint Rust initialized both plain `FFrame` forms and owned
script-call backing as well as event frame backing. Checked native leaves
retained class lookup and typed script-array indexing; constructor facades and
native local-byte stack allocation remained. Incomplete frames were never
published. Initialization preserved early field mutation and first-error order
across callback reentry, without per-call heap allocation or compatibility aliases.

The unchanged genuine SDK constructor/call consumer matched four construction
cases, **64 outer + 64 nested calls**, a script exception and independent recovery.
The unchanged reflected-event consumer also matched its preserved native output:
**64 events, five native calls, opcode/critical recovery, zero failures**.
All **16 focused CTests passed in 25.66 seconds**. The final installed WGPU app
completed **Adv8Forest, 120 ticks, data-prototype**, exit 0, with zero failure
markers and zero captured frames. The ABI generator check passed.

The temporary SDK hook is retired. Fresh **167/167** compiler dependency scans
still find **373 native files**, including **369** shipping game/launcher files:
**zero native files removed; zero complete removal families closed**.
Native object/class/property representation and typed lifetimes, dynamic local
bytes, other frame producers, derived state frames and native unwind scopes
keep M1 open. No foreground visual, Linux, sanitizer, retail or full-suite
acceptance is claimed. Exact commands and evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-frame-construction/acceptance.json`.

## Rust-owned VM local memory checkpoint

`CallFunction` and `ProcessEvent` now use shared Rust-owned local-byte scopes.
Buffers are 16-byte aligned and retained per thread/nesting depth; callbacks hold
no pool borrow, and nested growth cannot move suspended backing blocks. Scope
release survives ordinary errors and Rust panic unwinding. Rust's allocator owns
thread-exit cleanup independently of the native engine allocator. Parameter
copy-in, trailing zeroing, direct checked event invocation and copyback are Rust
operations; native `alloca` continuations and both obsolete frame FFI exports
are removed. Typed property operations and outer native trace scopes remain.

The unchanged local-memory SDK consumer matched **320 outer + 320 nested script
calls**, **256 outer + 256 nested events**, large buffers through **256 KiB**,
a typed native error and independent recovery, with zero failures. Both older
unchanged constructor/reflection consumers also matched their native baselines.
An isolated allocation probe measured **zero allocations, reallocations or
deallocations over 3,600 warmed scopes**, including **600 error returns**; this
claim covers the local owner, not all engine callbacks or first-time growth.
All **17 focused CTests passed in 27.96 seconds**. The final normal installed
WGPU app passed **Adv8Forest, 120 ticks, data-prototype**, exit 0, with zero failure
markers and zero captured frames. The ABI generator check passed.

The temporary SDK hook is retired. Fresh **167/167** compiler scans still find
**373 native files**, including **369** shipping game/launcher files:
**zero files removed; zero complete removal families closed**. Object/class/
property representation and typed lifetimes, other frame producers, derived
state frames and native unwind scopes keep M1 open. No foreground visual, Linux,
sanitizer, retail or full-suite acceptance is claimed. Exact commands/evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-vm-locals/acceptance.json`.

## Rust-owned persistent state-frame checkpoint

Rust now initializes and owns every persistent `UObject::StateFrame` allocation,
including reset, absent-frame archive load, stack-flag removal and object
destruction. The compiler-probed mirror is **72 bytes, alignment 8**, extending
the plain 40-byte frame. Fresh `LatentAction` and tail padding remain uninitialized
as in the native constructor; only raw initialized fields are read. A typed SDK
constructor facade and native diagnostics/archive leaves remain.

The unchanged genuine SDK consumer matched its native baseline: **641 successful
loads, 642 callback reentries, one typed archive error with partial state
preserved**, independent recovery, direct destruction and real GC, zero failures.
Three older unchanged VM/reflection SDK consumers also matched. An isolated probe
of the compiled Rust owner measured **8,192 allocations and 8,192 deallocations**,
zero reallocations and **589,824 bytes allocated/freed**. That is owner-only
balance, not a whole-engine memory claim.

Qualification exposed an unbounded configuration filename copy into a
256-element buffer: the legacy SDK's 256-character INI path failed before its
scenario began. The first failure and bootstrap diagnostic are retained. Rust
now keeps short filenames on stack and spills only when necessary; complete
paths, alias resolution and the legacy extension predicate are preserved.
The same HOME/INI bootstrap passed after the repair, and permanent cache-filename
coverage exercises the boundary, suffix expansion and both configured aliases.
All **27 focused CTests passed in 42.03 seconds**. The normally configured
installed WGPU app passed **Adv8Forest, 120 ticks, data-prototype**, exit 0,
with zero failure markers and zero captured frames. The ABI generator check passed.

Temporary SDK targets are retired. Fresh **167/167 compiler scans** still find
**373 native files**, including **369** shipping game/launcher files:
**zero files removed; zero complete removal families closed**. Native object/
class/property representation, typed lifetimes, other frame/scratch producers and
native unwind scopes keep M1 open. No foreground visual, Linux, sanitizer, retail
or full-suite acceptance is claimed. Exact commands, preserved failures and proof:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-state-frame/acceptance.json`.

## Rust-owned base-header and checked instance-constructor checkpoint

Rust now initializes the compiler-probed **72-byte, align-8 `UObject` data
header** for allocated, native, static and class-default forms. It stores fields
through raw pointers without constructing a reference to or copying an
uninitialized native object. Ordinary forms preserve native vptr/padding; the
static form also preserves `Class`. Class defaults deliberately zero data
padding, retain `_LinkerIndex=0` rather than `-1`, and set `Index=-1` and `Class`.
The native class-default leaf only installs the compiler-owned vptr before
delegating data initialization. Existing native `FName` checkSlow diagnostics
are retained; this is not a Rust replacement for C++ virtual dispatch.

Instance callbacks now have a checked `INT(void*)` ABI through compiler-typed
adapters. Rust saves the nullable callback in invocation-local storage before
destruction/reentry and restores it through the metadata leaf. Construction
reloads and directly invokes the live callback after allocation; the native
saved-constructor slot and native invoke continuation are removed. The compiled
editor transaction caller rethrows checked status but was not independently
runtime-exercised. Native typed constructor bodies and class storage remain.

Seven frozen genuine SDK consumers exit 0 with identical native/Rust stdout and
stderr and unchanged source hashes: header initialization, instance construction,
class-default headers, frame construction, reflected events, VM local memory
and persistent state frames. The default-header native baseline was captured
after the other cuts but before replacing its still-original native data
initializer; that narrower comparison is recorded explicitly. This platform's
real startup exercises `EStatic`, not the `ENative` alternative. The isolated
raw Rust owner probe covers all four forms: 1,024 headers, no allocations or
reallocations, preserved vptrs, 768 preserved ordinary padding cases and 256
zeroed default-padding cases. It does not manufacture fake native objects.

All **27 focused CTests pass in 37.23 seconds**: 25 `data-none`, two
`data-prototype`. After retiring temporary SDK hooks, normal configure/build and
ABI generation `--check` pass. The final installed WGPU app passes **Adv8Forest,
120 ticks, data-prototype**, exit 0, zero failure markers and **zero captured
frames**. Fresh **167/167 compiler scans** still find **373 native files**,
including **369** shipping files; no temporary SDK compile rows remain.
**Zero native files removed; zero complete removal families closed. M1 stays
open.** No foreground visual, Linux, sanitizer, retail or full-suite acceptance
is claimed. Exact commands, the preserved initial cbindgen failure and its
inline-nullable-callback repair, source hashes and final evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-object-header-and-constructor/acceptance.json`.

## Rust-owned class-default storage and lifetime checkpoint

Rust now owns ordinary/superclass `UClass` `ClassWithin`/`ClassCRC`
initialization, class-default array reset and inheritance, and explicit
`NetFields` / `Dependencies` / `PackageImports` / property-default destruction
sequencing. It reuses the existing Rust raw-array owner and native allocator
services, reloads fields across callbacks, and copies service tables before
reentry. The native reset/resize continuations and duplicate default initializer
are removed. Typed default initialization/exit, bind, text clearing and qualified
super destruction remain checked native leaves; implicit native typed array/
string construction/destruction and class/property representation remain.
No cleanup absent from the original explicit `Destroy` was added.

Actual native compiler probes preserve **`UClass` 2416 bytes, align 8**, and
**`FDependency` 16 bytes, align 8**. Metadata services change from 29 to 30
pointer slots; registration services remain 12. The new genuine SDK captures
the additional class-lifetime boundary after the previously accepted cuts,
not a wholly-native game baseline. Its unchanged source matches native output:
two inherited classes, six watched owned-allocation releases, typed error 8217
and an independent successful recovery.

The seven older frozen SDKs initially exposed a real state-frame load failure:
`Node` was already decoded, but offset calculation still used `Code` from the
previous node. Loading now decodes the wire offset before reconstructing code;
save checks and decoded-offset validation remain. The original failure and
four-case permanent regression suite are preserved: two load tests failed
before the fix, both save tests passed, and all four pass afterward. All eight
unchanged SDK sources now match their native exit/stdout/stderr results.

All **30 focused CTests pass in 15.16 seconds**: 28 `data-none`, two
`data-prototype`. Normal configure/build and ABI `--check` pass after retiring
SDK hooks. The installed WGPU app passes **Adv8Forest, 120 ticks,
data-prototype**, exit 0, zero failure markers and **zero captured frames**.
Fresh **167/167 compiler scans** still find **373 native files**, including
**369** shipping files, with zero SDK compile rows. **Zero native files or
complete removal families close; M1 remains open.** No foreground visual, retail,
Linux, sanitizer or full-suite acceptance is claimed. Exact commands, source
hashes, preserved failures, storage-prerequisite repairs and final proof:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-class-default-lifetime/acceptance.json`.

## Rust-owned reflected metadata data-construction checkpoint

The existing `hp2-core::class_lifecycle` raw owner now initializes native/static
and ordinary `UField` / `UStruct` scalar data, registration `UState` scalars and
native/static `UClass` registration data. Native duplicate initializer stores
are removed. Static superclass/within fields, registration FriendlyName,
state hash storage, native vptrs/padding and typed members retain their original
ownership. Config registration preserves the whole pointer stashed across the
four-byte FName and adjacent ClassCRC; ordinary inheritance reads parent layout
and the already-initialized root name without whole-object Rust references.

The genuine additional-boundary SDK passes **620 checks**, including six real
bootstrap classes, two structures, two states, two classes, six normal metadata
deletions, a reflected default/property and two explicit instance constructions.
An initial oracle incorrectly required an instance-constructor call during
default-header initialization. Actual `InitClassDefaultObject` does not call it:
that incidental count pin was removed, not changed to zero. The two explicit
instance calls are checked relative to their preceding count. The failed run
and exact source are retained; the corrected native baseline was frozen before
removing any production initializer. All **nine unchanged genuine SDK sources**
match native exit/stdout/stderr. Manual ENative construction and arbitrary
pre-registration native-byte preservation are not claimed; five additional
raw-Rust contracts separately cover aliases and untouched storage.

Actual native compiler probes preserve **UField 96, UStruct 192, UState 2264,
UClass 2416 bytes, all align 8**, PROPERTY_ALIGNMENT 4, parsed/compiled mask 18
and metadata/registration service widths 30/12 pointer slots.
All **30 focused CTests pass in 54.19 seconds**: 28 `data-none`, two
`data-prototype`. SDK-free normal configure/build and ABI `--check` pass.
The installed WGPU **Adv8Forest, 120-tick, data-prototype** smoke exits 0,
with zero failure markers and **zero captured frames**.
Fresh **167/167 compiler scans** still find **373 native files = 165 sources +
208 headers**, shipping **369 = 164 + 205**, and zero SDK compile rows.
**Zero native files or complete removal families close; M1 remains open.**
No foreground visual, retail, Linux, sanitizer or full-suite acceptance is
claimed. Exact commands, hashes, original oracle failure and final proof:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-metadata-data-construction/acceptance.json`.

## Rust-owned reflected property-construction checkpoint

The existing `hp2-core::native_property_offsets` owner now initializes default
property scalars, ArrayDim/flags before typed Category construction, and
Offset/CppProperty before virtual publication. It obtains the current owner/name
after publication, registers the original requested offset and preserves the
first error and committed mutations. Rust also owns the prepend and five
initialized subtype fields; unrelated links, flags and padding stay untouched.
The five C++ subtype constructors retain only typed base/vptr lifetimes and
Rust calls. The obsolete direct registration export is deleted.

The additional genuine SDK passes **206 checks**, including real defaults,
reflection/value operations, recursive virtual publication and a typed
constructor failure followed by recovery. Its first native run failed because
the fixture allocated `UEnum` outside its required `UStruct`; that source/output
remain. The corrected original baseline was frozen before changing native
constructors. All **ten unchanged genuine SDK sources** match native
exit/stdout/stderr. Four isolated raw-Rust contracts cover untouched bytes,
aliases, duplicate precedence, reentry, capacity and first-error mutations.

Actual native layouts remain **UProperty 160, byte/bool 168, object 176,
class 184, struct 176 bytes, all align 8**; the checked table is three pointer
slots. All **31 focused CTests pass in 51.01 seconds**: 29 `data-none`, two
`data-prototype`. Normal SDK-free configure/build and ABI `--check` pass.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 with zero
failure markers and **zero captured frames**. Fresh **167/167 compiler scans**
still find **373 native files = 165 sources + 208 headers**, shipping
**369 = 164 + 205**, and zero SDK rows. **Zero file/family closures; M1 stays
open.** No foreground visual, retail, Linux, sanitizer or full-suite acceptance.
Exact commands, source hashes, preserved fixture failure and final proof:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-property-construction/acceptance.json`.

## Rust-owned reflected property-layout checkpoint

The existing `hp2-core::property_link` owner now performs property alignment,
all eleven active type-specific Link data operations, bool packing and fixed/
dynamic-array inner publication. Native class tests, current-owner virtuals,
archive preload/child Link, checked narrowing and original guard/admission/count
checks remain typed leaves. Rust rereads mutable nested pointers, sizes, flags
and constructor links after callbacks; the service table is copied before calls.
Actual original compiler behavior establishes owner-size before alignment.
No new allocation, lock, property staging buffer or parallel ownership registry
is introduced. Native duplicate algorithms/stores are deleted.

The additional genuine SDK passes **221 checks** across **11 kinds**, nine
admissions, 59 deletions, three reentries, ten typed errors/recoveries and nine
registered reflected fields/defaults/instances. All **eleven frozen genuine SDK
sources** match original native exit/stdout/stderr. The first native fixture
failure and narrow diagnostic remain: ordinary C++ FString/TArray member
constructors cleared already-initialized reflected defaults. The fixture uses
the existing `E_NoInit` constructors; all assertions remain, and the corrected
native baseline predates production removal. The initial incorrect Rust ABI
import failure is also retained. Fifteen raw-Rust contracts separately cover
owned bytes, aliases, reentry, callback mutations, first errors and narrowing.

Actual new native layouts are **fixed array 176, dynamic array 168, map 176
bytes, all align 8**; the immutable native constants record is **64 bytes,
align 4**, checked services **48 bytes, align 8, six pointer slots**.
All **32 focused CTests pass in 63.73 seconds**: 30 `data-none`, two
`data-prototype`. Normal SDK-free configure/build and ABI `--check` pass.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 in **18.730819
seconds**, zero failure markers and **zero captured frames**. Fresh **167/167
compiler scans** retain **373 native files = 165 sources + 208 headers**,
shipping **369 = 164 + 205**, and zero SDK rows. **Zero file/family closures;
M1 stays open.** No foreground visual, retail, Linux, sanitizer, full-suite or
gate promotion claim. Genuine coverage excludes the unsupported PSX2 branch,
invalid admission assertions, fatal checked-size overflow, internal nonvirtual
classification-call order, registered map/struct members and allocator failure.
Exact commands, source hashes, preserved failures and final proof:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-property-layout/acceptance.json`.

## Rust-owned metadata field-ownership checkpoint

The existing `hp2-core::class_metadata` owner now reads/writes compiler-probed
object/field/struct/state/class/function/property data directly, resizes script
storage through the existing array owner, owns state hash operations and
installs checked native handlers. Its storage owns the loading-layout pointer;
the remaining native property-link leaf reads that same Rust-owned pointer.
Nine obsolete field/hash/script/handler callbacks and the C++ loading-layout
global are deleted. Typed archive, superclass, configuration, diagnostic and
dynamic-lookup leaves remain. No second registry, vtable emulation or per-call
allocation is introduced.

The additional genuine SDK passes **2,373 checks**, 12 round trips, nine script
bytes, 1,024 hash-slot checks, four real handler bindings, five typed errors,
one reentry, three live mutations and 12 loading-layout observations. All
**twelve frozen genuine SDK sources** match native exit/stdout/stderr.
The first native fixture incorrectly required legacy property alignment to
equal host C++ alignment. That unsupported assertion was removed; actual
values/size checks remain and a requested alignment-16 floor is now exercised.
The corrected original baseline predates native removal. The first post-cutover
build exposed cbindgen's opaque pointer-to-Option handler table; the C boundary
now uses a checked-function-pointer table and Rust reads nullable slots as
`Option`. Both failures and the diagnostic are preserved.

Actual native **UFunction is 216 bytes, UState 2,264 bytes, both align 8**;
the state hash is 256 pointers at offset 216. Narrow parameter/index fields
retain their actual one-/two-byte widths. The checked table is **23 pointer
slots**. All **32 focused CTests pass in 57.90 seconds**: 30 `data-none`, two
`data-prototype`. Normal SDK-free configure/build and ABI `--check` pass.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 in **19.046333
seconds**, zero failure markers and **zero captured frames**. Fresh **167/167
compiler scans** retain **373 native files = 165 sources + 208 headers**,
shipping **369 = 164 + 205**, with zero SDK rows. **Zero file/family closures;
M1 stays open.**

At this checkpoint, the configured nonblank source footprint was **69.7% Rust,
30.3% native**: 545 Rust files / 216,821 lines and 370 native files / 94,097
lines, excluding generated code, tests, external dependencies and `build.rs`.
Comments count as nonblank lines. This is not an effort/completion percentage;
including generated native files, the compiler census remains 373 files.
No foreground visual, retail, Linux, sanitizer, full-suite or gate promotion
claim. Genuine SDK limits include package-file index encoding, class archive
versions <=61, invalid-index/fatal diagnostics, allocator failure and parallel
native-global loading. Exact commands, hashes, counters, preserved failures:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-metadata-field-ownership/acceptance.json`.

## Direct Rust VM opcode-handler checkpoint

Twelve constant/end-parameter/stop opcodes now register direct checked Rust
function pointers in the existing Rust-owned `GNatives` and static-name lookup.
Their C++ member declarations, bodies and typed invocation adapters are removed.
The established exported handler-data names and bootstrap order are preserved;
there is no second registry, emulated vtable, member shim or extra allocation.
The existing `vm_flow` owner shares one services-free implementation between
the old service-dependent facade and new guarded entries. The remaining dispatch
match is exhaustive, not a silent wildcard fallback.

The additional genuine SDK passes **5,000 checks**, covering **12 opcodes**,
**1,065 calls** over frame-step/canonical/name-lookup routes, **1,053 scalar
cases**, 621 bytecode aliases, 12 cursor aliases (nine ordering cases), 648
unaligned operands, 576 unaligned results and 130,809 neighbor bytes. All
**thirteen frozen genuine SDK sources** match original exit/stdout/stderr.
The first original SDK build/run passed before replacement; no retry-to-green.
The isolated two-file patch-capture failure is recorded; recovery checked exact
unchanged original live hashes before copying the scoped retained Rust sources.

A separate **Rust-only executable passes 96 checks across all twelve actual
checked exports under data-none**, with only Rust archives as explicit link
inputs and only system `libiconv`/`libSystem` dynamic dependencies. This proves
the opcode subset, **not** native-free package loading, class/instance ownership,
property serialization, script parameters/out values or GC.

All **32 focused CTests pass in 60.49 seconds**: 30 `data-none`, two
`data-prototype`. Normal SDK-free configure/build and ABI `--check` pass.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 in **18.657306
seconds**, zero failure markers and **zero captured frames**. Fresh **167/167
compiler scans** retain **373 native files = 165 sources + 208 headers**,
shipping **369 = 164 + 205**, zero SDK rows and **zero file/family closures**.
M1 remains open; no foreground visual, retail, Linux, sanitizer, full-suite or
feature-gate promotion claim.

At that checkpoint the nonblank source footprint was **69.8% Rust / 30.2% native**,
545 Rust files / 217,084 lines versus 370 native files / 94,039 lines.
Generated code, tests, external dependencies, `build.rs` and every temporary
`out/` SDK are excluded; comments count. This is not effort completion.
Exact commands, source/link hashes, baseline/differential output and limits:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-plain-vm-handlers/acceptance.json`.

## Rust-owned scalar VM argument checkpoint

All **53 by-value bool/int/float/comparison natives** now register direct
checked Rust handlers in the existing canonical `GNatives` and static-name map.
Their C++ member declarations, bodies and invocation adapters are deleted,
along with four obsolete arithmetic-only C exports. Assignment/reference,
short-circuit and RNG handlers retain their existing contracts.

Rust steps every zero-initialized fixed argument, including omitted operands,
reloads the live frame object and native-table entry at each step, preserves
earlier argument-slot mutations through later/debug callbacks, and implements
the native `P_FINISH` sequence with this engine's **`0x38` debug token**.
The first nonzero status preserves its cursor/state prefix and prevents result
publication. No frame/argument references span callbacks; argument storage is
fixed stack storage, with no per-call heap allocation or second registry.

The added genuine SDK passes **128,210 checks** over **5,577 cases**:
1,859 on each of three routes, 450 typed first-error cases, 435 omitted-parameter
cases, 1,344 nested executions and 923,544 guard bytes. All **fourteen unchanged
SDK sources** match original exit/stdout/stderr exactly. The initial original
fixture failure is preserved: its expectation for `(6*7)+(10-2)` was incorrectly
44 and was corrected to 50 before freezing the original baseline. Production
sources remained unchanged during that correction.

A separate **Rust-only bytecode executable passes 488 checks under data-none**:
53 actual scalar entries plus twelve registered constant/flow handlers, two
nested expressions, four omitted-parameter cases and two first-error cases.
Its explicit link inputs are Rust archives; `otool -L` lists only system
`libiconv`/`libSystem`. It does not prove native-free package loading, object/
class/property lifetimes, out/reference values or GC.

All **33 focused CTests pass in 58.12 seconds**: 31 `data-none`, two
`data-prototype`. Normal SDK-hook-free configure/build and ABI `--check` pass.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 in
**18.797469 seconds**, zero failure markers and **zero captured frames**.
Fresh **167/167 compiler scans** retain **373 native files = 165 sources +
208 headers**, shipping **369 = 164 + 205**, zero SDK rows and **zero
file/family closures**. M1 remains open. No foreground visual, retail, Linux,
sanitizer, full-suite or feature-gate promotion claim.

Built first-party source footprint at that scalar checkpoint:

| Language | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 545 | 218,132 | 70.0% |
| Native | 370 | 93,571 | 30.0% |

Generated code, tests, external dependencies, `build.rs` and temporary `out/`
SDKs are excluded; comments count. This is **source footprint, not estimated
effort or full-port completion**. Generated native files remain in the
compiler-membership count above. Exact commands, source/link hashes, frozen
baselines, differential outputs, preserved failure and limits:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-scalar-vm-handlers/acceptance.json`.

## Rust-owned vector, rotator and extended-native VM checkpoint

Another **40 checked Rust handlers** replace native members and adapters:
two constants, fourteen by-value vector operations, eight by-value rotator
operations and all sixteen extended-native prefixes. Three obsolete arithmetic/
index-only C exports are deleted. Canonical registration and native data names
remain unchanged; reference/assignment, RNG and trig-table operations remain
outside this cutover.

Fixed stack argument slots stay live through later/debug callbacks. Float slots
start at zero; required vector/rotator slots use `MaybeUninit`, matching the
actual empty native constructors. Their evaluators must fully initialize them
before a successful read. The shared scalar/vector argument step reloads live
`Frame.Object` and the native table; finishing preserves the real `0x38` debug
sequence and first-error prefix without publishing a failed result. Constants
preserve sequential component reads/stores and source/cursor aliases. Extended
dispatch consumes the low operand before reading the live table and forwards
the incoming context, not `Frame.Object`.

The new genuine SDK passes **654,382 checks over 16,407 records**, including
all **4,096 extended indices on three routes**, 1,146 typed first-error cases,
174 omitted-parameter cases and 8,021,280 guard bytes. All **fifteen unchanged
SDK consumers** match original exit/stdout/stderr. The original fixture exposed
its own incorrect assumption that native struct locals started at zero.
That failure and source are preserved. Before baseline freeze, the observer was
corrected to supply explicit struct bytes for omitted required operands; only
omitted floats are checked for native zero initialization. Production code was
unchanged during this fixture correction.

The actual **Rust-only composite executable passes 1,189 checks, data-none**:
the prior scalar block's 488 plus 701 vector/extended checks. It exercises all
24 new value/constant entries, nested expressions, omitted floats, first errors
and all sixteen high prefixes. Explicit inputs are Rust archives; actual dynamic
dependencies are only system `libiconv` and `libSystem`. This is not native-free
package/object/property/GC ownership proof.

Focused CTests pass **33/33 in 55.82 seconds plus `vm_vector_contracts` 1/1 in
2.61 seconds**: 34 unique targets, 32 `data-none` and two `data-prototype`.
Normal SDK-hook-free configure/build and ABI `--check` pass. Installed WGPU
**Adv8Forest, 120 ticks, data-prototype** exits 0 in **18.956486 seconds**, zero
failure markers and **zero captured frames**. Fresh **167/167 compiler scans**
retain **373 native files = 165 sources + 208 headers**, shipping **369 =
164 + 205**, zero SDK rows and **zero native file/family closures**.

Built first-party source footprint at that vector/extended checkpoint:

| Language | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 545 | 218,925 | 70.1% |
| Native | 370 | 93,346 | 29.9% |

Generated code, tests, external dependencies, `build.rs` and temporary `out/`
SDKs are excluded; comments count. This is **source footprint, not estimated
effort or full-port completion**. M1 remains open. No foreground visual, retail,
Linux, sanitizer, full-suite or feature-gate promotion claim. A reflected-events
NDJSON metadata-collector failure and the focused-test selection correction
are preserved separately from successful runtime results. Exact commands,
source/link hashes, original fixture failure, all differential outputs and limits:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-vector-extended-vm-handlers/acceptance.json`.

## Single-expression Rust VM conversion checkpoint

Another **17 checked Rust handlers** replace native member declarations, bodies
and invocation adapters: twelve byte/int/bool/float conversions, object/name/
vector/rotator truth and vector-to-rotator conversion. Canonical registration
and data names remain unchanged. Existing generic conversion/cast exports still
serve remaining typed/string/trig operations; arithmetic kernels are shared,
not duplicated or replaced with dummy services.

Each handler owns a stable, explicitly zero/null/`NAME_NONE` initialized slot,
steps exactly one expression through live `Frame.Object` and the canonical table,
then publishes only its actual one-, four- or twelve-byte result on success.
These vector/rotator conversion locals explicitly initialize zero, unlike the
previous by-value arithmetic locals. Partial writers preserve untouched bytes.
There is **no `P_FINISH`, terminator or debug consumption**. Operand errors
preserve the exact status and callback effects without publishing a result;
no frame/table/slot references span callbacks or per-call heap allocation.

The new genuine SDK passes **82,064 checks over 1,830 records**, with 576 root
calls on each of three routes, 102 typed first-error cases, 51 omitted operands,
231 partial writers, 105 nested expressions, 435 object-reload checks, 102 live
mutations and 605,670 guard bytes. All **sixteen unchanged SDK consumers**
match original exit/stdout/stderr. The original new SDK passed on its first run,
with all six captured production/test sources unchanged before release.

The actual **Rust-only composite passes 1,669 checks, data-none**: prior blocks
488 + 701, plus 480 checks across 22 new literal cases, all 17 omissions and
first errors, three nested expressions and one opaque Rust-token truth case.
Explicit inputs are Rust archives; actual dynamic dependencies are only system
`libiconv` and `libSystem`. The initial standalone fixture failed because its
subtraction bytecode used opcode 173 (remainder), not 175 (subtraction).
That source and failed execution are preserved; only the opcode was corrected,
keeping the expected 255 and all production code/assertions unchanged.

All **35 focused CTests pass in 87.92 seconds**: 33 `data-none`, two
`data-prototype`, including four new isolated conversion-transition regressions.
Normal SDK-hook-free configure/build and ABI `--check` pass. Installed WGPU
**Adv8Forest, 120 ticks, data-prototype** exits 0 in **10.663823 seconds**, zero
failure markers and **zero captures**. Fresh **167/167 attributable compiler
scans** retain **373 native files = 165 sources + 208 headers**, shipping **369 =
164 + 205**, zero SDK rows and **zero file/family closures**. An initial
synchronous-helper/async collector error is preserved separately; its lost
statuses are not acceptance evidence. The original installed run was retained,
not repeated for that metadata correction.

Built first-party source footprint at this conversion checkpoint:

| Language | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 545 | 219,509 | 70.2% |
| Native | 370 | 93,210 | 29.8% |

Generated code, tests, external dependencies, `build.rs` and temporary `out/`
SDKs are excluded; comments count. This is **source footprint, not estimated
effort or full-port completion**. M1 and native package/object/class/property/GC
ownership remain open. No foreground visual, retail, Linux, sanitizer, full-suite
or gate promotion claim. Exact commands, hashes, frozen baselines, all
differentials, preserved fixture/collector failures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-primitive-conversion-vm-handlers/acceptance.json`.

## Rust object-storage header ownership checkpoint

**Ownership change, not a second registry:** `object_storage.rs` remains the
sole slot/free-index/hash owner. It now directly reads compiler-probed
`UObject.name`, `outer`, `index` and `hash_next`, publishes indices before hashing,
and repairs previous/head links during unlink. Header operations use raw field
access, not a whole-object read, fabricated vtable or padding access.
`Hp2ObjectStorageServices` retains only the existing checked diagnostic callback;
four primitive-field callbacks, their native access struct and its private
friend are removed. The hash export no longer takes an unused services argument.
No per-operation allocation, shadow object model or production compatibility shim
was introduced. Native allocation, typed constructors/destructors, virtual hooks
and checked diagnostics remain.

The new genuine `object-storage-header` SDK's **first original execution exits
0**: **65,656 checks**, 8,208 created / 8,207 destroyed objects, three diagnostic
errors, two reentrant destructions and three preserved unhash links. It exercises
real `UTextBuffer` objects, collisions, stable indices through growth, replacement,
live LIFO mutations and exact first-error prefixes under `data-prototype`.
The unchanged source matches exactly after cutover, as do all 16 earlier genuine
SDKs: **17/17 exact differentials**. An initial SDK build failure from missing
established fixture-only private-access settings is preserved; only the new
fixture's CMake settings were corrected before any production change or SDK run.

An actual Rust-only executable imports the production FFI module and uses its
canonical global storage, not a second local registry. It passes **149,598 total
checks**: 1,669 earlier VM checks and 147,929 storage checks, including 123,288
guard bytes, 2,050 growth headers, three first-error diagnostics, two reentrant
removals and three intrusive unlinks. First compile/run exit 0. Explicit inputs
are Rust archives; `otool -L` lists only `libiconv.2.dylib` and `libSystem.B.dylib`.
These raw opaque headers prove data operations, not C++ object/vtable/lifetime,
package loading or GC ownership.

The initial focused CTest run exits 8: **34/35 pass in 56.94 seconds**; only
`object_storage_contracts` fails to compile because one migrated fixture access
still used `.index` instead of `.header.index`. That access alone was corrected,
retaining the same expected value and assertion. Only the failed test was rerun:
**1/1 passes in 0.89 seconds**. Thus **35 unique focused tests pass**:
33 `data-none`, two `data-prototype`; the initial failure remains recorded.
Normal SDK-hook-free configure/build and ABI `--check` exit 0.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype**, exits 0 in
**19.288873 seconds**, zero failure markers/budget violations and **zero captures**.
Fresh compiler evidence is **167/167 successful scans**, no SDK compile rows:
373 native files (165 sources + 208 headers), shipping 369 (164 + 205), unchanged
membership and **zero file/family closures**.

Built authored source footprint at this storage checkpoint, excluding generated
code, tests, external dependencies, `build.rs` and temporary `out/` SDKs; comments count:

| Language | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 545 | 219,505 | 70.2% |
| Native | 370 | 93,194 | 29.8% |

This is **source footprint, not estimated effort or full-port completion**.
M1 and native package/class/object/property/GC ownership remain open. No
malformed-cycle, invalid-pointer, foreground visual, retail, Linux, sanitizer,
full-suite or feature-gate promotion claim. Exact commands, source/link hashes,
frozen outputs, all differentials, preserved failures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-object-storage-header-ownership/acceptance.json`.

## Parallel metadata archive and commandlet ABI checkpoint

Two independent implementation owners worked concurrently with exclusive files:
one extended the compiler-probe generator; the other moved metadata archive
field selection into the existing Rust owner. Only the generator owner edited
the manifest. A single integration owner generated outputs, configured/built,
ran consumers/contracts and exercised the installed game after both completed.
No concurrent builds or competing generated-header writers were used.

`class_metadata.rs` now chooses live raw field addresses and exact serialization
order for fields, structs, states, classes and functions, including legacy
width/version branches. Each typed operation checks status before the next
field. No Rust reference spans archive reentry. The native callback receives
only archive, value type and address; it retains actual typed reference/name,
scalar, GUID and array archive operators. The obsolete native part dispatcher,
part enum and redundant integer service are removed. The service table has
22 pointer slots. No second metadata/object/name owner was introduced.

The compiler-derived `UCommandlet` mirror covers its four `FString` members,
two 16-element `FStringNoInit` arrays and seven flag bits, preserving opaque
compiler-owned bytes. It does not supply a Rust constructor/destructor, native
vtable or complete package/class backend.

The genuine existing-class oracle loads actual version-79 `Core.u`, export 718
`Core.HelloWorldCommandlet`, constructs its instance and passes **114 checks**:
four observable script log branches, real indexed-native `GetAxes` out vectors,
property wire round-trip, archive failure prefix `[19,-2,-3]`, and GC slot/hash
removal. Its initial fixture incorrectly called the indexed native through
`ProcessEvent`; finite seeded outputs were not execution proof. That finding
and both earlier fixture compile failures remain recorded. The corrected frozen
consumer uses real `CallFunction`/`FFrame`, canonical four-byte object-slot
references, parameter-cursor and unit-vector checks. It passes before and after
production changes; it is still a mixed-runtime oracle, not M1 acceptance.

Integrated evidence: **18/18 unchanged SDK differentials**, exact exit/stdout/
stderr, and **35/35 focused CTests**, exit 0 in **80.70 seconds** (33 `data-none`,
two `data-prototype`). The initial batch build failed because the new leaf used
`WORD` instead of the engine's portable `_WORD`; only that type spelling changed.
The failure remains preserved. Compiler ABI generation/check and normal
SDK-hook-free verification/app builds exit 0.

Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 in
**19.143538 seconds**, zero budget violations and **zero captures**.
Fresh **167/167 compiler scans** retain 165 native sources + 208 headers,
shipping 164 + 205; no SDK compile rows, membership changes or file/family
closures. Current authored footprint, with the exclusions above:

| Language | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 545 | 219,518 | 70.2% |
| Native | 370 | 93,172 | 29.8% |

Source footprint is not effort/completion. M1/full port and native package/class/
object/property/GC ownership remain open. No foreground visual, retail, Linux,
sanitizer, full-suite or feature-gate promotion claim. Exact commands, hashes,
parallel ownership, preserved failures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-metadata-wire-and-commandlet-abi/acceptance.json`.

## Parallel canonical resources and direct VM handlers checkpoint

Ten workers completed five paired batches with exclusive implementation scopes.
The parent integrated shared native edits, generated compiler ABI output and ran
all builds and verification after worker edits settled; no competing builds or
generated-header writers were used.

Rust now owns the actual intrinsic-name catalog and name storage, VM property/
address slots, compiler-probed linker-record constructors and FArchive scalar
initialization, all former GMath table/constant storage, the shared 128-slot
conversion ring and published array-allocator resource. Direct checked GetAxes/
GetUnAxes and ANSI/Unicode literal handlers use these canonical owners. The old
native storage and replaced handler bodies are removed, not retained as fallbacks.
Archive scalar initialization writes only real fields: vptr, padding and borrowed
secondary-base tail padding remain untouched.

Concrete OS-file and borrowed-memory archive resources execute existing Rust
archive/string/array/linker policy. The real production hp2-ffi Rust library loads
Core.u through canonical names and linker maps: all 332 names, 16 imports and 720
exports exactly match the mixed native SDK oracle. This executable links OS
libraries/frameworks, not the C++ engine. It reads package tables; it does not yet
resolve/load native classes or implement the full object/property/GC backend.

Integrated macOS arm64 evidence: 20/20 frozen SDK output comparisons and 43/43
focused CTests, exit 0 in 75.63 seconds (41 data-none, two data-prototype).
Production Rust-only resource execution covers allocator publication and paired
release, initialized/empty string replacement, unsigned ANSI, unaligned UTF-16
surrogates/replacement, 1023-character truncation with full operand consumption,
ring wrap and raw archive guards. Ring-edge expectations are source/contract
checks, not an executed original ring oracle. Real file and memory consumers
also exercise truncated-read prefixes and sticky failures.

Normal SDK-hook-free verification/app builds pass. Installed WGPU Adv8Forest
completes 120 ticks under data-prototype in 19.180724 seconds, exit 0, zero
budget violations and zero captures. Fresh 167/167 compiler scans retain
165 native sources + 208 headers; shipping retains 164 + 205. No SDK compile
rows, source/header membership changes, classification promotions or native
file/family closures. Authored footprint uses the established exclusions:

| Language | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 552 | 221,489 | 70.4% |
| Native | 370 | 92,922 | 29.6% |

This is source footprint, not effort or completion. M1/full port remain open.
Preserved failures include compiler-probe/constant prerequisites, cbindgen
exposure of a private OS declaration, manual rustc dependency/panic settings,
stdio-cache assumptions and the missing native checked-token declarations.
No foreground visual, retail, Linux, sanitizer, full-suite or feature-gate
promotion claim. Commands, hashes, exact output comparisons and limitations:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-canonical-linker-records-and-vm-lvalues/acceptance.json`.

## Parallel checked catalog and string-argument checkpoint

The sixth pair separated catalog/native-boundary ownership from string execution
and lifetime policy. One worker owned shared C++ edits; the parent alone integrated
modules, generated ABI output, built and verified after both workers finished.
No simultaneous generated-header writers or mid-edit builds were used.

One Rust catalog now owns all 145 checked opcode entries and immutable named-token
cells. Bootstrap seeds it before external registration, once only; numeric
overrides remain live without rewriting named bindings. Symbolic opcodes are
compiler-probed from that same catalog. Normal native builds check generated ABI
freshness after Cargo, with compiler/header/catalog/configuration dependencies.

The 17 string operators and EatString now evaluate owning arguments directly
through checked Rust dispatch and the canonical allocator. Mid preserves the
65535 omitted-count default; finish/debug ordering, initialized result replacement,
reverse cleanup and first-error precedence are preserved. The actual Nothing
handler also moved, removing the remaining native dependency for omitted counts.
Replaced C++ handlers, declarations and checked registration/name lists are deleted.

Observed macOS arm64 evidence:

- All 21 unchanged SDK consumers match frozen exit/stdout/stderr and source hashes.
  The new pre-cutover native oracle contributes 109 actual string cases, including
  reflected-property result aliases and counted embedded NULs.
- Production Rust-only execution matches the 105 literal/optional/discard rows
  exactly. The four reflected-property alias rows are not native-free evidence.
  First external registration, repeated initialization, numeric replacement,
  immutable named binding and undefined-error propagation also pass.
- All 44 focused CTests pass in 90.12 seconds: 42 data-none, two data-prototype.
  Actual Core.u maps still match all 332 names, 16 imports and 720 exports.
- SDK-hook-free verification/app builds pass. Installed WGPU Adv8Forest completes
  120 data-prototype ticks in 19.663131 seconds, exit 0, no budget violations,
  no failure markers and zero captures.
- Fresh normal-build dependencies pass 167/167 scans with zero SDK rows and no
  membership changes: 165 native sources + 208 headers; shipping 164 + 205.

| Authored source | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 553 | 222,039 | 70.6% |
| Native | 370 | 92,298 | 29.4% |

Source footprint is not remaining effort. No classification promotion, native
file/family closure, M1 completion, foreground visual, retail, Linux, sanitizer,
full-suite or feature-gate promotion claim. The initial missing generated-header
include failure is preserved alongside its concrete fix. Native class/object/
property/GC backend integration remains the next shared prerequisite; additional
workers need disjoint ownership and fixed interfaces, not competing edits there.
Exact commands, frozen differentials, source/link hashes and limitations:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-checked-catalog-and-string-arguments/acceptance.json`.

## Parallel text-conversion and identity checkpoint

**Invariant:** the checked Rust handlers preserve native argument evaluation,
cursor movement, result widths, localized boolean text and ownership, using the
same live canonical names, object table, allocator and ANSI conversion ring.

The seventh pair kept all shared native/catalog edits under one worker and
primitive conversion/resource policy under the other. Parent integration moved
ObjectConst's remaining native table callback to the actual Rust object owner;
no shadow registry or fabricated C++ vtable was introduced. Replaced native
methods, registrations, declarations and obsolete service fields were deleted.
The sole checked catalog now contains 158 entries. Rust-only bootstrap exposes
the actual OS-backed ANSI services; mixed startup publishes the original
exception-preserving native leaves. GTrue/GFalse retain their exact mutable
64-TCHAR storage and exported symbols, including native localization writes.

Observed macOS arm64 acceptance:

- Before edits, a genuine native consumer produced 246 text/identity records,
  including localized/non-ASCII booleans, scalar bounds, raw float edge values,
  long/Unicode strings, result guards and different terminator rules.
- All 22 unchanged mixed SDK sources and frozen exit/stdout/stderr match.
- A production Rust-only process matches all 246 records, verifies parser
  resource preflight without frame/result mutation, and uses the sole canonical
  object table with Rust-owned compiler-probed header fixtures. Those fixtures
  do not claim package-backed class loading or completion of the class backend.
- The unchanged 105-case Rust-only string consumer and actual package/resource
  consumers still match. Core.u retains all 332 names, 16 imports and 720 exports.
- `ctest --preset macos-arm64 -R <44-target-regex> --output-on-failure`
  passes 44/44 in 136.14 seconds; exact command and JUnit are in the evidence.
- Hook-free normal verification/app builds pass. Installed WGPU Adv8Forest
  completes 120 data-prototype ticks in 24.311598 seconds, exit 0, no failure
  markers/budget violations, zero captures. No retail data was exercised.
- Fresh normal-build dependency scans pass 167/167 with zero SDK rows.
  Membership/classifications remain unchanged; no native file/family closure.

| Authored source | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 553 | 222,494 | 70.7% |
| Native | 370 | 92,153 | 29.3% |

Source share is not remaining effort. M1/full Rust completion remain open.
No foreground visual, retail, Linux, sanitizer, full-suite or gate-promotion
claim. The mistyped SDK build target and Rust-only consumer's signed-TCHAR
compile error are retained with their concrete fixes; neither is hidden as a
passing first attempt. Exact commands, hashes and observations:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-text-conversions-and-identity/acceptance.json`.

## Parallel name-conversion and reference-mutation checkpoint

**Invariant:** the checked Rust handlers capture the native lvalue before RHS
evaluation, preserve reentry, finish/debug ordering, result widths/aliases,
cursor movement and string ownership, and use the canonical names/object table.

Three workers split name/object conversion plus all shared C++/catalog edits,
scalar mutations and vector/rotator mutations. The parent regenerated ABI output
and built only after all three finished. Name services are published after
successful static initialization and cleared only by successful shutdown; object
paths reuse the existing identity orchestration. Scalar short-circuit operators
keep all 32 boolean bits and skip without P_FINISH. Obsolete conversion/scalar/
vector callbacks and exports are deleted; the catalog now has 192 entries.

Observed macOS arm64 acceptance:

- Before edits, a genuine native consumer produced 726 records. After cutover,
  its unchanged replay matches 696 exactly and differs in 28. All 28 come from
  30 reflected vector/rotator RHS rows whose fixture struct had no component
  fields; struct copy only copies fields, so those native rows hold
  uninitialized bytes. Production was not changed to match.
- A corrected fixture with typed X/Y/Z and Pitch/Yaw/Roll fields passes 726/726
  internal checks. For the 30 affected rows, expected values were derived from
  the native literal-mode rows; no native build ran the corrected fixture.
  Building it at HEAD `81f00fd` failed (the shared consumer body needs later
  APIs). Those 30 rows remain unresolved native evidence.
- Production Rust-only execution matches all 192 native name-conversion and
  literal/temp mutation records. The first compile used nonexistent ABI constants
  for vector/rotator literals; it is preserved alongside the fix.
- All 22 earlier SDK consumers and four production Rust-only consumers match
  exactly. The FFI name-provider lifecycle test passes.
- After the final hook-free rebuild and reinstall:
  `ctest --preset macos-arm64 -R <44-target-regex> --output-on-failure`
  passes 44/44 in 15.01 seconds. Bundle checks pass. Installed WGPU Adv8Forest
  completes 120 data-prototype ticks in 18.57 seconds, exit 0, with no failure
  markers, budget violations or captures. No retail data was used.
- Fresh dependency scans pass 167/167 with zero SDK rows. Reselecting all
  runtime-target native TUs gives the same 167 rows in both directions:
  165 native sources + 208 headers; shipping 164 + 205.

| Authored source | Files | Nonblank lines | Share |
|---|---:|---:|---:|
| Rust | 553 | 223,250 | 70.9% |
| Native | 370 | 91,715 | 29.1% |

The 30 typed reflected aggregate rows, other reflected-property lvalues and
short-circuit local side effects are not native-free or fully native-verified.
Source share is not remaining effort; M1/full Rust completion
remain open. No foreground visual, retail, Linux, sanitizer, full-suite or gate-
promotion claim. Exact commands, records and limitations:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-name-conversions-and-reference-mutations/acceptance.json`.

## Geometry conversion checkpoint

Moved to the Rust checked catalog: `StringToVector` (77), `StringToRotator`
(78), `VectorToString` (88), `RotatorToVector` (58), `RotatorToString` (89),
`LessLess_VectorRotator` (275), `GreaterGreater_VectorRotator` (276),
`OrthoRotation` (1030) and `Invert` (227). The nine C++ bodies, `IMPLEMENT_FUNCTION`,
`DECLARE_FUNCTION` and `MAP_NATIVE` rows are deleted, along with
`VmConvertFormat`. Formatting now uses the Rust `%f`/`%i` formatters. The dead
`vm_convert::convert`/`Hp2VmConversion`/`hp2_rs_core_vm_convert` path is removed;
`Hp2VmConvertServices` keeps only `is_a`/`is_class`/`is_child` (24 bytes,
compiler-checked `static_assert`).

- Before edits, a new genuine native consumer produced 126 records plus one
  native exception (out-of-range `StringToRotator`). Two runs were identical.
- After cutover, all 127 records are identical: values, cursors, `Invert`
  lvalue writes, guard bytes and the exception.
- 21 earlier SDK consumers and the typed 726-case mutation consumer still match
  exactly.
- After the final hook-free rebuild/reinstall: 44/44 focused CTests and 5/5 VM
  contract CTests pass; `check_bundle.py` passes; installed WGPU Adv8Forest
  completes 120 data-prototype ticks in 18.54 s with zero failure markers.
- Fresh compiler scans pass 167/167: 165 native sources + 208 headers.
  Authored footprint: Rust 554 files / 223,790 nonblank lines (71.0%), native
  370 / 91,583.
- Rust-only consumer (`GeometryRustConsumer.rs`, recorded after the next batch's
  cutover; geometry handlers unchanged): 121/121 records equal native, excluding
  the six `Invert` rows that need reflected struct-property locals.

No retail, Linux, sanitizer, full-suite, foreground visual or gate-promotion
claim; M1 remains open. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-geometry-conversions/acceptance.json`.

## Type-query and random checkpoint

Moved to the Rust checked catalog: `DynamicCast`, `MetaCast`, `ClassIsChildOf`
(258), `IsA` (303), `Rand` (167), `FRand` (195), `VRand` (252), `RotRand` (320)
and `RandRange` (1033). The nine C++ bodies, `IMPLEMENT_FUNCTION`,
`DECLARE_FUNCTION` and `MAP_NATIVE` rows are deleted, along with
`FVmScalarRandContext`/`VmScalarScopedRand` and seven Rust exports whose last C++
caller is gone. C++ publishes the frame-init, convert, scalar-rand, vector-rand,
object and ANSI-trace service tables once through static initializers, using the
existing published-services pattern. The catalog handlers read those tables and
open the Rust RNG native trace scope directly.

- The oracle was corrected before freezing: fixed libc seed, code placed inside
  `Main->Script` with per-case padding (script-relative offsets 0–22), and NULL
  references encoded as `INDEX_NONE`. With those fixes, two native runs agree,
  and trace-off stdout equals trace-on stdout.
- After cutover: all 252 records are identical (values, cursors, guard bytes,
  next `appRand`, logs), and the 322-draw RNG trace is byte-identical. That
  includes the native attribution and callsite offsets.
- Geometry oracle 127/127, 21 SDK consumers, the 726-case typed mutation
  consumer and four prior Rust-only consumers are unchanged.
- On the final hook-free binaries: 44/44 focused, 7/7 VM contract and
  `rng_seed_contracts` CTests pass; `check_bundle.py` passes; installed WGPU
  Adv8Forest completes 120 data-prototype ticks in 18.89 s with zero failure
  markers.
- Compiler scans 167/167: 165 native sources + 208 headers. Footprint: Rust 554
  files / 224,676 nonblank lines (71.1%), native 370 / 91,478.

There is no Rust-only consumer: these handlers still need C++-published leaf
services. `StructCmpEq/Ne` is deferred to the VmVariable family. No retail,
Linux, sanitizer, full-suite, foreground visual or gate-promotion claim; M1
remains open. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-type-queries-and-random/acceptance.json`.

## Object-service checkpoint

Ten numbered natives (`New` 17, `GotoState` 113, `Enable` 117, `Disable` 118,
`IsInState` 281, `GetStateName` 284, `Log` 231, `Warn` 232, `SaveConfig` 536,
`ResetConfig` 543) moved to the checked catalog. Seven `INDEX_NONE` natives
(`Localize`, `GetPropertyText`, `SetPropertyText`, `StaticSaveConfig`, `GetEnum`,
`GetLanguage`, `DynamicLoadObject`) moved to a new `named_natives!` list: exported
`intUObjectexec<Name>` tokens that `FindNative` resolves without a `GNatives`
slot. The 17 C++ bodies and their registrations, declarations and `MAP_NATIVE`
rows are deleted, along with eight orphaned FFI trampolines. The remaining C++
side is leaf services (object construction, config, localization, `debugf`,
`FFrame::Logf`, `GetDefaultObject`), published once through static initializers.

- Oracle design: a real `Engine.Console` with `InitExecution` (as the engine's
  console services do), so the state natives also run its
  `BeginState`/`EndState` script. Config natives run on the `Engine.Player`
  class default; both memory and `GConfig` are recorded after each case. Named
  natives are invoked through `UFunction::Func`. `ScriptWarning`/`Localization`
  are unsuppressed, so warning texts and their script offsets are recorded. A
  `new` with a NULL class crashes natively and is excluded. Two native runs
  agree, and the repo `System/` data is untouched.
- Two review deviations were fixed before replay: `StaticSaveConfig` now goes
  through `GetDefaultObject()` and keeps its check, and `Warn` goes through
  `FFrame::Logf` and keeps `FOutputDevice::Logf` suppression/truncation.
- After cutover: all 153 records are byte-identical. This includes the `Disable`
  warning, which says "Enable:", and `Enable` warning before its end-parms token.
- Unchanged: geometry, type/random (including the RNG trace), 21 SDK, typed
  mutation and five Rust-only consumers.
- On the final hook-free binaries: 44/44 focused and 9/9 VM/frame/RNG contract
  CTests pass; `check_bundle.py` passes; installed WGPU Adv8Forest completes 120
  data-prototype ticks in 18.99 s with zero failure markers.
- Compiler scans 167/167. Footprint: Rust 554 files / 225,548 nonblank lines
  (71.2%), native 370 / 91,305.

There is no Rust-only consumer: the handlers still call C++ leaves. No retail,
Linux, sanitizer, full-suite, foreground visual or gate-promotion claim; M1
remains open. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-object-services/acceptance.json`.

## Script-VM checkpoint

The remaining 27 `UObject` interpreter natives moved to the checked catalog via
`native_opcode!` rows: `LocalVariable`, `InstanceVariable`, `DefaultVariable`,
`ClassContext`, `ArrayElement`, `DebugInfo`, `DynArrayElement`,
`DynArrayCount`, `BoolVariable`, `StructMember`, `NativeParm`, `Switch`, `Case`,
`Jump`, `JumpIfNot`, `Assert`, `GotoLabel`, `Let`, `LetBool`, `Self`, `Context`,
`VirtualFunction`, `FinalFunction`, `GlobalFunction`, `StructCmpEq`,
`StructCmpNe` and `Iterator`. Their C++ bodies, registrations, declarations,
`MAP_NATIVE` rows and the `VmVariableExecute` shell are deleted. C++ now
publishes the variable, flow and new function-call leaf tables once.

- Guard history: the build has `DO_GUARD=1` and `DO_GUARD_SLOW=0`, so only
  `ClassContext`, `ArrayElement` and `StructMember` ever added unwind history.
  Their Rust handlers call the `VmUnwindGuard` leaf on the error path. It emits
  the original `guard` name and the `unguardexec` object/node/offset line, and
  installs its own `__Context`.
- `VirtualFunction`, `FinalFunction` and `GlobalFunction` are Rust handlers over
  whole-call C++ leaves. Each leaf runs `FindFunctionChecked`, the
  `FAppRandTracePreBeginRelevanceScope` RAII scope (virtual only) and virtual
  `CallFunction`, so destructor ordering is unchanged. `CallFunction` itself is
  still C++ (M1).
- Oracle design: real compiled script driven through `ProcessEvent` (Console,
  UWindow HTML text area, `UWindowBase` statics, combo-list sorting). For
  opcodes no level-free script reaches, single hand-assembled tokens are stepped
  through `FFrame::Step` with real function locals: default variables, class
  context, `Case` fallthrough, `GotoLabel` on `WindowConsole`'s `UWindow` state,
  `DebugInfo`, dynamic arrays on `Engine.Gesture` and a dynamic-array parameter,
  `NativeParm`, struct compare/member, and hand-encoded virtual/final/global
  calls. A missing-function error propagating through each handler records
  `GErrorHist`. A diagnostic `GNatives` counter confirmed all 27 opcodes are
  hit. Pointer-holding properties (`CPF_Native`, `Core.Object` internals,
  `vtbl*`) are excluded from snapshots because they change with every run or
  link. Two native runs agree byte-for-byte.
- Fixed before freezing: `VmFlowMissingLabel` passed an `FName` by value to
  `%s`, an original UE1 quirk (`Stack.Logf(..., N)`). On arm64 it dereferenced
  the name index and crashed. It now passes `*FName`, matching the sibling
  `GotoState` message.
- `Tests/AbiTests.cpp` now invokes the null-context probe through
  `GNatives[EX_Context]`. `Build/gesture_fixture_harness/` builds against pinned
  ref `5782e7f` and is unaffected.
- After cutover: all 209 records, the stderr and the 209-draw RNG trace are
  byte-identical. The 153-case object-service oracle also replays identically.
- Unchanged: geometry, type/random (including the RNG trace), 21 SDK, typed
  mutation and five Rust-only consumers.
- On the final hook-free binaries: 44/44 focused and 9/9 VM/frame/RNG contract
  CTests pass; `cargo test -p hp2-core` and `cargo test -p hp2-ffi --lib` pass;
  `check_bundle.py` passes; installed WGPU Adv8Forest completes 120
  data-prototype ticks in 19.18 s with zero failure markers.
- Compiler scans 167/167. Footprint: Rust 556 files / 227,892 nonblank lines
  (71.4%), native 370 / 91,099.

The handlers still call C++ leaves, and the object model, `CallFunction`,
`ProcessEvent` and frame stepping stay C++-owned. No retail, Linux, sanitizer,
full-suite, foreground visual or gate-promotion claim; M1 remains open.
Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-script-vm/acceptance.json`.

## Engine-native catalog checkpoint

All 175 Engine-package natives (173 rows plus `UCommandlet::execMain` and
`UObject::execUndefined`) are Rust handlers in
`crates/hp2-ffi/src/engine_natives_*.rs`, concatenated by
`engine_native_catalog::all()`. Every opcode matches
`registrations.json`. The C++ exec bodies, `DECLARE_FUNCTION`/
`AUTOGENERATE_FUNCTION`/`MAP_NATIVE` rows, `UnEngineNative.cpp`, the Engine
`DECLARE_NATIVE_TYPE`/`IMPLEMENT_NATIVE_HANDLER` lookups and the
`HP2StaticPackages.cpp` Engine cases are deleted. C++ keeps only leaf service
tables, published once.

- Argument stepping (`P_GET_*`/`P_FINISH`) is in Rust (`vm_frame::step_argument`/
  `finish_arguments`). Latent pollers (`execPoll*`, `AI_PollWaitForLanding`)
  step nothing, as in the originals. Natives in a section's `non_slow_guard`
  list call `unwind_guard` on the error path.
- The service structs introduced here (`Hp2PawnNativeFilesServices`,
  `Hp2ScriptAnimLogServices`, the five `Hp2Script*Services` console tables)
  are emitted into `hp2_rs.h` through `cbindgen.toml` `[export] include`, like
  the pawn Ai/Move tables, so the C++ initializers cannot drift from the Rust
  `#[repr(C)]` layout.
- Rust-policy fixes stay pinned against git originals: the AddPawn membership
  check and the StatLog null guards and `Level->Game` batcher checks
  (`reports/StatLog.md`, `LeadFindings.md`).
- The oracle caught two port regressions that the crate fixtures missed:
  `execMoveCacheEntry` converted strings before entering its files scope (a
  SIGSEGV), and six `APawn::execPoll*` handlers stepped `P_FINISH`. Both are
  fixed.
- After cutover: the frozen engine-native consumer oracle replays
  byte-identical (579 calls, 1,291,798 bytes, rc 0, sorted-identical stderr;
  both determinism runs identical). The six-map gameplay oracle
  (`gp.py run <tag> && gp.py compare <tag>` beside `eno.py`) matches the
  frozen manifest on RNG, ledger, tick, slots, creature, startup and
  normalized-log hashes.
  `cargo test --workspace` passes (1,813 tests);
  `ctest --preset macos-arm64 -E '^renderer_smoke_wgpu$'` passes 148/148; and
  the final hook-free acceptance passes (44/44 focused, 9/9 VM CTests,
  installed Adv8Forest smoke with zero failure markers, 166/166 scans).
  Footprint: Rust 563 files / 236,276 nonblank lines (72.5%), native 371 / 89,614.

`IMPLEMENT_FUNCTION`/`GRegisterNative` have no built callers; only the unbuilt
Editor sources use them. The unbuilt Editor header exporter
(`Editor/Src/UnEdExp.cpp`) would regenerate `AUTOGENERATE_FUNCTION` rows for
every native in `EngineClasses.h`; if it is ever built, it must skip the
Rust-owned natives. Neither oracle is a CTest; both are manual consumers. No
retail, Linux, sanitizer, foreground
`renderer_smoke_wgpu` or gate-promotion claim; M1 remains open. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-engine-natives/acceptance-engine-natives-cut3.json`.

## Leaf-elimination pilot: APawn move/reach

After the catalog cutover most remaining Engine C++ is glue, roughly 16k lines
of service-table leaves and 14k of forwarding shells. Leaves fall into three
kinds: (a) the callee is a C++ shell forwarding to `hp2_rs_*`, so the leaf is a
Rust→C++→Rust round trip that can be removed now; (b) Core object-model
accessors and virtuals, which wait for the Core step; (c) real C++ logic, which
is ported in slices. The pilot removes kind (a) for the APawn move/reach
methods.

- `APawn::walkMove`/`flyMove`/`swimMove`, `Reachable`, `walkReachable`/
  `flyReachable`/`swimReachable`, `jumpLanding`, `FindJumpUp`,
  `SuggestJumpVelocity` and `FindBestJump` are public Rust functions
  (`hp2_engine::pawn`, `hp2_engine::pawn_reach`). Their C++ shells and
  declarations, the 12 reach leaves, the path `walk_move` leaf, the route
  `goal_jump_landing` leaf, the native-move `suggest_jump_velocity` leaf and
  `AActor::TwoWallAdjust` are deleted. C++ publishes the reach table once
  (`hp2_rs_engine_pawn_reach_publish_services`); Rust callers use
  `pawn_reach::published_services()`.
- Guard history: C++ `unguard` is `catch(...){appUnwindf(Name); throw;}`, and
  every engine throw takes that branch. The Rust function that now *is* the
  method wraps its body in `native_guard::guarded(c"<original guard name>")`,
  which emits the same history innermost first. This reverses the earlier rule
  that recursion must pass through the native public guards. There is exactly
  one guard per call path: surviving C++ shells stay guarded only while their
  Rust body is not, and become guard-free forwards when it is.
- Tests that mocked the deleted leaves now script the underlying pawn move
  services (`crates/hp2-engine/tests/pawn_moves/publish.rs`), so the real
  `walkMove`/`jumpLanding` run. The route oracle stubbed `jumpLanding` as
  `Z -= 16`, so the route test gives the goal pawn a scripted landing with the
  same result. Its replay stays byte-identical, as does the pawn-reach replay.
- Evidence (data-prototype, macOS arm64): `cargo test -p hp2-engine` 423/423;
  the native consumer oracle `det all` is IDENTICAL and matches the frozen
  pre-cutover record (1,291,798 bytes); `gp.py compare cut5` matches 42/42
  hashes over six maps; `ctest --preset macos-arm64 -E '^renderer_smoke_wgpu$'`
  gives 147 passed and 1 skipped. The skip is `wgpu_surface_round_trip`, which
  exited 77 because the window was occluded. That is a foreground condition,
  so it does not count as evidence for or against the code. No retail, Linux,
  sanitizer or foreground-renderer claim. Evidence:
  `out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-engine-natives/acceptance-leaf-pilot-cut5.json`
  (logs in `cut5-logs/`, gameplay hashes in `gp-cut5/` beside it).

The same contract next applies hub by hub: `ULevel` move/trace/zone/
encroachment, `UModel::FastLineCheck`, the `AActor` collision shells,
`ULevel::SpawnActor` and the remaining APawn route/reach shells.

## Hub conversions, virtual dispatch and reporter port checkpoint (cut7)

**Invariant:** a call that was virtual in the original C++ still dispatches
through the C++ vtable while its class is C++. The ported diagnostic reporters
produce byte-identical reports and logs. SDL and OpenAL/Ogg output parity was
not compared (see Unresolved).

- Hubs converted under the pilot contract: `ULevel::SpawnActor`, the remaining
  APawn route/reach shells, the `AActor` trace/collision/physics shells, and
  `ULevel` move/trace/zone/encroachment. Rust owns the guard name, the check
  text, `FMemMark` and `GetLevelInfo`. The surviving C++ shells are guard-free
  forwards. `ULevel::FindSpot`'s guard moved into Rust; the shell had kept its
  own copy, which would have doubled the guard history.
- Virtual dispatch. cut6 called the Rust bodies of `SetActorZone`,
  `MoveActor`, `FarMoveActor`, `CheckEncroachment`, `Single*Check`,
  `Multi*Check`, `FindSpot` and `AdjustSpot` directly. That bypassed overrides
  such as `Tests/AbiTests.cpp`'s `SetActorZone`/`MultiLineCheck`, and
  `native_registration`/`core_app_format` crashed in `set_actor_zone_body`.
  - Ten `virtual_*` leaves now sit at the end of `Hp2LevActServices`
    (`static_assert` 36 → 46), and `virtual_spawn_actor` sits in
    `Hp2LevelSpawnActorServices`.
  - Each leaf is `((ULevel*)Level)->X(...)` inside `Hp2FfiCall`.
  - Every Rust call that was `this->`/`Level->` in the original goes through a
    leaf.
  - Exception: `level_init` calls `spawn_actor` directly, because the call sits
    in the `ULevel` constructor, where C++ binds statically.
- Leaves kept, each with a reason:
  - `UModel::FastLineCheck` and `IsMovingBrush`: the shell branches, and no
    `UModel` publish table exists yet.
  - The `SetCollision`/`SetCollisionSize`/`SetBase`/`GetCylinderExtent` leaves
    outside `physic.rs`.
  - `ScriptTraceActorsBegin`/`End`: they need a split-mark (begin/end)
    `FMemMark` API before they can move.
- Tests: four `actor_unactor.rs` tests were removed with the deleted leaves.
  They pinned behavior (FarMoveActor/MoveActor default flags, trace flag
  selection, hit writes), so they are being rewritten against the scripted
  `LevActServices`. Until then those defaults have no test. The `net_chan`
  `actor_received_bunch` SpawnActor path has no coverage.
- BMP: `CreateTextureFromBMP` reads the MSVC-packed on-disk header. The native
  oracle was re-baselined to `native-post-bmp-pack.json`; only records 123, 124
  and 430 change (602 bytes). It has its own acceptance record,
  `acceptance-native-rebaseline-bmp-pack.json`.
- Moved from `SDLLaunch.cpp` (9,132 → 5,682 non-blank lines across SDLLaunch)
  to `crates/hp2-launcher/src/reporters/`: the WorldCollision/Touch/Bump gate,
  StaticBsp probe, Lifecycle gate, Fidelity, ShadowAdmission,
  ActorTransitionLedger, CreatureGenerator, GlobalTick, ActorSlotDump and
  FireTexture reporters.
  - C++ keeps only service-table adapters (`HP2TraceReporterServices.cpp`,
    `HP2FireTextureServices.cpp`).
  - The `GTraceHookBindings` indirection is gone.
  - `%.4f` NaN spelling and `%g` are reproduced by `reporters::json`.
- Platform moves:
  - SDL window, cursor, display mode, input, version, GL-attribute and error
    calls moved to `hp2-platform`.
  - The `COMPILED_VERSION` (2.32.10) copy is pinned by a test against the
    fetched `SDL_version.h`.
  - The OpenAL `al*`/`alc*` and Ogg trampolines moved to `hp2-audio` (feature
    `audio-openal`, linked through `hp2_ffi`).
  - cbindgen renames `FCheckResult` → `Hp2CheckResult` and `FBox` → `Hp2Box`
    so they don't collide with the native types.
- Dead code deleted: all three `Amd3d.h` copies (`Render/Inc`, `Render/Src`,
  `Engine/Src`), `Render` `UnTest.cpp` and the `ASM3DNOW` include block, and the
  three `Fire` MASM files. The old `.dsp`, makefile and VSS project files still
  name them; that is left as drift.
- New oracle `rp.py`. It runs each reporter env var on the pre-port app
  (`reporters-before/`) and on the candidate app, then compares sha256 hashes
  of the reports and of normalized logs, with `WgpuDrv:` lines stripped. Its
  300-tick traces cover PrivetDr, Adv8Forest and Adv1Willow. The lifecycle-gate
  fixture already failed at Init before this round (`LifecycleGateGame` not
  found), so that reporter has unit-test coverage only.
- Finding: the gameplay oracle depends on presentation. With the window
  presented, the candidate's `rng.json`/`ledger.json` diverged on Adv12Chamber
  and Adv4Greenhouse, and `log.norm` diverged on five maps. When occluded, both
  the pre-port app and the candidate match the frozen hashes. The pre-port app
  was never run presented, so presented-mode parity is unproven. The `gp.py`
  manifest is valid only for occluded runs.
- Evidence (data-prototype, macOS arm64): cargo 730 passed, 0 failed. `det all
  cut7` is IDENTICAL and equals `native-post-bmp-pack.json` by sha256
  (1,291,196 bytes). `gp.py compare cut7b` matches 42/42 with every map
  occluded. `rp.py compare b0a cut7` matches 34/34, and b0a vs b0b also matches
  34/34. `ctest --preset macos-arm64 -E '^renderer_smoke_wgpu$'` passes
  148/148, including `native_registration`, `core_app_format` and
  `wgpu_surface_round_trip`. No retail, Linux, sanitizer or foreground-renderer
  claim. Record: `acceptance-hubs-cut7.json`, with logs in `cut7-logs/` in the
  same artifact directory as the pilot.
- Unresolved: SDL behavior (version log, GL attributes, window-create error,
  input) and OpenAL/Ogg output have no differential oracle; only Cargo tests and
  the passing CTest suite cover them. The hand-mirrored `SDL_Event` union now has
  header-backed size/field-offset checks and an alignment check. The omitted
  pointer-bearing SDL variants previously left the Rust union aligned to four
  bytes; a pointer member restores eight-byte alignment on supported arm64 targets.
  `cmake --build --preset macos-arm64 --target hp2_verification_binaries` passed,
  as did `ctest --preset macos-arm64 -R 'input|abi_widths' --output-on-failure`
  (6/6, including the actual `input_script_smoke`, data-prototype).
  ABI/script contract checks also cover data-none. Evidence is under
  `out/macos-arm64/Testing/HP2/`. No Linux claim.
- `renderer_smoke_wgpu` was excluded from the earlier checkpoint. The current
  completion work runs it rather than treating its exclusion as acceptance.
  Rust reporter console output uses Rust's stdout buffer, not C stdio;
  `rp.py` logs matched, but ordering under other pipes is not proven.
- Scope decision (integrator, round 8): the in-tree Emscripten/web build is
  retired on this branch. Rust has no wasm64-emscripten target, and the branch's
  web path already called `hp2_rs_*` while `hp2_ffi` was excluded for
  Emscripten. The `__EMSCRIPTEN__` sources, `HP2Web*.cmake` and the
  `web-wasm64` presets were removed. CI's web job builds the `web-cpp-freeze`
  tag. `Build/feature-gates.json` `FreeTypeEmscripten` and its wasm acceptance
  lines are unchanged gate records, referring to the freeze tag.

## macOS completion work: ownership and live-runtime evidence

The port is not complete. The current macOS compile graph still contains 157
unique first-party native translation units; typed object/resource lifetimes,
virtual interfaces and native storage remain. These counts describe the live
build, not a native-free Rust runtime.

- Rust object construction now reads authoritative generated object/class/
  property views directly and initializes the base header through the existing
  Rust owner. Typed construction, property lifetime, config and diagnostics
  remain native continuations.
- Property values now use generated metadata/container views and existing Rust
  string/array owners. Virtual property/archive operations, typed destruction,
  lazy class lookup and checked allocation/diagnostics remain native.
- Span allocations and marks call the Rust memory-stack owner directly, with
  failure status propagated to native guard frames.
- Scene traversal, clipping, typed scratch allocations and scene-stack marks now
  call the same Rust memory owner directly. Native placement construction and
  failure-only exception-history leaves remain; unused renderer memory callbacks
  and their adapters were removed.
- Scene lighting reloads the model's light-array header after reentrant scratch
  allocation, instead of retaining a stale backing pointer. Empty corona-cache
  entries are checked before copying their otherwise uninitialized payload.
- Live editor transactions now invoke Rust policy for capture, application,
  undo/redo, reset, preload and GC snapshot traversal. Native typed record arrays,
  archive storage and UObject lifetime are not yet Rust-owned.
- CoreText layout-handle allocation and destruction now live in Rust. Native
  compile-time size/alignment/offset assertions preserve the public handle ABI;
  the FreeType provider and bitmap compatibility mode are unchanged.
- File-stream manager identity, lifecycle, path conversion and streaming resources
  now live directly in Rust. The native `FFileStream` facade, `GFileStream`
  singleton, duplicate stream records and `UnFileStream.cpp` were deleted.
  Native audio retains synchronous typed `USound`/lazy-array extraction leaves.
- Package summaries now use the existing generated summary/generation views;
  Rust owns generation replacement and legacy-generation storage. Typed archive
  I/O and allocator/error leaves remain native. Virtual `Tell()` now uses the
  pending-exception transport instead of allowing a foreign exception across
  Rust frames.
- Replication-condition cleanup now owns its pointer-keyed map in Rust instead
  of constructing a native `TMap`. Fallible growth preserves partial chains and
  the first native exception, including failures while freeing rehash storage.
- Package remapping now owns its canonical-name pairs and independent lookup
  snapshots in Rust. The native global map, snapshot arrays and allocation/
  deletion callbacks were removed. Reentrant initial allocation sees only
  committed, initialized pairs.
- The unused `FEAXABlockDecoder` class and its native translation unit were
  deleted. Both native test consumers now call the existing Rust decoder
  directly through its canonical FFI lifetime.
- Class replication records and network-field arrays now use their canonical
  storage directly from Rust; native container callbacks were removed. Sorting
  reloads backing storage after reentrant native comparisons. The deleted
  conditional-expression adapter's source-copy temporary and extra OOM/cleanup
  stages are intentionally not recreated.
- Dependency append/update and inherited script CRC policy now read generated
  `UClass`, `FDependency` and `UTextBuffer` fields directly in Rust. Duplicate
  dependencies still all update and another record still appends.
- Rust owns registry driver/preference tables, their cached language and the
  former `FCacheStorage` enumeration/path/Public-section scratch. Independent
  result strings and pinned cache generations survive reentrant refresh.
  Reentrant readers see only initialized table records; a real native allocator
  callback reproduced the premature-count SIGSEGV before that publication fix.
  Filesystem/config access, typed parsing and general language notifications
  remain guarded native leaves.

Latest class/registry cutover verification (macOS arm64):

- Configure/build/install passed with generated native layout assertions and
  the signed app: `class-registry-integrated-build.log`.
- `ctest --preset macos-arm64 --output-on-failure` passed **153/153** in
  **622.00 seconds**, with no skips, under registered data-none/data-prototype
  coverage. The renderer reported **300 presented/0 occluded/0 outdated/0 other
  failures**. Evidence: `ctest-class-registry-full-153.log` and
  `class-registry-renderer-smoke.log`.
- The repaired named `property_link_contracts`, `class_lifecycle_contracts` and
  `object_registry_contracts` invocation passed **3/3**. The registry consumer
  then passed again with its files isolated under `HP2_ARTIFACT_DIR`, using
  real filesystem/config/parser operations. Evidence:
  `class-registry-repaired-contracts.log` and
  `class-registry-isolated-registry.log`; earlier failures are retained in
  `class-registry-first-contract-failures.log`.
- The real native-object consumer exited zero, verifying native text layout,
  CRC caching/superclass XOR, duplicate dependency updates plus append,
  freshness invalidation, three inherited plus two child replication records
  and child replication index three. Evidence:
  `class-storage-native-registered-runtime/{command.json,output.log}`.
- The installed app loaded retail `Adv1Willow`, started its cutscene and music,
  presented **120 frames with zero occluded/outdated/other failures**, and
  shut down with **24221 -> 0 objects**. Frame 20 was visibly rendered; frame
  119 was black. Whole-sequence black detection also found sustained initial
  and late black intervals. Successful presentation is not proof that those
  frames contain the expected scene. Stock fade commands exist, but the fade
  state at these captures is not yet established; investigation remains open.
  Evidence: `class-registry-retail-willow/stdout.log` and
  `class-registry-retail-willow-launch/command.json`. The run used an isolated
  test HOME and the previously accepted test INI, whose save paths remain in
  the earlier private artifact directory. Retail identity is backed by the
  preceding 537-file manifest size/SHA-256 check, not original-media
  authentication or runtime manifest validation. Stock cutscene diagnostics
  are still unwaived; physical input, audible output and full progression
  remain unproven.
- Strict-provenance Miri passed all **21 class-lifecycle** and **22 property-link**
  cases after fixing fixture allocation/provenance ownership and overlapping
  fixture borrows. Constructor-slot checks exercise invocation and object
  initialization rather than comparing separately reified function addresses;
  neither slot's behavior is skipped under Miri.
- The final named ownership-contract invocation passed **3/3** after those
  fixture corrections. `python3 Build/check_bundle.py` exited zero with a
  thin arm64 executable, valid ad-hoc signature, embedded OpenAL and no
  integrity warnings.
- A temporary per-frame renderer-input probe ran retail Willow for 360 ticks.
  Full-black captures at indices **0–2** and **72–116** matched black flash
  submissions with alpha **1** exactly; the later fade returned to alpha zero
  and frame 200 visibly contained Harry and dialogue. The game advanced to
  `00030WhompCrash` and `GSTATE010`, with additional cutscene command errors
  retained as unresolved findings. This diagnostic run was **0 presented/
  360 occluded**, so its GPU readbacks are not on-screen acceptance evidence
  and do not retroactively establish the original run's per-frame inputs.
  Evidence: `retail-willow-fade-probe/{stderr.log,stdout.log,Frames/}`,
  `retail-willow-fade-probe-launch/command.json` and
  `retail-willow-fade-probe-blackdetect/{command.json,output.log}`.
  Temporary renderer logging was removed without a rendering-policy change.
- After removing that probe, rebuilding/re-signing the uninstrumented app
  passed, but the subsequent named renderer smoke was **blocked**:
  **0 presented/300 occluded/0 outdated/0 other failures**, CTest exit 8,
  **36.97 seconds**. This is later than the full-suite pass above and remains
  an unresolved native visibility/presentation finding, not a waived result.
  No forced focus, occlusion bypass or retry-to-green was used. Evidence:
  `class-registry-restored-app-renderer-blocked.log`,
  `class-registry-restored-renderer-occluded.log` and the chronological
  `class-registry-verification.json`.
- A separate read-only native-window observation of the restored app recorded
  60 samples: its window was mapped/on-screen in 37 samples, at
  `(375,179,720,569)`, while the frontmost PID remained different from the
  game PID. That retail run shut down cleanly with **0 presented/180 occluded**
  and **24301 -> 0 objects**. This rules out an absent window in those samples,
  but does not prove whether ordinary covering or a native visibility defect
  caused acquisition to report occlusion. No window state or permissions were
  changed. Evidence: `retail-window-metadata-probe/window-metadata.json` and
  `retail-window-metadata-probe/stdout.log`.

Previous map/XA cutover verification (macOS arm64):

- The same configure/build/install command below passed after these changes.
  Full output: `maps-xa-integrated-build.log`.
- `ctest --preset macos-arm64 --output-on-failure` passed **152/152** in
  **586.86 seconds**, including the renderer, with no skips. This invocation
  exercised the registered data-none/data-prototype coverage, not retail.
  Evidence: `ctest-maps-xa-full-152.log`. The renderer reported **300 presented,
  zero occluded, zero outdated and zero other failures**; its frozen log is
  `maps-xa-renderer-smoke.log`.
- A real native-object consumer verified equivalent/distinct replication
  conditions, config/constructor chain tails, retained mutations after a
  throwing condition reader, the original allocation exception and successful
  relinking. It also verified 20 case-insensitive package-remap lookups, direct
  object precedence and the no-remap/explicit-filename/unmapped-package gates.
  The corrected harness exited zero:
  `property-map-native-concrete-runtime/output.log` (prototype root supplied).
  Earlier harness compile/abstract-object failures remain separate records.
- Strict-provenance Miri passed all **19** property-link cases after correcting
  callback-context identities in the fixture. Both failing and passing logs
  are retained as `miri-property-maps-{before,after}-fixture-fix.log`.
- Default Miri passed all **23** package-flow cases after correcting the remap
  allocator fixture's context lifetime. The complete strict-mode attempt is
  retained separately: the existing integer-handle callback protocol is not
  executable in that mode. Default Miri still warns that integer-to-pointer
  casts can hide pointer bugs; this is **not** strict-provenance acceptance.
  Evidence: `miri-package-flow-{before,after}-fixture-fix.log` and
  `miri-package-flow-strict-unsupported-integer-abi.log`.
- The real prototype package audit verified six packages and extracted 65
  serialized audio assets. The direct Rust FFI consumer verified all 65 corpus
  entries and decoded 61 EA-XA entries; both commands exited zero. Evidence:
  `xa-real-prototype-corpus/command.json` and
  `xa-direct-ffi-real-corpus/output.log`. No goldens or game assets changed.
- The installed app loaded verified-retail `Adv1Willow`, possessed Harry,
  spawned his wand and shut down with **24235 -> 0 objects**. It reported
  **32 presented/88 occluded** frames, with no outdated/other failures. The
  inspected `maps-xa-retail-willow/Frames/frame_000020.png` shows the stock
  flying-car cutscene; frame 119 is black. This is not uninterrupted visible
  rendering acceptance. Stock cutscene errors remain unwaived.
  The pre-run 537-file size/SHA-256 check is recorded under
  `retail-identity-before-map-smoke/`; this verifies consistency with the import
  manifest, not original-media authenticity or runtime manifest validation.
- `python3 Build/check_bundle.py` again exited zero: thin arm64 executable,
  valid ad-hoc signature, embedded OpenAL and no integrity warnings. Physical
  input, audible output, full retail progression, Linux and sanitizer
  acceptance remain unproven.

Previous stream/summary cutover verification:

- `python3 Build/gen_engine_abi.py && cmake --preset macos-arm64 && cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed; full output is `stream-summary-integrated-build.log` below.
- `ctest --preset macos-arm64 --output-on-failure -E '^renderer_smoke_wgpu$'`
  passed **151/151**, with no skips, in 593.87 seconds. The foreground renderer
  was deliberately run separately after CoreGraphics no longer reported the
  locked-session flag; this is not a single full 152-test invocation.
  Evidence: `ctest-stream-summary-nonforeground-151.log`.
- `ctest --preset macos-arm64 --output-on-failure -R '^renderer_smoke_wgpu$'`
  passed **1/1** in 29.15 seconds, reporting **300 presented, zero occluded,
  zero outdated and zero other failures**. Its log remains under
  `Testing/HP2/renderer_smoke_wgpu/Artifacts/wgpu-smoke/` in the build tree.
  The subsequent `python3 Build/check_bundle.py` exited zero with a thin arm64
  executable, valid ad-hoc signature and no integrity warnings.
- A native consumer round-tripped versions 67, 68 and 79 through real buffer
  archives, replaced existing generation storage and restored the legacy
  heritage cursor. A throwing virtual `Tell()` delivered its original exception
  back to C++ without any subsequent seek. The isolated run exited zero:
  `package-summary-native-run-with-artifacts/output.log`. An initial invocation
  lacked `HP2_ARTIFACT_DIR` and failed the surrounding lifecycle harness; its
  failure record is retained separately, not represented as a passing run.
- Strict-provenance Miri passed all **18** package-summary tests after exposing
  and correcting a test-fixture allocator-context alias violation. The fixture
  now owns its callback context through an explicit raw Box lifetime.
  Both `miri-summary-before-fixture-fix.log` and
  `miri-summary-after-fixture-fix.log` are retained. A warning in the separate
  class-dependency fixture is not claimed fixed.
  After that fixture correction,
  `ctest --preset macos-arm64 --output-on-failure -R '^package_summary_archive$'`
  also passed 1/1 in 16.02 seconds (data-none).
- The clean installed app loaded verified-retail `PrivetDr.unr`, accepted the
  stock replay keys to open and close the pause book, continued the title
  cutscene, started both music sources and shut down cleanly. It exited zero
  with 132 presented/148 genuinely occluded frames. Inspected captures:
  `retail-new-game-menu-resume/Frames/frame_000120.png` (book) and
  `frame_000210.png` (resumed title cutscene). These are engine captures, not
  physical-input verification or proof that every captured frame was visible.
- `CGPreflightPostEventAccess()` returned false. No permission prompt or event
  injection was attempted. Real macOS event-posting automation is an external
  prerequisite; replay/SDL contracts must not be called physical input evidence.
  Record: `mac-event-posting-prerequisite.json`.

Earlier ownership/rendering verification retained for this batch (macOS arm64):

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed after fixing native transaction argument conversions and optional
  reader construction.
- Focused CTest run passed 16/16, including construction/property/transaction
  contracts, native registration, object lifecycle/GC, CoreText typography and
  bitmap Canvas compatibility. Profiles: data-none and data-prototype.
- `cargo test --offline -p hp2-render --test span_reference` passed 5/5.
- `cargo test --offline -p hp2-render` passed the unit and integration suites
  after both scene-memory safety fixes.
- `ctest --preset macos-arm64 --output-on-failure` passed **151/151**, with
  no skips. The bare-executable `renderer_smoke_wgpu` reported 299 presented,
  one occluded, zero outdated and zero other-failure frames. Profiles:
  data-none and data-prototype; this is not a full retail acceptance run.
- The subsequently registered `scene_lighting_contracts` passed 1/1 with
  `ctest --preset macos-arm64 -R '^scene_lighting_contracts$' --output-on-failure`.
  The registry now contains 152 tests; a single full 152-test run is not yet
  claimed. Its deterministic light-array replacement regression failed before
  the fix (`[2, 1]` instead of `[3, 1]`) and passed afterward.
- `MIRIFLAGS=-Zmiri-strict-provenance cargo +nightly miri test --offline -p hp2-render --test scene_lighting_reentrancy`
  passed 1/1 without warnings, using isolated Cargo and artifact directories.
  This checks the reentrant lighting regression, not the entire renderer or
  the separate corona-cache fix.
- The unchanged frozen native span consumer ran against the current SDK:
  all 212 logical observable lines (210 prefixed records) matched. Raw bytes
  differ by two CR bytes at error-history continuation boundaries; the original
  artifact is unchanged. This is not a byte-identical capture claim.
- A native transaction consumer exercised real object undo/redo with recursive
  self-preload, array replacement and insertion undo/redo; it exited zero and
  printed the explicit pass record.
- `python3 Build/check_bundle.py` exited zero.
- The installed bundle loaded `Adv1Willow` for 120 ticks under both data-prototype
  and data-retail, initialized CoreText and OpenAL, captured the rendered
  cutscene, and shut down the object subsystem cleanly. Retail identity was
  checked against all 537 overlay manifest entries by size and SHA-256, with
  zero mismatches; original-media authenticity was not independently established.
  The retail harness reported exit zero and `script_deferred=0`.
- A separate retail input replay executed the actual `SAVEGAME 9` and
  `LOADGAME 9` bindings. The runtime wrote `Save9.usa`, loaded that exact file,
  possessed `harry Save9.Harry0` with the restored `Save9.baseWand0`, rebuilt
  the level's moving-brush tracker and exited zero. This establishes a basic
  same-session save/load path, not exhaustive persistent-state fidelity.
  Evidence: `save-load-runtime/runtime.log` and `result.json` in the directory below.

The initial prototype and retail runs reported zero presented/120 occluded
frames, including a LaunchServices foreground launch. A diagnostic experiment
identified uncommitted Core Animation transactions in SDL's manually pumped
AppKit loop: explicitly committing attached the Metal view's layer tree and
allowed genuine acquisition/presentation. The production fix commits after
surface creation and before acquisition, including resize changes; it does
not force window activation or override genuine occlusion.

The rebuilt production bundle, without diagnostic environment variables,
subsequently rendered the same Willow scene under both data-prototype and
manifest-verified data-retail with **120 presented, zero occluded, zero outdated
and zero other-failure frames**, initialized OpenAL and shut down cleanly.
Evidence: `surface-production/` and `surface-retail-production/`, including
`command.json`, `stdout.log` and captured frames, in the directory below.
The earlier flush experiment presented 22 frames before foreground loss and
truthfully counted the subsequent 98 as occluded.

The bare executable had a second, distinct activation issue: SDL 2.32.10
defaults unbundled macOS binaries to background-app behavior. A controlled run
changing only `SDL_MAC_BACKGROUND_APP=0` presented 300/300 frames. The shared
macOS video-initialization helper now sets that SDL hint before initialization;
explicit environment overrides retain SDL's normal precedence. The subsequent
integrated suite passed, including the real renderer smoke. This does not
force windows above other applications or override genuine occlusion.

Earlier retail runs logged stock-script `CutError` messages for releasing
FlyingFord, an unsupported BaseCam `Message`, and an unfinished Harry command.
The existing smoke failure-marker scanner missed those errors despite reporting
a pass. Those messages have not been established as Rust regressions or waived.
The initial `OpenBook MAIN` / `CloseBook` pair is not by itself a frontend
failure: temporary caller/bytecode instrumentation located the close inside
the stock `HPConsole.RenderUWindow` root-creation branch. That instrumentation
was removed and the production bundle rebuilt. The actual launcher New Game
destination, `PrivetDr.unr`, loaded with verified retail data; a replay using
the stock Enter and Escape bindings opened `FEInGamePage`, and the captured
frame showed the fully rendered pause book. This proves the engine/replay UI
path, not physical SDL input or on-screen presentation during that run.
Evidence: `retail-new-game-menu/Frames/frame_000150.png` and `stdout.log`.

The later frontend runs truthfully reported occluded frames. CoreGraphics
reported `CGSSessionScreenIsLocked=true`; WindowServer samples also recorded
full-screen shielding/login windows above the game. No occlusion bypass or
forced activation was added. Evidence: `surface-retail-frontend-normal-diagnostic/`
`session-visibility.json`, `windowserver-observation.json` and
`windowserver-covering-summary.json`. Live foreground UI acceptance remains
unearned under that locked session; earlier successful presents remain
separate evidence.

The New Game run also logged an invalid `*` cutscene subject. Its verified
retail intro file contains adjacent `line_2=/*` and `line_3=*/` entries.
The in-tree `CutScript.GetNextCommand` counterpart strips from the first `/`,
which leaves `*` for the closing line. This explains the observed input to
the interpreter, but is not a native-Windows baseline or a waiver of the error.
Retail data and script bytecode were not patched to suppress it.

Owned frame captures and successful presents do not establish audible output,
physical input correctness or complete retail playability.
No Linux or sanitizer acceptance is claimed.

Evidence directory:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`.
The earlier full CTest run was 145/150: four Cargo tests read intermediate edited
sources and subsequently passed after integration; `renderer_smoke_wgpu`
independently reported zero presented/300 occluded before the transaction-commit
fix. That failure remains part of the acceptance history, not a successful run.

## Existing tool/documentation drift

`Docs/OPERATIONS.md` contains older Cargo-era and macOS-only statements;
`CMakePresets.json` and the current target registrations govern live builds.
`Build/retail_flow_smoke.py` and `Build/verify_playable.py` have documented stale
`dist/macos-arm64-rs` defaults: pass the actual installed app path explicitly.
Other earlier-rewrite references listed in `AGENTS.md` are not current-port
acceptance evidence. This plan flags that debt; it does not silently repair
unrelated scripts or treat their old results as evidence.

## Transaction and network resource ownership checkpoint

Rust now owns editor transaction snapshots, stable record allocations, titles,
flip state and undo-history lifetimes. Native editor shells retain SDK vtable
identity, typed object/archive operations and editor notification calls. Original
SDK semantics remain: duplicate object saves are retained; GC traverses saved
references without rewriting snapshot bytes; transaction archives are
nonpersistent and retain their original array-address constraints.

Strict-provenance Miri exposed two production boundary problems: transaction
callbacks transported pointers through integers, and moving/reborrowing boxed
records invalidated pointers retained by archive callbacks. Callback arguments
and pointer results are now typed pointers. Record allocations retain raw,
uniquely owned identity until teardown rather than recreating mutable references
to the whole record during visits. The unused integer-returning transaction-info
export was removed; reset-reason access is a separate borrowed-pointer query.
Fixture mutations now use the same published raw object provenance as the SDK
consumer instead of competing Rust borrows.

Networking now owns bunch headers and buffers, channel/connection resource
lifetimes and scoped canonical memory marks in Rust. Native SDK construction,
destruction and archive dispatch remain typed leaves. Copy construction retains
its original snapshot-before-allocation behavior; assignment retains live-source
behavior across allocator callbacks. The network fixture's package URL now
provides a canonical terminated FString view, not a pointer to a Rust Vec.
General object language storage also moved to Rust; case-insensitive no-change
updates preserve the original spelling.

Observed verification for this checkpoint:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  succeeded and installed the signed Mac app after the ownership correction.
- `ctest --preset macos-arm64 -R '^(transaction_contracts|object_registry_contracts|mem_stack_contracts|core_net_archive)$' --output-on-failure`
  passed 4/4 in 21.25 seconds: three data-none tests and one data-prototype test.
- `cargo +nightly miri test --offline --package hp2-core --lib transaction::tests -- --test-threads=1`
  with `MIRIFLAGS=-Zmiri-strict-provenance` passed all six selected tests.
  Original pointer-transport, record-alias and fixture-alias diagnostics are
  retained; none was waived or hidden by changing Miri's checks.
- The rebuilt actual SDK transaction consumer passed object restore, recursive
  preload, array replace/insert/remove, GC traversal, history retention/reset,
  base-interface deletion and first-error preservation, data-prototype.
  The separate fatal-teardown child exited with the expected status 86.
- The actual SDK network consumer passed independent 13-bit copies, virtual
  bitstream dispatch, partial-byte mark rollback, reliable out-of-order delivery,
  duplicate suppression, ACK/NAK cleanup and channel closure, data-prototype.
  Transport was in-memory, not UDP.
- Targeted networking Rust tests passed 26/26 (`net_chan`, `net_demo`,
  `net_replay`); native/FFI network ABI tests passed 2/2 (`net_demo_abi`).
- Strict-provenance Miri exposed an outgoing/incoming bunch teardown bug:
  reconstructing a Box before callbacks completed invalidated raw header
  accesses. Both paths now reconstruct ownership only after buffer release and
  native destruction. All four selected `net_chan` bunch tests then passed
  strict Miri, including its leak check. The fixture now uses typed pointers,
  matches canonical zero-size Realloc, and retires reliable-send history through
  channel destruction.
- After that correction the relinked SDK network consumer passed all seven
  reported contracts; targeted networking Rust tests passed 26/26 and native/FFI
  ABI tests passed 2/2 again.
- `ctest --preset macos-arm64 --output-on-failure` returned status 8:
  152/153 passed in 392.07 seconds. `determinism_double_run` failed under
  data-prototype: normalized line 117 was an Owl cutscene command in run 1 and
  `WgpuDrv: Lock outcome code 4: no window frame presented this tick` in run 2.
  This is a retained failure, not a passing full-suite claim. The earlier broad
  run was cancelled when the teardown bug was found; it supplies no suite result.

Evidence remains under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`,
including `transaction-resource-miri-isolated/`,
`transaction-typed-sdk-isolated/`, `transaction-typed-native-fatal-teardown/`,
`network-resource-miri-complete-lifecycle/`, `network-reclaim-native-runtime/`,
`network-resource-final-rust-contracts/` and
`network-resource-final-ffi-contracts/`. Initial smoke invocations missing required
arguments or artifact environment are retained separately, not counted as
runtime failures or successful acceptance.

This is not native-free completion. Native UTransBuffer Undo/Redo editor
notification completion was not exercised: the game SDK consumer does not link
the full editor implementation, and no editor methods were stubbed. The
in-memory network run does not establish socket transport interoperability.
The previously observed Mac presentation/occlusion failure, retail cutscene
errors, physical input, audible output and complete retail progression remain
unresolved. No feature gate was promoted and no retail script or golden changed.

## Core startup storage and SDK object ownership checkpoint

`Core/Src/Core.cpp` is deleted from the source tree and the CMake runtime source
list. Its process globals, startup devices, package symbols and eager math-table
initialization now have Rust-owned storage. The configured macOS compile database
contains **156** distinct first-party `HarryPotter2/Unreal/` translation units,
down from 157 at the preceding checkpoint. This is a source-count observation,
not a completion percentage.

The six startup devices preserve the SDK's real virtual-call, RTTI, foreign
exception and nontrivial-return ABI. `GArchiveDummySave` and `GDummySave` live in
`hp2-core-runtime`, separating SDK-dependent archive objects from neutral FFI
consumers. Native `UnBits.cpp` still owns the `FArchive` base key function and
RTTI, including the native per-leaf signal and exception fences. That dependency
is explicit; the archive base and the overall Core object model are not finished
Rust ownership. There is no final `-dead_strip` workaround or replacement base
RTTI used to hide the dependency.

Observed verification:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries
  hp2_macos_app`: exit 0; verification binaries linked and the Mac app was
  recreated and signed.
- Actual SDK consumer, `data-prototype`:
  `core-startup-isolated-runtime-sdk-runtime/` exited 0 and reported
  `globals=PASS RTTI=PASS diagnostics=PASS foreign-callback-unwind=PASS
  pointer-exception=PASS nontrivial-sret=PASS archive-defaults=PASS
  archive-load-mask=PASS archive-error-transition=PASS`.
- `cargo test --offline --package hp2-ffi --lib`: 29 passed, exit 0,
  `data-none`, recorded in
  `core-startup-ffi-runtime-isolation-contracts/`.
- Permanent SDK regression:
  `ctest --preset macos-arm64 -R '^core_startup_devices$' --output-on-failure`:
  1/1 passed, 0.36 seconds, `data-none`. The actual SDK caller checks RTTI base
  relationships, a native exception unwinding through the Rust allocation
  device, exact const-pointer exception identity and nontrivial file-manager
  returns before `appInit` replaces the startup devices.
- `ctest --preset macos-arm64 --output-on-failure`: **152/153 passed**, exit 8,
  629.16 seconds. `determinism_double_run` failed under `data-prototype` at
  normalized line 136: `appRequestExit(0)` versus
  `WgpuDrv: Lock outcome code 4: no window frame presented this tick`.
  This remains a failure, not a waived acceptance condition.

Persistent logs are
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/core-startup-final-mac-build.log`
and `core-startup-full-mac-suite.log` in the same directory. Failed initial
link attempts and the rejected section-stripping diagnostic are not final
acceptance evidence. No Linux runtime, retail progression, physical input or
audible output claim follows from this checkpoint.


## Out-of-line math SDK ownership checkpoint

`Core/Src/UnMath.cpp` is deleted from the tree and the CMake runtime source list.
`hp2-core-runtime::math_sdk` now owns its SDK methods and both constructor forms,
using the existing canonical `hp2-core::math` algorithms and tables. The random
vector/spread header callers use the existing `appFrand` directly; the redundant
`Hp2MathFrand` wrapper and declaration are removed. The configured macOS compile
database now contains **155** distinct first-party runtime translation units.
Included C++ headers and other runtime families remain unfinished; this is not
native-free completion.

An actual SDK consumer was compiled and run before and after removal. Its
45-line numeric output is identical, covering vector/rotator/quaternion
aggregate returns, coordinate indirect returns, coordinate self-aliasing,
point/coordinate/transformed-box constructors, empty bounds, spheres,
interpolation and place conversion. Final relink/run:
`math-sdk-final-link/` and `math-sdk-final-runtime/`, exit 0. The compiler checks
that the SDK value classes retain trivial copy operations. No fake SDK objects
or replacement virtual tables were used for this math consumer.

Observed verification:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries
  hp2_macos_app`: exit 0, including the final RTTI C-string literal lint fix.
- `ctest --preset macos-arm64 -R
  '^(math_sdk_values|core_startup_devices|math_inline_contracts|abi_widths|renderer_smoke_wgpu)$'
  --output-on-failure`: **5/5 passed**, 13.70 seconds. The new SDK regression
  exercises three- and four-SIMD-register, integer-register and indirect
  aggregate returns from native callers; the existing inline suite passed
  9 Rust cases.
- The renderer smoke booted prototype `Entry` and ticked 300 times, reporting
  **299 presented frames, 1 occluded frame, 0 outdated and 0 other failures**.
  This is positive prototype presentation evidence, not retail progression,
  physical-input or audible-output evidence; the smoke explicitly used
  `-nosound`.
- `cargo test --offline --package hp2-core --test math_replay`: 2/2 passed,
  including bit-exact math-operation and global-table original-oracle replays.
- `cargo clippy --offline --package hp2-core-runtime --all-targets --no-deps
  -- -D warnings`: exit 0 after replacing a manually NUL-terminated RTTI name
  with a C-string literal. The initial lint failure is retained separately.
- `python3 Build/check_bundle.py`: pass for the installed arm64 app, ad-hoc
  signature verification, Info.plist and embedded OpenAL.
- All four affected oracle generator scripts passed individual `sh -n`
  checks. Their retired math compile-flag lookups now select live Core
  `UnAnsi.cpp` flags while still compiling their pinned historical C++ math
  sources. The actual shared Core flag lookup exited 0. No golden was
  regenerated or changed.

Evidence is under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`,
including `math-sdk-final-mac-build.log`, `math-sdk-ctest.log`,
`math-sdk-original-oracle-replay/` and `math-sdk-runtime-clippy-cstr/`.
The final startup SDK consumer also passed under `data-prototype` in
`core-startup-final-sdk-runtime-with-arguments/`; its preceding invocation
omitted the required test arguments and is retained as a harness invocation
error, not successful runtime evidence.

The preceding full-suite `determinism_double_run` failure is still open.
Prototype presentation succeeding here does not resolve the previously
observed retail occlusion/cutscene failures. Disposable SDK/probe source files
were removed after the permanent ABI regressions and native consumer runs;
their compiled consumers, command records and logs remain as evidence.

## Shared native exception ownership checkpoint

Rust now owns the shared pending C++ exception reference, first-error policy,
snapshot/restore state and thread-exit cleanup in `hp2-native-exception`.
`Hp2FfiException.h` no longer owns a C++ `thread_local std::exception_ptr`.
Object shutdown, package loading, linker exception-type inspection and editor
transaction constructor/destructor cleanup all use that one Rust-owned state.
Native leaf catch blocks and signal/setjmp fences remain intentionally intact.
The explicit rethrow export is `extern "C-unwind"` and does not pass through a
Rust panic catcher or a plain-C kernel export.

The system C++ runtime still owns exception payload allocation, type identity,
reference counting and unwinding. No exception metadata or exception object is
fabricated. An actual macOS arm64 compiler/runtime probe established the
pointer-sized, pointer-aligned representation, null empty state, x8 indirect
return convention and caller-owned by-value rethrow cleanup. SDK static asserts
pin size and alignment. Linux uses libstdc++'s exported retain/release methods;
its implementation is not runtime-verified by this Mac checkpoint.

Observed verification:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries
  hp2_macos_app`: passed after separating the system-only exception crate from
  the SDK-dependent Core runtime crate. The initial build failure is retained:
  combining them pulled the real native `FArchive` RTTI dependency into neutral
  codec consumers. No linker suppression or fake archive identity was added.
- `ctest --preset macos-arm64 -R
  '^(native_exception_handoff|core_startup_devices|object_lifecycle_runtime|object_gc_runtime|map_load_recovery_runtime|renderer_smoke_wgpu|transaction_contracts)$'
  --output-on-failure`: 7/7 passed, 15.10 seconds. Native error/lifetime and
  transaction contracts cover `data-none`; object/GC/load recovery and the
  actual renderer smoke cover `data-prototype`.
- The new native SDK regression preserves the first nontrivial exception,
  exact payload address, pointer-exception type/value, snapshot lifetime and
  consuming/non-consuming rethrow behavior. A real pthread exits with a pending
  exception; its payload is destroyed without leaking or replacing the main
  thread's pending exception.
- A separate compiled SDK consumer linked the real Core/Engine objects and
  Rust archive, then failed, retained/restored a snapshot and caught the same
  native payload after Rust rethrow. Exit 0; observed output:
  `status=1 live=1`, `tag=73 same-object=1 live=1`, then `live=0 pending=0`.
- `cargo clippy --offline --package hp2-native-exception --all-targets
  --no-deps -- -D warnings`: exit 0.

Evidence resides under the existing `mac-rust-completion/` artifact directory:
`native-exception-abi-probe-*/`, `native-exception-sdk-runtime/` and
`native-exception-neutral-runtime-clippy/`. This is removal of shared native
state ownership, not completion of UObject construction/virtual dispatch,
the VM, retail progression, audible output or physical-input verification.

Final full-suite result after rebuilding the final sources:
`cmake --build --preset macos-arm64 --target hp2_verification_binaries
hp2_macos_app && ctest --preset macos-arm64 --output-on-failure` built
successfully, then passed 155/156 tests in 213.95 seconds (overall exit 8).
`determinism_double_run` remains failed under `data-prototype`: normalized
line 115 is `WgpuDrv: Lock outcome code 4: no window frame presented this tick`
in run 1 versus `CutLog:Issuing to Owl:flyto owlstart Cue:_10UniqueCue` in run 2.
The failure was retained, not retried to green or normalized away.
The targeted renderer smoke independently presented 300/300 frames with zero
occluded/outdated/other-failure outcomes.

The rebuilt installed app subsequently passed `python3 Build/check_bundle.py`:
arm64 executable, expected bundle metadata, ad-hoc signature verification and
embedded OpenAL, with no warnings. These checks do not prove audible output.
Full output is preserved in `native-exception-full-mac-suite.log`,
`native-exception-full-ctest.log` and `native-exception-bundle-check/`.
Disposable probe/consumer sources were removed after verification; compiler
assembly, binaries, permanent native regression and command/output records
remain. No feature gate, retail data, golden or determinism assertion changed.

## Class replication-buffer ownership checkpoint

Invariant: each real `UClass::ClassReps` allocation is reclaimed exactly once
at its original member destruction boundary, including later constructor failure.

`ClassReps` now uses the specialized `FClassRepArray` raw-slot member rather
than inheriting `TArray`. Its native constructor initializes empty slots;
its destructor enters Rust `class_lifecycle::destroy_class_reps`. Rust owns
remove/reallocate-to-zero, checked post-remove callback, and final release.
There is no native base-array destructor left to perform a second cleanup.
Copying is deleted. Native member destruction order is unchanged.

Compiler assertions preserve the old array size/alignment and slot offsets;
the existing generated ABI assertions also passed in the real Core/Engine
build. Indexed access retains the old `checkSlow(Index <= ArrayNum)` behavior,
including its one-past-end assertion quirk; this port does not tighten it.
Explicit `UClass::Destroy` still does not prematurely release `ClassReps`.
Native typed allocation/exception leaves, other metadata member destructors,
constructors, and signal/guard handling remain native. No additional native
translation unit was removed by this checkpoint.

Verification under `out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed after explicitly selecting the installed `CARGO_HOME` and `RUSTUP_HOME`.
  An initial isolated-HOME configure failure is retained in
  `class-reps-mac-build/`; its accidental isolated toolchain was removed.
  Final build output is in `class-reps-final-mac-build/`.
- `ctest --preset macos-arm64 -R '^(class_buffer_lifetime_runtime|class_lifecycle_contracts|class_registration_contracts|native_registration|native_exception_handoff|object_lifecycle_runtime|object_gc_runtime|map_load_recovery_runtime)$' --output-on-failure`
  passed 8/8 in 9.92 seconds. The new native regression uses actual
  engine-allocated `UClass` objects, normal deletion, and a real derived C++
  constructor throwing `INT(8673)` after record allocation. Both paths observed
  one realloc-to-zero, no direct free of the original allocation, and no
  retained object-table entry. Evidence:
  `out/macos-arm64/Testing/HP2/class_buffer_lifetime_runtime/Artifacts/class-buffer-lifetime.json`.
- An independent C++ SDK consumer compiled and linked against actual SDK
  objects and the Rust static library, then exited zero. It observed record
  values 73/91, stride 16, one zero-size realloc on normal destruction and on
  constructor failure, no direct frees, and the original exception 8673.
  Commands, binary and output remain in `class-reps-final-sdk-*`; disposable
  consumer source was removed after proof.
- `python3 Build/check_bundle.py` passed for the rebuilt installed arm64 app,
  including ad-hoc signature verification and embedded OpenAL.
  Evidence: `class-reps-final-bundle-check/`.

Unresolved verification is not a pass:

- The earlier ten-test run passed nine cases but `renderer_smoke_wgpu` was
  blocked: zero presented frames, 300 occluded, zero outdated/other failures.
  Output is retained in `class-reps-targeted-ctest/`. It was not retried,
  forced into the foreground, or bypassed. Earlier presented-frame evidence
  belongs to the preceding checkpoint, not this run.
- `cargo clippy --offline --package hp2-core --package hp2-ffi --all-targets --no-deps -- -D warnings`
  failed on existing lint debt (88 library and 107 library-test diagnostics,
  including the unchanged class-registration argument count). The command
  did not establish a clean FFI lint result. Full output is retained in
  `class-reps-clippy/`; no lint was suppressed.
- The prior full-suite determinism failure remains unresolved; a new full-suite
  pass is not claimed. Retail progression, physical input, audible audio,
  saves, Linux execution, and allocation failure during active destructor
  unwinding were not established by this checkpoint. Feature gates, retail
  data, and goldens remain unchanged.

Additional end-to-end reflection verification:
`ctest --preset macos-arm64 -R '^erased_class_carcass$' --output-on-failure`
passed 1/1 under `data-prototype` (0.55 seconds). This exercises actual reflected
construction, native property serialization, exception propagation and GC.
Native runtime evidence is written beneath each test's isolated
`HP2_ARTIFACT_DIR`; the manual SDK consumer's compile/link/run records are
separate acceptance evidence under `Testing/HP2/Manual/Artifacts/mac-rust-completion/`.
The overall macOS Rust port remains incomplete.

## Rust-owned process signals and deferred exit checkpoint

Invariant: an exit signal must not jump over live Rust frames or skip their
destructors. Fatal signals and repeated exit signals still terminate.

The actual SDK reproduced the old failure before the change. A separately
compiled `no_std` Rust object held a live `Drop` guard while raising `SIGTERM`
inside the native `guard` macro. The original handler jumped back to C++:
`native catch=1 exit-requested=1 Rust destructors=0`, exit 7. A native exception
fence did not make that stack jump safe.

`hp2-core::unix_signal` now owns the signal handlers, cumulative exit-signal
counter, and atomic graceful-exit flag. The first HUP/QUIT/TERM requests exit
and returns normally. A later exit signal reaching the handler, or a fatal
signal, uses `_exit(1)`. The original `SA_RESETHAND` behavior is retained:
repeating the same exit signal without reinstalling the handler invokes the
OS default disposition instead. `SIGPIPE` remains ignored.

The handler uses only lock-free atomics, static-byte `write`, and `_exit`.
It no longer logs through the SDK, calls `appRequestExit`, flushes stdio,
runs atexit handlers, or jumps into a saved native stack. The unreachable
legacy `AlreadyAborting`/`appExit` branch was removed. The launcher still
reinstalls its crash reporter after common signal initialization.

All shipping native callers and native tests now use
`hp2_rs_core_unix_initialize`, `hp2_rs_core_exit_requested`, and
`hp2_rs_core_set_exit_requested`. `__Context`, its saved environment,
native signal recovery, the mutable `GIsRequestingExit` export, and the old
handler-taking/bootstrap exports were removed. Native guards retain their
real C++ exception fences and diagnostic unwind labels.

Verification on macOS arm64:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed, exit 0, 27.454 seconds.
- `ctest --preset macos-arm64 --output-on-failure -R '^(unix_signal_contracts|rng_seed_contracts|transaction_contracts|native_exception_handoff|class_buffer_lifetime_runtime|class_lifecycle_contracts|core_startup_runtime|object_lifecycle_runtime|map_load_recovery_runtime)$'`
  passed all eight matching registered tests, 7.80 seconds. No registered
  test matched `core_startup_runtime`; no startup-test result is claimed.
  Five matching tests were `data-none`, three `data-prototype`.
  Signal subprocess contracts cover live Rust cleanup for HUP/QUIT/TERM,
  distinct/same repeated signals, fatal termination, and actual pipe EPIPE.
- The same Rust probe object linked against the rebuilt actual SDK produced
  `native catch=0 exit-requested=1 Rust destructors=1`, exit 0. This is an
  actual SDK consumer, not synthetic engine types or a fake vtable.
- The actual `hp2_game`, under isolated HOME and `data-prototype`, reached
  `Entering main loop.` with the Metal device initialized. Sending `SIGTERM`
  then produced the signal/exit-request diagnostics, normal engine teardown,
  `Garbage: objects: 20467->0`, `Object subsystem successfully closed.`, and
  process exit 0. Sound was disabled. The one observed frame was occluded;
  this proves deferred process exit, not visual renderer acceptance.
- `python3 Build/check_bundle.py` passed, exit 0: arm64 executable,
  `com.hp2.native`, valid ad-hoc signature, embedded OpenAL, no warnings.

Manual compile/link/run commands and output are retained under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/` in
`signal-mac-build`, `signal-targeted-ctest`, `signal-drop-before-runtime`,
`signal-drop-fixed-sdk-compile`, `signal-drop-fixed-combine`,
`signal-drop-fixed-sdk-link`, `signal-drop-fixed-sdk-runtime`, and
`signal-final-bundle-check`. The game command, configuration, output, and
result are in `signal-game-runtime/Artifacts/`. Signal child output is in
the registered test's isolated `HP2_ARTIFACT_DIR`. Disposable SDK probe
sources were removed after proof; compiled before/after evidence remains.

Historical oracle generators deliberately retain their original `__Context`
storage, symbol filters, and oracle-local `GIsRequestingExit` state: they
compile archived original headers and TUs, not the shipping SDK. For example,
`gen_bits.sh` archives `4f69478` before compiling its harness. Removing that
storage would break the original oracle, not finish a runtime migration.
No oracle fixture or acceptance golden was regenerated.

This removes unsafe native signal ownership, not another native translation
unit or the remaining engine/platform SDK shells. The preceding renderer
zero-presented-frame block, full-suite determinism failure, and Clippy debt
remain unresolved and were not retried to green. Retail progression,
physical input, audible audio, saves, and Linux runtime acceptance remain
unverified. The full macOS Rust port is not finished.

## Rust-defined FName and FMemCache SDK checkpoint

Invariant: every `FName` and `FMemCache` SDK method that C++ callers link
against is defined by the Rust staticlib. Each failure still reaches the C++
caller as the original exception, with the original `GErrorHist` unwind labels
and the original absolute `__FILE__`/line assertion text.

`Core/Src/UnName.cpp` and `Core/Src/UnCache.cpp` are deleted. `hp2-ffi` now
exports the C++ symbols (`#[unsafe(export_name)]`, Itanium-mangled names):

- `FName`: the constructors, `IsValid`, `GetMaxNames`, `GetInitialized`,
  `GetEntry`, `Hardcode`, `StaticInit`, `StaticExit`, `DisplayHash`,
  `DeleteEntry`, `operator<<(FArchive&, FNameEntry&)` and `AllocateNameEntry`.
- `FMemCache`: `Init`, `Exit`, `Flush`, `Create`, `Tick`, `CheckState`, `Exec`,
  `Status`, `Get`, `GHash`, `FCacheItem::Unlock` and `FCacheItem::GetSize`.

Both modules are `cbindgen:ignore`, so `hp2_rs.h` does not redeclare SDK
classes. `Hp2NameServices` and `Hp2CacheServices` are exported explicitly
because no exported signature names them now. The old
`hp2_rs_core_cache_*` and `hp2_rs_core_name_*` kernel entrypoints were
removed with their C++ shells. Two exceptions remain:

- `hp2_rs_core_name_hardcoded_*`.
- `hp2_rs_core_name_source_file`, which the native assertion leaf uses for
  `__FILE__`.

Rust cannot catch C++ exceptions or append `appUnwindf` history, so a new
native exception/rethrow bridge, `Core/Src/Hp2SdkGuard.cpp`, remains. The
existing native leaf tables `Core/Src/Hp2NameServices.cpp` and
`Core/Src/Hp2CacheServices.cpp` (allocator, `appErrorf`/`appFailAssert`,
logging) also remain; they are not ported by this checkpoint. The bridge:

- `Hp2CoreSdkGuardFlags` publishes `DO_GUARD_SLOW`, `DO_SLOW_CLOCK` and
  `DO_CLOCK_SLOW`.
- `Hp2CoreSdkRethrow` rethrows the pending status inside nested per-label
  `guard` frames.

The Rust side lists the labels the original frames would have recorded, for
example `FMemCache::CheckState` or the walk/hash sub-frames.
`crates/hp2-ffi/build.rs` passes the canonical Core source directory, so
assertion messages keep the original absolute paths.

`hp2_dxt1_tests`, `hp2_eaxa_tests` and `hp2_data_identity_tests` link the
staticlib without `hp2_core`. The codegen-unit archive members they pull also
contain the SDK methods, which reference Core statics. These targets now link
with `-dead_strip` (Apple) or `--gc-sections` (other platforms). The Linux
branch has not been built or run.

Defect found and fixed by the SDK smoke: the first `GHash` export omitted the
implicit `this`, so it hashed the object pointer. No shipping C++ calls
`FMemCache::GHash` outside the deleted inline header code, so the defect was
latent. The export now takes `(this, id)`.

Verification on macOS arm64:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed, exit 0.
- Seventeen targeted ctests passed 17/17: `abi_widths`, `vm_convert_contracts`,
  `file_stream_contracts`, `package_flow_contracts`, `core_name_replay`,
  `linker_name_archive`, `class_buffer_lifetime_runtime`,
  `package_map_runtime`, `map_load_recovery_runtime`,
  `object_lifecycle_runtime`, `erased_class_carcass`, `package79_manifest`,
  `native_exception_handoff`, `dxt1_codec`, `eaxa_decoder`,
  `data_identity_contracts` and `abi_inventory_contract`. The cache replay
  oracle is not registered in ctest. Run directly,
  `cargo test -p hp2-core --test cache_replay` passed 1/1.
- Full suite `ctest --preset macos-arm64 -j6` passed 157/158, exit 8. The only
  failure is `determinism_double_run` (`data-prototype`). Its first divergent
  normalized line, 174, is `WgpuDrv: Lock outcome code 4: no window frame
  presented this tick` in run 1 versus `CutLog:Issuing to Owl:flyto owlstart
  Cue:_10UniqueCue` in run 2. Unpresented-frame lines numbered 270 versus
  254. This is the same window-presentation divergence recorded above. It
  was not retried or normalized away.
- An actual-SDK consumer linked the Core objects, `Hp2SdkGuard.cpp.o` and
  `libhp2_ffi.a`, then passed all 17 checks with exit 0:
  - FName: add, find, miss and the hardcoded `NAME_Critical`.
  - FName errors: construction before `StaticInit` produced history
    `FName::FName`; a second `StaticInit` produced `FName::StaticInit` with the
    `.../Core/Src/UnName.cpp] [Line: 135]` assertion.
  - FMemCache: Create, Get hit, Get miss leaving the slot untouched, GetSize,
    GHash, CheckState and Flush/Exit.
  - FMemCache errors: a second `Init` produced `FMemCache::Init` with
    `UnCache.cpp] [Line: 31]`; `CheckState` after `Exit` produced
    `FMemCache::CheckState` with `[Line: 205]`.
  - `StaticExit`.
  The disposable source was removed. The binary and output remain under
  `name-cache-sdk-runtime`.
- The installed app ran `python3 Build/game_test.py run --app
  dist/macos-arm64/HarryPotter2.app --data-root HarryPotter2/Unreal --map
  Maps/Adv1Willow.unr --renderer wgpu --ticks 600 --timeout 180 --no-sound`
  under `data-prototype` and passed, exit 0:
  - Both levels were brought up for play.
  - Teardown logged `Garbage: objects: 22384->0`, `Object subsystem
    successfully closed.` and `Name subsystem shut down`.
  - No frame was logged as unpresented.
- `python3 Build/check_bundle.py` passed, exit 0, with no warnings.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`name-cache-sdk-cutover/` and `name-cache-sdk-runtime/`.

Oracle generators compile archived original TUs at `4f69478`, not the
working tree. Because `compile_commands.json` no longer lists the retired TUs,
`gen_name.sh`, `gen_cache.sh`, `gen_light.sh` and `gen_skeletal.sh` now take
their product flags from `UnAnsi.cpp`. Before the change, the flags were
verified identical. No oracle fixture or golden was regenerated.
`Tests/Fixtures/abi-inventory.json` is a pinned original-source inventory and
was left unchanged.

Still unresolved:
- The renderer zero-presented-frame block.
- The determinism failure.
- Clippy debt.
- Linux.
- Retail progression, audible audio and saves.

The full macOS Rust port is not finished.

## Rust-defined FMemStack and static native lookup checkpoint

Invariant: the `FMemStack`/`FMemMark` SDK methods, `GNativeLookupFuncs`,
`FindNative` and `FindCoreUObjectNative` that C++ links against are defined by
the Rust staticlib. Failures reach the C++ caller as the original exception,
with the original `GErrorHist` labels and absolute `__FILE__`/line text.

`Core/Src/UnMem.cpp` and `Core/Src/UnCoreNative.cpp` are deleted, as are the
`hp2_rs_core_mem_*` stack/mark entrypoints, `hp2_rs_core_native_lookup`,
`hp2_rs_core_vm_checked_lookup`, `Core/Inc/Hp2CoreNativeServices.h` and the
`FMemStack::PushBytesSourceFile` constant in `UnMem.h`.
`hp2_rs_core_mem_swap_bytes` (`appMemswap`) stays exported.

- `crates/hp2-ffi/src/core_mem.rs` holds an inline `cbindgen:ignore` `sdk`
  module. It exports `FMemStack::{Init, Tick, Exit, GetByteCount, PushBytes,
  AllocateNewChunk, FreeChunks}`, both `FMemMark` constructors and
  `FMemMark::Pop`, and reads the C++ `GHp2MemServices` table.
  - Guard labels follow the originals. `Tick` uses `FMemStack::InitStack`.
  - A failing nested `Tick` inside `Exit` unwinds `InitStack` and then `Exit`.
  - A growth failure inside `PushBytes` unwinds `AllocateNewChunk`. The
    `guardSlow` frames and `PushBytes` slow checks apply only under
    `DO_GUARD_SLOW`.
- `crates/hp2-ffi/src/core_native_lookup.rs` is `cbindgen:ignore`. It owns
  the 32-slot table, which `HP2StaticPackages.cpp` still fills. `FindNative`
  keeps ascending first-hit traversal and calls each slot as `C-unwind`, so an
  exception from a C++ handler still propagates to the caller.
  `FindCoreUObjectNative` is the checked catalog. The original's fallback
  `GCoreUObjectNatives` table was empty and was dropped.
- `crates/hp2-ffi/build.rs` now publishes the canonical Core directory
  (`HP2_CORE_DIR`). `sdk_guard::core_file!` covers both `Src/*.cpp` and
  `Inc/*.h` `__FILE__` values.
- `gen_mem.sh` and `gen_light.sh` take the archived `UnMem.cpp` flags from
  `UnAnsi.cpp`; before the change they were verified identical in
  `compile_commands.json`. No oracle fixture or golden was regenerated.

Verification on macOS arm64:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed, exit 0, after each cutover. `nm` on `hp2_game` and on the installed
  bundle shows the `FMemStack`, `FMemMark`, `FindNative`,
  `FindCoreUObjectNative` and `_GNativeLookupFuncs` definitions coming from the
  staticlib. `hp2_rs.h` no longer declares any of them.
- `cargo test -p hp2-core --test mem_replay --test mem_stack_contracts`
  passed 1/1 and 4/4. `cargo test -p hp2-ffi --lib core_native_lookup` passed
  1/1.
- FMemStack build: full `ctest --preset macos-arm64 -j6` passed 157/158, exit
  8. The only failure is `determinism_double_run` (`renderer.log_diverged`,
  `data-prototype`), with 258 vs 267 unpresented-frame lines. It is the same
  window-presentation divergence as above and was not retried.
- Native-lookup build:
  - 16 targeted ctests passed 16/16, including `abi_widths`, which covers the
    `FindNative` miss, plus `native_registration`, `vm_registry_contracts`,
    `mem_stack_contracts`, `scene_lighting_contracts`, `core_memswap` and the
    dead-stripped `dxt1_codec`, `eaxa_decoder` and `data_identity_contracts`.
  - `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
    157/157. The determinism test was excluded so the failed test was not
    rerun. A cancelled partial full run in between is not evidence.
- An actual-SDK consumer linked the Core objects without `UnMem.cpp.o`, plus
  `Hp2SdkGuard.cpp.o` and `libhp2_ffi.a`. It passed 9/9 with exit 0:
  - Aligned pushes and growth chunks, `GetByteCount`, `FMemMark::Pop`, and
    `Tick`/`Exit` on an empty stack.
  - `Tick` with a live chunk produced history `FMemStack::InitStack`, and
    `Exit` produced `FMemStack::InitStack <- FMemStack::Exit`. Both carried
    `Assertion failed: TopChunk==NULL [File:.../Core/Src/UnMem.cpp] [Line: 43]`,
    which matches line 43 of the retired source.
  The disposable source was removed. The binary and output remain under
  `mem-sdk-runtime/`.
- The installed app ran `game_test.py run` on `Maps/Adv1Willow.unr`, WGPU, 600
  ticks, `--no-sound`, under `data-prototype`, exit 0:
  - Both levels were brought up for play.
  - Teardown logged `Garbage: objects: 22384->0`, `Object subsystem
    successfully closed.` and `Name subsystem shut down`.
  - No unpresented frame was logged.
- `python3 Build/check_bundle.py` passed, exit 0, with no warnings.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`mem-native-sdk-cutover/` and `mem-sdk-runtime/`.

Still unresolved: the renderer zero-presented-frame block, the determinism
failure, Clippy debt, Linux, retail progression, audible audio and saves. The
full macOS Rust port is not finished.

## Rust-defined UnAnsi SDK checkpoint

Invariant: the `UnFile.h` math, memory, string, RNG and rand-trace functions,
and the `FAppRandTrace*Scope` constructors and destructors, that C++ links
against are defined by the Rust staticlib. Failures reach the C++ caller as
the original exception, with the original guard labels and `__FILE__`/line
text.

- `crates/hp2-ffi/src/core_ansi_sdk.rs` (`cbindgen:ignore`) exports 89 mangled
  symbols over the existing `core_ansi*.rs` kernels and reads the C++
  `GHp2AnsiServices` / `GHp2AnsiTraceServices` tables.
  - Before the C++ was removed, the C-collated export list was verified equal to
    the corresponding `UnAnsi.cpp.o` definitions.
  - `appStrncat` keeps its `appStrncat` guard label. `appAtoi`/`appStrtoi`
    report `GHp2AnsiAtoiFile` at lines 2213 and 2229.
- Scope classes:
  - Constructors write the declared member initializers before the kernel
    fills the handle.
  - Apple arm64 constructors and non-deleting destructors return `this` in
    `x0` (disassembly of the retired C++ showed `mov x0, x19; ret`), so every
    `C1`/`C2`/`D1`/`D2` export returns `this`.
  - A failing destructor status calls `std::terminate`, as the original rethrow
    out of an implicitly `noexcept` destructor did. The libc++ message differs
    because no exception is in flight.
- 71 `hp2_rs_core_ansi_*` forwarders lost their C export and are now
  crate-internal. `native_scope_begin`/`_end` remain Rust-called from
  `core_vm_frame.rs`.
- `Core/Src/UnAnsi.cpp` is reduced, not deleted. It keeps only the native ABI
  boundaries no compiler-verified Rust bridge covers:
  - `appSprintf`: C varargs; the AArch64 Linux `va_list` differs from Apple's.
  - `appGetGMTRef`: returns an `FString` indirectly.
  - `appQsort`: a comparator that may raise through libc frames.
  - `Hp2AnsiTraceCheckFail` and `GHp2AnsiAtoiFile`: they carry this file's
    `__FILE__`.
  - `gen_*.sh` fixture scripts still take their flags from this TU.

Verification on macOS arm64 (`data-none` unless stated):

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  passed, exit 0, both before and after `rustfmt`.
  - `nm` shows `UnAnsi.cpp.o` now defines only the adapters above plus inline
    template instances, and `libhp2_ffi.a` defines all 89 ported symbols.
  - `hp2_rs.h` declares no removed forwarder.
- 23 targeted ctests passed 23/23 (`core_parse_*`, `core_pure_ansi`,
  `rng_seed_contracts`, `vm_string_contracts`, `string_storage_contracts`,
  `fstring_archive`, `core_app_format`, `core_startup_devices`, among others).
  After `rustfmt`, 10 of those were rerun and passed 10/10.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157. The determinism failure recorded above stands and was not rerun.
- `cargo test -p hp2-ffi --release core_ansi` built and linked the test
  binary; no tests matched the filter.
- An actual-SDK consumer compiled against the real headers linked the live
  Core objects with `libhp2_ffi.a` and passed 45/45, exit 0:
  - Math, memory, string and UTF-8 results.
  - `appSprintf`/`appGetGMTRef` through the kept adapters.
  - RNG ranges and the trace ordinal.
  - Three passes over every scope class, including heap `new`/`delete`, with
    the member initializers checked. Correction, recorded later: the smoke
    object references neither `FFrame` constructor. The four `FFrame&`-taking
    scopes (`FunctionScope`, `RelevanceScope`, `RelevanceCallScope`,
    `PreBeginRelevanceScope`) therefore received storage that was never
    constructed as an `FFrame`. Those cases prove only that the entry points
    link, construct and destruct. They are not evidence of frame-dependent
    behavior.
  - Out-of-range `appAtoi`/`appStrtoi` threw through the Rust frames with
    `Assertion failed: Value>=-static_cast<long>(MAXINT)-1 && Value<=static_cast<long>(MAXINT) [File:/Users/mike/Code/Default/hp2/HarryPotter2/Unreal/Core/Src/UnAnsi.cpp] [Line: 2213]`
    (and `[Line: 2229]`).

  The disposable source was removed; the binary and output remain under
  `ansi-sdk-runtime/`.
- The installed app ran `game_test.py run` on `Maps/Adv1Willow.unr`, WGPU, 600
  ticks, `--no-sound`, under `data-prototype`, both before and after
  `rustfmt`.
  - Both runs: exit 0, `passed: true`, no failure markers.
  - The final log shows both levels brought up for play,
    `Garbage: objects: 22384->0`, `Object subsystem successfully closed.` and
    `Name subsystem shut down`.
- `python3 Build/check_bundle.py` passed, exit 0, before and after `rustfmt`.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`ansi-sdk-cutover/` and `ansi-sdk-runtime/`.

Still unresolved:

- The four native adapters in `UnAnsi.cpp`.
- The renderer zero-presented-frame block and the determinism failure.
- Clippy debt.
- Linux.
- Retail progression, audible audio and saves.

The full macOS Rust port is not finished.

## Rust-defined UnMisc plain-ABI SDK checkpoint (first slice)

Invariant: `appMemswap`, `appMemCrc`, `appStrCrc`, `appStrCrcCaps`,
`appCeilLogTwo`, `appMD5Init`/`Update`/`Final`/`Transform`/`Encode`/`Decode`,
`appIsPureAnsi`, `appFExt` and `appPathLeaf` are defined only in Rust and
return what the C++ forwarders returned.

What moved:

- `crates/hp2-ffi/src/core_misc_sdk.rs` (`cbindgen:ignore`) exports the 14
  mangled names. Each calls the existing kernel directly.
  - None has a failure path, so all use the plain `extern "C"` ABI.
  - The originals' `guardSlow` frames are compiled out (`DO_GUARD_SLOW` 0).
  - `appPathLeaf(NULL)` still returns an empty `TCHAR` string.
- The 13 kernels these forwarded to lost `#[unsafe(no_mangle)]` and left
  `hp2_rs.h`; `UnMisc.cpp` was their only C/C++ caller:
  - `hp2_rs_core_mem_swap_bytes`, `_mem_crc`, `_string_crc`,
    `_bits_ceil_log_two`, the six `_md5_*` and `_ansi_is_pure_ansi`.
  - `_app_fext` and `_app_path_leaf`.
- The 14 C++ definitions were removed from `UnMisc.cpp`. `GCRCTable` stays
  native because C++ `appStrihash` and the name hashing read it.

What stays native in `UnMisc.cpp`, for now:

- Every other definition still in that file.
  - FString by-value returns, varargs (`Logf`, `Printf`, `appThrowf`,
    `appUnwindf`) and own-`__FILE__` leaves follow the native-adapter rule.
  - The remaining plain-ABI symbols have not been ported yet: `Parse*`,
    `Localize*`, the FString ctor/dtor/`operator=`, `appInit`/`appExit`, file
    helpers and the persistence cache.

Verification:

- C-collated (`LC_ALL=C`) symbol sets, taken before deletion:
  - The 14 names `UnMisc.cpp.o` defined equal the 14 `export_name`s
    (`comm -3` empty).
  - After the build, the rebuilt `.o` defines and references none of them.
  - `libhp2_ffi.a` defines all 14 as `T`, and `hp2_rs.h` declares none of the
    13 internalized kernels.
- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  exited 0. `rustfmt --edition 2024 --check` on the new module was clean.
- 17 targeted ctests passed 17/17:
  - `core_parse_guid`, `core_native_path`, `core_pure_ansi`, `core_memswap`,
    `core_crc`, `core_md5`, `core_app_path_leaf`.
  - `fstring_archive`, `vm_string_contracts`, `string_storage_contracts`.
  - `package_file_contracts`, `package79_manifest`, `core_name_replay`,
    `linker_name_archive`, `package_summary_archive`.
  - `config_ini_cache_filenames`, `engine_path_define_paths`.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157. The determinism failure recorded above stands and was not rerun.
- An actual-SDK consumer linked the live Core objects with `libhp2_ffi.a`. It
  exited 0, and 19/19 results matched independent Python oracles:
  - A reimplemented UE1 `appMemCrc` (MSB-first, `0x04C11DB7`).
  - `hashlib` MD5 of `abc` and of 1000 split-update bytes.
  - The `appMD5Transform` and single-block `appMD5Update` states agree.
  - Encode/decode round trip, `appCeilLogTwo` boundaries, caps-CRC
    equivalence, pure-ANSI (`0x100` fails), and `appFExt`/`appPathLeaf`
    including NULL and a trailing slash.

  The source was removed; the binary and outputs remain under
  `misc-sdk-runtime/`.
- `game_test.py run` on `Maps/Adv1Willow.unr`, WGPU, 600 ticks, `--no-sound`,
  under `data-prototype`:
  - Exit 0, `passed: true`, no failure markers.
  - `Garbage: objects: 22470->0`, then `Exiting.`
- `python3 Build/check_bundle.py` passed, exit 0.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`misc-sdk-cutover/` and `misc-sdk-runtime/`.

Still unresolved:

- The rest of `UnMisc.cpp`, listed above.
- The UnAnsi adapters.
- The renderer zero-presented-frame block and the determinism failure.
- Clippy debt.
- Linux.
- Retail progression, audible audio and saves.

The full macOS Rust port is not finished.

## Rust-defined UnMisc Parse SDK checkpoint (second slice)

Invariant: the buffer, scalar, `FGuid` and object `Parse` overloads,
`ParseParam`, `ParseUBOOL`, `ParseCommand`, `ParseNext`, and the buffer forms
of `ParseToken` and `ParseLine` (18 symbols) are defined only in Rust. Their
results, NULL-stream handling and failure diagnostics, including the guard
frame label, match the C++ they replace.

What moved:

- `crates/hp2-ffi/src/core_parse_sdk.rs` (`cbindgen:ignore`) defines the 18
  mangled names over the existing kernels.
  - Status kernels (integer, float, bool, GUID, object) use `C-unwind` and
    rethrow a captured exception with `sdk_guard::finish` inside the
    original's single `guard` label: `ParseINT`, `ParseDWORD`, `ParseGUID`,
    `ParseUObject` and so on.
  - Value-only kernels cannot raise and use plain `extern "C"`.
  - `ParseObject` passes the authoritative `hp2_core::name::NAME_SIZE`
    (128).
  - `ParseUBOOL` passes the `GTrue` array.
- The 11 kernels lost `#[unsafe(no_mangle)]`; `UnMisc.cpp` was their only
  C/C++ caller.
- `crates/hp2-ffi/cbindgen.toml` now names `ParseIntegerServices` and
  `ParseObjectServices` in `export.include`, because no exported signature
  references them any more.
- The `GHp2ParseIntegerServices` and `GHp2ParseObjectServices` tables have
  external linkage. They are declared in the new
  `Core/Inc/Hp2ParseServices.h`.
- The 18 C++ definitions and the `ParseIntegerValue` helper were removed.

What stays native:

- The `Hp2FfiCall` service leaves (`appAtoi`, `appStrtoi`, `appStricmp`,
  `StaticFindObject`). They catch C++ exceptions.
- `Parse(FString&)`, `Parse(FName&)`, `ParseToken(FString&)`, the by-value
  `FString ParseToken`, and `ParseLine(FString&)`. They assign through C++
  operators inside their `guard` frames, and the by-value one returns an
  FString through sret.

Verification:

- Before deletion, the C-collated symbol set of the 18 `UnMisc.cpp.o`
  definitions equalled the Rust export names (`comm -3` empty).
- After the build:
  - The `.o` defines none of them. It references 5 of them from the native
    FString/FName overloads and other native code.
  - `libhp2_ffi.a` defines all 18 as `T`.
  - The `.o` exports both tables as `S`.
  - `hp2_rs.h` declares no internalized kernel and keeps both structs.
  - Every one of the 32 names from both UnMisc slices, compared as an exact
    `LC_ALL=C` list, is defined (`T`) in the installed
    `HarryPotter2.app/Contents/MacOS/HarryPotter2`. None is missing.
- The `ParseObject` export loads the immediate `#0x80` (128). An initial draft
  hard-coded 64. Review caught it; it was confirmed against `UnName.h`, fixed,
  and verified in a fresh build.
- The build exited 0.
- The 16 intended targeted ctests passed:
  - `core_parse_integer`, `core_parse_scalar`, `core_parse_guid`,
    `core_guid_generation`, `core_parse_object`.
  - `command_line_load`, `runtime_init_contracts`, `config_archive_runtime`,
    `core_startup_devices`.
  - The six `config_ini_*` tests and `render_console_contracts`.
- Finding: the targeted selection regex (`ini`) accidentally matched
  `determinism_double_run`, which ran once in that batch and failed.
  - `reason_code` `renderer.log_diverged`, normalized logs diverge at line
    172.
  - The runs presented 47 frames with 253 occluded, and 45 frames with 255
    occluded. That is consistent with the standing occluded-window finding,
    but no baseline comparison proves the cause.
  - It was not rerun.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157.
- An actual-SDK consumer linked the live Core objects with `libhp2_ffi.a` and
  exited 0:
  - Buffer parse with a long value: 100 characters at a 128 bound, 63 at a 64
    bound.
  - `ParseParam`, all eight integer widths including 64-bit values, `FLOAT`,
    the missing key, three `ParseUBOOL` forms, and `FGuid`.
  - `ParseObject` for `None` (NULL result) and a missing key (destination
    unchanged).
  - `ParseCommand` matching and mismatching, `ParseNext(NULL)`, quoted
    `ParseToken`, `ParseLine`, `ParseLine(NULL)`, and the native FString
    overloads through the Rust exports.
  - Out-of-range `Parse(INT&)` and `Parse(DWORD&)` threw through the Rust
    frames. The messages were the original `UnAnsi.cpp` assertions at
    `[Line: 2213]` and `[Line: 2229]`, and `GErrorHist` showed the frames
    `ParseINT` and ` <- ParseDWORD`.
  - A first smoke draft fed `Parse(DWORD&)` 4000000000 on the normal path and
    aborted. That value is out of range for the kernel's `appStrtoi`, which
    is unchanged, so the input became the second failure-path case.

  The source was removed; the binary and output remain under
  `parse-sdk-runtime/`.
- `game_test.py run` on `Maps/Adv1Willow.unr`, WGPU, 600 ticks, `--no-sound`,
  under `data-prototype`:
  - Exit 0, `passed: true`, no failure markers.
  - `Garbage: objects: 22470->0`, then `Exiting.`
- `python3 Build/check_bundle.py` passed, exit 0.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`parse-sdk-cutover/` and `parse-sdk-runtime/`.

Observed, not investigated or changed: `crates/hp2-engine/src/demo_pending.rs`
defines a local `NAME_SIZE = 64`; `UnName.h` has 128. Whether the local value
bounds something other than `FName` is unknown.

Still unresolved:

- The rest of `UnMisc.cpp`: `Localize*`, the FString members and
  constructors, `appInit`/`appExit`, file helpers, the persistence cache, and
  the native adapters.
- The UnAnsi adapters.
- The renderer zero-presented-frame block and the determinism failure.
- Clippy debt.
- Linux.
- Retail progression, audible audio and saves.

The full macOS Rust port is not finished.

## Rust-defined UnMisc text SDK checkpoint (third slice)

Invariant: `appStaticString1024`, `appAnsiStaticString1024`, `appStrfind`,
`appSpc`, `appTrimSpaces`, `appToAnsi`, `appToUnicode`,
`appFromAnsi(const ANSICHAR*)` and `appFromUnicode` (9 symbols) are defined
only in Rust. They return the same ring-buffer storage and converted text as
before.

What moved:

- `crates/hp2-ffi/src/core_text_sdk.rs` (`cbindgen:ignore`) defines the 9
  mangled names.
  - The status forms (both static rings, `appToAnsi`, `appFromAnsi`,
    `appFromUnicode`) are `C-unwind`. Their kernels report only a caught
    panic, which `sdk_guard::finish` raises inside the original's `guard`
    label, as `Hp2RethrowPending` did.
  - `appStrfind`, `appSpc` and `appTrimSpaces` are value-only.
- `appToUnicode` was the one inline C++ body, together with its private
  `appUnicodeStaticString1024` ring. It is now `hp2_core::ansi::to_unicode`:
  - Its own 128 × 1024 `u16` ring.
  - UTF-16 encoding with surrogate pairs above U+FFFF.
  - U+FFFD for surrogates, negative units and values above U+10FFFF.
  - A 1023-unit limit, with a pair that would reach unit 1023 ending the
    string.
- The 8 forwarded ffi kernels lost `#[unsafe(no_mangle)]`; `UnMisc.cpp` was
  their only C/C++ caller. The `#else` branches for builds without
  `HP2_HOST_WCHAR_TCHAR` went with the C++. Every supported target defines it
  (`UnGnuG.h`), and `hp2-core` refuses other targets.
- Stays native: `appFromAnsi(TArray<TCHAR>&, const ANSICHAR*)`, which resizes
  a C++ `TArray`.

Verification:

- Before deletion, the 9 C-collated `UnMisc.cpp.o` definitions equalled the
  Rust export names.
- The first build failed with exit 1: `UnMisc.cpp:1191` still defined
  `appFromUnicode` over the de-exported kernel, because the cut range missed
  it. It was removed and rebuilt; the build exited 0.
- After the build:
  - The `.o` defines none of the 9.
  - `libhp2_ffi.a` defines all 9.
  - The installed app defines all 9 exactly once.
  - `hp2_rs.h` declares none of the 8 kernels.
- Targeted ctests passed 9/9 (`determinism_double_run` explicitly
  excluded): `core_pure_ansi`, `core_trim_spaces`, `core_spc`,
  `fstring_archive`, `property_text_contracts`, `vm_string_contracts`,
  `string_storage_contracts`, `procedural_texture_runtime`,
  `config_ini_unicode_roundtrip`.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157.
  - In that run, `renderer_smoke_wgpu` presented 300/300 frames with 0
    occluded, `status: pass`.
  - So the earlier zero-presented-frame block was not present in this
    session's window environment.
- An actual-SDK consumer exited 0, and 24/24 lines matched expectations
  computed independently in Python:
  - Ring periods of 128, with slots zeroed on reuse, for the TCHAR, ANSI and
    UTF-16 rings. `appToAnsi` shares the ANSI ring.
  - `appStrfind` with and without a non-alphanumeric lead-in.
  - `appSpc` at 5, 300 (clamped to 255) and -1.
  - `appTrimSpaces`, including its one-byte floor.
  - Latin-1 narrowing with `?`.
  - `appToUnicode` across the BMP, an astral pair (from Python's `utf-16-le`
    codec), a lone surrogate, a negative unit, 0x110000, and the
    1021/1022/1023-prefix truncation boundaries.
  - `appFromAnsi`, `appFromUnicode` (pair, lone low, unpaired high), every
    NULL input, and the native `TArray` overload.

  The source was removed; the binary and output remain under
  `text-sdk-runtime/`.
- `game_test.py run` on `Maps/Adv1Willow.unr`, WGPU, 600 ticks, `--no-sound`,
  under `data-prototype`:
  - Exit 0, `passed: true`, no failure markers. 600/600 frames presented.
  - `Garbage: objects: 22384->0`, then `Exiting.`
  - Compared with the Parse-slice run (22470 objects): this run lasted 13.6 s,
    unoccluded, against 60.9 s with 559 frames occluded. `game_test.py` uses
    wall-clock delta time unless `--fixed-dt` is passed.
  - The normalized logs differ only in later cutscene lines missing from the
    shorter run, timestamps and draw-call totals.
  - The object-count difference is attributed to that shorter simulated time
    and was not separately proven.
- `python3 Build/check_bundle.py` passed, exit 0.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`text-sdk-cutover/` and `text-sdk-runtime/`.

Still unresolved:

- The rest of `UnMisc.cpp`: `Localize*`, the FString members and
  constructors, `appCmdLine`/`appInit`/`appExit`, file helpers, the
  persistence cache, and the native adapters.
- The UnAnsi adapters.
- The determinism failure.
- Clippy debt.
- Linux.
- Retail progression, audible audio and saves.

The full macOS Rust port is not finished.

## Rust-defined Localize SDK checkpoint (fourth UnMisc slice)

Invariant: `Localize` and `LocalizeError`/`Progress`/`Query`/`General`, in
both `TCHAR` and `ANSICHAR` key forms (10 symbols), are defined only in Rust.
Each returns the same ring slot and text as before:

- Before startup, or without `GConfig`: the key.
- Otherwise: `Section`/`Key` from `<Package>.<LangExt>`, retried once as
  `<Package>.int`.
- Unless `Optional`: a miss logs `No localization: …` under
  `NAME_Localization` and returns `<?LangExt?Package.Section.Key?>`.

What moved:

- `hp2_core::localize::localize` owns the control flow:
  - Takes the result slot before any leaf runs.
  - The case-insensitive `int` test uses the same `ansi::stricmp` as
    `appStricmp`.
  - Both `appSprintf` formats go through the shared `printf_format::put` and
    `append_text`, so overflow follows `appVswprintfCompat` exactly: output
    is bounded at 1024 elements, an overflowing result is emptied, and NULL
    `%s` prints `(null)`.
- `crates/hp2-ffi/src/core_localize_sdk.rs` (`cbindgen:ignore`) defines the
  10 mangled names.
  - A leaf's captured exception is rethrown in `guard(Localize)`.
  - `ANSICHAR` forms widen through `appFromAnsi`, `Section` before `Key`,
    each in its own `guard(appFromAnsi)`.
  - The removed object's disassembly of
    `Localize(const ANSICHAR*, const ANSICHAR*, …)` confirms that call order.
- Native leaves (`GHp2LocalizeServices` in `UnMisc.cpp`, declared by
  `Core/Inc/Hp2LocalizeServices.h`; `LocalizeServices` is in the cbindgen
  `[export] include` list):
  - `GIsStarted && GConfig`.
  - `UObject::GetLanguage()`.
  - A capturing `GConfig->GetString(…, 1024, Filename)`.
  - The varargs `debugf`.
- `ansi::STATIC_STRING_LEN` (1024) now names the TCHAR ring slot length the
  kernel passes to `GetString`.
- Behaviour that was undefined before is now bounded:
  - Before startup, a key longer than 1023 is truncated to the slot; the
    original `appStrcpy` overran it.
  - The filename is formed under `appSprintf`'s 1024 bound in a buffer of
    that size; the original wrote it, under the same bound, into a
    256-element local.

Verification:

- Before deletion, the 10 C-collated `UnMisc.cpp.o` definitions equalled the
  Rust export names (`localize-removed-cpp.txt`, `localize-rust.txt`).
- The build exited 0, with only pre-existing warnings.
- After the build:
  - The `.o` defines none of the 10.
  - The `.o` references one: `LocalizeError(const ANSICHAR*, …)`, from the
    native runtime leaf `Hp2RuntimeLeaf_MissingDefault`.
  - `_GHp2LocalizeServices` is `S`.
  - `libhp2_ffi.a` defines all 10.
  - The installed app defines each exactly once.
  - `hp2_rs.h` declares `LocalizeServices`.
- `cargo test -p hp2-core --lib localize` passed 8/8, with mock leaves:
  - Key before startup.
  - Single-file hit.
  - `int` retry, marker and log arguments.
  - Case-insensitive `INT` without retry.
  - Optional miss.
  - Each leaf status stopping the lookup.
  - The marker at exactly 1023 elements, and emptied at 1024.
- Targeted ctests passed 8/8 (`determinism_double_run` excluded):
  `localization_contract`, `object_registry_contracts`,
  `vm_object_contracts`, `vm_registry_contracts`, `linker_runtime_contracts`,
  `config_archive_runtime`, `config_ini_typed_getters`,
  `config_ini_cache_filenames`.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157, including `renderer_smoke_wgpu` and `renderer_smoke_maps`.
- An actual-SDK consumer exited 0. It used a real `FConfigCacheIni`
  subclass whose `GetString` override records calls and raises `appErrorf`
  for one key. All 32/32 lines matched Python-computed expectations:
  - Ring slots: the TCHAR form 1/next 2, the `ANSICHAR` `Localize` 3/next 4,
    and the `ANSICHAR` shorthand 2/next 3.
  - The pre-start key, including the default `GPackage`; a 1500-element key
    yields 1023.
  - The hit, retry, all four shorthands, and the active language `int`.
  - The miss marker and its `NAME_Localization` log line.
  - The optional miss with no log.
  - `INT`.
  - A NULL package as `(null)` in the filename, marker and log.
  - The 1000-element package filling 1023, and 1001 emptying the result.
  - A thrown leaf exception surfacing as `code=1`, `GErrorHist=[Localize]`.

  The source was removed; the binary and output remain under
  `localize-sdk-runtime/`.
- `game_test.py run`, `Maps/Adv1Willow.unr`, WGPU, 600 ticks, `--no-sound`,
  under `data-prototype`:
  - Exit 0, `passed: true`, 600/600 frames presented, `Garbage: objects:
    22406->0`, `Exiting.`
  - This run, the text-slice run and the Parse-slice run all logged no
    `No localization` line and no `<?` marker.
  - Against the text-slice run, the normalized log differs only by 13 added
    lines: CutScene13 starting, plus its first `CutLog` commands. Cutscene
    threads are loaded through script `Localize`. The added lines are
    consistent with wall-clock delta time (no `--fixed-dt`), as recorded for
    the text slice; that cause was not separately proven.
- `python3 Build/check_bundle.py` passed, exit 0.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/`, in
`localize-sdk-cutover/` and `localize-sdk-runtime/`.

Still unresolved:

- The rest of `UnMisc.cpp`: the FString members and constructors,
  `appCmdLine`/`appInit`/`appExit`, file helpers, the persistence cache, and
  the native adapters.
- The UnAnsi adapters.
- The determinism failure.
- Clippy debt.
- Linux.
- Retail progression and localized text (no `data-retail` run), audible
  audio and saves.

The full macOS Rust port is not finished.

## Rust-defined UNetPendingLevel checkpoint

`hp2_engine::net_pending` now owns the `UNetPendingLevel` methods that were in
`Engine/Src/UnPenLev.cpp`:

- The constructor (driver load, `InitConnect`, HELLO, or driver discard).
- `NotifyReceivedText`: UPGRADE, FAILURE, FAILCODE (password options removed
  from a URL copy), USES, USERFLAG, CHALLENGE (ngWorldStats checksum, `game=`
  removal, NETSPEED/LOGIN), DLMGR, and WELCOME (package scan, download
  refusal, first download request).
- `TrySkipFile`, `NotifyReceivedFile`, `NotifySendingFile`, `Tick` and
  `SendJoin`, plus the trivial notify answers.
- `ReceiveNextFile` is Rust-internal. Its declaration was removed from
  `Engine/Inc/UnPenLev.h`; it had no other callers.

The C++ file keeps `UPendingLevel`, both `IMPLEMENT_CLASS`es, the guarded
method shells and a `Hp2NetPendingServices` leaf table. The leaves only
construct objects, dispatch virtuals, mutate `FString`/`FURL`/`TArray`
storage, call `Parse`, and format the two runtime (localized) printf strings.
Literal-format text (HELLO, NETSPEED, LOGIN, the log lines, `Downloading '%s'
not allowed`) is formatted in Rust and passed through `"%s"` leaves. The
checksum uses `hp2_core::md5` over `stat_log::encode_utf16le`, the same
encoding as `AStatLog`. The `check` lines keep the original numbers
(87/251/273/274/312/313); the leaf supplies `__FILE__`.

The WELCOME `Filename` for the `DLLEXT` probe is built in a Rust buffer rather
than the original 256-element local. An FName (under `NAME_SIZE`) plus the
extension cannot exceed it, so no truncation case is lost.

The shared `scoped`/`fmt`/`lit!` helpers moved from `level_net` into
`hp2_engine::native_text`. `level_net` now imports them.

Demo pending-level regressions fixed in the same change. Each was confirmed
against the pre-port source (`git show da56c30:.../UnDemoPenLev.cpp`):

- `DemoPendingDriverInitConnect` cast `void*` straight to `FNetworkNotify*`.
  `ULevelBase : UObject, FNetworkNotify` puts that base at a nonzero offset,
  and the original passed `this`. The leaf now converts from
  `UDemoPlayPendingLevel*`.
- `demo_pending.rs` used a local `NAME_SIZE = 64`; the original is 128. It now
  uses `hp2_core::name::NAME_SIZE`.
- `FLAGS=` was parsed with the INT overload (`appAtoi`); the original used the
  DWORD overload. A `parse_dword` leaf was added, and the test service count
  went from 23 to 24.

Verification (macOS arm64):

- `UnPenLev.cpp.o` defined-symbol set, C-collated, before vs after. Removed:
  `UNetPendingLevel::ReceiveNextFile` (intended), plus inline/template
  instantiations this TU no longer emits (`TArray<FDownloadInfo>::Remove`,
  the `FString` copy ctor/assignment, `UDownload::StaticClass`,
  `UObject::GetName`). The link succeeded. Nothing was added. The object now
  imports the ten `hp2_rs_engine_net_pending_*` exports.
- `cargo test -p hp2-engine --lib -- net_pending demo_pending level_net`
  passed 16/16. The 15 `net_pending` tests use mock leaves and cover:
  - Constructor success and discard.
  - The line-87 check.
  - Both UPGRADE outcomes and FAILURE.
  - FAILCODE: case-insensitive `PASSWORD=` removal from the copy only.
  - USES, including DWORD flags `0xffff8001`.
  - USERFLAG.
  - CHALLENGE: login text, `game=` removal from the live URL, host cleared
    on the copy only.
  - The checksum with no pawn, an empty secret, no PRI, and a named PRI.
    The MD5 values were computed with Python
    `hashlib.md5(...encode('utf-16-le'))`.
  - DLMGR unknown-class removal.
  - WELCOME refusal (old and current remote version) and success.
  - Both `NotifyReceivedFile` branches, and the 251/274 checks.
  - Both `Tick` states, and the 312/313 checks.
  - `TrySkipFile` with and without a download, `SendJoin`,
    `NotifySendingFile`.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157, including `abi_inventory_contract`.
- `game_test.py run`, `Maps/Adv1Willow.unr`, WGPU, 600 ticks, `--no-sound`,
  under `data-prototype`:
  - Exit 0, `passed: true`, `Garbage: objects: 22470->0`, `Exiting.`
  - No `No localization` line and no `<?` marker.
  - `WgpuDrv: frames presented=83 occluded=517` (earlier checkpoints: 600
    presented); the run took 57 s against about 14 s before. The window was
    reported occluded for most ticks. The cause was not investigated, and the
    single-player run does not reach `UNetPendingLevel` (no
    `PendingLevel`/`DevNet` lines).
- `python3 Build/check_bundle.py` passed, exit 0.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/net-pending-cutover/`.

Still unresolved:

- The network client path has only mock-leaf coverage. `data-prototype` has
  no IpDrv native or `.u`, and `Default.ini` names `IpDrv.TcpNetDriver`, so
  no runtime client handshake or download can run.
- `Build/net_fixture_harness/gen_net.sh` compiles `UnPenLev`/`UnDemoPenLev`
  (among other net TUs) as standalone sources. These are now Rust shells and
  need `hp2_rs.h` and the FFI library. The harness was not run or changed.
- The occluded-frame count in this checkpoint's game run.
- Everything listed as unresolved in the Localize checkpoint above.

The full macOS Rust port is not finished.

## Rust-defined UCC launcher checkpoint

`hp2_engine::ucc::session` now owns the `ucc` policy that was in
`UCC/Src/UCC.cpp`:

- Argv assembly. Leading and interleaved data-root switches are skipped
  (`hp2_launcher::bootstrap_paths::argument`, the authority behind
  `hp2_rs_path_bootstrap_argument`). Command and parameters go through UTF-8
  checks into 1024-unit buffers. The `len + separator + arg >= 1024` overflow
  rule is unchanged, and the three stderr rejections still exit 1 before
  `appInit`.
- Global setup: client, server, editor and scriptable set to 1; `GLazyLoad`
  set to 0.
- The empty-command hint.
- `ucc help`: the `Core.Commandlet` `verify`, which keeps its expression
  text and line 167. The `ParseToken` command form, or the sorted listing
  (`hp2_core::sort::quicksort` over a permutation, `appStricmp` order,
  `*RightPad` visible text).
- The `Make` `LOAD_DisallowFiles` flag.
- Command lookup: exact name, then `+Commandlet`, then a registry short-name
  match.
- Per-commandlet help text.
- The run sequence:
  - The `LogToStdout` `GLog`/`AuxOut` swap and restore.
  - The banner.
  - `Executing <FullName>`.
  - The default-object process flags.
  - Construct, `InitExecution`, `ParseParms`, `Main`.
  - The `Success - 0 error(s), N warnings` line.
  - `appPreExit` while the registry list is still alive, as in the original
    destructor order.

`RightPad`, `Compare`, `Sort<FString>` and `ShowBanner` were deleted from
C++. The C++ `main` keeps the path bootstrap, native lookups, crash reporter,
`hp2_rs_core_unix_initialize`, the `try`/`catch` (`Error.HandleError()`), the
`GModule` copy, `appExit`, and the device globals. A `Hp2UccServices` leaf
table wraps `appInit` + `RegisterHP2RuntimeClasses`, `SetLanguage`, the
`GetRegistryObjects` stack scope, `StaticLoadClass`, `appCmdLine`, default
object, `GetFullName`, `ConstructObject`, the commandlet virtuals, the
`Warn`/`GWarn`/`debugf` output calls, and `appPreExit`. The LOAD flag values
are passed in `Hp2UccProcess` from `UnObjBas.h`, which is their only
authority. The FString text helpers moved from `net_pending` to
`native_text`.

Verification (macOS arm64, `data-prototype`):

- 19 `hp2_ucc` cases were captured before the rewrite and re-run after:
  - none, help, `help make`, `help nosuch`, nosuch, `help server`,
    `help hello`, `help HelloWorldCommandlet`.
  - run `hello` in short, full and suffix forms.
  - make.
  - Invalid-UTF-8 command and parameter (Python byte argv).
  - A leading data-dir switch.
  - Command lines of 1023 units, separator overflow, 1024 units, and fits.

  All 57 stdout/stderr/exit-code files are byte-identical. The 1024-unit
  case exits 1 with `ucc: command line is too long`.
- `UCC.cpp.o` defined-symbol set, C-collated, before vs after. Removed:
  `ShowBanner`, `RightPad`, `Compare`, `Sort<FString>` (and its
  `__FUNC_NAME__`), `FString::operator+`, plus the weak
  `ConstructObject<UCommandlet>` and `UCommandlet::StaticClass` copies, which
  are now inlined into leaves. Nothing was added. New imports:
  `hp2_rs_ucc_session` and `FOutputDevice::Log(const TCHAR*)`.
- `cargo test -p hp2-engine --lib -- ucc net_pending` passed 18/18. The three
  `ucc` mock-leaf tests cover:
  - The run sequence: data-dir skipping with empty-argument separators;
    `GLog` routed to `Warn` during `Main`, and file log and null `AuxOut`
    restored after; default flags; the warning-count line; `appPreExit` last.
  - `make` (case-insensitive) adding `LOAD_DisallowFiles` to every load, and
    the registry short-name fallback.
  - The help listing: padding, the hidden padding after an embedded NUL, a
    skipped unloadable entry, and case-insensitive sort.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` passed
  157/157, including `ucc_smoke_contract`, `ucc_help_smoke` and
  `abi_inventory_contract`.
- `game_test.py run`, `Maps/Adv1Willow.unr`, WGPU, 600 ticks, `--no-sound`:
  - Exit 0 in 14.5 s, `WgpuDrv: frames presented=600 occluded=0`.
  - `Garbage: objects: 22406->0`, `Exiting.`
  - No `No localization` line and no `<?` marker.
  - The previous checkpoint's occluded run did not recur. Its cause is still
    not established.
- `python3 Build/check_bundle.py` passed, exit 0.

Artifacts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/ucc-cutover/`
(`before/`, `after/`, `before-after.diff`, `symbols-*`, `ctest-full.log`,
`game.log`, `check_bundle.out`).

Still unresolved:

- `Tests/Fixtures/abi-inventory.json` still lists the two removed
  `(UCommandlet*)Class` casts (`UCC.cpp:253:30`, `:292:28`).
  `abi_inventory_contract` passed anyway. The golden was not refreshed;
  that needs its own acceptance record.
- No case runs a commandlet with `LogToStdout`, `ShowErrorCount` or the
  registry short-name fallback against real packages. `data-prototype`
  exposes only `checksum`, `HelloWorld` and `server`. These paths have
  mock-leaf coverage only.
- Everything listed as unresolved in the UNetPendingLevel checkpoint above.

The full macOS Rust port is not finished.

## Rust-defined UModel archive traversal checkpoint

`hp2_engine::model_archive::serialize` replaces `UModel::Serialize` policy.
The native method is a guarded checked-FFI shell. Rust orders the fields,
selects the version-61 legacy conversion versus modern tagged arrays,
preloads non-null legacy objects before storage exchange, and preloads Polys
only outside transactions. Modern zone traversal retains a native
`FPushMemTag("Zones")` scope; each tagged array retains its original tag.
The legacy frame owns its five nullable typed object slots and one reused
discard slot on the native stack. Native leaves retain typed array archive
operators, exchange operations and object virtual dispatch.

This is not completion of the model family: the three struct serializers,
other model lifecycle methods and native array storage still remain.
No general-purpose archive abstraction was added. The obsolete commented-out
light compaction algorithm disappeared with the replaced method body.

Observed verification, macOS arm64:

- `cmake --build --preset macos-arm64 --target
  hp2_verification_binaries hp2_macos_app -- -k 0` succeeded after fixing the
  generated enum prefix annotation.
- A throwaway `cargo run -p hp2-engine --example model_archive_smoke`
  consumer passed 12 version/transaction/legacy-object scenarios (versions
  61, 62, 79; transaction on/off; old arrays absent/present). It also injected
  failure at every fallible traversal operation and checked that later
  fields, exchanges and preloads did not run. This is mock-leaf traversal
  evidence, not original-C++ byte parity or a real legacy-package load.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'` exited 8:
  155 passed, `wgpu_surface_round_trip` skipped, `renderer_smoke_wgpu`
  failed with `renderer.zero_frames_presented` (0 presented, 300 occluded).
  The failing check was not rerun.
- Installed-app `Build/game_test.py run`, explicit
  `--app dist/macos-arm64/HarryPotter2.app`, prototype data,
  `Maps/Adv1Willow.unr`, WGPU, 600 ticks, no sound: exit 0 in 65.8 seconds,
  level entered play, `Garbage: objects: 22470->0`, `Exiting.` However,
  presentation was 0 frames and 600 occluded; this is not rendering acceptance.
- `CGSessionCopyCurrentDictionary` reported `CGSSessionScreenIsLocked = 1`
  after those runs. Visible presentation is blocked by the locked desktop;
  no attempt was made to bypass the lock. This observation does not establish
  the historical cause of earlier occluded runs.
- `python3 Build/check_bundle.py` exited 0.
- Added `model_archive_runtime` as a `data-prototype` CTest. The final
  `ctest --preset macos-arm64 -R '^model_archive_runtime$'` passed 1/1
  (final boundary run: 0.51 s). A test-only copy of the original traversal compares bytes,
  object/name references and preload order through native `FArchive`
  operations in 12 version/transaction/saving combinations. Native exception
  injection compares the original and Rust-driven paths on fresh models.
  Two additional native-storage cases inject real legacy array objects and
  verify both sides of each exchange, shared-side count, zone data and preload
  order with transactions on/off. Zero and negative zone counts also preserve
  untouched zone storage, matching the original loop rather than an unsafe
  signed-count `memcpy`. These are synthetic native archives, not
  legacy package files. Structured evidence is
  `out/macos-arm64/Testing/HP2/model_archive_runtime/Artifacts/model-archive-runtime.json`.
  Earlier test-development failures are retained: an incorrect assertion
  counted superclass preloads, and the initial exception loop reused a
  partially serialized object. The final test counts Polys specifically and
  compares exceptional paths against the original on fresh native objects.
- Final C-collated global defined-symbol identity sets for `UnModel.cpp.o`
  are identical (no additions/removals); `symbol-delta.json` records the sets.
  The app was rebuilt/reinstalled after the zone boundary change, and
  `Build/check_bundle.py` passed again (`check-bundle-final.log`).

Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/model-archive-cutover/`
contains build, branch-smoke, CTest, app and bundle logs, native symbol
snapshots and `display-session.json`.

Unresolved: actual legacy package loading and real package saving/reloading;
visible rendering on an unlocked desktop; audible audio, retail gameplay
and the remaining ownership backlog. No golden fixtures changed.
The full macOS Rust port is not finished.

## ResetConfig ownership checkpoint

`UObject::ResetConfig` now delegates source selection, section snapshot/parsing,
and live class/object reload traversal to `hp2-core::object_config`. The native
shell retains typed config-map access and UObject calls; this is not native-free
Core completion.

The reset boundary no longer calls the shared historical `GetSection` writer:
its key-only capacity predicate can admit a value larger than its output buffer.
Rust retains that admission predicate, uses the original 32767-unit inline
storage for fitting sections, and spills only an oversized admitted entry.
The snapshot precedes writes, preserving first-equals splitting and duplicate
application order. The unrelated shared writer is unchanged.

Verification under macOS arm64:

- `cargo test -p hp2-core --lib object_config::tests`: six passed, including a
  40000-unit value, the key-capacity boundary, live iterator growth and failure
  propagation.
- `cmake --build --preset macos-arm64 --target hp2_abi_tests`: passed.
- `ctest --preset macos-arm64 -R '^config_archive_runtime$'`: passed, 0.40 s,
  `data-prototype`. Real FConfigCacheIni and USystem objects verified System/User
  source selection, missing-source reloads, unsupported categories, embedded
  equals and oversized snapshot storage.
- Evidence:
  `out/macos-arm64/Testing/HP2/config_archive_runtime/Artifacts/reset-config-runtime.json`.

Development failures were not discarded: the native test originally read the
oversized result through the bounded convenience GetString overload, observing
4095 units. It now checks the actual stored FString and observes 40000 units.
This checkpoint does not provide retail, visible rendering, input, audible audio,
save/reload, or full-port acceptance.

## BSP struct serializer cutover

`FBspSurf`, `FPoly`, and `FBspNode` archive traversal now runs in
`hp2-engine::model_struct_archive`. Native leaves retain individual typed
archive operations and flag access. Original traversal bodies were removed
from shipping C++; test-only originals remain as differential oracles.
This does not close model lifecycle or native storage ownership.

- Original-native characterization: `ctest --preset macos-arm64 -R '^model_archive_runtime$'`
  passed, 0.40 s.
- Post-cutover differential coverage includes saving, reference traversal,
  loading into fresh structs, transient flag cleanup, zero/negative polygon
  vertex counts, compact-index boundaries, and exact native exception/prefix
  behavior at every saving byte operation. Final focused CTest passed, 0.63 s.
- `cmake --build --preset macos-arm64 --target hp2_verification_binaries`
  and `cmake --build --preset macos-arm64 --target install` succeeded.
- Installed `Adv1Willow` WGPU smoke: 60 ticks, exit 0, 60 presented frames,
  zero occluded, level entered play and shutdown reclaimed all objects.
  `data-prototype`, `--no-sound`; no audible-audio or retail claim.
- `ctest --preset macos-arm64 -R '^renderer_smoke_wgpu$'`: 1/1 passed,
  27.66 s. Rerun followed observed restored presentation, not retry-to-green
  under the unchanged locked-desktop condition.
- `python3 Build/check_bundle.py`: all bundle integrity checks passed
  (`data-none`, packaging only).
- Game evidence:
  `out/macos-arm64/Testing/HP2/Manual/Artifacts/mac-rust-completion/model-struct-cutover/game.log`.

Current configured compile database contains 152 distinct first-party native
translation units under `HarryPotter2/Unreal/`. This is a configuration count,
not a fresh preprocessed header census or an effort-completion percentage.

## Model lifecycle policy cutover

`UModel::Modify`, `EmptyModel`, constructor initialization policy and
`ShrinkModel` now call `hp2-engine::model::lifecycle`. Native C++ retains member
construction, typed array storage/operations, UObject allocation, and brush
initialization leaves. No native translation unit was removed; model lifetime
and the native-free ownership criterion remain open.

Rust preserves empty/shrink order, nonzero reset arguments, optional polygon
modification/replacement, the replacement object's outer/transaction flags,
per-surface decal cleanup, zone visibility/connectivity reset without touching
render timestamps, and constructor owner wiring. Model itself is intentionally
not transaction-modified.

`model_archive_runtime` gained real native lifecycle characterization before
cutover: nine surface/polygon reset combinations (0, 1 and -1), retained
geometry/decal contents, replacement polygon identity/outer/flags, all zones,
and a real brush/model constructor. Original-native baseline passed in 0.54 s;
the Rust-routed run passed in 0.50 s:

`ctest --preset macos-arm64 -R '^model_archive_runtime$'`

Build:
`cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
passed. Profile: `data-prototype`. This is policy cutover evidence, not full
native storage ownership or retail/input/audio/save acceptance.
The focused lifecycle fixture does not call `Modify` or `ShrinkModel`;
their dispatch/effects remain without targeted runtime verification.

Subsequent verification of the lifecycle build:

- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'`:
  158/158 passed, no skips, 157.90 s. Includes renderer smoke, surface
  round-trip and scripted input smoke. Determinism was explicitly excluded.
  Tests retain their configured `data-none` / `data-prototype` profiles.
- Installed WGPU `Adv1Willow`, sound enabled, 120 ticks: exit 0 in 10.14 s,
  55 presented / 65 occluded, no other frame failures. Do not describe this
  later run as uninterrupted presentation. ALAudio initialized, started
  `sm_bur_playfulFail_01.ogg`, and shut down; the object count reached zero.
  Backend/music-source evidence is not a human audibility check.
- Bundle integrity passed and wrote `bundle-integrity.json`.
- Exact runtime command, observed results and unresolved acceptance conditions:
  `out/macos-arm64/Testing/HP2/model_archive_runtime/Artifacts/lifecycle-cutover-acceptance.json`.
  Runtime log: `lifecycle-game.log` in that same artifact directory.

## Reflected command-line property parsing cutover

`UObject::ParseParms` now delegates property selection, matching and import
ordering to `hp2-core::object_instance`. It reuses the Rust field iterator in
`class_residue` rather than invoking native `TFieldIterator` for this path.
The same iterator still serves cleanup and return-property lookup. Native
leaves retain virtual superclass dispatch, name lookup, text parsing and
typed property import; other native iterator callers remain.

The original shipping parser body was removed. A test-only native oracle
checks eight real `USystem` cases: null/empty/unknown input, quoted text,
case-insensitive keys, embedded equals, duplicate-key precedence, invalid
numbers, empty values, Unicode and exclusion of UObject-owned properties.
The original-native baseline for `object_parse_parms_runtime` passed in
0.47 s. The initial Rust cutover passed all six focused parser, object,
model and UCC tests in 7.48 s.

An actual `hp2_ucc CheckSum` launch against the 48-byte acceptance fixture
exited 0 and printed checksum `82fc70412a0d4cbd1b9732ae34932d52`.
This exercises the commandlet parser call; real property imports are proved
by the `USystem` runtime cases, not by the property-less CheckSum commandlet.
Evidence is under
`out/macos-arm64/Testing/HP2/object_parse_parms_runtime/Artifacts/`:
`object-parse-parms-runtime.json`, `checksum-input.txt`, and `ucc-checksum.log`.
Profile: `data-prototype`. No native translation unit removal, full Core
ownership, retail, physical-input, audible-output or save/reload claim.

Final shared-iterator verification:

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app`
  succeeded.
- `ctest --preset macos-arm64 -j6 -E '^determinism_double_run$'`:
  159/159 passed in 194.49 s. Determinism explicitly excluded; no retail claim.
- Real CheckSum commandlet repeated after the shared-iterator cutover:
  exit 0, same checksum; `ucc-checksum-rust-iterator.log`.
- `python3 Build/check_bundle.py`: passed, `data-none` packaging evidence in
  the parser test's `bundle-integrity.json`.
- After the final whitespace-only iterator formatting,
  `env CARGO_HOME=/Users/mike/.cargo RUSTUP_HOME=/Users/mike/.rustup RUSTC_WRAPPER= cargo test -p hp2-core --lib class_residue::tests`:
  4 passed, 202 filtered. The preceding full CTest run used binaries built
  before that whitespace-only edit.

## Native-owner oracle and selected-product provenance gate

The historical 159-test result above is unchanged. The macOS native-owner
cutover freezes a new original-build oracle before changing ownership. Evidence
root: `out/macos-arm64/Testing/HP2/native_owner_oracle/Artifacts/`.

- Configure and `hp2_verification_binaries` build succeeded; the latter took
  26.77 s. `oracle.json` records copied executables/libraries, hashes, arguments,
  environment, statuses, fixtures and profiles; `ctest-original.json` records
  the configured original 160-test set.
- Original focused CTest command:
  `ctest --preset macos-arm64 -R '^(object_storage_contracts|object_construction_contracts|object_gc_contracts|property_archive_contracts|vm_dispatch_contracts|vm_call_contracts|package79_manifest|ucc_help_smoke)$' --output-on-failure`.
  Observed 8/8 passed in 3.18 s: six `data-none` contracts and two
  `data-prototype` package/UCC cases.
- Actual prototype HelloWorld prints the integer 73 and quoted string
  `two words`. The qualified `Core.SimpleCommandlet` executes the script call
  returning 666 and the conversion/min/string sequence. Bare `Simple` and
  `SimpleCommandlet` are absent from the current public `.int` lookup and
  report not found with exit 0; the frozen corpus preserves this distinction.
  A half-truncated isolated Core.u fails with exit 1, followed by successful
  loading from a separate complete copy. No prototype package was modified.
- `inventory/native-ownership-ledger.json` reconciles 739 successful compiler
  dependency scans: 152 authored production translation units and 205
  included first-party headers, assigned exactly once to 14 families.
  Two generated headers have explicit family owners. Service providers occupy
  62 translation units and 32 unique declaration headers; these are file
  counts, not table counts. No owner family has been accepted or removed.

`Build/check_native_free.py` consumes the configured CMake manifest/File API,
Ninja commands, compile commands, locked Cargo closure, actual archive members
and Rust object symbols. External native inputs require pinned source or
locked-registry provenance; generated zlib configuration has an explicit
pinned-input/output producer record. No whole-build-root exemption is allowed.

Observed `data-none` gate evidence:

- The initial importer check failed on missing generated `zconf.h` provenance,
  including the registered CTest failure (39.60 s). That finding is retained;
  the producer record was corrected before checking again.
- Corrected configure succeeded. Manual importer and pure Rust fixture checks
  passed. The pure fixture also builds an isolated, unlinked native oracle.
- Actual compiled first-party source, generated source, direct object, renamed
  archive, forbidden exception crate and tainted pinned-checkout fixtures all
  failed the gate as required. Missing manifest/target fail closed.
- An actually executed installed Rust fixture exited 0; its exact installed
  copy passed the gate and an altered copy failed artifact identity.
- `ctest --preset macos-arm64 -R '^native_free_import$' --output-on-failure`:
  1/1 passed in 49.37 s after correction. This adds test 161 to the configured
  tree. `native-free-fixture-acceptance-v4.json` and
  `native-free-installed-and-failclosed-acceptance.json` retain commands and
  statuses; per-case reports and stdout/stderr are beside them. Registered
  test report: `Testing/HP2/native_free_import/Artifacts/native-free.json`.

The existing mixed `hp2_game` and `hp2_ucc` both fail the new gate, as expected.
The importer result is not Core closure, game completion or installed-app
qualification. Final bundle signing/rpath transformations still need an
artifact-identity strategy; the current exact-copy fixture does not prove that
path. Native-free Core/UCC ownership, all 14 owner-family removals, final
release enforcement, retail, visible/input/audible and save/reload acceptance
remain open. `determinism_double_run` was not run.

Focused review subsequently found fail-open paths outside those exercised
fixtures: command-token matching did not establish the artifact producer;
Cargo feature/platform/output-directory selection could differ from the
selected product; build-script native links, nested linker arguments,
search-path shadowing, renamed native objects and compiler-discovered headers
were not fully traced; and the Cargo.lock parser could return an empty closure.
The generated configuration record also needs independent producer validation.
These are unresolved acceptance blockers while the guard corrections are in
progress. The recorded v4 fixture and importer passes are evidence only for
their specific executions, not certification of a general native-free closure.

Current Core integration verification (still the mixed C++/Rust products):

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries`
  succeeded. The first focused eight-test run then passed the six `data-none`
  contracts but failed `package79_manifest` and `ucc_help_smoke` under
  `data-prototype`: export serial offsets were decoded incorrectly and UCC
  reported `Invalid file-reader seek`.
- The extracted compact-index codec accumulated three-to-five-byte payloads in
  the wrong order. Decoding now folds high bytes back toward the first byte;
  encoding retains each emitted byte for that same reconstruction. Independent
  positive/negative wire vectors cover 8192, 8193, 16383, 1048576 and both i32
  limits. `cargo test -p hp2-core compact_index::direct_tests -- --nocapture`
  passed both selected tests.
- After that code correction and rebuilding, the exact focused CTest command
  recorded above passed 8/8 in 14.78 s. The original failing execution remains
  in `core-owned-integration-focused.stdout`; the corrected execution is
  `compact-index-fixed-focused.stdout`, both under the oracle artifact root.
- Direct execution of `out/macos-arm64/hp2_ucc Core.SimpleCommandlet
  -datadir=HarryPotter2/Unreal` (absolute data path in the recorded command)
  exited 0 under `data-prototype`, printing function result 666, integer cast 3,
  min result 32 and the expected strings. `compact-index-fixed-commandlet.json`
  and its stdout/stderr retain this smoke evidence. This is not a Rust-only
  UCC run, and no C++ owner-family removal is established by these passes.

After the provenance review corrections, the registered `native_free_import`
check was exercised once under `data-none`: failed in 16.61 s with 228 findings.
The observed failures include ambiguous selected-binary dep-info, unavailable
Rust object inputs for ABI inspection, missing compiler dependency records and
a build-script fingerprint/output binding failure. This is not merely the
anticipated missing Cargo producer-record finding. The current importer guard
is not accepted; the earlier v4 pass is historical, not its current status.
`native-free-review-corrections.stdout`/`.stderr` retain the execution under the
oracle artifact root; `Testing/HP2/native_free_import/Artifacts/native-free.json`
contains the current failed report. No repeat-to-green execution was made.

### Current concrete cutover evidence

No owner family is accepted (0/14). The configured shipping `hp2_game` and
`hp2_ucc` remain mixed products: their captured dependency graphs contain 151
and 140 first-party C++/Objective-C++ source inputs respectively. The actual
source-file removals are `Core/Src/UnName.cpp` (283 lines), `UnMem.cpp` (142)
and `UnMath.cpp` (333); these 758 deleted lines do not establish removal of a
complete native owner or a native-free shipping product.

The separate `hp2_ucc_rust` target now builds:
`cmake --build --preset macos-arm64 --target hp2_ucc_rust` exited 0.
It is not the installed UCC cutover. Direct execution of its Corrosion Cargo
artifact with `Core.HelloWorldCommandlet intparm=73 'strparm="two words"'
-datadir=<absolute prototype root>` still exits 1 under `data-prototype`.
The latest error is `Export serial size mismatch`, context `3/12`, while
loading the `Min` property of `RandRange` in prototype `Core.u`. The direct
property-record serialization path is not complete. The same-owner recovery
consumer in `ctest --preset macos-arm64 -R '^rust_core_runtime$'
--output-on-failure` also fails on that mismatch.

These runs exercised the explicit Rust SDK array allocator, native compact
array counts, the UClass serialization sentinel, inherited PostLoad routing,
and export detachment before linker storage is released. Earlier failures are
retained; the latest run no longer reports the secondary invalid linker index.
Build logs are `direct-core-build-*.log` in the oracle artifact root. Latest
runtime logs are in
`Testing/HP2/native_free_ucc_smoke/Artifacts/hello-postload-and-detach.*.log`;
the consumer log is `direct-core-runtime-postload-and-detach.log`.

Under `data-none`, the corrected `native_free_producer_contracts` CTest passed.
The selected-product `native_free_core` audit still failed: registry packages
and build-script packages lacked matching lock/source provenance, and two
build-script output paths lacked hash bindings. Both results are retained in
`direct-core-producer-gates-corrected.log`. Passing validator contracts is not
a passing product audit. Neither standalone runtime acceptance, native-free
Core closure, a shipping-target switch, retail qualification nor an owner-family
cutover is claimed.

### Native-free Core consumer: prototype execution now passes

This checkpoint supersedes the standalone runtime and selected-product failures
above, not the shipping cutover inventory. No additional production C++ source
was deleted in this checkpoint; accepted owner families remain 0/14 and the
shipping game/UCC remain mixed products.

The direct owner now loads the unchanged prototype `Core.u`, executes its
commandlets and completes the integrated replacement/GC/shutdown scenario.
Class defaults are the class-owned `Defaults` byte buffer, matching native
`UClass::GetDefaultObject()`, not separately allocated and registered
`Default__...` instances. The obsolete extra allocation/publication/constructor
path was removed. The previous shutdown failure was an unreachable native-flagged
extra default object surviving object purge after its synthetic FName was purged.
Default initialization now shares the ordinary Rust kernel with the legacy
adapter; config/localization operate on the defaults buffer and tagged defaults
compare against the superclass buffer.

Observed commands and artifacts:

- `ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure`:
  exit 0, 1/1 passed in 5.62 s, `data-prototype`.
  `native_owner_oracle/Artifacts/direct-core-canonical-default-buffer-acceptance.log`
  retains the result. This covers actual reflected Vec/GetAxes calls, explicit
  script return 666, nested log-callback reentry, error recovery, scalar wire and
  FString growth/shrink/empty transitions, replacement identity/free-slot reuse,
  rooted/unrooted collection and successful owner shutdown. The isolated Cargo
  test profile uses optimization level 1 to bound deeply linked Core field
  traversal stack use; debug assertions and overflow checks remain enabled.
- `cmake --build --preset macos-arm64 --target hp2_ucc_rust`: exit 0.
  `native_owner_oracle/Artifacts/direct-core-canonical-default-buffer-build.log`.
- Actual Corrosion artifact
  `cargo/hp2_86ca4/aarch64-apple-darwin/release/hp2_ucc_rust`,
  `Core.HelloWorldCommandlet intparm=73 'strparm="two words"'` with explicit
  prototype `-datadir` and isolated HOME/TMPDIR: exit 0; logs
  `Executing Class Core.HelloWorldCommandlet`, `Hello, world!`, `73`, `two words`.
- The same artifact with `Core.SimpleCommandlet`: exit 0; logs
  `Executing Class Core.SimpleCommandlet`, return 666, Min 32, `woo`, `alker`.
  Both actual-product logs are in
  `native_free_ucc_smoke/Artifacts/{hello,simple}-canonical-defaults.log`
  and use the engine's UTF-16 log format.
- `ctest --preset macos-arm64 -R
  '^(native_free_core|native_free_producer_contracts)$' --output-on-failure`:
  exit 0, 2/2 passed in 0.99 s, `data-none`, against this rebuilt artifact.
  `native_owner_oracle/Artifacts/native-free-core-canonical-default-buffer-gates.log`.
  The selected product audit, not only its validator contracts, now passes.

All relative artifact paths above are under `out/macos-arm64/Testing/HP2/`;
the Corrosion artifact path is relative to `out/macos-arm64/`. Earlier failures
remain preserved. This is actual native-free Core consumer evidence, not
shipping-game, installed-bundle, retail, sanitizer or Linux qualification.
Remaining Core operation parity and shared-kernel/mixed-product compatibility
checks are still open; no native Core implementation is deleted from its
remaining native consumers on the strength of this isolated product.

### Resumed Rust-owned package-save acceptance

Invariant: a package containing Rust-owned Core objects can be finalized as
package79 and loaded by a fresh Core owner without losing scalar/FString values,
object/name references, or conform export-slot identities. An archive/traversal
error removes the incomplete temporary file; a failed final rename returns false,
preserves the destination, and removes the finalized temporary file.

The interrupted `core_save.rs` implementation now builds and is exercised through
`CoreAccess::save_package`. Shared save policy remains in `package_save.rs`; global
FName enumeration is distinct from package-local name indices, vacant object/name
slots are skipped, and conform matching validates the original candidate identity.
Direct traversal now excludes the package root itself from `IsIn` containment,
visits each export's class during import tagging, propagates export load contexts
to referenced names, and clears all transient traversal/context flags. Struct
super indices use the object's `SuperField`; Class export class indices are zero,
matching the existing mixed adapter. These metadata branches are not a general
script-class-save acceptance claim.

`core_package_save_acceptance` loads unchanged prototype `Core.u`, saves reflected
integer and Unicode FString properties plus a Struct/IntProperty reference graph,
shuts down the owner, and checks the restored values in a fresh owner. It then
conforms against the original package, checks GUID/generation continuity and old
export slots (including an empty deleted slot), appends an export, and loads the
result in a third owner. It also exercises missing-parent creation, an
unreplaceable destination directory, and invalid referenced-name traversal.
The Commandlet fixture explicitly clears its class-inherited transient flag on
each instance before saving; the runtime's normal transient behavior is unchanged.
The existing integrated runtime acceptance captures script output only after its
synthetic function wrapper has been constructed/replaced, so native-parity DevKill
logging is not mistaken for script output.

Observed commands/results (`data-prototype` for runtime scenarios; pure contract
and selected-product provenance checks use `data-none`):

- `cmake --build --preset macos-arm64 --target hp2_verification_binaries`:
  exit 0; `resumed-verification-build.log`.
- `ctest --preset macos-arm64 -R
  '^(rust_core_runtime|package_map_contracts|package_file_contracts|package_flow_contracts|package_summary_archive|linker_runtime_contracts|linker_name_archive|linker_record_archive|class_metadata_contracts|property_archive_contracts|property_tag_contracts|vm_registry_contracts|object_registry_contracts|package_map_runtime|package_path_runtime|package79_manifest|ucc_smoke_contract|ucc_help_smoke|native_free_core|native_free_producer_contracts)$'
  --output-on-failure`: initially 16/20 passed; four Cargo checks failed because a
  new test inspected a crate-private name accessor. The internal inspection
  assertion was removed rather than widening the public API.
  `resumed-core-coverage.log` retains that run.
- `ctest --preset macos-arm64 -R
  '^(rust_core_runtime|package_summary_archive|linker_name_archive|linker_record_archive)$'
  --output-on-failure`: the three archive checks passed. The save test exposed
  a real private-import divergence. This checkpoint incorrectly described native
  save as warning-only and replaced the failure case with an invalid name.
  `UnObj.cpp` actually throws `FailedSavePrivate`; the correction below restores
  rejection and the reachable regression. `resumed-core-coverage-fixed.log`
  preserves the original failure.
- `ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure`:
  exit 0, 1/1 CTest passed in 0.96 s, executing both integrated acceptance tests.
  `resumed-save-roundtrip-final.log`. Across these corrected runs all 20 selected
  checks have passing results; no single clean 20/20 rerun is claimed.
- A throwaway standalone Rust executable invoked the same complete save/reload
  consumer outside libtest, using the built hp2-core/hp2-abi/libc rlibs, isolated
  HOME/TMPDIR, and `HP2_ARTIFACT_DIR`: exit 0,
  `PASS: Rust-owned save, fresh-owner reload, conform reload, and failure cleanup`.
  `core-save-smoke-final-build.log` and `core-save-direct-final.log`.
- Rebuilt Corrosion artifact
  `out/macos-arm64/cargo/hp2_86ca4/aarch64-apple-darwin/release/hp2_ucc_rust`,
  `Core.HelloWorldCommandlet intparm=73 'strparm="two words"'
  -datadir=<absolute prototype root> ABSLOG=<artifact log>`: exit 0; output
  `Hello, world!`, `73`, `two words`. The same artifact with
  `Core.SimpleCommandlet`: exit 0; function return 666, Min 32, `woo`, `alker`.
  `resumed-ucc-{hello,simple}.log` and their `.utf16.log` engine logs.
- After documenting the unsafe API's Rust-owner-only contract, rebuilt
  `hp2_verification_binaries` again (exit 0) and ran
  `ctest --preset macos-arm64 -R
  '^(rust_core_runtime|native_free_core|native_free_producer_contracts)$'
  --output-on-failure`: exit 0, 3/3 passed in 8.86 s.
  `resumed-final-ownership-contract-{build,checks}.log`. The throwaway smoke
  source/binary were removed; observed logs and generated package artifacts remain.

The logs named above are under
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/`; earlier failed runs
are retained. The selected-product `native_free_core` report was also observed
with `ok: true` and an empty findings array for the rebuilt standalone UCC.

**Shipping integration is still open.** `UnObj.cpp::UObject::SavePackage` calls the
shared Rust transaction through native storage/serialization leaves; it does not
call `CoreAccess::save_package`. Native subclass virtual serialization and
pending-kill queries are not connected to the direct owner path. In particular,
the ordinary owned-Core roundtrip does not prove the native pending-kill
skip/warning behavior. Do not route native graphs into this API, delete the
production adapter, promote a gate, or claim native-Core/owner-family completion
from these runs. No production C++ was deleted. Shipping game/UCC remain mixed;
installed-bundle, gameplay-save, retail, sanitizer and Linux acceptance are not
established here.

### Production Core conform storage and metadata teardown cutover

Invariant: production conform saves preserve serialized name slots, GUID and
matched export indices, leave empty slots for removed exports, append new exports,
and reload their payloads. Class destruction stops at its first error before
dependent property/default/superclass teardown.

- `package_flow.rs` now owns conform name/export snapshots, used flags and the
  path-to-original-index map. `UnObj.cpp` no longer owns their `TArray`/`TMap`
  scopes or publishes stack scratch pointers. Native leaves only read/append
  export records and transfer formatted path strings. Rust retains native
  FString comparison/hash semantics and releases transferred buffers through
  their allocator. Export snapshots preserve potentially uninitialized native
  padding without materializing it as initialized Rust bytes.
- `UStruct::Destroy` releases its script buffer through Rust. `UClass` releases
  `DefaultPropText` as one FString/TArray<TCHAR>, not TArray<FString>; the native
  text-assignment callback is removed. The same representation bug is corrected
  in direct Core destruction. The shared teardown kernel now preserves
  fail-fast ordering for both native and direct-owner consumers.
- Production `UStruct::CleanupDestroyed`, `UField::GetOwnerClass` and
  `UFunction::GetReturnProperty` now call the existing Rust residue kernels.
  Native class identity, validity/pending-kill checks and virtual Modify remain
  checked leaves; these are not native-free class ownership claims.
- Direct Rust save now rejects external private imports. Its restored regression
  references a private external Struct from a saved Struct, checks rejection,
  verifies the committed package is unchanged and checks temporary-file cleanup.
- The production `package_path_runtime` scenario now saves/reloads a real Entry
  level, conforms with a deleted and an added TextBuffer, verifies name slots
  directly in the wire tables, checks export slots and reloads the added payload.
  A retained unused name can legitimately load as NAME_None after its load flags
  become zero; comparing context-filtered loader tables would be incorrect.
- `class_storage_kernel_contracts` registers the actual lifecycle/residue unit
  tests in CTest. Obsolete native conform mock leaves are removed; the holes/
  duplicate/reversed-index regression exercises shared correspondence policy.
  A forwarding-only conform-summary trace test is removed rather than re-pinned.

Observed acceptance, macOS arm64:

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries
ctest --preset macos-arm64 -R '^(rust_core_runtime|class_lifecycle_contracts|class_metadata_contracts|class_storage_kernel_contracts|package_flow_contracts|class_buffer_lifetime_runtime|object_gc_runtime|package_path_runtime|native_free_core|native_free_producer_contracts)$' --output-on-failure
```

Build exit 0; **10/10 CTests passed in 44.00 s**. Pure kernel/provenance checks use
`data-none`; actual Core/class/GC/package runtime scenarios use `data-prototype`.
The full `cargo test --offline --manifest-path Cargo.toml --package hp2-core --lib`
target also passed **221/221**, with target/output directed under the runtime's
test artifact tree. Logs are `production-core-acceptance{,-build}.log` and
`production-core-lib-unit-contracts-final.log`.

Production save evidence:
`../package_path_runtime/Artifacts/package-path-runtime.json` records 17 actors
before/after reload, 11 original exports, 12 conform exports, one removed empty
slot, reloaded appended payload and missing-parent rejection.

Actual game, not only the SDK/test consumer:

```sh
cmake --build --preset macos-arm64 --target hp2_game
python3 Build/smoke_maps.py --app out/macos-arm64/hp2_game --engine-bin out/macos-arm64/hp2_game --data-root HarryPotter2/Unreal --renderer wgpu --maps Entry --ticks 60 --timeout 60 --capture-frames --capture-keep-last --output out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/production-core-entry-smoke.json
```

The smoke ran with isolated HOME/TMPDIR, `LC_ALL=C`, `TZ=UTC` and
`HP2_ARTIFACT_DIR` set to the existing runtime artifact directory. Build and game
exit 0; **1/1 maps passed**, 60 captured frames, no reported failure markers,
timeout, orphan or cleanup error. The retained `frame_000059.png` was inspected:
Harry and the textured Entry room render. The script uses `-nosound`; this is
not audible-audio or input verification. The report records the effective
engine argv and package SHA-256. No retail-data claim is made.

All `production-core-*` logs/reports above are under
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/`. Earlier failures remain:
the initial build had a missing unsafe block/result wrapper; the lifecycle run
caught non-fail-fast teardown; the new conform scenario initially omitted
BeginLoad around GetPackageLinker and then incorrectly compared filtered names;
the lib target exposed obsolete mock leaf references and a bad fixture class
pointer. Each was corrected before the final clean run. An optional standalone
probe failed to compile because manually selected rlibs used a different rustc;
it is not acceptance evidence and its temporary source was removed. Temporary
package assertion instrumentation was removed before final acceptance.

No native translation unit or entire ownership family closes here. Shipping
game/UCC remain mixed, and direct Core save is still only for Rust-owned graphs.
The tested game was the rebuilt build-tree executable, not a reinstalled bundle.
Installed-bundle, retail, Linux, sanitizer, audible audio, interactive input and
full-suite qualification remain open; no feature gate is promoted.

### Bit storage removal and canonical Engine.LevelSummary ownership

Invariant: bit buffers use the Rust raw-array owner, and a Rust-owned
LevelSummary retains its canonical class, strings and package identity through
archive loading, localization, GC, save/reload and shutdown.

The bit writer/reader constructors and reader replacement now allocate through
`array_storage`; network bunch construction uses the same path. Native
`Hp2BitsServices.cpp` and its compiled source entry are deleted. The remaining
two-field bit service table supplies only the native assertion policy; native
archive ABI shells remain. `bits_replay` compares wire results and buffer/archive
states against the unchanged oracle, not echoed operations or callback traces.
Replacement coverage includes partial-byte source consumption, empty replacement,
failed allocation and failed release. The real package-map consumer constructs
and replaces a native reader before decoding its index/sentinel; its report
records matching 30-bit source and sliced streams.

`CoreOwner::with_native_classes` stages immutable external declarations in the
existing canonical name/object/class tables. Complete layouts and Rust ClassOps
are retained through native class loading; explicitly qualified inheritance edges
cannot silently fall back to Core. Packed record field offsets remain unchanged
while allocations satisfy the UObject prefix alignment. Native class records are
owner-lifetime GC roots, so collecting with zero keep flags cannot retain a native
class while freeing its package or metadata. Package lookup reuses top-level
native package roots rather than creating another Engine namespace under Transient.
The unsafe `CoreAccess::assign_fstring` operation uses Core's internal allocator;
no allocator service table is exposed to the consumer.

At this checkpoint, the now-replaced
`hp2_engine::level_summary::LevelSummaryOwner` owned only Core plus
Engine.LevelSummary, not the remaining Engine graph. Its actual class layout and
offsets are compiler-probed. Its Rust PostLoad and native adapter share the
superclass-first localization policy. The `level_summary_owner` consumer
stages real Core.u, Engine.u and Entry.unr in its artifact sandbox, applies an
isolated Entry.int, changes Author, retains the rooted object across GC, saves,
collects the unrooted object, reloads in a fresh owner and shuts down.

This is selected-class loading, not a full Engine/world verification:
`LOAD_Verify | LOAD_NoVerify` opens metadata without constructing unrelated
exports and defers unused imports. The dependency linker is opened explicitly
because the measured `LOAD_Propagate` mask is zero. Imports used by the selected
object still resolve normally. No missing Sound, actor or resource class is
replaced with a stub or a native graph; ownership of those types remains open.

Final acceptance commands:

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_game
ctest --preset macos-arm64 -R '^(rust_core_runtime|level_summary_owner|bit_archive_contracts|core_ceil_log_two|package_map_runtime|class_lifecycle_contracts|class_metadata_contracts|class_storage_kernel_contracts|package_flow_contracts|class_buffer_lifetime_runtime|object_gc_runtime|package_path_runtime|native_free_core|native_free_producer_contracts)$' --output-on-failure
```

Observed: build exit 0; **14/14 CTests pass**, 26.33 seconds, with eight
`data-none` and six `data-prototype` tests. Logs:
`canonical-engine-owner-final-build.log` and
`canonical-engine-owner-final-behavior.log`.

Additional configured-toolchain commands used `/opt/homebrew/bin/cargo`,
`CARGO_HOME=/Users/mike/.cargo`, `RUSTC=/opt/homebrew/bin/rustc`, isolated HOME and
TMPDIR, and `CARGO_TARGET_DIR=out/macos-arm64/Testing/HP2/rust_core_runtime/Cargo`:

```sh
cargo test --offline --manifest-path Cargo.toml --package hp2-core --lib -- --test-threads=1
cargo test --offline --manifest-path Cargo.toml --package hp2-engine --lib --test net_chan -- --test-threads=1
cargo run --offline --config=profile.dev.opt-level=1 --manifest-path Cargo.toml --package hp2-engine --example level_summary_owner_smoke
```

Observed: **221 Core unit tests, 137 Engine unit tests and five network integration
tests pass**. The throwaway public-API example independently loaded/localized the
real Entry summary, mutated and persisted Author, exercised rooted/unrooted GC,
reloaded with canonical class identity and completed both shutdowns. Its
`level-summary-public-api.json` records the observed strings and lifetime
transitions. The example source and temporary diagnostics were removed after
verification. Logs: `canonical-engine-owner-core-contracts.log`,
`canonical-engine-owner-engine-contracts.log`, `canonical-engine-owner-public-api.log`.

The final rebuilt game was also run, not inferred from tests:

```sh
python3 Build/smoke_maps.py --app out/macos-arm64/hp2_game --engine-bin out/macos-arm64/hp2_game --data-root HarryPotter2/Unreal --renderer wgpu --maps Entry --ticks 60 --timeout 60 --capture-frames --capture-keep-last --output out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/canonical-engine-owner-entry-smoke.json
```

Observed: exit 0, one playable map passed, 60 captures, no reported failure
markers, timeouts, cleanup errors, orphaned process groups or resource leaks.
The retained `canonical-engine-owner-entry-smoke-frames/000001-Entry/frame_000059.png`
was opened and visually inspected: Harry and the textured stone room render.
The launch used `-nosound`; this is not audible-audio or interactive-input proof.
The report records the unchanged prototype Entry package SHA-256 as
`81c91b452dd9651d86de2305b1173cfaa1137ca3bd57247765a3ed93fb281edd`.

All `canonical-engine-owner-*` logs/reports above and the standalone report live
under `out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/`; individual CTests
retain their own isolated artifact directories. Earlier failures are preserved:
test compile defects, packed-layout rejection, unavailable name-only map search,
unsupported eager Engine imports, duplicate native package roots, inherited
bindings replacing authored operations, and untraced native class dependencies
after GC. The initial ad-hoc bit network command failed in the isolated HOME
before launching Cargo; the configured-toolchain invocation passed. These are
recorded failures, not retries counted as clean acceptance.

The bit storage-service translation unit is removed, but no complete owner-removal
family closes here. Shipping game/UCC remain mixed. Native bit archive shells and
the shipping C++ LevelSummary shell remain until their production consumers move
to the canonical Rust graph. No retail, Linux, sanitizer, installed-bundle,
audible-audio or full-suite qualification is claimed; no feature gate is promoted.

### Model typed archive and final buffer lifetime cutover

Invariant: Rust owns model/database payload traversal, nested surface storage
destruction and final backing-buffer release, preserving native wire formats,
transaction boundaries, failure prefixes and superclass-before-storage teardown.
Native pointers remain borrowed through checked services; they never enter
`CoreAccess`.

`hp2-engine` now accesses compiler-probed `UModel`, `UBspNodes`, `UBspSurfs`,
`UVectors`, `UVerts` and `UPolys` layouts directly. The native model field/archive
matrices and legacy exchange frames are removed. Typed surface, polygon, node,
vector, vertex, leaf, box, light-map and zone traversal is Rust-owned. Array
serialization reuses Core's array archive state machine and storage kernel;
native scalar byte-order serialization remains distinct from raw bulk bytes.
The little-endian bulk path includes vector arrays. Object arrays retain native
pointer width, including all model light references.

Model lifecycle policy now performs transactional checkpoints, array emptying,
surface/decal nested destruction, shrink, zone reset and legacy payload exchange
in Rust. Final model/database array storage is released in Rust rather than by
implicit C++ array member destructors. The native facades retain their original
layout in anonymous unions, with explicit construction and noexcept destruction.
`ConditionalDestroy` still precedes buffer release. The existing transaction
destructor exception policy is shared from `Hp2FfiException.h`: an already
propagating exception is preserved; otherwise a cleanup failure terminates with
the original native exception active.

`UTransactor::Create` is the common production factory used by
`UEditorEngine::CreateTrans` and the real undo consumer. The regression uses
production internal transactions, `GUndo`, record serialization and `Apply`;
there is no mock undo callback or fabricated editor object.

Commands and observed results:

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_game
ctest --preset macos-arm64 -R '^model_archive_runtime$' --output-on-failure
ctest --preset macos-arm64 -R '^(rust_core_runtime|level_summary_owner|model_archive_runtime|transaction_contracts|array_storage_contracts|array_archive_contracts|bit_archive_contracts|package_map_runtime|class_lifecycle_contracts|class_metadata_contracts|class_storage_kernel_contracts|package_flow_contracts|class_buffer_lifetime_runtime|object_gc_runtime|package_path_runtime|native_free_core|native_free_producer_contracts)$' --output-on-failure
```

- Build exited 0 (`model-owner-isolated-child-build.log`).
- The focused model runtime passed, then the combined run passed **17/17** in
  **57.09 seconds**: ten `data-none`, seven `data-prototype` CTests
  (`model-owner-combined-behavior.log`).
- `model_archive_runtime` compares original native wire/identity results for
  versions 61/62/79 in saving/reference and transactional/nontransactional modes,
  including exception output prefixes. It also exercises all five real legacy
  database loaders, complete superclass payloads, compact zone counts, flag
  cleanup, real names/object references, replacement of populated nested buffers,
  full-model reload, pointer-width light arrays, zone timestamp retention and
  actual undo of model geometry and polygon replacement.
- Direct destruction of populated models, legacy surfaces and polygons releases
  watched backing buffers and retires native object registration. This includes
  zero-element/nonzero-capacity storage and nested surface/decal node arrays.
- Three isolated actual-process cleanup probes inject a native allocator failure:
  ordinary destruction terminates with error 7197 active; C++ unwinding and an
  already-pending failure preserve error 7198 and leave no pending exception
  after consumption. The latter cases require that the failing allocator callback
  was reached. The test temporarily restores default SIGCHLD disposition because
  the engine's installed handler otherwise reaps its subprocess status.
- The focused report is
  `out/macos-arm64/Testing/HP2/model_archive_runtime/Artifacts/model-archive-runtime.json`.
  These are `data-prototype` native consumers, not canonical model graph ownership.

An isolated offline Cargo run also passed **137/137** Engine unit tests
(`model-owner-engine-contracts.log`):

```sh
cmake -E env CARGO_HOME=/Users/mike/.cargo RUSTC=/opt/homebrew/bin/rustc \
  CARGO_TARGET_DIR="$A/Cargo-model-owner" HP2_ARTIFACT_DIR="$A/Artifacts" \
  HOME="$A/Artifacts/bit-owner-home" TMPDIR="$A/Artifacts/bit-owner-tmp" \
  LC_ALL=C TZ=UTC /opt/homebrew/bin/cargo test --offline \
  --manifest-path Cargo.toml -p hp2-engine --lib
```

Here `A` is
`/Users/mike/Code/Default/hp2/out/macos-arm64/Testing/HP2/rust_core_runtime`.
Build/check reports below live under `$A/Artifacts`.

Actual production game command:

```sh
cmake -E env HP2_ARTIFACT_DIR="$A/Artifacts" \
  HOME="$A/Artifacts/canonical-game-home" TMPDIR="$A/Artifacts/canonical-game-tmp" \
  LC_ALL=C TZ=UTC python3 Build/smoke_maps.py \
  --app out/macos-arm64/hp2_game --engine-bin out/macos-arm64/hp2_game \
  --data-root HarryPotter2/Unreal --renderer wgpu --maps Entry \
  --ticks 60 --timeout 60 --capture-frames --capture-keep-last \
  --output "$A/Artifacts/model-owner-entry-smoke.json"
```

Observed: exit 0, one playable map passed, 60 captured frames, no failure markers,
timeout, orphaned process group, cleanup error or reported resource leak.
The retained `frame_000059.png` was opened: Harry, wand effects and textured
stone room geometry are visible. This is the rebuilt build-tree game, silent
(`-nosound`), `data-prototype`; it is not installed-bundle or retail acceptance.
Entry SHA-256 remains
`81c91b452dd9651d86de2305b1173cfaa1137ca3bd57247765a3ed93fb281edd`.

Earlier failed checks remain recorded rather than counted as clean passes:
callback pointer casts, protected owner-offset probing, invalid native array
casts, duplicate macro-generated destructors, invalid typed fixture references,
omitted superclass wire, fresh-object identity normalization and child status
consumed by the engine reaper. The differential failure-prefix regression also
caught a real production mismatch: vector arrays were serialized elementwise
instead of through the native little-endian bulk path. That dispatch was fixed
before acceptance.

Remaining ownership is explicit: C++ model/database object and class shells,
native registration/default construction, superclass virtual serialization,
allocator resources, real archive/name/object/preload operations, GUndo and
brush-owner initialization. Other native consumers still use typed array
facades. `UnModel.cpp` remains compiled. No complete native-owner family closes,
no shipping product becomes native-free, and no gate is promoted. Canonical
model class/construction ownership is the next boundary; no retail, Linux,
sanitizer, audible-audio or full-suite qualification is claimed.

### Canonical Engine model class ownership

Invariant: a canonical model and its genuine Polys export share one Rust Core
graph; construction, typed geometry archives, surface adjacency reconstruction,
reference tracing and final storage teardown do not invoke a native object graph.

`hp2_engine::EngineOwner` replaces the narrow `LevelSummaryOwner`; its callers
now use the single combined owner. The declarations preserve the actual
`Model → BoxPrim → Box → Primitive → Object` chain and register BspNodes,
BspSurfs, Vectors, Verts, Polys and LevelSummary in the existing Core registry.
Primitive bounds and the version-61 sphere format are serialized by Rust.
Model and legacy database construction binds transaction-array owners, resets
zones and initializes native constructor state. Model PostLoad rebuilds each
surface's ordered node indices before base PostLoad. Model/database ClassOps
release their own buffers while preserving the first base-destruction error.

The canonical adapter reuses the existing typed model and struct archive
kernels with `ObjectArchive`, canonical preload and Core's paired system
allocator. It does not feed C++ pointers into CoreAccess. Native callers retain
their distinct real archive/allocator/GUndo services. Missing native memory-tag
and editor-undo resources are represented by `None`, not fake callbacks.
Canonical preload runs only for package-loading archives; transaction readers,
tag traversals and reference visitors preserve ordinary FArchive's inert preload.
Raw loading writes into uninitialized storage without forming initialized byte
slices. Reference-bearing elements initialize their slots before fallible I/O.
Fixed polygon/zone arrays reject counts exceeding their physical capacity.
Model allocation stages its replacement header and commits only after success,
so rejected counts cannot corrupt later GC.

Compiler probes now expose the primitive superclass layouts and model flags.
Adding the typed superclass correctly made the generated Model mirror packed;
the two lighting consumers now read its array data pointer unaligned rather
than forming an invalid field reference. No copied descriptor/allocation was
introduced for those lookups.

Acceptance evidence is under
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/`, abbreviated `A`:

```sh
A=/Users/mike/Code/Default/hp2/out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts
python3 Build/gen_engine_abi.py \
  --compile-commands out/macos-arm64/compile_commands.json
cmake --preset macos-arm64
cmake --build --preset macos-arm64 \
  --target hp2_verification_binaries hp2_game
ctest --preset macos-arm64 -R '^model_owner$' --output-on-failure
ctest --preset macos-arm64 -R '^(model_owner|level_summary_owner)$' \
  --output-on-failure
ctest --preset macos-arm64 -R '^(rust_core_runtime|model_archive_runtime|transaction_contracts|array_storage_contracts|array_archive_contracts|bit_archive_contracts|package_map_runtime|class_lifecycle_contracts|class_metadata_contracts|class_storage_kernel_contracts|package_flow_contracts|class_buffer_lifetime_runtime|object_gc_runtime|package_path_runtime|native_free_core|native_free_producer_contracts)$' \
  --output-on-failure
```

- ABI generation passed after supplying the new opaque sphere-field mapping:
  `canonical-model-abi-opaque-field.log`. The final native build passed:
  `canonical-model-failure-safety-build.log`.
- `model_owner` passed in 5.85 seconds after the archive-safety fixes:
  `canonical-model-failure-safety-consumer.log`. Its one consumer exercises
  genuine model/Polys identity, populated vectors/points/nodes/surfaces/polygons,
  primitive bounds, persistent save/fresh-owner reload, transient flag clearing,
  ordered surface adjacency, same-object transaction restoration with geometry
  omitted, reference retention, collection and recreation.
- Construction/save/GC first run with no game packages. Reload then copies the
  real prototype `Core.u` and `Engine.u`; it is **data-prototype metadata**
  evidence, not a data-none reload or a loaded prototype world. As in the
  LevelSummary consumer, Engine metadata is opened explicitly with
  `LOAD_Verify | LOAD_NoVerify`: unused imports are deferred, while selected
  model imports and payloads resolve normally. `LOAD_Propagate` is zero in this
  ABI; silently inventing Engine metadata or registering dummy Sound classes
  would not be an acceptable substitute.
- The existing `level_summary_owner` passed with the combined owner in
  `canonical-model-prototype-consumers.log`; that invocation also retains the
  then-failing model fixture. It is not counted as a clean two-test run.
- The final remaining named CTest group passed **16/16**, 4.73 seconds,
  ten data-none and six data-prototype tests:
  `canonical-model-final-combined-behavior.log`. Together with the separately
  accepted canonical-model and LevelSummary consumers, 18 distinct named
  CTests have passing evidence; this is not a full-suite claim.

The malformed-count regression was compiled against the retained pre-fix
Homebrew Rust libraries before rebuilding them. It failed with live array
metadata `(-1, -1)` instead of `(1, 1)`:
`canonical-model-before-fix-{compile,regression}.log`. The permanent consumer
now rejects both `-1` and `INT_MAX`, retains its original vector and allocation
header, and completes actual GC after each failure. Allocation headers are also
staged for ordinary empty/reserve and legacy database paths. Actual OOM was not
injected; the pre-fix failure evidence is retained, not retried away.

The isolated Rust Engine library run passed **137/137**:

```sh
cmake -E env CARGO_HOME=/Users/mike/.cargo RUSTC=/opt/homebrew/bin/rustc \
  CARGO_TARGET_DIR="$A/../Cargo-model-owner" HP2_ARTIFACT_DIR="$A" \
  HOME="$A/bit-owner-home" TMPDIR="$A/bit-owner-tmp" LC_ALL=C TZ=UTC \
  /opt/homebrew/bin/cargo test --offline --manifest-path Cargo.toml \
  -p hp2-engine --lib
```

Log: `canonical-model-final-engine-contracts.log`. An actual standalone CLI,
not the test harness, loaded the saved canonical graph using the prototype
metadata, observed vector `[301,-307,311]` and surface nodes `[0,1]`, retained
Polys through rooted-model GC, rejected overflowing zone/polygon counts, saved
again after those errors, collected both objects and shut down successfully.
It ran via the same isolated environment with
`HP2_MODEL_FIXTURE=/Users/mike/Code/Default/hp2/out/macos-arm64/Testing/HP2/model_owner/Artifacts/model-owner-60973`
and `cargo run --offline --manifest-path Cargo.toml --package hp2-engine
--example model_owner_probe`. Its throwaway source was moved out of the crate
to `A/canonical-model-cli.rs`; report and log:
`canonical-model-cli.json`, `canonical-model-final-cli-smoke.log`.

The rebuilt native shipping-path compatibility smoke also passed:

```sh
cmake -E env HP2_ARTIFACT_DIR="$A" HOME="$A/canonical-game-home" \
  TMPDIR="$A/canonical-game-tmp" LC_ALL=C TZ=UTC \
  python3 Build/smoke_maps.py \
  --app out/macos-arm64/hp2_game --engine-bin out/macos-arm64/hp2_game \
  --data-root HarryPotter2/Unreal --renderer wgpu --maps Entry \
  --ticks 60 --timeout 60 --capture-frames --capture-keep-last \
  --output="$A/canonical-model-final-entry-smoke.json"
```

Observed: one map passed, exit zero, 60 captures, no failure markers, timeout,
orphan/cleanup errors or reported resource leaks. The retained last frame was
opened and inspected: Harry and textured room geometry were visible.
This is a build-tree, silent `-nosound`, **data-prototype** compatibility run;
it does not route the shipping game through the canonical EngineOwner.
Report: `canonical-model-final-entry-smoke.json`; captures:
`canonical-model-final-entry-smoke-frames/000001-Entry/`.
Entry map SHA-256 remains
`81c91b452dd9651d86de2305b1173cfaa1137ca3bd57247765a3ed93fb281edd`.

Earlier failed checks remain in the `canonical-model-*` logs: a missing opaque
ABI scalar mapping, packed lighting references, a fixture error constructing
RuntimeError, transaction restoration into an unrelated identity, absent real
Engine metadata and accidental eager Engine import verification. The reviewed
malformed-count defect was reproduced and fixed. Review also found premature
preloading during non-package traversal; that adapter is now mode-restricted.
No dedicated pending-export GC scenario was run for the latter.

Production ownership is still unfinished: native Model/database class shells,
registration and shipping construction; the native Engine object graph and
superclass dispatch; allocator/archive resources; and real editor GUndo remain.
No additional C++ translation unit was removed by this canonical checkpoint.
Accepted owner families remain **0/14**; game/UCC remain mixed products. No gate
is promoted. Installed-bundle, retail, Linux, sanitizer, input, save-game,
audible-audio and full-suite acceptance remain unearned here.

### Canonical animation and primitive-shape checkpoint

**Invariant:** one canonical Core graph owns animation nested arrays, non-owning
master-key aliases and gesture storage; native-object pointers never enter
`CoreAccess`. Cylinder specializations use their actual native superclass and
inherited primitive archive behavior.

`animation_owner.rs` provides `Animation`/`Gesture` ClassOps and ordered cleanup.
Animation wire data, transaction restoration, package-import name tagging and
GC name traversal use the canonical archive. Native `UGesture` inherits UObject
serialization; Core still handles reflected fields. Shape declarations add
`OrientedCylinder`, `AlignedOvalCylinder` and `OrientedOvalCylinder`; the middle
class's reflected superclass is `Primitive` (the native `UCylinder` typedef).
Core startup property declarations now support explicit shared-word bool masks;
the external native-class consumer saves/reloads masks 64/128/256 independently
without persisting an unrelated reserved bit. Actual Actor registration remains
unfinished.

Artifact base `A` is the absolute directory established in the model checkpoint.
Commands and observations:

```sh
cmake --preset macos-arm64
ctest --preset macos-arm64 \
  -R '^(rust_core_runtime|animation_owner|model_owner|level_summary_owner)$' \
  --output-on-failure
```

Configure exited zero. The first consumer run passed `rust_core_runtime`,
`model_owner` and `level_summary_owner`, but animation failed to compile because
the fixture accessed an opaque base as `.base`. Log:
`canonical-world-prerequisite-consumers.log`. After using the UObject prefix,
the next animation run reached runtime and rejected the fixture's null outer
(`Class has no outer package`); log:
`canonical-animation-consumer-after-cast-fix.log`.
The fixture now constructs a genuine canonical Package and uses it as outer:

```sh
ctest --preset macos-arm64 -R '^animation_owner$' --output-on-failure
```

Observed exit zero, one consumer passed:
`canonical-animation-consumer-canonical-package.log`. This **data-none**
consumer covers nested wire values/key aliases, transaction restore after a
mutation, name tagging, truncated-load cleanup and rooted/unrooted GC. Together
the logs establish four distinct named passes, not a single clean 4/4 run.

The standalone `A/canonical-resources-cli.rs` was compiled against the already
verified animation consumer's rlibs, then executed independently:

```sh
D=out/macos-arm64/Testing/HP2/animation_owner/Cargo/debug/deps
/opt/homebrew/bin/rustc --edition=2024 "$A/canonical-resources-cli.rs" \
  -L "dependency=$D" \
  --extern "hp2_engine=$D/libhp2_engine-06606b7a7b157fe8.rlib" \
  --extern "hp2_core=$D/libhp2_core-ea6bc7419e05ec50.rlib" \
  --extern "hp2_abi=$D/libhp2_abi-6038a07e9333ffee.rlib" \
  -o "$A/canonical-resources-cli"
cmake -E env HP2_ARTIFACT_DIR="$A" \
  HP2_CORE_DATA_ROOT=/Users/mike/Code/Default/hp2/HarryPotter2/Unreal \
  HOME="$A/bit-owner-home" TMPDIR="$A/bit-owner-tmp" LC_ALL=C TZ=UTC \
  "$A/canonical-resources-cli"
```

Both exited zero. The CLI saved a populated canonical package, destroyed its
owner, copied real prototype Core/Engine metadata, and loaded it in a fresh
owner. Four nested animation names, sequence/notify values and all three
shapes' primitive bounds round-tripped; rooted names survived GC, the unrooted
animation was collected, and shutdown succeeded. This is
**data-prototype-metadata**, not gameplay or collision-dispatch evidence.
Logs: `canonical-resources-cli-homebrew-build.log`,
`canonical-resources-cli-run.log`; report: `canonical-resources-cli.json`.
The initial CLI build used Corrosion's rustc 1.97.1 against Homebrew 1.98.1
consumer rlibs and failed with E0514; that evidence remains in
`canonical-resources-cli-build.log`. The corrected build used the matching
Homebrew compiler, without rebuilding a sibling's mid-flight Core sources.

The ABI generator also successfully compiler-probed animation/gesture layouts
and then Mesh/LodMesh/lazy-loader sizes, fields, alignments and multiple-base
offsets. Logs: `canonical-world-prerequisite-abi.log`,
`canonical-mesh-lazy-abi-probe.log`. Mesh/lazy runtime integration is not credited
by the earlier resource CLI or by probe success alone.

Native production class shells/serialization remain until the actual graph
cutover; no native owner family closes at this checkpoint (**0/14**). No
installed-game, retail, Linux, sanitizer, input, audible-audio or save-game
acceptance is earned. Actual `EngineOwner`-in-game routing is still unverified.

### Canonical mesh, skeletal-mesh and deferred-archive checkpoint

Invariant: canonical `Mesh`, `LodMesh` and `SkeletalMesh` records own their
native member storage through Rust ClassOps. Deferred arrays retain the real
canonical linker and shared package bytes, not a temporary archive context.
No C++ virtual dispatch is performed on these records.

The measured FLazyLoader/TArray base offsets preserve the native saved-archive
identity and lazy load/unload policy. Buffered archive positions and sizes stay
payload-relative; explicit file-coordinate operations emit absolute lazy-array
skip offsets. A linker's shared file cursor may be in another export, while
each deferred body's reads and seeks remain bounded to its own export.
Linker deletion clears every saved archive/position before releasing storage,
including when force-loading malformed deferred data returns an error.

Mesh/LodMesh include native wire ordering, animation names/references and LOD
vertex remapping. SkeletalMesh includes inherited member teardown, actual
Animation references, bone-name traversal, the native repeated-X joint wire
quirk, and average `BoundingBoxSum` calculation. These are resource operations,
not an Actor/Level graph or production engine owner.

Artifact base `A` remains the model checkpoint's artifact directory. Commands:

```sh
cmake --preset macos-arm64
ctest --preset macos-arm64 \
  -R '^(rust_core_runtime|animation_owner|mesh_owner|skeletal_owner|model_owner|level_summary_owner)$' \
  --output-on-failure
```

Configure exited zero. The first batch failed to compile because the lazy
adapter called an unsafe cleanup function without an unsafe block
(`canonical-mesh-batch-consumers.log`). Subsequent Mesh attempts found missing
`animation_owner` imports/private COMPACT visibility
(`canonical-mesh-consumer-after-unsafe-fix.log`), then a genuine save-format
failure: export-relative lazy skip offsets
(`canonical-mesh-consumer-after-import-fix.log`). The actual export origin is
now supplied by the package saver; no loader fallback accepts relative offsets.
After explicit file-coordinate integration, all six consumers passed in one
run: `canonical-mesh-batch-explicit-file-coordinates.log`.

The standalone CLI wrote `MeshCli.u` in process 5456, shut down its owner, then
loaded the same file in another process with real prototype Core/Engine
metadata. Its first reader exposed the cross-export shared-cursor bug
(`canonical-mesh-cli-read.log`). The permanent Mesh consumer now loads Mesh
vertices after loading another export; it passed after correcting the shared
cursor's file bounds:

```sh
ctest --preset macos-arm64 -R '^mesh_owner$' --output-on-failure
```

Log: `canonical-mesh-cross-export-regression.log`. This consumer also verifies
unload/reload from retained bytes and linker deletion after malformed deferred
data, with all surviving lazy headers detached.

The next CLI attempt reached a faulty smoke assertion against Primitive's
`BoundingBox`; native SkeletalMesh computes the separate `BoundingBoxSum`
(`UnSkeletalMesh.cpp:133-143`). That probe was corrected, not the implementation.
The failed observation remains in
`canonical-mesh-cli-read-shared-cursor-fix.log`.
The final reader, process 18408, exited zero against the original saved file.
It verified vertices `[11,29]`, bone `SmokePelvis`, weapon coordinates, native
average bounds, lazy load/unload/reload, referenced Animation retention, graph
collection after unrooting, and shutdown. Reports:
`canonical-mesh-cli-write.json`, `canonical-mesh-cli-read.json`; final log:
`canonical-mesh-cli-read-native-bounds-field.log`.
The throwaway CLI source was removed after proof; its executable, package and
logs remain under `A`. The writer was **data-none**; the reader was
**data-prototype-metadata**, not gameplay or game-save acceptance.

```sh
ctest --preset macos-arm64 \
  -R '^(abi_widths|array_archive_contracts|linker_runtime_contracts|package_flow_contracts|property_archive_contracts|object_storage_contracts|object_construction_contracts|property_layout_contracts)$' \
  --output-on-failure
cmake --build --preset macos-arm64 --target hp2_verification_binaries
```

Observed eight shared-contract passes under **data-none**
(`canonical-mesh-batch-shared-contracts.log`) and successful native integration
build after the cursor fix
(`canonical-mesh-batch-shared-cursor-native-build.log`). Build success is not
credited as runtime ownership evidence.

Native resource shells still serve the native production world. Accepted
native-owner removal remains **0/14**. Installed-game canonical routing,
Actor/Level/engine ownership, retail, Linux, sanitizers, actual input/audio and
game save/reload remain unverified; no gate is promoted.

### Canonical Palette and explicit execution initialization

Invariant: Palette owns its measured FColor array through canonical ClassOps,
and explicit execution initialization dispatches the object's ClassOps hook
without admitting another owner's object. Construction does not initialize
execution automatically.

`Palette` adds the native SafeReplace flag and R,G,B,A wire order, independently
of native packed-color byte order. Its array uses paired allocation/destruction
and supports transaction restoration after mutation or a partial color load.
The compiler probe now measures FColor's channel offsets and four-byte alignment.

`ClassOps::initialize_execution` and `CoreAccess::initialize_execution` reuse
the existing state-frame policy for every UObject-derived Core record kind.
Core commandlet initialization now uses that dispatch. The existing `post_load`
hook remains separate. Object and Package consumers verify real initialized
frames; the cross-owner case checks that rejected dispatch leaves the foreign
frame's probe mask unchanged.

Commands:

```sh
python3 Build/gen_engine_abi.py \
  --compile-commands out/macos-arm64/compile_commands.json
cmake --preset macos-arm64
ctest --preset macos-arm64 \
  -R '^(rust_core_runtime|palette_owner)$' --output-on-failure
cmake --build --preset macos-arm64 --target hp2_verification_binaries
```

The ABI probe and configure exited zero. The first integrated compile found
that the new execution hook had replaced, rather than accompanied, `post_load`:
`canonical-palette-execution-consumers.log`,
`canonical-palette-execution-native-build.log`. Restoring the existing field
fixed compilation; the native integration build exited zero in
`canonical-palette-native-after-post-load-restore.log`.

Runtime checks then exposed two fixture mistakes
(`canonical-palette-execution-after-post-load-restore.log`):
transaction wire includes Name/Outer identity and cannot be copied into a
differently named instance without invalidating its object hash, and a fresh
owner's native abstract Object class cannot be constructed before its metadata
is loaded. Palette now restores the same instance after mutating its colors.
Its **data-none** consumer passed in
`canonical-palette-execution-correct-fixture-identities.log`.
Setting scriptable mode did not change the fresh owner's abstract-class
restriction; that failed Core observation remains in the same log. The final
cross-owner fixture uses a real concrete Package and proves inherited execution
initialization, not a relaxed construction check:

```sh
ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure
```

All three internal Core consumers passed:
`canonical-execution-concrete-foreign-owner.log` (**data-prototype**).
Together these are two distinct named CTest passes after fixes, not a claimed
single clean 2/2 run.

The standalone Palette CLI was compiled with matching Homebrew rustc against
the verified Palette consumer's rlibs, then run as separate write/read processes:

```sh
cmake -E env HP2_ARTIFACT_DIR="$A" \
  HP2_CORE_DATA_ROOT=/Users/mike/Code/Default/hp2/HarryPotter2/Unreal \
  HOME="$A" TMPDIR="$A" HP2_TEST_NAME=canonical_palette_cli LC_ALL=C TZ=UTC \
  "$A/canonical-palette-cli" write
cmake -E env HP2_ARTIFACT_DIR="$A" \
  HP2_CORE_DATA_ROOT=/Users/mike/Code/Default/hp2/HarryPotter2/Unreal \
  HOME="$A" TMPDIR="$A" HP2_TEST_NAME=canonical_palette_cli LC_ALL=C TZ=UTC \
  "$A/canonical-palette-cli" read
```

Both exited zero. Writer process 62490 verified repeat initialization resets
mutated frame state, saved actual RGBA colors plus the initialized frame, and
shut down. Reader process 62503 loaded the same package with real prototype
Core/Engine metadata and verified both colors, canonical frame object/class
identity, rooted retention, collection after unrooting, and shutdown.
Reports: `canonical-palette-cli-write.json`, `canonical-palette-cli-read.json`;
logs: `canonical-palette-cli-build.log`, `canonical-palette-cli-write.log`,
`canonical-palette-cli-read.log`. The throwaway source was removed after proof.
The writer was **data-none**, the reader **data-prototype-metadata**.

This still does not implement the production Actor/Level/client/engine graph.
Native owner removal remains **0/14**; installed canonical ingress and game
load/play/save/reload are not credited by resource APIs or compile success.


### Canonical load completion and external Rust VM natives

Canonical package/object/class loads now finish the complete resolved-object queue
before postload, localized configuration and nonnative-import dissociation.
Preload/postload ordering and export load-context filtering reuse the native Rust
policy. Linker context and archive client/server/editor flags come from the
declared `CoreProcessMode`; package-loading tooling must declare its role rather
than relying on the previously ignored zero context.

The complete-load change exposed a conform tombstone being constructed as a
live export. The retained package has a zero-load-flags deleted slot whose name
index resolves to `HelpDesc`; its empty payload is not a live StringProperty.
The fix is the native export/context intersection guard, not an empty-payload
exception. Failure evidence remains in
`canonical-engine-resources-and-core-load-completion-regressions.log`.

Core's function-owned canonical binding now admits an immutable externally
registered Rust-native catalog alongside Core intrinsics. Core identities retain
precedence. Reflected, numeric-bytecode and named dispatch use the canonical
binding, never `GNatives`, `UFunction::Func` or a C++ UObject callback. A refused
second registration leaves the first catalog unchanged.

Verification artifacts are under
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/`:

- `ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure`
  passed after the context fix and the permanent external-native reentry
  regression were included:
  `canonical-core-context-and-external-vm-reentry-regressions.log`.
  The named suite includes prototype-data package save/conform/reload; the new
  extension regression itself uses **data-none**.
- The standalone `canonical-vm-cli/native_extension` exited zero under
  `HP2_ARTIFACT_DIR` pointing to that artifact directory. Actual reflected,
  encoded-numeric and named calls each performed two nested position increments
  on a canonical TextBuffer, then shut down. Report:
  `canonical-vm-cli/report.txt`; log:
  `canonical-vm-external-reflected-numeric-and-named-reentry-smoke.log`.
  Profile: **data-none**. The throwaway source was removed after proof.

These are Core/VM boundary results, not Engine gameplay-native implementations
or installed-world acceptance. The earlier ten-test integration batch was
6/10, including stale Level field callers and the conform failure; its failure
record is retained and is not a passing batch. Native owner removal remains
**0/14**. Canonical shipping ingress and game load/play/save/reload remain
unverified.

### Canonical world-policy leaves and private scratch pools

Canonical scratch memory now executes the same stack/chunk policy as the native
entry points, but each `MemStackOwner` owns its unused-chunk list. Native callers
retain their native allocator domain; canonical owners never borrow or drain it.
The engine context contains its own scratch owner. URL internal/local/equality
operations now expose the existing shared URL policy.

Actual standalone consumer verification, **data-none**, with
`HP2_ARTIFACT_DIR=out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts`:

- `canonical-world-policy-cli/owner_policy` exited **0**. It performed absolute
  and aliased relative travel, option replacement/inheritance, URL equality and
  invalid-URL rejection; wrote/read/evicted real cache payload; and verified
  64-byte scratch alignment, active-stack Tick rejection, and that exiting one
  scratch owner leaves the other owner's live bytes intact.
- Report: `canonical-world-policy-cli/report.txt`. Build log:
  `canonical-world-policy-native-fatal-cache-contract-build.log`. Run log:
  `canonical-world-url-cache-private-memory-with-native-fatal-flush-contract-smoke.log`.
- The original consumer exited **101** because it wrongly expected a failed
  locked single-item cache Flush to preserve hash lookup. The shared native
  policy unhashes before raising the locked-item fatal error; recovery is not
  transactional. That failure remains in
  `canonical-world-url-cache-and-private-memory-real-consumer-smoke.log`.
  The corrected consumer exercises that destructive error in a separate owner,
  observes the lookup miss, and separately exercises successful cache lifetime.
  No cache implementation or permanent assertion was changed to hide it.
- The throwaway source was removed after proof; executable, report and logs
  remain under the artifact directory.

`cmake --build --preset macos-arm64 --target hp2_verification_binaries`
exited **0** after correcting canonical memory diagnostics to use platform
`ansi::Tchar`. Log:
`canonical-native-verification-with-platform-tchar-memory-owner.log`.
The next exact ten-test integration batch was **7/10**, not green; its three
failures exposed further test-fixture references to the former flat Level layout.
Evidence: `canonical-rebuilt-engine-core-exact-regression-batch.log`.

These results do not construct or run the shipping canonical world. Native
owner removal remains **0/14**. Installed canonical load/play/save/reload is
still unverified.


### Canonical Level, Actor execution and mesh collision consumer

The standalone `canonical-world-owner-cli/owner_world --mesh-world` exited
**0** with `HP2_ARTIFACT_DIR` set to
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts` (**data-none**, macOS
arm64). Report: `canonical-world-owner-77882/report.txt`; run log:
`canonical-world-real-mesh-actor-level-initialize-and-collision-smoke.log`.

It constructed real canonical Level, LevelInfo, Actor and Mesh objects; initialized
both actors' actual execution frames; and verified zero native vptrs. The consumer
exercised inherited mesh-box point/line/radius collision, remove/move/reinsert,
hash teardown while caller-owned results remained live, scratch-mark restoration
and owner shutdown. No native UObject service table supplied these operations.

Startup now resolves the registered Vector fields and qualified Mover object
references, including NavigationPoint/Triggers/Trigger metadata and native-probed
bitfield positions. The earlier startup failures remain in their artifact logs.
The previous rebuilt fifteen-target batch was **11/15** before these fixes, not
a passing batch. The consumer still correctly rejects abstract `Engine.Engine`;
concrete GameEngine lifecycle/network dependencies remain integration work.

This is component/world-owner evidence without a running GameEngine. It does not
prove installed-game routing, gameplay, saving or reloading; native owner removal
remains **0/14**. Linux and retail data were not exercised.


### Canonical package-map admission, ranges and linker copy

Core package-map AddLinker, Compute and CopyLinkers now use shared native/Core
policy kernels. The Core adapters read actual canonical linker imports and
Summary.PackageFlags, preserve global import-name identities, and use Core's
array/map storage without a C++ UObject service table.

The standalone package-map consumer exited **0** on macOS arm64 with
`HP2_ARTIFACT_DIR=out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts`
(**data-prototype**, isolated copies of prototype `Core.u` and `Engine.u`).
It loaded the real linkers, admitted Engine's Core import exactly once, checked
duplicate admission identity, contiguous name/object ranges, copied linker
identities and computed ranges into a second canonical map, and shut down.
Report: `package-map-owner-77018/report.txt`; command:
`HP2_ARTIFACT_DIR=out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/package-map-owner-smoke`.
Run log: `canonical-package-map-real-import-smoke.log`. The throwaway source was
removed after proof; the executable, report and logs remain in that directory.

The first Core build failed on a dangerous implicit raw-pointer autoref; the
explicit scoped borrow correction built successfully. Both results are retained:
`canonical-package-map-core-build.log` and
`canonical-package-map-core-build-after-explicit-borrow-fix.log`.
`python3 Build/gen_engine_abi.py --check` exited **0**; the matching ABI library
tests passed **7/7**, including compiler-probed layout assertions
(`canonical-network-player-transactional-array-generated-layout-check.log`,
`canonical-network-player-transactional-array-layout-regressions.log`).

The affected named acceptance targets also passed **2/2**:
`ctest --preset macos-arm64 -R '^(rust_core_runtime|package_map_contracts)$' --output-on-failure`.
`package_map_contracts` exercised **data-none** and `rust_core_runtime` exercised
**data-prototype**; log:
`canonical-package-map-and-core-runtime-contracts.log`.

These checks do not verify Demo recording, WelcomePlayer, GameEngine startup or
installed-game routing. Native owner removal remains **0/14**; installed canonical
load/play/save/reload, Linux and retail data remain unverified.


### Canonical Engine world lifecycle and movement integration

The direct owner modules now build together. SpawnActor, DestroyActor,
MoveActor, MoveSmooth, FindBase, tracing, zone assignment, CheckEncroachment,
TouchTo and BeginTouch reuse native/Core policy kernels. The canonical adapters
operate on Core-owned records; native C++ service tables are not their fallback.
Decoration, Projectile and Inventory now have compiler-sized native class
identities and inherited Actor lifecycle operations; package metadata still
supplies their script-backed fields.

The standalone world consumer exited **0** on macOS arm64, **data-none**, with
`HP2_ARTIFACT_DIR=out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts`:

```sh
out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/canonical-world-owner-smoke
```

Observed report `canonical-world-owner-43290/report.txt` records base-engine
initialization and five canonical primitives; invalid/duplicate/Unicode URL deep
copy; positive Actor execution; cylinder point/line/radius queries; actual
MoveActor translation, rotation-only movement, static rejection and blocked
movement; hash removal, slot unlinking, deleted-chain publication and conditional
destruction; a typed lifecycle observer seeing the deleted flag and unpublished
actor slot; and shutdown. All objects are canonical records without C++ UObject
vptrs. This uses an initialized GameEngine object, not the installed game loop.
Run log: `canonical-world-movement-with-inherited-native-actor-classes-runtime.log`.

The throwaway consumer source was removed after proof; executable, reports and
logs remain under that artifact directory.

The same consumer's `--mesh-world` and `--mover-world` modes exited **0**:
`canonical-world-owner-30528/report.txt` and
`canonical-world-owner-30529/report.txt`. Their logs are
`canonical-game-world-owner-runtime-mesh.log` and
`canonical-game-world-owner-runtime-moving-brush.log`.

The first destruction run failed with `Actor remains referenced by collision
hash`: the reference audit incorrectly scanned free-list entries retaining stale
actor identities. It now walks reachable bucket chains. The existing collision
fixtures assert that thorough removal leaves no reachable references. Initial
movement runs also exposed eager class checks that violated source
short-circuiting and missing Projectile/Decoration identities; both causes were
corrected. MoveActor now releases its scratch mark on rejection and error paths.
Failure logs are retained alongside the passing evidence.

Affected acceptance passed **7/7**, **24.68 seconds**:

```sh
ctest --preset macos-arm64 -R '^(spawn_owner|actor_spawn_contracts|engine_world_collision_contracts|model_owner|package_map_contracts|bit_archive_contracts|rust_core_runtime)$' --output-on-failure
```

Log: `canonical-engine-world-owner-final-affected-acceptance.log`. Five targets
use **data-none**; `model_owner` and `rust_core_runtime` use **data-prototype**.
The new spawn test verifies static-class rejection without reserving an actor
slot, followed by successful canonical spawning with world/slot/class identity
and a real execution frame. Its initial packed-field assertion compile failure
was corrected by copying pointer fields before comparison; the failure is
retained in CTest's first batch output.

`python3 Build/gen_engine_abi.py` exited **0** after adding the actual native
Actor.Deleted field to the probe manifest. ABI library tests passed **7/7**:
`canonical-actor-deleted-chain-generated-layout.log` and
`canonical-actor-deleted-chain-layout-regressions.log`.

The complete native/Rust verification consumers rebuilt successfully:
`cmake --build --preset macos-arm64 --target hp2_verification_binaries`
completed at **222/222**, **52.54 seconds**. Ninja reported a premature-end
recovery warning; compilation and final links completed. An output excerpt and
full harness-output reference are retained in
`canonical-engine-owner-verification-binaries-build.log`.
The rebuilt acceptance targets passed **3/3**, **7.32 seconds**:

```sh
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|class_registration_contracts)$' --output-on-failure
```

Log: `canonical-engine-owner-rebuilt-native-abi-and-registration.log`.
`abi_widths` and `class_registration_contracts` use **data-none**;
`native_registration` uses **data-prototype**. This is compatibility evidence,
not proof of installed canonical EngineOwner routing.

These are component acceptance results, not native-free completion. Canonical
network connection/control-channel/demo adapters build, but this checkpoint does
not prove full demo receive/playback, production audio/observer publication,
installed-game routing, gameplay or save/reload. Native-owner removal remains
**0/14**. Linux and retail data were not exercised.

### Direct PlayerZone field parity

Canonical SetActorZone now writes the compiler-probed
`APlayerReplicationInfo.player_zone` field directly, matching
`UnLevAct.cpp::Hp2LevActPriSetPlayerZone`. Its native offset is **992**, asserted
in both generated Rust layout and C++ ABI checks. This removes an unnecessary
name intern, property-class lookup and reflected field lookup from movement.
The existing null/netmode guards and captured `new_region` value are unchanged.

A focused standalone smoke used real canonical Level/LevelInfo/Model/Pawn
objects and an **external, layout-only PRI fixture**, detached before teardown.
It is **data-none**, macOS arm64 evidence, not admission or reflection evidence
for the full PlayerReplicationInfo class. The prior compiled adapter exited
**1** with `PlayerReplicationInfo has no canonical PlayerZone property`;
the direct-field adapter exited **0**. Actual Pawn MoveActor calls assigned
the new zone in standalone/dedicated/listen-server modes and preserved the old
zone in client mode. Report:
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/canonical-player-zone-60889/report.txt`.
Before/after logs in the same artifact directory:
`canonical-player-zone-before-reflection-removal.log` and
`canonical-player-zone-after-direct-field-assignment.log`.

After this correction, the seven-target acceptance command above passed
**7/7**, **37.91 seconds**
(`canonical-engine-world-owner-after-direct-player-zone-acceptance.log`).
`cmake --build --preset macos-arm64 --target hp2_verification_binaries` also
exited **0**; complete log:
`canonical-engine-owner-after-direct-player-zone-verification-build.log`.
The throwaway source was removed; its executable, reports and failure/passing
logs remain under the artifact directory. Installed-game routing and the
native-free acceptance gates remain open.

### Canonical network bit-archive property execution

Core's `ObjectArchive` now borrows a typed Engine network reader rather than a
C++ `FArchive` vtable. Network scalar/raw reads retain the actual bit cursor;
bounded reads preserve the native partial result and overflow flag. Object
properties carry their declared expected class into the network mapping.

Verification used the configured `network_archive_contracts` test environment
and **data-none**, not a running or installed game:

```sh
ctest --preset macos-arm64 -R '^(network_archive_contracts|bit_archive_contracts|property_value_contracts)$' --output-on-failure
ctest --preset macos-arm64 -R '^network_archive_contracts$' --output-on-failure
```

After concrete package-map compilation fixes, the combined command passed
`bit_archive_contracts` and `property_value_contracts` but failed the new test:
its property fixture incorrectly used a Package as a Struct-constrained outer.
Changing that fixture to the actual canonical `Core.Object` UClass resolved the
construction error; the second command passed **1/1**, **4.49 seconds**.
This was a fixture correction, not a relaxed construction constraint.

A separately compiled runtime consumer then exited **0**, executing actual
canonical Int/Bool/Float property `NetSerialize` at unaligned bit offsets,
uninitialized raw output, full/truncated bounded reads, and owner shutdown.
Its report is `network-archive-36938/report.txt`. Logs are
`canonical-network-archive-fixed-cache-types.log`,
`canonical-network-archive-correct-property-outer.log`,
`network-archive-runtime-smoke-build.log`, and
`network-archive-runtime-smoke-run.log`, all under
`out/macos-arm64/Testing/HP2/network_archive_contracts/Artifacts/`.
Earlier compilation failures remain in that same artifact directory.
The throwaway consumer source was removed; executable and reports remain.

This does not verify actor replication, complete connection/file ingress,
canonical player handoff, installed routing, or retail data. Native-owner
removal remains **0/14**; the production cutover gates remain open.

### Canonical Core console checkpoint; Engine cutover still open

Invariant: canonical console commands operate on Rust-owned class/default
storage and route diagnostics to the source-selected output, without calling
native UObject virtual methods.

On macOS arm64 with **data-none**, a standalone runtime consumer started a real
`CoreOwner` and exercised `SET`/`GET Core.System SavePath`, `MEM` through the
Core `Exit` log rather than the command output, `GTIME` through the shared
`GTempTime` counter and Core log, unknown-command fallthrough, and shutdown.
The timing diagnostic uses the existing typed `FTime::GetFloat` equivalent;
it does not substitute process uptime or print raw fixed-point ticks.

Verification:

```sh
ctest --preset macos-arm64 -R '^network_archive_contracts$' --output-on-failure
"$HP2_ARTIFACT_DIR/canonical-core-console-smoke"
```

The named test passed **1/1**, **4.52 seconds**. The separately compiled
consumer exited **0**. Its report is `canonical-console-50991/report.txt`;
build/run logs are `canonical-core-console-checkpoint-smoke-build.log` and
`canonical-core-console-checkpoint-smoke-run.log`. The CTest log is
`canonical-core-console-checkpoint-acceptance.log`. All paths are under
`out/macos-arm64/Testing/HP2/network_archive_contracts/Artifacts/`, using the
configured isolated test environment. The throwaway source was removed.

Integration first exposed a duplicate `AGameInfo` mirror declaration, missing
configuration error/result imports, command-host type mismatches, and an
implicit raw-pointer autoref. Those were corrected before the passing run;
the failing logs remain in the artifact directory. The compiler-probed font
page/character ABI generation also exited **0**; its log is
`canonical-font-mirrors-deduplicated-game-info-generation.log`.

This is not exhaustive static-command parity.
`OBJ LIST` CountMem's saving-archive route was not accepted at this checkpoint;
the allocation-counting checkpoint below records its later correction.
Profile reset is not exercised with this target's `slow_checks == 0`.
The borrowed file-archive capability does not establish persistent Actor
save/load integration.

The combined canonical Engine graph, complete scene drawing, production
startup routing, and installed-game load/play/save/reload remain unverified
or incomplete. The shipping path still uses native `UGameEngine`; no new
native-owner removal family is accepted. The completion gate remains **0/14**.
Neither Linux nor prototype/retail gameplay was exercised by this checkpoint.

### Canonical console allocation counting and byte-backed startup structs

Invariant: `OBJ LIST` counts retained object/string allocations without saving
an object, skipping unchanged defaults, or allocating a serialized payload.
`ObjectArchive::memory_count` now carries the source default versions with
neither loading, saving nor persistence set. Object/name slots remain unchanged;
metadata expression archives retain the source no-cursor `Tell == -1` behavior.
Reflected FString accounting uses the live native header's Num and Max.

The real canonical `Core.System` consumer reproduced the old undercount:
**200/200 bytes**, versus **304/304** including its unchanged default string.
After the fix it reports **304/304**. A second run reserved actual string
capacity without changing the text and reported **304/476**, matching the live
allocation headers. Both standalone consumers exited **0**, under **data-none**.
The first harness attempt used an invalid null outer; that setup error was
corrected before reproducing the actual counter bug. Failure logs are retained.

`StartupScalarKind::Byte` now emits and validates one-byte native struct fields.
A separate real Core owner registered a four-channel color, executed console
SET/GET at byte boundaries, observed `(A=255,B=254,G=1,R=0)` and raw channels
`[0, 1, 254, 255]`, then shut down successfully. This proves the Core metadata
path needed by Canvas DrawColor, not Canvas rendering.

Commands executed in the configured isolated environment:

```sh
ctest --preset macos-arm64 -R '^network_archive_contracts$' --output-on-failure
ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure
"$HP2_ARTIFACT_DIR/canonical-console-memory-smoke"
"$HP2_ARTIFACT_DIR/canonical-byte-struct-smoke"
```

The named tests passed **1/1** each. `rust_core_runtime` passed in **9.04 seconds**
under its **data-prototype** suite label; its new isolated memory regression
itself uses **data-none**, with report
`out/macos-arm64/Testing/HP2/rust_core_runtime/Artifacts/canonical-console-memory/report.txt`.
Standalone reports are `canonical-console-memory-12905/report.txt` and
`canonical-byte-struct-12887/report.txt`, under
`out/macos-arm64/Testing/HP2/network_archive_contracts/Artifacts/`.
That directory also contains `canonical-core-runtime-memory-capacity-regression.log`,
the `canonical-console-memory-*` build/run/failure logs, and
`canonical-byte-struct-smoke-{build,run}.log`. Throwaway sources were removed.

Compiler-probed UI, texture capability, integer/character map-pair and animation
flag additions support ongoing Engine producers; generator exit **0** is not
Engine runtime acceptance. Full object-family memory accounting, the combined
Engine build, both Canvas text paths, production startup/world drawing,
installed-game load/play/save/reload, Linux and retail remain unaccepted here.
The shipping world remains native `UGameEngine`; final removal gate **0/14**.

### Canonical script timing checkpoint; production renderer still open

Invariant: nested canonical script calls share the original script-entry depth
and cycle counter; only the outer dispatch is timed, including error unwinding.
Core now owns `GScriptEntryTag`, `GScriptCycles`, and `GUglyHackFlags`; the FFI
surface reuses their storage rather than introducing per-owner copies.

Executed in the configured isolated environment:

```sh
ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure
"$HP2_ARTIFACT_DIR/canonical-script-timing-smoke"
```

The named test passed **1/1**, in **8.40 seconds**. The standalone executable
loaded an isolated copy of prototype `System/Core.u` and actually executed
`Core.SimpleCommandlet.TestFunction`: result **666**, malformed-bytecode error,
then result **666** again after restoration. Nested dispatch did not double
clock; final entry depth was **0**, the outer counter was **32841**, and explicit
reset returned it to **0**. The original ugly-hack symbol round-tripped **0x27**;
owner shutdown succeeded. The executable exited **0**, under **data-prototype**.

Evidence is under
`out/macos-arm64/Testing/HP2/network_archive_contracts/Artifacts/`:
`canonical-script-timing-13377/report.txt`,
`canonical-shared-script-timing-core-runtime.log`, and
`canonical-script-timing-smoke-{build,run}.log`. The throwaway source was removed.

This accepts the Core timing path, not the combined Engine/FFI native link,
the production ProcessState caller, scene rendering, or installed-game
load/play/save/reload. Linux and retail were not exercised. The shipping world
still uses native `UGameEngine`; final removal gate remains **0/14**.

### Shared renderer recovery and canonical float-parser checkpoint

The overwritten dynamic-renderer policy was restored from the original producer
history and independently compared with the preserved authoritative source.
The focused actual-source executable passed **18 tests** under **data-none**.
This was recovery of previously implemented work, not a new production cutover.

Before the subsequent child-frame, clip-helper and Actor/Dynamic provider edits,
the shared renderer passed:

```sh
ctest --preset macos-arm64 -R '^scene_lighting_contracts$' --output-on-failure
```

The named test passed **1/1** in **1.97 seconds**. The configured isolated Cargo
invocation of `/opt/homebrew/bin/cargo test --offline
--manifest-path=/Users/mike/Code/Default/hp2/Cargo.toml -p hp2-render` passed
**148 tests**: **63 unit** and **85 integration**, with no failures or ignored
tests. These are **data-none** policy results, not canonical Engine or installed
game rendering acceptance. They must be rerun after the ongoing provider edits.

Canonical input float parsing now shares Core's key-search and conversion
continuation with the Native scalar parser. It retains the source ASCII word
boundary, ANSI narrowing and `atof` behavior: a missing key preserves output;
a found invalid value writes zero. Canonical calls use explicit UTF-32 pointer
casts rather than the unavailable `libc::wcstod`.

Executed in the isolated fixture environment:

```sh
/opt/homebrew/bin/cargo test --offline \
  --manifest-path=/Users/mike/Code/Default/hp2/Cargo.toml \
  -p hp2-core --lib parse_scalar::tests -- --nocapture
ctest --preset macos-arm64 -R '^rust_core_runtime$' --output-on-failure
```

The focused Core tests passed **3/3** under **data-none**. The named Core runtime
test passed **1/1** in **28.29 seconds**, labelled **data-prototype**. A standalone
Cargo executable compiled the actual `input_owner.rs::parse_float` helper with
the real Core parser and ANSI conversion. It observed `MAXSPEED` not matching
`SPEED`, missing-key output preservation, invalid text becoming **0**, lowercase
exponent input becoming **-125**, and independent `SPEEDBASE=-0.25`; exit **0**.
Its first build caught the signed/unsigned TCHAR pointer mismatch; that failure
is preserved alongside the corrected run. This exercises the numeric helper,
not full canonical `UInput::Exec` dispatch.

Evidence is under
`out/macos-arm64/Testing/HP2/network_archive_contracts/Artifacts/`:
`canonical-dynamic-history-comparison.log`,
`canonical-dynamic-recovery-run.log`,
`canonical-scene-provider-scene-lighting-contracts-after-type-fixes.log`,
`canonical-scene-provider-render-policy-suite.log`,
`canonical-shared-float-parser-unit.log`,
`canonical-float-sdk-core-runtime.log`,
`canonical-input-float-smoke-run.log`,
`canonical-input-float-smoke-after-tchar-fix.log`, and
`canonical-input-float-smoke-report.txt`. Throwaway smoke sources were removed.

Compiler probes also succeeded for warp-zone/default-texture fields and
`ENGINE_REN_ZONES` / `SHOW_PLAYER_CTRL`; this is ABI evidence only. The combined
Engine build is still unaccepted: the latest integration check failed on a
duplicate Actor sprite entrypoint during ongoing edits. Actor/mesh/particle,
occlusion/draw, lighting and game/level-tick hosts still require integrated
build and consumer verification. No Rust-owned game, installed load/play/save/
reload, Linux or retail claim is earned. Shipping remains native `UGameEngine`;
final removal gate remains **0/14**.

### Canonical scratch byte-count SDK checkpoint

`MemStackOwner::byte_count` now exposes the existing checked
`FMemStack::GetByteCount` policy to canonical renderer statistics: count the live
cursor in the newest chunk and full data capacities of older active chunks;
exclude reusable chunks after their marks are popped. No Native callback or
new allocation is introduced on success.

```sh
ctest --preset macos-arm64 -R '^mem_stack_contracts$' --output-on-failure
```

Observed **1/1 passed**, **12.77 seconds**, **data-none**. A separate configured
Cargo invocation of `-p hp2-core --test mem_stack_contracts --test mem_replay`
also passed **6/6**. An actual Core-owner smoke executable allocated and wrote
aligned scratch data, grew a second chunk, popped nested/root marks, checked
Tick, and released the unused pool. Observed live counts were **0 → 13 → 27 →
624 → 13 → 0**, exit **0**. This tests the new SDK, not DrawFrame integration.

Evidence under the same isolated artifact root:
`canonical-stack-accounting-contracts.log`,
`canonical-stack-accounting-ctest.log`,
`canonical-stack-accounting-smoke-run.log`, and
`canonical-stack-accounting-smoke-report.txt`. Throwaway sources were removed.
Combined Engine, installed-game and final-removal acceptance remain unchanged.

