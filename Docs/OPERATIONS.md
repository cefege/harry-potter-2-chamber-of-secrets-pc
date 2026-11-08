# Operations: build, test, and verification

This is the single source of truth for configuring, building, and verifying
HP2 across both eras: the legacy C++ tree (CMake/CTest, below) and the Rust
rewrite (`crates/`, "Cargo era" section). Every command below matches a preset
in `CMakePresets.json`, a target in `Build/CMake/HP2Targets.cmake`, a cargo
workspace member, or a registered CTest test name. Nothing here is invented;
if a knob is not listed, it does not exist yet.

Platform: macOS 15+ on arm64 only. Requirements: CMake >= 3.24, Ninja, and a
Python 3 interpreter (found via `find_package(Python3)`).

## Retail-flow smoke

Retail-flow smoke automates the scriptable portion of the product gate: a
windowed launch against the retail data root with sounds (process alive +
`presented_frames > 0`), and the continue-flag step when a save exists:

```sh
python3 Build/retail_flow_smoke.py
```

## Configure/build/test presets

| Preset | Build type | Sanitizers | Full map smoke | Binary directory |
| --- | --- | --- | --- | --- |
| `macos-arm64` | Release | off | off (`HP2_ENABLE_FULL_MAP_SMOKE=OFF`) | `out/macos-arm64` |
| `macos-arm64-asan-ubsan` | RelWithDebInfo | ASan + UBSan (`HP2_ENABLE_ASAN_UBSAN=ON`) | off | `out/macos-arm64-asan-ubsan` |
| `macos-arm64-tsan` | RelWithDebInfo | TSan (`HP2_ENABLE_TSAN=ON`) | off | `out/macos-arm64-tsan` |
| `macos-arm64-full-smoke` | Release | off | on (`HP2_ENABLE_FULL_MAP_SMOKE=ON`) | `out/macos-arm64-full-smoke` |
| `macos-arm64-retail` | Release | off | off | `out/macos-arm64-retail` |

ASan+UBSan and TSan are mutually exclusive; configuring with both enabled fails
at configure time.

## Exact commands

Configure (first time and after preset changes):

```sh
cmake --preset macos-arm64
```

Build every binary the behavioral test graph can invoke (this is the target the
commit gate builds; script-backed tests need no compiled target):

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries
```

Run the default verification suite:

```sh
ctest --preset macos-arm64
```

Run one test by exact name (substitute any name from
[BEHAVIOR_MATRIX.md](BEHAVIOR_MATRIX.md)):

```sh
ctest --preset macos-arm64 -R '^abi_widths$'
```

Sanitizer and full-smoke variants use the matching preset name:

```sh
cmake --preset macos-arm64-asan-ubsan
cmake --build --preset macos-arm64-asan-ubsan --target hp2_verification_binaries
ctest --preset macos-arm64-asan-ubsan

cmake --preset macos-arm64-full-smoke
cmake --build --preset macos-arm64-full-smoke --target hp2_verification_binaries
ctest --preset macos-arm64-full-smoke
```

## Retail-only test data

The behavioral suite targets an imported retail-only data overlay, not the
committed prototype tree. Import once from media you own (see
`RETAIL_IMPORT.md`):

```sh
cmake --preset macos-arm64 -DHP2_TEST_DATA_ROOT="$HOME/Library/Application Support/Harry Potter 2/Data/Retail"
```

or use the `macos-arm64-retail` preset, which points
`HP2_TEST_DATA_ROOT` at that default location.

Data-derived goldens (`package79-reference.json`, `spell-interactions.json`)
are machine-local under `<binary-dir>/goldens/` and no longer committed.
Regenerate them for your data root before the first suite run:

```sh
cmake --build --preset macos-arm64 --target regenerate_goldens
cmake --build --preset macos-arm64 --target hp2_verification_binaries
ctest --preset macos-arm64
```

## Test environment contract

Every behavioral test registered through `hp2_add_behavior_test` runs with:

- an isolated `HOME` and `TMPDIR` (HP2 derives its writable Application Support
  tree from `HOME`; immutable package data is selected explicitly with
  `-datadir` when runtime bootstrap is needed),
- `LC_ALL=C`, `TZ=UTC`,
- `HP2_TEST_NAME=<test name>`,
- `HP2_ARTIFACT_DIR=<artifact directory>` (pre-created),
- working directory = repository root,
- a 900-second timeout. A timeout is a first-class failure with retained
  artifacts; deterministic contracts must never hang a verification run.

## Artifact layout

Per test, under the configured binary directory:

```text
out/<preset>/Testing/HP2/<test>/Home/         # isolated HOME for the run
out/<preset>/Testing/HP2/<test>/Tmp/          # physical isolated temporary directory
out/<preset>/Testing/HP2/<test>/Artifacts/    # structured reports, logs, captures (HP2_ARTIFACT_DIR)
```

On Unix, `TMPDIR` points through a private short symlink to that physical `Tmp/`
directory. Compiler-cache startup sockets must fit the Unix socket address
limit even in a deeply nested checkout; artifacts and temporary files still
land in their isolated build-tree directories, and compiler caching remains
enabled. CMake keeps the private alias root in its internal
`HP2_TEST_TMP_ALIAS_ROOT` cache entry and recreates it if OS temporary cleanup
removes it.

The renderer smoke scripts additionally write their JSON report next to those
directories: `out/<preset>/Testing/HP2/renderer_smoke_wgpu/smoke-maps.json`
plus per-map engine logs under the sibling `smoke-maps-logs/` directory.

CTest failure output is captured by the presets (`outputOnFailure`); a preset
with zero matching tests is an error (`noTestsAction=error`), never a silent
pass.

### Rust port scoreboard (macOS arm64)

Generate the static inventory and G1–G11 acceptance report without launching
the game or running gates:

```sh
HP2_ARTIFACT_DIR=out/macos-arm64/Testing/HP2/port_progress_contract/Artifacts \
  python3 Build/port_progress.py
```

An explicit `--output PATH` overrides the artifact-directory default.
The command prints counter and gate tables and writes JSON; exit 0 means
the report was produced, not that the port is complete. Exit 2 means a
usage or inventory error. Without `--run-gates`, CTest-backed gates remain
`not-evaluated`; absent tests are never passing evidence.

The user-approved acceptance policy is now **functional gameplay**, not exact
framebuffer parity. `--parity-report PATH` retains strict reports as diagnostics;
their pass counts cannot promote or block G4/G10. The comparator is unchanged.
Use repeatable `--functional-report PATH` for exercised outcome evidence.
Without that evidence, functional checks remain `not-evaluated`, not passing.
The report format is version 2 with `acceptance_policy: functional-gameplay`.

G4 requires `data-prototype` checks `all_maps_load_run`, `movement_collision`,
`projectile_hit`, `world_interaction`, `cutscene_return_control`, and `map_travel`.
G5 adds `cross_process_persistence`: save, quit, start another process and
verify persisted gameplay state. Same-process reload alone is insufficient.
G6 requires launcher contracts and `data-none` `launcher_actual_ui` evidence.
G7 requires G1–G6, native-free product audits, bundle integrity and prototype
evidence for `oracle_archive`, `installed_launch`, `installed_persistence`.
G8 additionally requires the actual user's installed-Rust gameplay approval
before native deletion. G9 runs the macOS functional/component suite; existing
`parity-full` exclusions keep exhaustive parity diagnostic. G10 requires
`data-retail` evidence for `data_provenance`, `retail_launch`, movement, a real
projectile hit, world interaction, cutscene/control, map travel and cross-process
persistence. Prototype evidence never substitutes for retail. G11 remains
actual human audio/input/visual/continue observations. No gate is promoted by
the policy change. Required functional failures, crashes and script errors
remain blocking; missing assets needed by a required route remain blocking.

A functional evidence file is a version-1 JSON object with `profile`,
`executable` (repository-relative or absolute), `executable_sha256`, and
`checks`. Each check has `status: pass|fail|not-evaluated`. A passing check
also requires `command` (nonempty string array), `observations` (nonempty
descriptions of actual outcomes), and `artifacts` (nonempty array of objects
with `path` and `sha256`). The scoreboard verifies current executable and
artifact digests; it is an evidence ledger, not a substitute for scenario
verification. Never hand-mark an unexercised consumer as passing.

Use `--run-gates --preset macos-arm64` only when intentionally qualifying a
candidate; it executes CTest-backed checks, bundle integrity and the full
functional/component suite. The native denominator remains frozen in
`Build/engine-native-inventory.json`; `--freeze-native-inventory` regenerates
it only while the legacy catalog exists and requires exactly 173 entries.
`Build/human-acceptance.json` remains false until a human exercises the
installed bundle. Installed approval also needs `observer`, `date`, and
`installed_product: {executable, sha256}` matching the actual installed binary.
Approval cannot be inferred from the assistant's automated runs.

The exercised prototype cross-process persistence record is
`cross-process-persistence-observation/report.json` under canonical boot
artifacts. An earlier real replay process saves AOLDemo and exits0; a new
`hp2_game_rust Entry.unr -LOAD=9` process reuses that isolated HOME and
loads the unchanged Save9.usa. At ticks30/90 the observed saved Harry
location/rotation equal the source package, and CutScene0/CutScene9 remain
Finished with source played counts1. No strict failure marker is reported.
`functional-prototype-evidence.json` binds source save, reports and logs to
the current executable digest. This is prototype evidence, not retail,
installed-app human acceptance, or persisted-health proof.

Canonical audio native coverage is exercised without game data by
`ctest --preset macos-arm64 -R '^actor_audio_vm_native_owner_contracts$'`.
It dispatches indexed and named natives through real script frames, including
music FString ownership, non-null sound routing with no hearers, and the
source's no-audio-device results. This is not evidence of audible playback;
that remains a human-acceptance item.

`ctest --preset macos-arm64 -R '^pawn_path_vm_native_owner_contracts$'`
exercises canonical path search/reset and visibility natives without game
data: two-node ClearPaths state, authored next-node selection, anchored
reachability, Weight references, describeSpec outputs and index boundaries.
The strict prototype package probe is `cargo test --locked -p hp2-engine
--test engine_package_load -- --test-threads=1`, with `HP2_CORE_DATA_ROOT`
and `HP2_ARTIFACT_DIR` set. It is not registered as G1 until every package
loads successfully; metadata-only loads are not substitutes for that gate.

`ctest --preset macos-arm64 -R '^pawn_move_vm_native_owner_contracts$'`
exercises all 13 movement/noise natives through canonical script frames
without game data. It verifies action completion/cancellation, unchanged
script cursors for latent polls, null goals, speed clamping, strafe and
turn state, low-gravity steering against the zone's class default, and
MakeNoise client/throttle rules. A real scripted HearNoise event consumes
the packed `(FLOAT, Actor*)` parameters and publishes its received values.

`ctest --preset macos-arm64 -R '^actor_script_vm_native_owner_contracts$'`
exercises all 17 actor latent/animation natives without game data. Coverage
includes Sleep's strict half-tick boundary, latent poll cursors, animation
completion, classic versus linked/default skeletal sequences, real CPU
skeletal pose queries, and protected/persistent/transient channel lifetimes.
The world has an initialized engine/cache and empty BSP model; an authored
Core-owned AnimChannel subclass exercises class loading and root playback.
This does not exercise GPU rendering or prototype/retail animation assets.

`bit_archive_contracts` also exercises writer/reader payloads and overflow
at a forced four-byte-packed archive address. SDK FOutBunch/FInBunch packing
does not guarantee the eight-byte alignment of a standalone archive base.
The existing bit, channel and download captures remain unchanged.

```sh
ctest --preset macos-arm64 -R \
  '^(console_vm_native_owner_contracts|scripted_texture_vm_native_owner_contracts|core_unix_file_manager)$'
