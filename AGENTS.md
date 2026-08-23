# AGENTS.md — Operating Handbook for AI Agents

Rules of engagement for autonomous agents working in this repository. The
canonical operational reference is `Docs/OPERATIONS.md`; this handbook defines
behavior and boundaries, not procedure detail.

## Ownership / Subsystem Map

| Subsystem | Path | Notes |
| --- | --- | --- |
| Core | `HarryPotter2/Unreal/Core/` | Object model, serialization (`FName`/`FString`, package79 archives), memory, names |
| Engine | `HarryPotter2/Unreal/Engine/` | Game loop, `UEngine::InputEvent` dispatch, level flow |
| Render | `HarryPotter2/Unreal/Render/` | Scene rendering shared by driver backends |
| XOpenGLDrv | `ThirdParty/XOpenGLDrv/` | GL driver + text seam (`FCanvasTextRequest`, `FNativeTextPlatformBackend`). Pinned upstream; see ThirdParty rules below |
| SDLDrv | `HarryPotter2/Unreal/SDLLaunch/` | SDL2 window/input (`USDLViewport` → `CauseInputEvent`), launch policy/store, `HP2MacLauncher.mm` |
| Launcher | `HarryPotter2/Unreal/Launch/` | Bootstrap paths, static packages, editor runtime entry |
| ALAudio / codecs | `HarryPotter2/Unreal/ALAudio/`, `EAAudioCodec/`, `Vorbis/`, `OpenAL/` | Sound pipeline |
| Build scripts | `Build/*.py` | `game_test.py`, `smoke_maps.py`, `check_bundle.py`, `prepare_retail_data.py`, `repair_save.py`, `abi_inventory.py`; CMake modules in `Build/CMake/` |
| Tests | `Tests/` | C++ `*Tests.cpp` (`main()`-style, minimal TU), Python `*Tests.py` (`unittest`, no app launch), fixtures in `Tests/Fixtures/` |
| Data roots | `HarryPotter2/Unreal/` (prototype), retail overlay via `Build/prepare_retail_data.py` | Prototype root is `HP2_UNREAL_ROOT` in `Build/CMake/HP2Sources.cmake` |
| Dist bundle | `dist/macos-arm64/HarryPotter2.app` | Installed app; verify with `Build/check_bundle.py` |

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
   the matching data profile, recorded in the acceptance evidence. Native text
   is `experimental`/opt-in today; do not flip gates in drive-by changes.
5. **Bitmap-font compatibility mode is intentional.** Do not "fix" fallback
   glyph rendering to use native text paths; the compatibility mode exists for
   stock-font fidelity.
6. **MACOSX macro caution.** Apple platform code uses UE1-era `MACOSX` guards
   (not `__APPLE__` alone). New platform conditionals must match surrounding
   convention or `TCHAR`/UTF-32 assumptions silently diverge.

## Command Cheatsheet

Full procedure, presets, and interpretation: `Docs/OPERATIONS.md`. Quick map:

| Task | Command |
| --- | --- |
| Configure + build | `cmake --preset macos-arm64 && cmake --build --preset macos-arm64` |
| Run verification suite | `ctest --preset macos-arm64` (sanitizer variants: `macos-arm64-asan-ubsan`, `macos-arm64-tsan`) |
| Single test | `ctest --preset macos-arm64 -R <name>` — e.g. `abi_widths`, `package79_manifest`, `game_test_contract`, `renderer_smoke_xopengl` |
| List maps | `python3 Build/game_test.py maps --data-root HarryPotter2/Unreal` |
| Launch one map | `python3 Build/game_test.py run <map> ...` (see `Docs/OPERATIONS.md`) |
| Renderer smoke | `python3 Build/smoke_maps.py --renderer=xopengl --output=<report.json> [--maps=...]` |
| Bundle integrity | `python3 Build/check_bundle.py` (exit 0 pass / 1 fail / 2 blocked) |
| Retail data overlay | `python3 Build/prepare_retail_data.py` → writes `overlay-manifest.json` |

Test registration: C++/script tests are registered in
`Build/CMake/HP2Targets.cmake` via `hp2_add_behavior_test` (isolated HOME +
`HP2_ARTIFACT_DIR` per test). Renderer smoke requires the installed bundle and
prototype data at configure time; without them the tests do not exist — that
is a configuration gap, not a skip.

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
