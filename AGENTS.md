# AGENTS.md — Operating handbook

This is a Rust application workspace for a UE1-derived engine (Harry Potter and the
Chamber of Secrets, PC). **All first-party code is Rust**: the former C++ engine, its
`hp2-ffi` static library and the egui launcher are gone; do not recreate them from
historical guidance. CMake imports the shipping Rust executables through Corrosion; C/C++
remain only in pinned third-party dependencies (SDL2, OpenAL Soft, libarchive, fonts).

## Start with the smallest relevant context

1. Read this handbook, then [Docs/ARCHITECTURE.md](Docs/ARCHITECTURE.md) to find the
   owning crate, production call path, features, and starting tests.
2. For Rust, read [.agents/skills/rust-best-practices/SKILL.md](.agents/skills/rust-best-practices/SKILL.md)
   and [Docs/RUST_STYLE.md](Docs/RUST_STYLE.md). Also use
   [rust-review](.agents/skills/rust-review/SKILL.md) for review and
   [rust-port](.agents/skills/rust-port/SKILL.md) for behavior-preserving migration.
3. Use [Docs/WORKER_CONTRACTS.md](Docs/WORKER_CONTRACTS.md) for bounded tasks and
   handbacks. Do not load all historical documentation for a small change.
4. Use [Docs/BUILDING.md](Docs/BUILDING.md) for prerequisites and
   `CMakePresets.json` for actual platform presets. `Docs/OPERATIONS.md` contains
   useful procedures **and historical build descriptions**; check commands against
   the current manifests, presets, and configured CTest list before using them.

The BeautifulRust skills are pinned, MIT-licensed local copies; their provenance
and update rules are in [.agents/skills/README.md](.agents/skills/README.md).
Repository compatibility rules take precedence over generic style guidance.

## Ownership and products

| Area | Current owner / entry point |
| --- | --- |
| Core, objects, archives, configuration, VM | `crates/hp2-core/` |
| Game loop, world, input, runtime resource owners | `crates/hp2-engine/` |
| Scene, canvas, native/bitmap text | `crates/hp2-render/` |
| Reactive UI toolkit (signals, flex layout, widgets, draw list) | `crates/hp2-ui/` |
| Menus and HUD on the toolkit (`ui_pages`, `ui_mirror`, `ui_hud`, `ui_host`) | `crates/hp2-engine/src/ui_*.rs` |
| GPU device, renderer, uploads, mipmaps, quality features | `crates/hp2-wgpu/` |
| SDL window/input and native file dialogs | `crates/hp2-platform/` |
| Audio and codecs | `crates/hp2-audio/`, `crates/hp2-codecs/` |
| Boot sequence policy / game process (no launcher UI) | `crates/hp2-launcher/`, `crates/hp2-game/` |
| Data import and generated INI policy | `crates/hp2-import/`, `crates/hp2-ini/` |
| ABI, FFI support, paths, hashing, fire, UCC | See the complete 18-crate map in `Docs/ARCHITECTURE.md` |
| Build, test, packaging, release | `Build/`, `Build/CMake/`, `Tests/`, `.github/workflows/` |
| Prototype and legacy reference assets/scripts | `HarryPotter2/Unreal/`; not the current Rust source root |

Shipping binaries are `hp2_game_rust`, `hp2_ucc_rust`, and `hp2_import`.

