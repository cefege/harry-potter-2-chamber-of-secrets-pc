# Operations: build, test, and verification

This is the single source of truth for configuring, building, and verifying HP2.
Every command below matches a preset in `CMakePresets.json`, a target in
`Build/CMake/HP2Targets.cmake`, or a registered CTest test name. Nothing here is
invented; if a knob is not listed, it does not exist yet.

Platform: macOS 15+ on arm64 only. Requirements: CMake >= 3.24, Ninja, and a
Python 3 interpreter (found via `find_package(Python3)`).

## Configure/build/test presets

| Preset | Build type | Sanitizers | Full map smoke | Binary directory |
| --- | --- | --- | --- | --- |
| `macos-arm64` | Release | off | off (`HP2_ENABLE_FULL_MAP_SMOKE=OFF`) | `out/macos-arm64` |
| `macos-arm64-asan-ubsan` | RelWithDebInfo | ASan + UBSan (`HP2_ENABLE_ASAN_UBSAN=ON`) | off | `out/macos-arm64-asan-ubsan` |
| `macos-arm64-tsan` | RelWithDebInfo | TSan (`HP2_ENABLE_TSAN=ON`) | off | `out/macos-arm64-tsan` |
| `macos-arm64-full-smoke` | Release | off | on (`HP2_ENABLE_FULL_MAP_SMOKE=ON`) | `out/macos-arm64-full-smoke` |

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
out/<preset>/Testing/HP2/<test>/Tmp/          # isolated TMPDIR for the run
out/<preset>/Testing/HP2/<test>/Artifacts/    # structured reports, logs, captures (HP2_ARTIFACT_DIR)
```

The renderer smoke scripts additionally write their JSON report next to those
directories: `out/<preset>/Testing/HP2/renderer_smoke_xopengl/smoke-maps.json`
plus per-map engine logs under the sibling `smoke-maps-logs/` directory.

CTest failure output is captured by the presets (`outputOnFailure`); a preset
with zero matching tests is an error (`noTestsAction=error`), never a silent
pass.

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
`retail-verified`, `default-enabled`. Native text rendering is experimental /
opt-in today; the machine-readable gate state file
(`Build/feature-gates.json`) and per-preset state snapshot
(`out/<preset>/hp2-state.json`) are landing — see the placeholder section below.

Label application to individual tests and gate metadata registration are being
rolled out alongside this document; until they land in
`Build/CMake/HP2Targets.cmake`, filter tests by name with `-R`, not by label.

## Smoke tiers

Two renderer smoke tiers exist, both requiring the packaged app bundle at
`dist/macos-arm64/HarryPotter2.app` and prototype data at
`HarryPotter2/Unreal/System/Default.ini` at configure time. Without them the
tests are **not silently skipped — they simply do not exist**, which surfaces as
a configuration gap rather than a green run.

1. **Commit gate** — `renderer_smoke_xopengl`: launches the XOpenGL renderer on
   a fixed three-map subset (`PrivetDr`, `Entry`, `Ch2Skurge`) for 120 ticks
   each with a 90-second per-map timeout, writing `smoke-maps.json`
   (`format_version` 2). Registered whenever the app bundle and prototype data
   exist, so it runs in every preset including the sanitizer ones.
2. **Full sweep** — `renderer_smoke_full`: launches every playable map for 300
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
the vendored trees under `ThirdParty/` (UT469eSDK, XOpenGLDrv, UT99VulkanDrv,
vgmstream) are already in-tree and need no download.

## Provenance bundle (placeholder)

Retail imports produce `overlay-manifest.json` (SHA-256 per file, provenance,
classification, profile metadata) via `Build/prepare_retail_data.py`; see
[../RETAIL_IMPORT.md](../RETAIL_IMPORT.md). A dedicated provenance-bundle
verification flow (validating a manifest bundle as a first-class artifact with
its own reason codes and artifacts) is planned and will be documented here once
it lands. Runtime enforcement of import-time manifests is landing separately;
today validation happens at import time.

## State-file reference (placeholder)

Two machine-readable state files are planned:

- `Build/feature-gates.json` — repository-tracked feature gate registry
  (name, state from the taxonomy above, owning tests).
- `out/<preset>/hp2-state.json` — per-preset snapshot emitted by verification
  runs (which gates were exercised, which tests ran).

Neither file exists yet; this section will become the normative reference when
they land. Do not parse or hand-edit either path before then.
