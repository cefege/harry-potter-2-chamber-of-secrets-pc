# Behavior matrix

One row per registered CTest test, as of the current
`Build/CMake/HP2Targets.cmake`. Run environment (isolated `HOME`/`TMPDIR`,
`LC_ALL=C`, `TZ=UTC`, `HP2_TEST_NAME`, `HP2_ARTIFACT_DIR`, 900 s default
timeout) is documented in [OPERATIONS.md](OPERATIONS.md). Every test owns
`out/<preset>/Testing/HP2/<test>/{Home,Tmp,Artifacts}`; "Artifacts" below lists
what lands there beyond that baseline. Data profiles: `data-none`,
`data-prototype` (`HarryPotter2/Unreal`), `data-retail`.

## Registered tests

| Test | Invariant | Command | Data profile | Artifacts |
| --- | --- | --- | --- | --- |
| `abi_widths` | Engine integer/pointer type widths match the arm64 contract (1/2/4-byte ints, 8-byte pointers). | `ctest --preset macos-arm64 -R '^abi_widths$'` | data-none | — |
| `render_clip` | Render clip-edge classification is stable across signed-zero and epsilon boundaries. | `ctest --preset macos-arm64 -R '^render_clip$'` | data-none | — |
| `projection_fov` | Effective FOV preserves the authored FOV at 4:3 and handles widescreen scaling without drift. | `ctest --preset macos-arm64 -R '^projection_fov$'` | data-none | — |
| `command_line_load` | Command-line map/load parsing rejects invalid `-SAVESLOT=` / `-LOAD=` forms and accepts only well-formed input. | `ctest --preset macos-arm64 -R '^command_line_load$'` | data-none | — |
| `compact_index` | Compact index serialization round-trips boundary values exactly. | `ctest --preset macos-arm64 -R '^compact_index$'` | data-none | — |
| `fstring_archive` | `FString` archive serialization round-trips across wide/ANSI transitions. | `ctest --preset macos-arm64 -R '^fstring_archive$'` | data-none | — |
| `native_registration` | Native class/package registration succeeds against the prototype data tree. | `ctest --preset macos-arm64 -R '^native_registration$'` | data-prototype | — |
| `package79_manifest` | Data-root package audit matches the machine-local golden `<binary-dir>/goldens/package79-reference.json`. | `cmake --build --preset macos-arm64 --target regenerate_goldens && ctest --preset macos-arm64 -R '^package79_manifest$'` | data-prototype | audit report in stdout; mismatch details on failure |
| `spell_interaction_manifest` | `Build/spell_interaction_audit.py --check` output equals the machine-local golden `<binary-dir>/goldens/spell-interactions.json`. | `cmake --build --preset macos-arm64 --target regenerate_goldens && ctest --preset macos-arm64 -R '^spell_interaction_manifest$'` | data-prototype | — |
| `spell_runtime_contracts` | Stock Core/Engine/HGame packages load through real engine paths within boundary conditions. | `ctest --preset macos-arm64 -R '^spell_runtime_contracts$'` | data-prototype | — |
| `dxt1_codec` | DXT1 encode/decode (Squish) stays within tolerance for reference blocks. | `ctest --preset macos-arm64 -R '^dxt1_codec$'` | data-none | — |
| `eaxa_decoder` | EAXA decoder produces exact expected output for fixture buffers (minimal-TU build: decoder unit + Core headers only). | `ctest --preset macos-arm64 -R '^eaxa_decoder$'` | data-none | — |
| `audio_lifecycle` | OpenAL device/context lifecycle (open, play, stop, shutdown) completes cleanly and deterministically. | `ctest --preset macos-arm64 -R '^audio_lifecycle$'` | data-none | — |
| `native_launcher_contract` | Launcher store/path/policy contracts hold under `HP2_LAUNCHER_TESTING=1`. | `ctest --preset macos-arm64 -R '^native_launcher_contract$'` | data-none | — |
| `game_test_contract` | `Build/game_test.py` contract behaviors against a synthetic data root; never launches the app. | `ctest --preset macos-arm64 -R '^game_test_contract$'` | data-none | — |
| `repair_save_contract` | `Build/repair_save.py` save-repair behaviors on synthetic saves; never launches the app. | `ctest --preset macos-arm64 -R '^repair_save_contract$'` | data-none | — |
| `native_typography_contracts` | `FNativeTextPlatformBackend` metrics/shaping contracts (registered only when the native text backend exists; Apple-only today). | `ctest --preset macos-arm64 -R '^native_typography_contracts$'` | data-none | — |
| `canvas_compatibility_contracts` | `FCanvasTextRequest` compatibility with the native text backend (registered only when the native text backend exists). | `ctest --preset macos-arm64 -R '^canvas_compatibility_contracts$'` | data-none | — |
| `renderer_smoke_xopengl` | Commit-gate smoke: XOpenGL renderer launches maps `PrivetDr`, `Entry`, `Ch2Skurge` for 120 ticks each with no failure marker; report `format_version` 2. Requires `dist/macos-arm64/HarryPotter2.app` + prototype data at configure time. | `ctest --preset macos-arm64 -R '^renderer_smoke_xopengl$'` | data-prototype | `smoke-maps.json` + per-map logs under sibling `smoke-maps-logs/` |
| `renderer_smoke_full` | Full sweep: every playable map launches for 300 ticks with no failure marker. Only registered when `HP2_ENABLE_FULL_MAP_SMOKE=ON`; holds `hp2_gpu` resource lock; 4-hour timeout. | `ctest --preset macos-arm64-full-smoke -R '^renderer_smoke_full$'` | data-prototype | `smoke-maps.json` + `smoke-maps-logs/` |

