# AGENTS.md — Operating Handbook for AI Agents

Rules of engagement for autonomous agents working in this repository. The
canonical operational reference is `Docs/OPERATIONS.md`; this handbook defines
behavior and boundaries, not procedure detail.

This is a single-era C++ codebase (legacy UE1-derived engine, modernized).
There is no Rust code in this tree — an earlier effort to rewrite the engine
in Rust (`crates/`, `cargo`) was removed prior to public release. A handful of
scripts and doc sections still reference that removed code (see "Known Stale
References" below); treat those as dead, not as a second live subsystem.

## Ownership / Subsystem Map

|Subsystem|Path|Notes|
|---|---|---|
|Core|`HarryPotter2/Unreal/Core/`|Object model, serialization (`FName`/`FString`, package79 archives), memory, names, portable SHA-256|
|Engine|`HarryPotter2/Unreal/Engine/`|Game loop, `UEngine::InputEvent` dispatch, level flow|
|Render|`HarryPotter2/Unreal/Render/`|Scene rendering shared by driver backends|
|XOpenGLDrv|`ThirdParty/XOpenGLDrv/`|GL driver + text seam (`FCanvasTextRequest`, native text backends: CoreText on macOS, FreeType on Linux). Pinned upstream; see ThirdParty rules below|
|SDLDrv|`HarryPotter2/Unreal/SDLDrv/`|SDL2 window/input (`USDLViewport` → `CauseInputEvent`)|
|SDLLaunch|`HarryPotter2/Unreal/SDLLaunch/`|Launch policy/store, desktop shell integration (`HP2ShellLauncher`), `HP2MacLauncher.mm`|
|Launcher|`HarryPotter2/Unreal/Launch/`|Bootstrap paths, static packages, editor runtime entry, platform path resolution (`HP2Paths.cpp`)|
|ALAudio / codecs|`HarryPotter2/Unreal/ALAudio/`, `EAAudioCodec/`, `Vorbis/`, `OpenAL/`|Sound pipeline|
|Launcher shell UI|`Launcher/Quickshell/`|Quickshell-based desktop launcher scaffold|
|Build scripts|`Build/*.py`|`game_test.py`, `smoke_maps.py`, `check_bundle.py`, `prepare_retail_data.py`, `repair_save.py`, `abi_inventory.py`, `retail_flow_smoke.py`; CMake modules in `Build/CMake/`|
|Tests|`Tests/`|C++ `*Tests.cpp` (`main()`-style, minimal TU), Python `*Tests.py` (`unittest`, no app launch), fixtures in `Tests/Fixtures/`. Registered via `hp2_add_behavior_test` in `Build/CMake/HP2Targets.cmake`|
|Data roots|`HarryPotter2/Unreal/` (prototype), retail overlay via `Build/prepare_retail_data.py`|Prototype root is `HP2_UNREAL_ROOT` in `Build/CMake/HP2Sources.cmake`|
|Dist bundle (macOS)|`dist/macos-arm64/HarryPotter2.app`|Verify with `Build/check_bundle.py`|
|Dist bundle (Linux)|`dist/linux-arm64/` (`bin/`, `lib/`, `share/`)|Installed by the `linux-arm64*` presets via `Build/CMake/HP2Install.cmake`|

## Platforms

Two supported platforms, both arm64, built from the same CMake tree:

- **macOS 15+** — presets `macos-arm64`, `macos-arm64-asan-ubsan`,
  `macos-arm64-tsan`, `macos-arm64-full-smoke`, `macos-arm64-vulkan`,
  `macos-arm64-retail`.
- **Linux (arm64)** — presets `linux-arm64`, `linux-arm64-asan-ubsan`,
  `linux-arm64-tsan`, `linux-arm64-full-smoke`, `linux-arm64-retail`.

`Docs/OPERATIONS.md` predates the Linux presets and still says "macOS 15+ on
arm64 only" in places — the presets in `CMakePresets.json` are the source of
truth for what actually exists, not that sentence.

## Known Stale References

Left over from the removed Rust rewrite; do not treat as live, do not extend:

- `Docs/OPERATIONS.md` "Cargo era (hp2rs)" section — describes a `crates/`
  workspace that no longer exists in this tree.
- `Build/package_rust_app.py`, `Build/check_bundle_rs.py`,
  `Tests/RustBundleTests.py` — reference `dist/macos-arm64-rs/` and a
  `target/release/hp2rs` binary that are never produced by this repo's build.
- Anything mentioning `cargo`, `crates/`, or `hp2rs` outside this section.

If a task touches these paths, flag the drift to the user rather than quietly
"fixing" cross-cutting doc/script debt as a side effect of an unrelated
change.

## Non-Negotiables

1. **Never claim done from compile alone.** A green build proves nothing about
   behavior. Run the named ctest targets that cover the change and report the
   observed result.
2. **Artifacts go to `HP2_ARTIFACT_DIR`.** Test env provides isolated
   `HOME`/`TMPDIR`, `LC_ALL=C`, `TZ=UTC`, `HP2_TEST_NAME`, and a pre-created
   `HP2_ARTIFACT_DIR`. Structured reports, logs, captures land there — never
   in ad-hoc stdout or the repo root.
3. **Data-profile honesty.** State which profile a claim holds for:
   `data-none` (no game data), `data-prototype` (`HarryPotter2/Unreal`),
   `data-retail` (overlay from `prepare_retail_data.py`). A result under one
   profile is not evidence under another. The runtime does not yet validate
   `overlay-manifest.json`; retail claims must say how data was verified.
4. **Feature-gate promotion is earned.** `experimental` → `runtime-verified` →
   `retail-verified` → `default-enabled` requires passing verification under
   the matching data profile, recorded in the acceptance evidence (see
   `Build/feature-gates.json`). Native text is `experimental`/opt-in today;
   do not flip gates in drive-by changes.
5. **Bitmap-font compatibility mode is intentional.** Do not "fix" fallback
   glyph rendering to use native text paths; the compatibility mode exists for
   stock-font fidelity.
6. **MACOSX macro caution.** Apple platform code uses UE1-era `MACOSX` guards
   (not `__APPLE__` alone). New platform conditionals must match surrounding
   convention or `TCHAR`/UTF-32 assumptions silently diverge. Linux code paths
   use standard `__linux__`/POSIX guards — do not conflate the two.

## Command Cheatsheet

Full procedure, presets, and interpretation: `Docs/OPERATIONS.md` (CMake/CTest
sections are current; ignore the "Cargo era" section per above). Quick map:

|Task|Command|
|---|---|
|Configure + build (macOS)|`cmake --preset macos-arm64 && cmake --build --preset macos-arm64 --target hp2_verification_binaries`|
|Configure + build (Linux)|`cmake --preset linux-arm64 && cmake --build --preset linux-arm64 --target hp2_verification_binaries`|
|Run verification suite|`ctest --preset macos-arm64` / `ctest --preset linux-arm64` (sanitizer variants: `*-asan-ubsan`, `*-tsan`)|
|Single test|`ctest --preset <preset> -R '^<name>$'` — e.g. `abi_widths`, `package79_manifest`, `game_test_contract`, `renderer_smoke_xopengl` (see `Docs/BEHAVIOR_MATRIX.md` for the full list)|
|List maps|`python3 Build/game_test.py maps --data-root HarryPotter2/Unreal`|
|Launch one map|`python3 Build/game_test.py run <map> ...` (see `Docs/OPERATIONS.md`)|
|Renderer smoke|`python3 Build/smoke_maps.py --renderer=xopengl --output=<report.json> [--maps=...]`|
|Bundle integrity|`python3 Build/check_bundle.py` (macOS `.app`; exit 0 pass / 1 fail / 2 blocked). Linux install is verified by the `linux-arm64*` ctest presets, not a separate script|
|Retail data overlay|`python3 Build/prepare_retail_data.py` → writes `overlay-manifest.json`|
|Retail-flow smoke|`python3 Build/retail_flow_smoke.py` (windowed retail launch + continue-flag step; manual steps printed at end)|

Test registration: tests are registered in `Build/CMake/HP2Targets.cmake` via
`hp2_add_behavior_test` (isolated HOME + `HP2_ARTIFACT_DIR` per test).
Renderer smoke requires the installed bundle and prototype data at configure
time; without them the tests do not exist — that is a configuration gap, not
a skip.

## PR / Change Checklist

Every change ships with, in the PR description:

1. **Invariant** — the one-sentence behavior that must hold.
2. **Command** — the exact ctest/script invocation run.
3. **Observed result** — actual pass/fail output, artifact paths under
   `HP2_ARTIFACT_DIR`; not a summary of what you expected.
4. **Unresolved conditions** — anything not covered (data profile not
   exercised, gate not promoted, platform not verified). Stated explicitly,
   never implied.

## Forbidden Actions

- **Golden updates without a separate acceptance record.** Refreshing
  `Tests/Fixtures/*` goldens requires its own acceptance evidence (command +
  observed result) — never bundle a golden bump silently into a feature commit.
- **Retry-to-green.** Re-running a failing test until it passes is evidence
  tampering. A flaky result is a finding; report it.
- **Touching ThirdParty pinned sources casually.** `ThirdParty/` is pinned
  upstream (`sources.json`). Changes need an explicit reason, a note of the
  upstream version, and must be separable from unrelated work.
- **Committing without the checklist above** (Lead responsibility).
- **Suppressing symptoms** — no special-casing inputs, swallowing errors, or
  skipping assertions to make a run pass.