```

These canonical console/platform/miscellaneous contracts use `data-none`:
real GameEngine pending-travel cancellation, declaring-class native override
selection, single-pass console parameter import, PreClientTravel script reentry,
key boundaries, URL formatting, cache-file replacement and independent cache.ini
flush, Unicode time-demo persistence, autonomous physics, authored PlayerCalcView
outputs, BSP surface/decal selection, font ownership and strict script errors.
Configured `ini:` class names resolve through Core; a loaded Rust native module
permits an absent package file but never bypasses an existing corrupt file.

The ScriptedTexture contract checks indexed pixels, palette mapping, FLOAT out
parameters, replacement and source-bitmap restoration through real PostLoad and
virtual Tick/Get/Lock paths. It does not exercise a GPU or native-font drawing.
`core_unix_file_manager` checks real multi-chunk file copying and read-only
destination failure; it is not evidence of an actual cross-device filesystem move.
Captured console, input, movement-file, gesture, player, physics and scripted-texture
replays remain separate parity checks; do not regenerate their goldens.

Script clipboard natives preserve Core Unix's intentionally disabled clipboard
API (copy has no effect; paste is empty), not SDL's clipboard implementation.
The no-audio-device ClientHearSound smoke is not evidence of audible playback.
The strict prototype package probe remains required before G1 can pass.

```sh
ctest --preset macos-arm64 -R \
  '^(actor_file_vm_native_owner_contracts|actor_file_transfer_owner_contracts|decal_vm_native_owner_contracts|actor_screenshot_vm_native_owner_contracts)$'
```

These B5 contracts execute all 11 file/image/decal/query natives through real
canonical VM frames. The `data-none` CPU cases cover BSP attach/detach and
inclusive polygon boundaries, indexed/24-bit BMP pixels, palette reuse, masking,
animation links, generic object-file round trips and the five GameSaveInfo
fields. File creation uses the shared Unix manager, including captured INT-size
limits; save-slot paths preserve FString concatenation rather than replacing
the base with an absolute-looking leaf. Unix FArchive FNames use their UTF-16
text wire; base file archives do not invent UObject package-index mappings.
An empty owned FString saves its native compact count of zero, without an
extra terminator byte; reflected property strings use that same owned codec.
Relative object/image filenames resolve beneath the canonical System directory.
Actor file transfers retain the legacy GNull distinction: an opened transfer
returns true after deletion even if ordinary I/O set `ArIsError`; that bool is
not proof of a complete write. Genuine callback exceptions still propagate.
Binary image loading separately uses the source `appLoadFileToArray` Close
result and rejects an ordinary failed read.

The screenshot contract is a main-thread executable (`harness=false`) with
`sdl,renderer-wgpu`. CMake links its final executable to the existing SDL archive
and reads that archive's generated Apple dependency flags. It submits a real
wgpu frame, reads its pixels, executes `snap 8`, and checks the owned 128x64
texture and zero padding. `SNAP` takes an exponent: a 512x256 viewport produces
a 2x1 snapshot at `2^8` shrinkage. `HP2_CAPTURE_FRAMES` enables the renderer's
existing GPU capture path when Aqua reports Occluded; an offscreen capture is
not evidence that a window frame was presented or visually accepted.

The native-free file/texture checks are not gameplay, retail, audible-audio or
physical-input acceptance. G1 still requires every prototype package to load
with strict flags; these component results do not promote a feature gate.

`ctest --preset macos-arm64 -R '^texture_owner$'` now loads real prototype
Core/Engine metadata without `NoVerify`, saves and restores its texture graph,
and collects the full metadata graph. Canonical GC reuses the one active
reference archive and keeps large non-reference metadata match frames off the
recursive reference path. Visits remain synchronous and source-ordered;
this is not a larger test-thread stack or deferred-reference workaround.

The canonical StatLog batch executes all sixteen named natives over real
StatLogFile, GameInfo, Mutator and PlayerPawn records:

```sh
ctest --preset macos-arm64 -R '^stat_log_vm_native_owner_contracts$'
```

This `data-none` scenario writes and flushes UTF-16LE/CRLF log bytes, checks
world-log XOR and MD5 output, replaces read-only files, and reenters FileLog
through real script events. Its synthetic `.u` files are sized for the source
filename-plus-size checksum; they are never parsed as script packages.
Batcher calls use the existing Unix fork/system kernel and produce actual
filesystem effects. The canonical graph currently installs that kernel's
SIGCHLD reaper before its first StatLog fork, rather than at the C++ source's
`appPlatformInit` point. Unix `appLaunchURL` is intentionally empty in the
source; BrowseRelativeLocalURL does not claim to open a browser.

StatLog's archive/context ownership remains in the shared Rust runtime map;
the legacy internal INT fields do not hold truncated host pointers. Native
FString fields retain their compiler-probed four-byte packing. Reflected
text/config export reads their headers unaligned and uses the existing
`FString::operator*` empty-string semantics, not a null character pointer.
These component checks do not satisfy strict all-package loading, gameplay,
retail or human acceptance, and do not promote a feature gate.

Canonical package lookup uses the shared `appFindPackageFile` policy over the
live Core-owned `System` object and the same Unix file-manager leaves as file
transfer. Relative search paths resolve from the game's `System` directory;
imports are not forced to `.u`. Class registration loads config/localization
on the class default object, as the native metadata bridge does.

```sh
ctest --preset macos-arm64 -R \
  '^(core_package_file_owner_contracts|core_unix_file_manager|package_file_contracts|class_registration_contracts|object_construction_contracts)$'
```

These five `data-none` contracts cover configured asset-extension lookup,
path precedence and live updates, relative data roots, localized packages,
Unix case recovery, non-truncating GUID-cache append-open, and the native
256-TCHAR output boundary. Filesystem lookup fixtures are not real game
packages; they do not satisfy G1's strict prototype metadata acceptance.

The strict G1 package gate and canonical Fire resource contracts are registered:

```sh
ctest --preset macos-arm64 -R \
  '^(canonical_engine_package_load|editor_transaction_owner_contracts|fractal_owner_contracts)$'