Dependency ordering enforced by CTest `DEPENDS`: `compact_index`,
`fstring_archive`, `native_registration` after `abi_widths`;
`package79_manifest` after `compact_index`, `fstring_archive`,
`native_registration`; `spell_runtime_contracts` after `native_registration`,
`package79_manifest`, `spell_interaction_manifest`; `dxt1_codec`,
`eaxa_decoder` after `abi_widths`; `audio_lifecycle` after `eaxa_decoder`,
`native_registration`.

## Planned tiers (placeholders — not registered yet)

These rows describe verification tiers that are specified but have no CTest
entry today. They carry no command because inventing one would be wrong; each
will gain a row above when its test registers.

| Planned tier | Invariant it will defend | Status |
| --- | --- | --- |
| Scripted-input replay contract | Deterministic input sequences (serialized `FReplay::FInputEvent` bytes) drive `CauseInputEvent` → `UEngine::InputEvent` with reproducible state transitions. | planned |
| Visual capture tier | Captured frames of known scenes match approved references within a stated tolerance; requires GPU and human-approved baselines. | planned |
| Vulkan renderer smoke tier | Same smoke gate as the XOpenGL tiers against the vendored VulkanDrv path; end-of-present frame capture (`Hp2FrameCapture`, graded ScreenshotPipeline path) writes frame PNGs + `frame_meta.json` on both legs, verified faithful against the live window. Cross-driver last-frame compare at fixed ticks is currently NOISE: the Ch2Skurge/PrivetDr intro cutscene advances non-deterministically (same tick 120, different camera/lighting state across runs of the same binary — console FlyTo LOCATION cues differ), so warm/cool frame deltas measure cutscene state, not renderer shading. Upstream lightmap channel swap in `TextureUploader_BGRA8_LM` verified correct (reverted an attempted change); matrix needs deterministic framing (post-intro state or state-hash gating) before tolerance compare is meaningful; `Tools/matrix_compare.py --maps=PrivetDr,Entry,Ch2Skurge --ticks=120 --tolerance=8`. | capture live; matrix gated on cutscene determinism |
| UCC smoke | `hp2_ucc` executes a scripted commandlet end-to-end against real data. | planned |
| Save-format contracts | Save read/write round-trip across editions with byte-level layout checks. | planned |
| Launcher-config contracts | `User/Launcher.ini` folder assignment and profile-state persistence rules. | planned |
| Config-INI contracts | Generated `Default.ini`/`DefUser.ini` content matches the importer's profile contract. | planned |
| Crash diagnostics | Structured failure reports with reason codes for launch-time crashes. | planned |

## Rust-era gate map (working copy during hp2rs migration)

Master checklist for the clean-room Rust engine (`crates/`, binary
`hp2rs`). Gate definitions live in the rewrite plan; oracle disposition is
recorded in
[ORACLE_BASELINE.md](ORACLE_BASELINE.md). Status updated at each phase gate.

| Gate | Rust-side proof | Ports these ctest contracts | Status |
| --- | --- | --- | --- |
| G0 | `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --check` + `cargo test --workspace`; C++ suite re-run identical (now 41/41 after the loader fix, see ORACLE_BASELINE.md addendum) | (none — scaffold) | DONE 2026-08-24 |
| G1 format parity | `cargo test -p hp-format -p hp-ini` (63 tests); `diff` of `Build/package79_reference.py --data-root` audit vs `cargo run -p hp-format --example p79audit` → EMPTY (21,888,652 bytes both sides); whole-root round-trip 214/214 packages incl. v61 heritage-gap | `compact_index`, `fstring_archive`, `eaxa_decoder`, `dxt1_codec` (decode side), audit half of `package79_manifest` | DONE 2026-08-24 |
| G2 object runtime | `cargo test -p hp-uobject`: three stock packages bind zero unbound classes/natives (`data-prototype`); double-run snapshot-hash equality | `native_registration`, `spell_runtime_contracts`, determinism intent of `determinism_double_run` | OPEN |
| G3 headless sim | `game_test.py run PrivetDr/Entry/Ch2Skurge --engine-bin target/release/hp2rs --ticks 120` all `"passed": true`, zero failure markers | launch half of `input_script_contract`/`input_script_smoke`; replaces C++ determinism tier via seeded RNG (own acceptance record) | OPEN |
| G4 renderer | Trio WITH real GPU rendering passed ×3 balanced; determinism double-capture sha256 identical; structural tests 40+; user-approved side-by-side (captures opened for review 2026-08-25) | `renderer_smoke_xopengl`, `projection_fov`, `render_clip`, canvas metric cases | DONE 2026-08-25 |
| G5 audio/saves/launcher | save-fixture script over all `Tests/Fixtures/save-format-golden/` entries; `repair_save.py --engine target/release/hp2rs` contract; `HP2_LAUNCHER_TESTING=1 cargo test -p hp-app`; audio lifecycle suite | `repair_save_contract`, `audio_lifecycle`, `native_launcher_contract`, `command_line_load`, replay round-trip cases | OPEN |
| G6 product | fresh clone `cargo test --workspace`; packaged app opens; retail flow completes; `python3 Build/check_bundle_rs.py` exit 0 | full-suite parity row: C++ 41/41 + Rust 253/0 at the same commit (2026-08-25) | OPEN |