**Product direction.** No external/startup launcher: the game boots straight into the
stock `Startup.unr` menus. Missing data exits 1 with `hp2_import` guidance; from the
macOS app bundle the game instead welcomes the player, lets them pick their game folder
or archive (Finder chooser) and runs the bundled `hp2_import` (`Contents/MacOS`;
`--profile retail-only --output <data dir>`; its CLI is a pinned contract) before
continuing (`crates/hp2-game/src/first_run.rs`, `HP2_FIRST_RUN_SOURCE` for automated
checks). Settings belong in the in-game Options pages. Every menu page and the message
box are drawn and driven by `hp2-ui`: Sound/Video has a hand-written view (`ui_pages`);
every other page is mirrored (`ui_mirror`: stock controls stay alive and in place, their
own `Paint`/pointer handlers are swallowed by script pre-call hooks, actions run through
the controls' own script functions). The HUD subtitle bar, letterbox and status counters
are Rust (`ui_hud`); the health bar and the minigame HUD managers are still stock.
`HP2_STOCK_MENUS=1` / `HP2_STOCK_HUD=1` restore the stock drawing. See
[Docs/UI_REWRITE.md](Docs/UI_REWRITE.md) and
[Docs/STOCK_MENU_LAYOUT.md](Docs/STOCK_MENU_LAYOUT.md). Do not restore the launcher
window; preserve reusable boot/settings/import policy.

**Runtime behaviour to preserve.** Saving forks a copy-on-write child that writes the slot
while the game keeps running (`game_save_owner`; `HP2_SYNC_SAVE=1` forces the blocking path;
loads, travel and shutdown wait for it). The level-replacing fade is 25% of
`Console.FadeoutTime` (`HP2_LOAD_FADE_SCALE`). Image-quality features (`Mipmaps`,
`AlphaToCoverage`, `SmoothLightmaps` in `[WgpuDrv.WgpuRenderDevice]`, render scale 50-200%)
default on. Performance and quality evidence is in [Docs/PERFORMANCE.md](Docs/PERFORMANCE.md).

Generated ABI declarations and pinned third-party sources are not style-cleanup
candidates. Inspect a policy's real caller and resource owner before claiming a
production cutover.

## Platforms and data profiles

- macOS 15+ on arm64; Linux on arm64 and x64. Use the existing matching preset.
- Cargo features that expose SDL, OpenAL, native dialogs, text providers, or
  libarchive need the relevant SDK and linked build. Do not use unrestricted
  `--all-features` as proof of a supported configuration.
- `data-none` has no game data. `data-prototype` uses the configured prototype
  root, normally `HarryPotter2/Unreal`. `data-retail` uses a prepared overlay.
- A result for one platform, feature set, or data profile proves nothing about
  another. Retail evidence must explain data and index provenance.
  `hp2-bootstrap-paths` verifies the overlay manifest/checksums when either passes
  its readable-file check. It accepts legacy roots when neither does. Bootstrap
  acceptance alone does not establish a verified retail profile.

## Non-negotiables

1. **Preserve concurrent work.** Inspect status and relevant diffs before editing.
   Do not reset, stash, overwrite, commit, or format somebody else's changes.
   Stop on overlapping ownership rather than silently expanding a task.
2. **Preserve contracts.** Keep ABI layouts/packing/discriminants, TCHAR/FString
   encoding, package/save/network bytes, callback order and synchronous reentry,
   allocation ownership, error/partial side effects, RNG consumption, numerical
   behavior, and teardown order. A cleaner API is not authority to change them.
3. **Separate change classes.** Structural moves, behavior fixes, formatting,
   dependency/toolchain updates, and optimizations need distinct evidence. Do not
   introduce traits, builders, crates, or generality solely for a style checklist.
4. **Never claim done from compilation.** Run the named behavioral CTest targets
   for the changed domain. Use exact selections with `--no-tests=error`. Check
   each required name, not merely a combined regular expression's nonzero count.
   Missing or disabled required tests are blocked, never passes.
5. **Keep evidence under `HP2_ARTIFACT_DIR`.** CTest supplies isolated `HOME`,
   `TMPDIR`, XDG directories, `LC_ALL=C`, `TZ=UTC`, and `HP2_TEST_NAME`. Standalone
   checks must manage their own isolation and artifact directory. Retain commands,
   results, logs, input/profile provenance, and executable identities.
6. **Do not retry to green.** Report the first failure and any flakiness. A rerun
   needs a specific investigated change or a distinct measurement purpose; never
   replace the failure evidence with a later success.
7. **Do not suppress symptoms.** No swallowed errors, skipped assertions,
   input-specific special cases, broad warning suppression, or weakened gates to
   make a check pass. Distinguish pre-existing lint/format debt from new findings.
8. **Feature promotion needs acceptance.** Preserve defaults and gate states.
   Historical native-text prose and `Build/feature-gates.json` disagree; report
   the disagreement instead of changing runtime policy to match a document.
   `experimental` → `runtime-verified` → `retail-verified` → `default-enabled`
   requires matching profile evidence and a separate acceptance record.
9. **Bitmap text compatibility is intentional.** Keep `[Display] NativeText=False`
   and its page-backed Canvas/cursor-advance behavior supported. Use the current
   `canvas_compatibility_contracts` and relevant process tests, not removed C++
   test paths. Do not route compatibility glyphs through native text as a fix.
10. **Protect reference evidence.** Do not overwrite the independent native oracle
    with a packaged Rust product or call Rust-versus-Rust a parity result. Verify
    provenance and identity before packaging or parity runs. A configured path
    named `HP2_CPP_ORACLE_APP` does not prove that its contents are independent.
11. **Goldens need separate acceptance.** Fixture/golden refreshes require their
    own command, observed result, and acceptance record. Never hide a golden
    change inside a refactor or optimization.
12. **Keep pins and licenses.** `ThirdParty/sources.json`, exact Cargo dependency
    pins, and the toolchain change only in explicit, separable tasks. State the
    upstream version and reason. Do not casually edit pinned upstream sources.
13. **Platform representation matters.** Rust uses the actual `target_os` and
    feature conditions. Historical Apple C++ uses `MACOSX`, not `__APPLE__` alone;
    keep that distinction when comparing original layouts and UTF-32 assumptions.
    Do not conflate Apple and Linux/POSIX behavior.
14. **Measure performance.** File size, idiom, and clean lints do not prove speed.
    Record equivalent inputs, builds, features, repeated measurements, noise,
    allocations/latency where relevant, and behavioral checks. Keep runtime and
    development-loop performance claims separate.

## Verification commands

```sh
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_verification_binaries
ctest --preset macos-arm64 --show-only=json-v1
ctest --preset macos-arm64 --no-tests=error --output-on-failure -R '^core_config_safety$'
```

Use `linux-arm64` or `linux-x64` on a matching Linux host. Sanitizer and full-map
presets are separate qualifications; a preset name does not prove that Rust was
instrumented. Inspect the actual compiler/link flags and reports.

UI, HUD and rendering changes need pictures, not just tests: drive the game with
`Tests/InputScriptSmoke.py --smoke` fixtures, capture frames with `HP2_CAPTURE_FRAMES` /
`HP2_CAPTURE_FRAME_INDICES`, compare against `HP2_STOCK_MENUS=1` / `HP2_STOCK_HUD=1` or the
previous build pixel by pixel, and look at the images. `HP2_LAYOUT_DUMP=1` and
`HP2_UI_DEBUG=1` log page geometry and UI input. Performance claims use the uncapped
harness (`Build/perf_baseline.py`, `Build/saveload_bench.py`, `HP2_FRAME_TIMING=1`) plus
`Build/rng_equiv.py` / `Build/frame_equiv.py` for equivalence, and the 42-map retail sweep
(`Build/smoke_maps.py`) before a release.

Tests are registered through `hp2_add_behavior_test` in
`Build/CMake/HP2Targets.cmake` and `CMakeLists.txt`. Rust tests also live in crate
`tests/` directories and inline modules. Python contracts live in `Tests/`.
Conditional data-backed tests must be present before qualification. See
`Docs/ARCHITECTURE.md` for domain entry points and the configured commands for
actual execution scope. In particular, `wgpu_surface_round_trip` currently runs
non-ignored library tests; its name is not proof of real surface presentation.

## Distribution and historical tools

- macOS bundle: `dist/macos-arm64/HarryPotter2.app`; use `Build/check_bundle.py`.
  Linux layouts are `dist/linux-arm64/` and `dist/linux-x64/`, installed through
  `Build/CMake/HP2Install.cmake` and checked with their matching test presets.
- `hp2_macos_dmg` creates release DMGs. Never publish, tag, or release as a cleanup
  side effect. A release is an explicit owner request: bump `VERSION` in `CMakeLists.txt`,
  push tag `vX.Y.Z` (must match) and `.github/workflows/release.yml` builds ALL + the DMG,
  smokes it, publishes the release and stamps the cask. Locally, packaging is blocked while
  `HP2_CPP_ORACLE_APP` points at `dist/macos-arm64`; verify the CI build in a separate
  `git worktree` (no oracle configured) instead of overriding the guard. Do not modify
  personal saves or game-data installations.
- The Homebrew tap is this repository. Its tap name must match the repository
  name, and its URL must be explicit. `.github/workflows/release.yml` and
  `Build/stamp_cask.py` own `Casks/harry-potter-2.rb` version/hash fields; never
  hand-edit them.
- Use explicit current `--app` paths with legacy flow/replay tools until their
  defaults are repaired and tested. `dist/macos-arm64-rs/`, `hp2rs`, `hp-app`,
  `hp-engine`, and older `target/release/` instructions can refer to the removed
  rewrite. `Docs/BEHAVIOR_MATRIX.md`, `Docs/OPERATIONS.md`, and completion logs
  retain historical evidence; do not silently rewrite or delete it. When a task
  touches a stale path or tool, flag the drift and avoid unrelated cross-cutting
  repairs; make any necessary repair explicit in the task and evidence.
- The CI web job checks out `web-cpp-freeze`, a separate frozen C++ implementation.
  Its success does not qualify the current Rust desktop tree or a Rust web port.

## Required handback

Report **invariant**, **changed paths**, **exact commands**, **observed results**,
**artifact paths**, and **unresolved conditions**. Include the platform, features,
and data profile. Mark planned → implementing → review → integrated → verified
honestly; blocked work is not complete. Do not commit without the repository
checklist and explicit task authority.