```

G1 loads all eleven prototype script packages with strict flags, checks
`Engine.Actor.EInputKey`, constructs `Engine.GameEngine`, and shuts down the
loaded graph. Its UTF-16LE runtime log retains source-defined conflict
diagnostics: same-named Palette/Texture pairs are logged, not rejected.

The Fire contract uses real prototype Core/Engine/Fire metadata and canonical
Water/Wave/Wet/Ice resources. It checks Water base allocation/clear, partial
state and observer edits across a failing random callback, Wet source
resampling, actual Ice CPU pixels, and normal shutdown. FireTexture archives
use the shared eight-byte spark serializer rather than dropping native tails.
Optional Fire observers expose only the three existing source report events.
The Rust game's `RuntimeReporters` binds their canonical launcher adapters;
paired Fire runtime acceptance is still separate from the resource contracts.
A host table-publish hook is optional only when no C++ shadow tables exist;
shared Rust tables and RNG initialization still run.

G1 and Fire use `data-prototype`; transaction contracts use `data-none` and
do not claim a live editor UI or editor-engine undo integration. None of
these checks establishes game-loop, GPU presentation, retail, audible-audio,
physical-input, or human acceptance.

The Step 3 map-asset preflight also runs in `canonical_engine_package_load`.
It loads `LevelInfo0` and `MyLevel` from the real prototype `Entry.unr` using
an explicit filename, rather than treating the filename extension as an
object-path segment. A supplied package remains the linker root, not the
outer of a duplicate package. Reusing a cached linker still checks the
expected package GUID. Map PostLoad also retargets every actor in the map's
package to the real `ULevel` through `XLevel`. This exercises canonical asset
loading and shutdown, not game boot or presentation; G2/G3 remains separate.

The native-free Rust product and early egui launcher prerequisite are available:

```sh
cmake --build --preset macos-arm64 --target hp2_game_rust
ctest --preset macos-arm64 -R '^launcher_ui_contracts$'
```

The egui 0.37 SDK's `default_fonts` feature is explicit: its default feature
set contains no fonts. `launcher_ui_contracts` exercises the real shared
presentation policy and settings controls with egui input under `data-none`.
For a bounded real-window smoke, set `HP2_LAUNCHER_SMOKE_FRAMES=3` and
`HP2_ARTIFACT_DIR` to an existing isolated artifact directory, then launch
`out/macos-arm64/hp2_game_rust --datadir=HarryPotter2/Unreal`. The counter
advances only after actual surface presentation. `launcher.bmp` is read back
from the acquired surface, not an offscreen replacement.

The macOS arm64 `data-prototype` smoke presented three frames, exited zero,
and produced a visually inspected readable capture. Evidence is under
`Testing/HP2/canonical_game_boot_entry/Artifacts/launcher-ui-acceptance.json`
and `launcher-fonts.png`. This closes the early frontend prerequisite, not
G6, retail, physical-input, or human acceptance. The separate G2 Entry runtime
test has passed; G3 and paired game/render parity remain open.

`rust_core_runtime` retains regressions for consumed bool-reference opcodes,
distinct expression receivers, literal class cast operands, independent
statement/return storage, constructor-generated native-property names, and
ordinary events on frameless or zero-probe-mask receivers. Indexed natives
are exercised through script expressions, not the source-rejected direct
`ProcessEvent` route. Native property offsets and package checks remain strict.
Function reference/config/constructor chains use `GetInheritanceSuper()`:
an overridden UFunction never borrows its super-function's local fields.
`canonical_engine_package_load` checks that constructor fields belong to and
fit their actual function. `EX_New` consumes four operands without an
`EndFunctionParms`; its runtime regression verifies that the following return
and the newly constructed canonical object both survive.

Canonical event wrappers resolve active-state methods before class methods.
The Entry smoke creates the real UWindow root and opens MAIN. Native-text
requests now use macOS TCHAR scalar units rather than UTF-16 code units;
`canvas_native_text_scalar_contracts` covers supplementary-plane text.


Game boot performs the source `appPlatformInit` System phase before engine
construction: load the rooted `Core.System`, reject five or fewer search
paths, and apply only configured `Suppress` names to real FName flags.
Unconfigured categories remain visible; no warning or comparison rule was
changed. `rust_core_runtime` covers both sides of the search-path boundary
and configured/unconfigured suppression.

Observed macOS arm64 `data-prototype` commands:

```sh
ctest --preset macos-arm64 -R '^(rust_core_runtime|vm_dispatch_contracts)$' --output-on-failure
ctest --preset macos-arm64 -R '^(canonical_game_boot_entry|canonical_renderer_smoke_maps)$' --output-on-failure
```

Both commands pass. All three maps complete 120 ticks; the affected 64-test
batch, nine unchanged captured-fixture replays, and 20 shared Fire tests also
pass. Physics argument records now match the four-byte reflected parameter
ABI, and model members remain live until after virtual Destroy has restored
moving-brush pools. The shutdown regression observes the restored nonempty
BSP pools before final member release. Singular script calls reject recursive
entry and permit a subsequent independent call.

State startup enters the source `Begin` label, not the `BeginState` event.
Canonical actor state steps supply the source `MAX_CONST_SIZE` scratch space:
discarded assignment expressions still write their result. The affected
`actor_process_state_contracts`, `rust_core_runtime`, `vm_state_contracts`,
and `vm_object_contracts` pass, followed by the named G2/G3 smoke above.

Light-manager initialization is once per owner. Previously a nested per-light
callback cleared the surface's live light/fog maps and cache-unlock state,
leaving paletted format zero and zero UV scales. The retained
`renderer_light_manager_contracts` regression fails before this correction
and passes after it; `scene_lighting_contracts` also passes. Actual captures in
`three-map-visual-smoke-preserved-live-light-maps-frames` established the
textured Entry room. Subsequent source fixes restore sustained zoned-world
draw lists, native-direction vector shifts, nested cutscene localization,
case-insensitive FString switch matching, dynamic actor creation, and pawn
weapon-attachment fields without moving the pawn outside its collision hash.
LOD material locks now populate the shared texture-info records consumed by
the renderer, rather than private records with no submitted palette.
`three-map-visual-smoke-indexed-native-triangles.json` records three successful
120-tick prototype captures with intact Harry geometry and real
PrivetDr/Ch2Skurge worlds and cutscenes. LOADING was an eight-byte reference
written into the four-byte HubStackLevel, corrupting adjacent LevelAction;
the reflected scalar writer now copies the referenced value. Harry's torn
mesh was indexed triangles incorrectly expanded as a fan. Actual debugger
captures now match the C++ oracle bit-for-bit for all 600, 669 and 42 submitted
body vertices, including UV/light/fog values. LOD clipping also uses native
NearClip and full-screen clamp dimensions. Decal clipping passed an actor
pointer where the shared helper requires an FDecal record; this zeroed the
material scale/glow and produced infinite UVs. The corrected call supplies the
record. Actual first-quad GPU inputs now match all six native vertices
bit-for-bit across all 13 position/UV/light/fog components
(`step4-rust-correct-decal-record-gpu-proof.log`). Wand lighting and full image
parity remain separate findings.

The full shared Browse policy has a canonical provider for owned URLs,
links, entry/failure, restart/pop, save-slot, local, network and source-disabled
external URLs. Real save paths and persistent cache transactions follow the
shared runtime initialization policy. Unmapped transient-private references
serialize as null; nontransient-private dependencies still reject saves.
ServerTravel carries the native FString descriptor, not a raw TCHAR pointer.
The input harness now requires a completed saved-map LoadMap, not just a
written save file.

Named runtime verification (`data-prototype`):

```sh
ctest --preset macos-arm64 -R '^(canonical_game_boot_entry|canonical_renderer_smoke_maps|canonical_input_script_smoke|canonical_save_reload_smoke)$' --output-on-failure
```

G2 Entry and G3 three-map tests pass. Save/reload passes with actual
`ProcessServerTravel: ?load=9`, loading Save9.usa and possession of
`harry Save9.Harry0`. The initial four-test command exited 8: gameplay input
recursively expanded an alias through Viewport::Exec until the main stack
overflowed. A local boolean did not survive viewport reentry. Both input
adapters now share the native process-wide alias scope.

```sh
ctest --preset macos-arm64 -R '^(canonical_input_script_smoke|canonical_save_reload_smoke)$' --output-on-failure
ctest --preset macos-arm64 -R '^(input_edge_contracts|input_axis_order|input_release_all|input_script_contract)$' --output-on-failure
```

Both live G5 tests pass; all four input boundary/parser tests pass. The
unchanged `input_uncon` captured native fixture and fused-axis test also
pass. Logs are `step5-shared-native-alias-scope-live-gates.log`,
`step5-input-edge-axis-release-and-parser-contracts.log` and
`step5-unchanged-native-input-fixture.log` under
`Testing/HP2/canonical_game_boot_entry/Artifacts`.

The approved G4 selector is now `authored-or-tick`: authored events retain
their existing checkpoints; proven absence permits explicit snapshots after
the first, middle and final simulation ticks. The existing package79 reader
checks absence, and tick manifests carry the actual map's SHA-256. Missing,
ambiguous or malformed authored inputs still fail. Comparison fields,
availability rules, quantization and map selection are unchanged.

`game_test_contract` passes, and all 188 launcher-library tests pass. A valid
tick manifest exposed the shared JSON scanner matching `"tick"` as a string
value before finding the actual `"tick":` key; generic member lookup now
requires the colon and its regression passes.

The first real Entry pair (`entry-paired-authored-or-tick-initial.json`) found
an empty C++ ReadPixels and a one-unit HarryShadow Y difference in the existing
1/1024 quantization. Both are corrected without changing comparison:

- The oracle now reads its actual completed GPU scene, preserving native
  FColor byte order and logical viewport resampling. Its buffer/byte-order
  regression passes. Occluded frames use the existing real offscreen render
  path regardless of capture or fidelity configuration, so ordinary engine
  readback also works. An offscreen scene is not counted as presented.
- Core dynamic-actor bounds previously treated every mesh as ordinary UMesh,
  omitting skeletal GetRenderBoundingBox's root-only animation-cache update.
  The existing typed mesh-bounds provider now supplies this dispatch.

Two further measured producer defects are corrected: Core lightmap coordinates
now apply native `Inverse().Transpose()` rather than inverse alone, and the
C++ CoreText glyph projection converts origin/size rectangles to the min/max
extents required by the shared raster bridge. Before the latter correction,
native glyph 38 requested 9×11 coverage while the provider produced 10×11;
the raster callback returned false and triggered bitmap fallback. The corrected
native draw now succeeds, and its actual debug-text pixels match Core.

Entry paired parity now passes: `entry-paired-native-lod-sprite-lighting-owner.json`
records both executables exiting zero for 300 ticks, delta 0.02, RNG seed 1,
with every observation and real frame hash matching at ticks 1/150/300.
The last 138/135 differing wand pixels came from the LOD lighting owner:
native `UnMeshRnLOD.h` uses `Sprite->Actor` for lighting, not the mesh owner.
Core now matches that operand while retaining mesh ownership for geometry and
textures. The off-path ordinary-mesh experiment was rolled back.

G4 still requires every playable prototype map, not just Entry. Its explicit
CTest invocation bypasses preset label exclusions:

```
ctest --test-dir out/macos-arm64 --output-on-failure \
  --output-log out/macos-arm64/Testing/HP2/canonical_game_boot_entry/Artifacts/step4-full-prototype-paired-gate.log \
  -R '^parity_paired_maps$'
```

The regular test presets exclude `parity-full`, even with `-R`; a zero-test
preset listing is not a missing registration. This gate first smoke-scans all
oracle maps, then compares every `playable_level` record with unchanged fields,
availability rules and thresholds. Reports are
`Testing/HP2/parity_paired_maps/Artifacts/{oracle-smoke,parity}.json`.

The initial full gate failed before paired comparison: 94/100 oracle records
passed (88 playable levels and 12 package-structure checks were selected).
Adv12Chamber, Adv3DungeonQuest, Adv4Greenhouse, Adv6Goyle, Grounds_Night and
Grounds_hub all failed when cutscene screenshot commands requested ReadPixels
without capture/fidelity configuration while the window was occluded.
The failed report and all logs are preserved in
`g4-initial-oracle-baseline-failure.tar.gz`, alongside the original CTest log.
The renderer now keeps the real completed scene available independently of
optional reporting. All six maps subsequently passed 300 ticks with exit zero,
zero captured PNGs and no failure markers in
`oracle-six-cutscene-maps-normal-readback.json`. Strict Entry pairing also
passes after this correction in `entry-paired-normal-occluded-scene-readback.json`.
The subsequent full run passes all 100 oracle records, preserved in
`g4-oracle-baseline-100-pass.tar.gz`, but pairing stops at AOLDemo's unresolved
authored-source heuristic. That heuristic resolved only 23/88 playable maps;
raw filename/name-table matches did not establish semantic source coverage.
The sweep now records each per-map GameTestError as an explicit failure and
continues, retaining a nonzero aggregate result. The actual two-map reporting
smoke (`g4-two-map-source-error-aggregation.json`) reports AOLDemo's error and
then a passing strict Entry comparison, exiting one with 1/2 passed.
This is reporting proof, not G4 acceptance. The complete gate must still pass.

Semantic discovery now resolves effective active actors, inherited defaults
and instance overrides instead. The 88-map CPU preflight resolves 46 authored
and 40 genuinely tick-only cases; two maps retain required-source errors:
Quidditch_Intro references QudditchCamLesson1/2, and Studies/TrenchRun references
00308QuidditchTrenchRun. Those exact runtime files are absent; differently
named scripts are not aliases or replacements.

Both observers now identify authored boundaries by source kind, owning
CutScene path, thread slot and real cursor: curScriptLine for disk scripts,
curScriptPosition for inline scripts. Inline map paths and hashes belong to
source provenance, not an invented disk locator. The existing observation
fields and availability rules remain unchanged. `game_test_contract` and all
188 launcher-library tests pass. The first boundary-contract run failed on
NUL-terminated localization and a bare-CR expectation; the parser now stops
at NUL, and the expectation follows native GetNextLine's two-character
advance. The original failing log is retained.

The real owner smoke also exposed two canonical defects. Queued autosave
assembled UTF-16 while both owners expose UTF-32 TCHAR paths; the scratch
buffer now uses the correct units. Adv3DungeonQuest subsequently writes a
valid 5,016,813-byte Save0.usa containing Save0.MyLevel and Save0.Harry0.
Audio initialization now publishes the native-equivalent USound::Audio
callbacks and clears them before initialized normal/error teardown.
PrivetDr completes 300 ticks, and an independent LLDB stop observes the
real note_destroy call from CanonicalDestroyActorOps; that probe is
intentionally killed, not counted as a completed run.
`canonical_input_script_smoke` and `canonical_save_reload_smoke` both pass
after these changes.

Startup rebasing now matches native `UnGame.cpp`: LoadMap SetBase/ClearBase
do not notify the actor, while normal rebases still execute BaseChange.
The extra notification had switched InvisibleSpawn0 from PHYS_None to
falling, then walking, repeatedly separating it upward from the floor.
`actor_physics_owner_contracts` passes with a concrete Scout exercising
the real Pawn.BaseChange script. The initial abstract-Pawn fixture failure
is retained in `step4-bootstrap-rebase-physics-owner-contracts.log`.

Actor-sprite leaves 108/109 now own actor hit push/pop, matching native
`UnSprite.cpp`, rather than invoking global renderer PreRender/PostRender
for every sprite. Those extra frame scopes consumed gameplay RNG draws.
The canonical viewport tracks nested hit sizes and calls the real WGPU
boundary; WGPU intentionally does not produce hit payloads. The new
`actor_sprite_hit_owner_contracts` exercises normal and nested hit scopes
without a window/GPU surface; it and `renderer_host_contracts` pass.
Strict Entry still passes in `entry-paired-actor-hit-scope-fixed.json`.
Diagnostic AOLDemo random-call totals fall from 737765 to 175452; native
remains 175563. These raw diagnostics are not accepted trace comparisons:
the existing trace validator rejects startup samples with null consumers.

Dynamic sprite splitting now uses the native solid-plane predicate:
`NumVertices > 0 && !(NodeFlags & 0x21)`, not the unrelated `NF_PolyOccluded`
bit. The new terminal-admission regression first observed `None` instead of
`Final` for a normal four-vertex solid plane; after correction,
`renderer_host_contracts` passes, covering normal/occluded solid, non-CSG,
new and empty planes. Initial fixture borrow and uninitialized-stack failures
are preserved separately from that behavioral reproduction.
Both real 300-tick processes still exit 0. Strict Entry passes in
`entry-paired-dynamic-bsp-solid-fixed.json`; its AOLDemo run retained six matches,
fifteen reached actor differences and 285 unreached boundaries. That correction
alone did not earn the frog or G4 target.

Canonical selected-object/class loading now acquires lazy linker metadata;
explicit `CoreAccess::load_package` separately constructs all eligible exports,
including when the linker was previously cached. `get_package_linker` retains
parent/GUID validation without materializing unrelated exports, and import
verification resolves metadata rather than creating imported objects.
The saved-package regression first failed because selecting one TextBuffer
created its unrelated sibling; it now covers nested requested outers, exact
UTF-32 payloads and later explicit full-package promotion.
Named `rust_core_runtime`, `canonical_engine_package_load`, `level_summary_owner`,
`mesh_owner`, `texture_owner` and `renderer_host_contracts` pass under
`data-prototype`.

The first real lazy-loading launch exposed an LOD adapter defect: its array
and loader addresses were computed from UMesh instead of each embedded lazy
record. A live stack captured the resulting registration-invariant failure.
The adapter now uses the actual verts/tris/connects/vert_links records, matching
ordinary-mesh and canonical mesh-owner descriptors. Both real 300-tick
processes exit 0 after correction; strict Entry matches all three checkpoints
in `entry-paired-lazy-linker-correct-lod-descriptors.json`.

Native XA stream-source extraction calls `USound::Data.Load` before exposing
its bytes; the canonical callback now performs the same bounded lazy-array
materialization with Core access. Before correction, every reached AOLDemo
audio comparison lacked its native playing ambience and registration failed.
Afterward, all 21 reached audio snapshots match. `sound_owner` passes under
`data-prototype`; `actor_audio_vm_native_owner_contracts` and
`file_stream_contracts` pass under `data-none`. Both actual 300-tick owners
exit 0, and strict Entry remains a complete three-checkpoint pass.

Complex-surface texture biases now use the same fused `FVector::operator|`
implementation as native, rather than separate multiplication/addition.
Matching live native/Core facet inputs reproduce a one-ULP Y-bias difference.
The actual Core adapter regression fails before with bits `0x4338d2b6` instead
of `0x4338d2b7`, then passes after using `hp2_core::math::vector_dot`.
`renderer_host_contracts` now enables `renderer-wgpu` without opening a GPU or
window, covering seven real owner cases. Its initial test-fixture visibility
compile failure is retained separately from the numeric reproduction.
Both actual 300-tick owners exit 0; strict Entry still passes completely.

The remaining localized glow mismatch came from sprite-icon depth, not a
proven corona-policy defect. All 80 live native/Core icon requests matched
rectangles, colors, fog and flags, but Core supplied Z zero while native used
the actual sprite MinZ. Core now submits both the live sprite span buffer and
MinZ, matching `UnSprite.cpp` case 106. Renderer-host contracts pass all seven
cases, and `canvas_compatibility_contracts` preserves supported bitmap mode.

`g4-native-sprite-icon-depth.json` now matches all 21 reached AOLDemo boundaries
completely, including every framebuffer hash; both 300-tick processes exit 0.
`entry-paired-native-sprite-icon-depth.json` remains a complete three-checkpoint
pass. These claims hold under `data-prototype` only.
G4 is still unearned: AOLDemo's other 285 authored checkpoints remain unreached
failures; the full 88-map sweep below is not a qualification pass.
PrivetDr previously reached 44/615 required boundaries in each process.
No checkpoint, field, availability rule or threshold is dropped. Current
reports and source/owner proof logs remain under
`Testing/HP2/canonical_game_boot_entry/Artifacts`.

The completed `g4-all-playable-native-lazy-depth.json` sweep reports 28/88
strict passes, with every map reported and none skipped. It exposes 22 Rust
process failures, two independent native process crashes, and two maps whose
required script assets are missing. Other failures include reached state/hash
differences and unreached authored boundaries. The archived 100-pass native
baseline has not been retried to conceal these later native crashes.

Parametric mip access now retains the generator-owned live bitmap instead of
reloading its deliberately empty serialized body. Ordinary textures still
materialize their lazy bytes before use; `InfoMipPublish` only publishes them,
matching native Lock. A real saved/reloaded `WaveTexture` regression fails
before with zero bytes instead of 256, then passes after actual generation
and lock publication. `fractal_owner_contracts`, `texture_owner` and
`renderer_host_contracts` pass together in
`step4-generated-parametric-mip-contracts-after-fix.log`.

The actual 300-tick `g4-parametric-mip-affected-maps.json` follow-up exercises
all seven formerly failing mip/Water/Ice maps plus Entry and AOLDemo. Both
processes exit 0 for all nine maps; strict Entry and pactors2 pass, while
AOLDemo retains all 21 complete reached matches and 285 unreached failures.
The remaining maps still have honest parity/coverage failures. These are
`data-prototype` results, not retail or full G4 acceptance.

Hub screening now treats `Actor.ExcludeGameStates` as one FString, matching
native `Caps().InStr()`, rather than interpreting its UTF-32 characters as
FString array headers. Live LLDB stopped the original fault in memmove with
source address `0x5300000047`; actual owner substring contracts now pass.

Metadata-only import verification can acquire a real package linker without
populating the top-level import's XObject. Canonical resolution now uses that
existing linker root when XObject is null, preserving existing resolved roots
and still rejecting a missing root. A real saved nested-resource consumer
fails before with `Unresolved package import`, then passes: SafeReplace
resources become null and non-SafeReplace resources still error. No load-count
guard, resource alias or class flag was changed. The apparent recovery
`EndLoad` error masked the earlier import failure.

`step4-metadata-root-and-hub-owner-contracts.log` passes eight named contracts;
the saved CTest output includes the new real import and scalar-string cases.
`g4-core-root-hub-all-prior-runtime-failures.json` then exercises all 22
originally failing Rust maps plus Entry/AOLDemo for 300 ticks. All 24 native
and Rust processes exit 0, with 10/24 strict passes. The seven BSP bounds
failures also disappear with the import-root correction: no model validation
or game data was changed. Strict Entry and all 21 reached AOLDemo checkpoints
remain complete matches; AOLDemo's 285 unreached cases still fail. This is
prototype runtime recovery, not a fresh full-88 pass or retail/G4 acceptance.

VM `Actor.Spawn` now preserves the native distinction between automatic
UObject names and SpawnTag. The real VM-native consumer fails before and
passes after; Quidditch's spawned Hoop/Trail/CameraTarget IDs now agree.
Interpolation-manager virtual physics dispatch is also restored.

Dispatch alone did not move the spline actors: the canonical native class
registry lacked Keypoint/InterpolationPoint/InterpolationManager layouts
and the manager's nineteen CPP property bindings. These now use measured
ABI fields, including seven newly compiler-probed camera/fog fields, rather
than inferred offsets. The regression executes the real prototype Init
script before physics; it fails before at owner position 0 versus 128 and
passes after with the expected position, velocity and acceleration.
`step4-interpolation-native-binding-and-ogg-source-after-fix.log` passes four
targeted contracts; `step4-interpolation-audio-broader-owner-contracts.log`
passes seven broader contracts. The actual eleven-map
`g4-interpolation-native-bindings-and-streamed-music.json` reports both owners
exit 0, exact moving-ghost state in Ch2Skurge, twelve complete reached matches
in Adv11aCorridor, strict Entry retained and twenty-one complete reached
AOLDemo matches. Unreached authored boundaries still fail.

Streamed Ogg filenames are relative to System and must remain native TCHAR
buffers until the shared stream manager converts them to UTF-8. The initial
direct decoder smoke missed that source-width contract: the actual duel
still could not register its music. The replacement regression drives real
Core-owned USound filenames through actual stream creation and decoding,
checking the stock fixture's 441344 PCM frames, 44100 Hz and stereo format for
native relative and absolute filenames. It fails before the UTF-32 correction
and passes afterward; absolute filenames now borrow their existing storage.
`step4-real-ogg-stream-consumer-and-audio-contracts-after-utf32-fix.log` passes
five named contracts. `g4-duel-real-ogg-native-source-utf32.json` then observes
`PlayMusic started source 2` and fourteen complete reached matches, with both
owners exiting 0. Its ninety-four unreached authored boundaries remain
failures. These are prototype results; no retail or full G4 promotion follows.

The next complete strict sweep,
`g4-all-playable-runtime-recovery-native-bindings-ogg.json`, passes 41/88 maps
(previous full result: 28/88). It reports five Rust and four native aborts;
all original abort logs identify `SaveSlotPath exceeds Tick filename buffer`.
The longer isolated paths exposed the shared 256-TCHAR save limitation.
Missing authored-script sources remain map errors, not tick fallbacks.

Queued-save logging, slot filenames, hub-copy buffers and Core path formatting
now share the platform filesystem path capacity. Only the standalone Rust
product was rebuilt; the frozen native app retains its old limit. In
`g4-canonical-platform-long-save-path-and-real-package-group-leaf.json`, all
five affected Rust processes exit 0. The five actual Save0 files have
267–272-character absolute paths and parse with Level/Harry exports;
`g4-platform-long-save-package-proofs.json` records each file. Frozen-native
long-path aborts remain explicit failures. No path truncation or shorter
test-root workaround is used.

Native `ULinkerLoad::Create` searches class/name with INDEX_NONE rather than
requiring an export directly under the package root. Canonical selected-object
loading now follows that group-search policy. Its real saved nested-text
consumer fails before for `LazyObjects.Selected`, then passes for both leaf
and fully qualified names while retaining lazy unrelated exports and explicit
whole-package promotion. Seven affected owner contracts pass in
`step4-package-group-leaf-and-save-path-owner-contracts.log`; actual input and
save/reload pass in
`step4-post-platform-path-and-group-leaf-input-save-live-contracts.log`.
Quidditch's three camera differences disappear and seventeen reached complete
checkpoints match, but three dialogue-audio differences remain.

The same follow-up records new AOLDemo reached-state drift, including changed
Harry yaw in the unchanged native binary; it does not establish the loader
correction as the cause. Triggers diagnostic admission traces separately show
the first recursive bounds gate admits native shadows but rejects Core
shadows. The update traces were not published by either owner. These are open
findings, not full G4 acceptance or grounds to alter snapshots or native policy.

Bounded live shadow probes then capture identical box coordinates, scene
transforms, near clip, viewport dimensions and render mode. The only differing
visibility inputs are four recursive-frustum coefficients. Native warp-child
creation passes null screen bounds; canonical creation incorrectly forwards
the portal rectangle for all child kinds. The adapter now retains full warp
frusta while preserving sky/mirror clipping. Three named renderer/decal/script
contracts pass in `step4-native-warp-child-frustum-owner-contracts.log`.
All 4096 recorded admission decisions match afterward. The diagnostic report's
three complete checkpoints match, but its requested unpublished update traces
still make that diagnostic report fail; no such failure is hidden.

The standard strict `g4-native-warp-frustum-core-regression.json` separately
passes Entry and Triggers completely. AOLDemo again matches all twenty-one
reached checkpoints, with 285 unreached boundaries still failing. This
source-changed regression does not erase the earlier input/startup drift:
paired configuration enables physical mouse capture and joystick input, and
the exact input cause is not yet observed. No new permanent mock/copying test
was added for child-frame argument forwarding; actual renderer admission and
full checkpoint observations provide the behavioral regression evidence.

Quidditch's separate stream-lifetime diagnostic records the same real
53523-sample XA voice, 32768-byte chunks, three primed buffers and successful
stream creation on both owners. Native/Core destroy it 3.0010/3.0019
wall-clock seconds after creation; the authored after-Talk audio observations
occur at 2.7450/2.6608 seconds and both still report the voice playing.
`g4-quidditch-stream-lifetime-live/voice-lifetime-summary.json` records the
measurements. Debugger timing perturbs playback, so these matches are not G4
acceptance and do not erase the original three audio differences.

Audio completion reads real OpenAL AL_STOPPED while game ticks use the fixed
simulation delta. Paired configuration also enables live mouse capture and
joystick input. A reproducible verification input/clock envelope, and the
frozen native app's long-path save limit, require explicit resolution before
claiming stable full parity. No audio field is suppressed and no native
binary was rebuilt. Both temporary diagnostic probe scripts are removed.

The user subsequently approved a narrow native-oracle amendment: repair the
save-path capacity and add an explicit deterministic verification mode to both
owners. Production input/audio behavior remains unchanged. The original app is
preserved as `native-oracle-before-verification-amendment.tar.gz`; the artifact
`native-oracle-verification-amendment.json` records its hashes, authorization
and separate candidate acceptance requirements. All artifacts are under
`out/macos-arm64/Testing/HP2/canonical_game_boot_entry/Artifacts`.

Paired runs now request `HP2_DETERMINISTIC_VERIFICATION=1` and require observed
physical-input isolation and real loopback-audio activation. Against the real
pre-amendment binaries, Entry still matches all three checkpoints and both
processes exit 0, but the harness correctly rejects both ignored mode requests
with `verification.mode_not_observed`. The evidence is
`g4-reject-uninstrumented-verification-binaries.json`. This amendment does not
relax any field, hash, checkpoint, authored coverage or missing-source error;
G4 remains unaccepted.

The separately rebuilt native candidate is UUID
`20C1BB26-9E40-3D6B-9998-B50EFEC2910D`; its executable SHA-256 and the canonical
executable identity are recorded in the amendment artifact, not substituted
for the archived baseline. Verification mode is selected only by the exact
environment value `1`, cached once per process. It drops physical gameplay
input and focus-driven replay-key releases while retaining window lifecycle
and controlled engine/replay injection. Normal launches retain their original
input and real-time audio paths.

Audio verification uses the real OpenAL Soft loopback context, real decoded
buffers/source states, synchronous real stream refills and a tick-driven
audio clock. Fractional sample frames carry across ticks; a fixed scratch
buffer avoids per-frame PCM allocation. Unsupported or disabled audio fails
explicitly instead of reporting a successful deterministic run. The input
smoke therefore omits `-nosound` only when this explicit mode is requested.

`verification_audio_contracts` proves that sub-sample ticks consume an actual
queued PCM source and that advancement fails after teardown.
`verification_input_contracts` exercises production and isolated physical
events, resize and focus transitions. `verification_mouse_isolation` checks
actual cursor visibility under SDL's dummy driver; its unsupported grab and
relative-mode readbacks are not claimed as proof. The stream contract now
runs the owning `hp2-audio` tests rather than filtering the thin FFI re-export.

The first actual four-map run,
`g4-approved-deterministic-input-real-loopback-first-four.json`, observes both
activation markers and exit 0 from every owner. Entry and Triggers pass all
three checkpoints each; AOLDemo matches all 21 reached checkpoints and
Quidditch all 20, including the three former dialogue-audio differences.
Their 285/442 unreached authored boundaries remain failures.

The initial twelve-contract invocation records eleven passes and three
harness-fixture failures inside `game_test_contract`; after verified-launch
fixtures were corrected, that named contract passes separately. Canonical
replay screenshots and actual Save9 save/reload pass with the mode off and
with real loopback enabled. The native Entry replay smoke still produces no
required K=SHOT screenshots in either envelope despite exit 0 and 240
presented frames. That independent native scenario remains failed, not
suppressed or accepted as replay-delivery proof.

The completed full sweep,
`g4-all-playable-approved-deterministic-verification.json`, reports **42/88
strict passes**. Both processes exit 0 on all 86 maps whose checkpoint
sources resolve; the original native crash findings are retained, not
retroactively erased. Two maps still report missing authored disk scripts.
The summary records 878 complete matches, 82 reached framebuffer-hash
differences and 14835 cutscene failures. The reached hash defects are confined
to Adv1Willow, Arena, Ch3Diffindo, Ch6WizardCard, Duel01, Grounds_Night, CV and
Ch1Rictusempra_sound. G4 is not accepted.

`g4-deterministic-reached-repeatability.json` compares both independent runs,
both owners and all seven existing fidelity fields without normalizing any
snapshot or actor order. All 21 reached AOLDemo and 20 reached Quidditch
observations repeat exactly; their mandatory coverage failures remain failed.

Separate native-capacity acceptance uses deliberately long paths rather
than shortening the test location. The six-map run records exit 0 on every
owner; all ten actual Save0 packages parse with Engine.Level and HGame.harry
consumers. Native paths are 276–281 characters, canonical paths 277–282.
`g4-native-platform-long-save-package-consumer-proofs.json` binds those
results to the new candidate identity. This proves filesystem capacity, not
binary save equivalence or complete authored coverage. Duel02 retains all
fourteen complete reached matches with real streamed music.

A separate native controlled-replay Save9 scenario also exits 0 but writes
no required Save9 and does not reload it. Along with the failed native SHOT
scenario, this remains an explicit unresolved condition; canonical replay
success is not substituted as native replay-delivery evidence.

The next real CV diagnostic finds matching first-64 water kernel scalar,
active-drop and height-field inputs, but 280 native versus 428 canonical
calls over 300 ticks. Both textures have the same 15/30 Hz rate bounds and
128-step priming. Native `UnCamera.h` declares `CurrentTime` as `FTime`;
canonical viewport locking incorrectly stored raw double bytes there while
all texture consumers read fixed-point `FTime`. The canonical writer now uses
the existing `hp2_core::time::from_double` conversion. No native binary or
rendering algorithm was changed.

Three named renderer/Core/package contracts pass after that correction, and
the actual canonical water consumer now executes 280 calls, matching the
preserved native observation. The separate live regression retains strict
Entry and all 20 reached Quidditch matches, but CV still differs at three
frame hashes and AOLDemo now has six complete reached matches plus fifteen
frame-hash differences and 285 unreached failures. The last full 42/88 result
predates this source change; it is not reasserted as the current full score.

The user subsequently approved extending that verification-only clock to
viewport and texture-animation timestamps on both owners. The input/audio
candidate was archived first as
`native-oracle-before-visual-clock-amendment.tar.gz` (SHA-256
`d1fd6fe7ae132f47932b1edf3c7c5c07a1bce148cc06b64da6eae4ce72b48c64`);
its executable, including the archived member, hashes to
`c515168867f30acd8d44701d10e6ab07676d9aee441ce5ef4d2d355b2c350ee0`.
Production clocks remain unchanged. Paired verification now requires the
observed `visuals=tick` marker as well as real loopback and input isolation;
the preserved input/audio-only binaries correctly fail that requirement.

The viewport FTime repair also exposed two light-owner readers that still
interpreted native FTime bytes as a double. The real pulsing-actor ambient
consumer regression fails before their correction and passes afterward.
AOLDemo returns to all 21 complete reached matches; the fifteen introduced
frame-hash differences are not accepted or re-pinned as expected behavior.

Live water probes confirm equal render/wave tables and equal bitmap outputs
at equal phases. Both actual owner clocks advance through 300 identical
float deltas, totaling 5.999999865889549 seconds, but native water scheduling
still followed debugger delays. The remaining override was the SDL viewport
`StampCurrentTime` leaf, which bypassed the engine timestamp selection.
It now selects the same verification clock. ScriptedTexture local timestamps
and source-texture locks likewise select that clock on both owners.

`g4-complete-scripted-texture-tick-clock-affected.json` records all 24 owner
processes exiting 0. CV now passes all three strict checkpoints; Entry and
Triggers retain their three each. Ch6WizardCard matches all fourteen reached
checkpoints but still fails nineteen unreached authored boundaries.
AOLDemo/Quidditch retain all 21/20 reached matches and 285/442 unreached
failures. Six maps still have reached frame-hash differences; Arena has
twenty in this run, not the earlier eighteen. Real ScriptedTexture VM and
bitmap-compatibility consumers pass with the mode off and on; normal and
verification canonical replay screenshots and Save9 reload also pass.
Paired-map and replay observations use `data-prototype`; the ScriptedTexture
VM contract is `data-none`. These are not G4 acceptance. The water/table
debugger scaffold is removed; its raw observations are retained.

The subsequent complete report,
`g4-all-playable-complete-visual-clock-verification.json`, records **41/88
strict passes**, 790 complete reached matches, 170 reached frame-hash
differences across fourteen maps and 14835 cutscene failures. All 172
source-resolved owner runs exit 0. CV is a new strict pass, while
Dumbledore_Office and GreatHall_R lose their earlier strict passes. The two
missing-script source errors remain errors. The focused water recovery does
not establish full-suite improvement; G4 remains failed.

Native replay delivery was subsequently traced to the smoke fixture rather
than repaired in native replay code. The fixture seeded `HP.ini`, but native
bootstrap selects `Game.ini`; creating the absent system configuration also
refreshed `User.ini`, removing the injected command bindings. Both owners now
explicitly select the staged `Game.ini`. Its value must be quoted for the
native command-line parser because the normal user path contains spaces;
the first unquoted experiment and its failures remain in the evidence.
`input_script_contract` passes. The actual native/canonical screenshot
smokes and canonical Save9 reload pass with verification off and on.
The separate `native-quoted-active-ini-real-save-reload/report.json`
records native completed loads of Entry, Adv1Willow.unr and Save9.usa.
This resolves those exercised native command-delivery failures without
changing production configuration or replay policy. `frames_replayed`
still reports fixture length, not a measured dispatch count.

The remaining reached framebuffer hashes were then localized on
Grounds_Night (12 of 12 tick checkpoints diverged). Under minimal
read-only capture the first reached frame differs by **36 RGB pixels**
with exact alpha and per-channel deltas no larger than three. Every
observed input on that frame now compares byte-identical: all four
submitted `SceneFrame` packets, all 512 complex-surface requests
including ordered vertices, axes, flags and texture scalars. The one
real divergence found is in the decoded texture uploads: 126 of 500
uploaded RGB payloads differ, and in 124 of those the difference is
confined to a trailing column suffix (16-wide: column 15; 8-wide:
columns 6-7; 32-wide: columns 23-28,31). This is consistent with
light-map padding, but per-surface `UClamp`/`VClamp` was not captured;
the exact initialized extent and sampling contribution remain unproven.

Two independent native observations repeat the first 300 uploaded byte
digests and the same frame hash. Repeatability does not prove that all
those bytes were initialized, nor does shared lighting code alone prove
the cause of differing bytes. No padding normalization or comparison
relaxation is approved.

A temporary canonical cache trace subsequently identifies two omitted
440-byte `CID_InputMap` creates. Removing just those two native events
leaves identical ordered create sizes (1454 events); object-derived IDs
are not assumed equal. The instrumentation was removed and rebuilding
restored the exact prior canonical SHA-256
`bcc0517eab88d6a9568fef5d07eabf887ab1e9cab9240e34065bb7aa04f85323`.
The source defect is explicit: canonical input walked reflected fields
without the native class-keyed `InputVarCache`. Native `ReadInput`
acquires that item before held-key execution; button/axis name lookups
also acquire it. There is no missing script-native registration to fix.

Canonical input now builds and uses the same counted property-pointer
cache, holds its item lock across binding execution, and releases its
cache-owner borrow before synchronous reentry. The first `ReadInput`-only
correction still leaves all twelve Grounds_Night hashes different.
After button/axis lookup uses the same cache too,
`g4-canonical-input-variable-cache-affected.json` records all twelve
reached Grounds_Night checkpoints matching exactly; its 150 unreached
authored boundaries remain failures. AOLDemo/Quidditch retain 21/20 reached
matches; Entry, CV and Dumbledore_Office pass their three checkpoints.
Arena still has twelve reached hash differences, Duel01 nine, and
GreatHall_R one. This is bounded `data-prototype` evidence, not G4 acceptance.

The actual cached-input consumer covers reflected input-axis scaling,
non-input fields remaining unchanged, cache eviction/rebuild, nested lookup,
and the `delta=-1` axis-clear path. Its first fixture attempted to construct
an abstract viewport; after replacing that fixture with an existing concrete
Camera and caller-owned viewport record, `renderer_host_contracts` passes.
The named native input edge/order/release and script contracts also pass;
canonical actual replay screenshots and Save9 reload pass in verification
mode. Raw failures remain recorded, not replaced by the corrected result.

The completed full input-cache sweep,
`g4-all-playable-native-input-cache-verification.json`, reports **43/88
strict passes**, 865 complete matches and 95 reached framebuffer differences
across ten maps. All 172 owner processes exit 0. Of the remaining 45 maps,
33 fail only authored coverage, ten combine reached framebuffer differences
with missing coverage, and two cannot resolve required script assets.
All 14835 authored failures are shared: 13515 checkpoint-missing and 1320
checkpoint-not-reached on both engines; there are no one-sided missing
boundaries in this report. This does not establish the missing scenes work.
No reached non-frame fidelity mismatch is reported in this run.
`g4-current-remaining-problems.json` preserves the per-map classification.
It was derived from saved reports without launching another game process.
Next priority is actual scene routes and asset prerequisites, not treating
shared idle-run coverage failures as proven Rust runtime defects. G4 remains
failed, and exact hash/coverage requirements are unchanged.

### Exercised authored replay routes

`Tests/Fixtures/aoldemo_tutorial_route.txt` uses ordinary W/D input to reach
the map's existing CutScene9 player-bump tutorial. The saved forward-only
endpoint is `(1327.592,30.147,44.6)`; the scene's thin collision box is centered
at `(1279.932,124.038,53.261)`, so a lateral crossing is necessary. The
earlier gear scene is a different puzzle outcome: Trigger5/6/9/10 accept
`HGame.firecrabSmall`, dispatch `Gear01Dis`, and Dispatcher3 emits `Gear01CS`.
Walking Harry into those volumes cannot legitimately activate that scene.

Run the real tutorial route with all authored checkpoints retained:

```sh
python3 Build/game_test.py paired \
  --app dist/macos-arm64/HarryPotter2.app \
  --engine-bin out/macos-arm64/hp2_game_rust \
  --data-root HarryPotter2/Unreal --map Maps/AOLDemo.unr \
  --renderer wgpu --ticks 1180 --fixed-dt 0.033333335 \
  --replay-fixture Tests/Fixtures/aoldemo_tutorial_route.txt \
  --replay-url AOLDemo.unr --checkpoints authored --timeout 180 \
  --output "$HP2_ARTIFACT_DIR/aoldemo-tutorial-route.json"
```

`g4-aoldemo-real-route-consumption-verified.json` records exit 0 and passed
per-process launch checks for both owners, 43 complete matches, eight reached
frame-hash differences and 255 unreached authored observations. CutScene9
contributes thirty newly reached observations: 25 exact matches and five
hash differences. This earns route delivery, not map acceptance. The
300-tick idle sweep is a separate scenario and its score is unchanged.

The first route experiment also exposed a known stale rewrite-era replay
marker in `game_test.py`. It is replaced, not imitated: the canonical real
replay reader counts successfully decoded frame delimiters and emits
`<HP2_REPLAY>` only after observing file EOF and completing replay reseeding.
The paired harness requires the observed URL, delimiter count, exact f32
first-tick bits and seed to match the compiled scenario. Missing, partial or
mismatched observations still fail. The route observes 1163 consumed
delimiters; real Entry screenshot and Save9 reload consumers observe 127/215.
This proves decoded replay consumption, not arbitrary unseen input dispatch;
the actual scene and screenshot/save consumers supply separate delivery proof.
`game_test_contract`, `input_script_contract`, and actual canonical replay
screenshot/save-reload smokes pass. No synthetic event, checkpoint fallback,
hash tolerance or native-oracle change is introduced.

### Verified gameplay command dispatch correction

The subsequent normal-cast route exposes two owning dispatch defects, not
another pixel-only discrepancy. Canonical `UViewport::Exec` followed
`Viewport.Actor.Player` for its final fallback; native invokes the receiving
viewport's own `UPlayer::Exec`. The canonical fallback now uses that same
receiver, including when the actor's Player backlink is temporarily null.

Canonical metadata linking also built function hash tables only for `UState`,
omitting `UClass`, which inherits that behavior in the native engine.
Package-loaded inherited console commands consequently failed lookup.
`property_link.rs` now builds the same hash tables for both record kinds.
The real inherited `BehindView True` regression fails before correction
and passes afterward without manually inserting that function in a hash.
The separate viewport-receiver regression executes a real script assignment
with the actor's Player backlink null and verifies it remains null.

`step4-complete-command-dispatch-regressions-and-contracts.log` records
four passing named targets: `renderer_host_contracts`,
`property_link_contracts`, `class_metadata_contracts`, and
`console_vm_native_owner_contracts`. Early test attempts used the wrong
generated bitfield owner, and the inherited-command fixture initially
lacked XLevel; those fixture failures are retained rather than presented
as additional product defects.

The actual post-fix AOLDemo cast run exits 0, creates the required screenshot
and Save9 package, and now plays the aiming sounds. Its saved SpellCursor
location is exactly the native run's
`(1619.273681640625,483.0667724609375,97.72893524169922)`; before class-hash
linking the cursor remained at its initial position. This verifies real
command/aim delivery under `data-prototype`, not successful padlock opening:
the exercised sequence still records zero spells fired. No new blanket
sweep or pixel investigation is used as evidence for this correction.
The temporary LLDB cast observer is removed; raw observations remain.



The five named light-transform, light-manager, scene-lighting, native-text
scalar and bitmap-compatibility contracts pass. The new transpose regression
initially used inverse-only expectations for a shear basis; its expectations
were corrected from the native inverse/transpose semantics, not a product
change. The initial failure remains in
`step4-light-transform-and-canvas-compatibility-contracts.log`, and the corrected
`renderer_host_contracts` run is in
`step4-correct-native-inverse-transpose-regression-expectations.log`.

Earlier named `renderer_host_contracts` and `canonical_renderer_smoke_maps` pass
under `data-prototype`. In the same CTest command, `renderer_smoke_wgpu` fails
its presentation gate with `renderer.zero_frames_presented`: 0 presented,
300 occluded, 0 other failures. This is a blocked visible-surface proof, not a
pass or a reason to retry. Reports and logs remain under
`Testing/HP2/canonical_game_boot_entry/Artifacts`, including
`step4-named-renderer-readback-and-dynamic-bounds-acceptance.log`.

The fail-closed raw-product audit
`native-free-game-all-native-producers.json` reports `ok: true` with zero
findings for `hp2_game_rust` after concrete generated/copied-header producer
proofs, exact object-library ownership, selected Cargo unit closure, and pinned
XZ headers were integrated. Libarchive's generated configuration is captured
from its pinned root after validating the target's `libarchive/` source
directory. This audit does not constitute installed-product cutover or G7
acceptance.

Rust UCC now constructs the full `EngineOwner` without boxing an owner or
factory. The standalone Core UCC entry retains its `CoreOwner` construction.
Actual macOS arm64 `data-prototype` verification:

```sh
python3 Tests/UccSmokeTests.py --smoke --ucc-binary=out/macos-arm64/hp2_ucc_rust --data-root=HarryPotter2/Unreal
out/macos-arm64/hp2_ucc_rust Core.SimpleCommandlet -datadir="$PWD/HarryPotter2/Unreal"
out/macos-arm64/hp2_ucc_rust Core.HelloWorldCommandlet intparm=42 strparm=canonical-ucc -datadir="$PWD/HarryPotter2/Unreal"
```

All exit zero. The help smoke reports `pass` and verifies the data-root
inventory is unchanged. SimpleCommandlet reports function result 666, cast
result 3, and min result 32; HelloWorld receives intparm 42 and strparm
canonical-ucc. Logs are
`Testing/HP2/canonical_game_boot_entry/Artifacts/step6-engine-owner-ucc-*.log`
under `out/macos-arm64`. This is not retail verification or installed-product
cutover.





The independent composed movement oracle is exercised by CTest
`pawn_native_move_fixtures`. Its 576 records cover 18 path/movement entries on
32 controlled floor worlds. Regenerate a candidate (not the checked-in golden):

```sh
HP2_ARTIFACT_DIR=out/macos-arm64/Testing/HP2/pawn_native_move_oracle/Artifacts \
  sh Build/pawn_native_move_fixture_harness/gen_pawn_native_move.sh out/macos-arm64
```

The generator compares two native runs and writes `pawn_native_move-v2.txt`
under the artifact directory. It composes original C++ movement/reach/sense
at `32dcfda` with pre-port routing at `8e34caa`, without linking Rust.
The approved replacement of the obsolete mocked-leaf golden is recorded
separately in `Tests/Fixtures/pawn-native-move-oracle-acceptance.json`.


## Label taxonomy

Tests carry layer labels plus data-profile labels:

| Layer | Meaning |
| --- | --- |
| `fast` | pure in-process contracts, no data tree, no GPU |
| `integration` | loads prototype or synthetic packages through real engine code paths |
| `smoke` | launches the installed app against real maps |
| `visual` | captures frames for human comparison (planned) |
| `manual` | requires operator interaction (planned) |

Data-profile labels follow the taxonomy `data-none`, `data-prototype`
(`HarryPotter2/Unreal`), `data-retail`.

Feature gates have four states: `experimental`, `runtime-verified`,
`retail-verified`, `default-enabled`. Native text is `runtime-verified` with
`default_enabled: true` — smooth TrueType rendering is the shipped default on
the macOS arm64 / CoreText provider, evidenced under `data-prototype` only. It
is **not** `retail-verified`: no `data-retail` run of this configuration
exists, and the Linux arm64 / FreeType provider compiles but has not been run
with the default on. The machine-readable gate registry is
`Build/feature-gates.json`; per-preset capability snapshots are
`out/<preset>/hp2-state.json`.

Label application to individual tests and gate metadata registration are being
rolled out alongside this document; until they land in
`Build/CMake/HP2Targets.cmake`, filter tests by name with `-R`, not by label.

## Smoke tiers

Three renderer smoke tiers exist, all requiring the packaged app bundle at
`dist/macos-arm64/HarryPotter2.app` and prototype data at
`HarryPotter2/Unreal/System/Default.ini` at configure time. Without them the
tests are **not silently skipped — they simply do not exist**, which surfaces as
a configuration gap rather than a green run.

1. **Commit gate** — `renderer_smoke_wgpu`: launches `Entry` with the wgpu
   renderer for 300 ticks and requires at least one presented frame.
2. **Three-map smoke** — `renderer_smoke_maps`: launches `PrivetDr`, `Entry`,
   and `Ch2Skurge` for 120 ticks each with a 90-second per-map timeout, writing
   `smoke-maps.json` (`format_version` 2).
3. **Full sweep** — `renderer_smoke_full`: launches every playable map for 300
   ticks. Only registered when `HP2_ENABLE_FULL_MAP_SMOKE=ON` (the
   `macos-arm64-full-smoke` preset). It holds the `hp2_gpu` resource lock,
   serializing against other GPU consumers, and has a 4-hour timeout.

A map passes only when its process exits cleanly with no case-insensitive
script/native/package/render/assert/critical failure marker; the report records
per-map classification and pass state.

## Offline operation

All third-party dependencies are pinned by commit/hash in
`ThirdParty/sources.json`; updates are deliberate source changes, never moving
tags. SDL2, OpenAL Soft, Ogg, Vorbis, and Squish are fetched by FetchContent at
configure time into `out/<preset>/_deps`. Once a build directory has been
configured once, offline reconfiguration is supported:

```sh
cmake --preset macos-arm64 -DFETCHCONTENT_FULLY_DISCONNECTED=ON
```

The first configure of a fresh preset needs network access to populate `_deps`;
the vendored trees under `ThirdParty/` (UT469eSDK, vgmstream) are already in-tree and need no download.

## Distribution

Two artifacts, both built locally from the same bundle:

| Target | Produces | Notes |
| --- | --- | --- |
| `hp2_macos_app` | `dist/macos-arm64/HarryPotter2.app` | default target; part of `ALL` for release presets |
| `hp2_macos_dmg` | `dist/macos-arm64/HarryPotter2-<version>-macos-arm64.dmg` | the release artifact |

```sh
cmake --build out/macos-arm64 --target hp2_macos_dmg
```

The DMG contains the app, a symlink to `/Applications`, and `How to play.txt`
(a copy of `Docs/PLAYING.md`). Game-data import runs in-process in the launcher;
the bundle does not ship a separate importer or require Python on the player
machine.

The bundle ships exactly one `Contents/Frameworks/libopenal.1.dylib` — the
soname the executable resolves. It must stay a real file with no symlink
siblings: quarantine propagates onto symlinks during a DMG download or cask
install, which invalidates the code seal and makes `codesign --verify` fail on
the *installed* app.

### Homebrew

This repository is its own tap; there is no separate formula repository. The
tap name must match the repository name (`cefege/hp2`) or Homebrew will not
find `Casks/` in the clone.

```sh
brew tap cefege/hp2 https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
brew install --cask cefege/hp2/harry-potter-2
```

The cask `postflight` strips `com.apple.quarantine` from the installed app.
Without it Gatekeeper refuses to open the ad-hoc signed app
("Apple could not verify ... is free of malware") on first launch.

`version` and `sha256` in `Casks/harry-potter-2.rb` are owned by the release
workflow via `Build/stamp_cask.py`. Hand-editing them desynchronizes the tap
from the published artifact and every `brew install` fails its checksum.

### Releasing

Bump `VERSION` in `CMakeLists.txt`, then:

```sh
git tag -a v0.2.0 -m "0.2.0" && git push origin v0.2.0
```

`.github/workflows/release.yml` builds, verifies the bundle, mounts the DMG and
verifies the app's seal inside it, publishes the release, then commits the
cask checksum back to the **default branch** (never to the tag, which is the
immutable pointer at the source that built the artifact). The workflow fails
if the tag's version does not match `CMakeLists.txt`.

## Provenance bundle (placeholder)

Retail imports produce `overlay-manifest.json` (SHA-256 per file, provenance,
classification, profile metadata) through the Rust `hp2-import` crate's
`RunImport`, called directly by the standalone importer CLI and by the
in-engine launcher's `hp2-launcher::import_session::run_retail_import`.
`Build/prepare_retail_data.py` remains the reference oracle for importer parity;
see [../RETAIL_IMPORT.md](../RETAIL_IMPORT.md). A dedicated provenance-bundle
verification flow (validating a manifest bundle as a first-class artifact with
its own reason codes and artifacts) is planned and will be documented here once
it lands. Runtime enforcement of import-time manifests is landing separately;
today validation happens at import time.

## State-file reference

Two machine-readable state files exist:

- `Build/feature-gates.json` — repository-tracked feature gate registry
  (name, state from the taxonomy above, `default_enabled`, and the concrete
  evidence each promotion rests on). It is the gate policy of record: read it,
  and change it only with the matching verification recorded alongside.
- `out/<preset>/hp2-state.json` — per-preset snapshot written by
  `Build/hp2_state.py` at configure time (toolchain identity and the capability
  flags the preset actually compiled, e.g. `native_text_backend`). It describes
  a build, not a verification result.

`Build/provenance.py` assembles both into a promotion bundle
(`out/<preset>/provenance.json`) for a feature-gate promotion. Note that
`native_text_backend` in `hp2-state.json` is a *capability* flag and is
independent of the gate state in `feature-gates.json`: a preset that compiles
no text provider reports it false regardless of the gate, and the browser build
(`HP2_DISABLE_NATIVE_TEXT_BACKEND=ON`) compiles the backend in as `0`.
