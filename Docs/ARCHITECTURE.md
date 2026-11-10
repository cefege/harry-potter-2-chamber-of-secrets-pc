# Architecture navigation

This is a map of the current in-place Rust port, not a completion claim. Start with [AGENTS.md](../AGENTS.md) for repository rules, [RUST_STYLE.md](RUST_STYLE.md) for port constraints, and [WORKER_CONTRACTS.md](WORKER_CONTRACTS.md) for task handbacks. Check the current source and test registration before each task; this tree can change during parallel work.

## Products and crates

`Cargo.toml` includes `crates/*`. `Build/CMake/HP2Targets.cmake` imports these products through Corrosion:

- `hp2-game` builds the `hp2_game_rust` game executable (no startup launcher UI).
- `hp2-ucc` builds the `hp2_ucc_rust` tool executable.
- `hp2-import` builds the `hp2_import` CLI with its `archive` feature, as well as a library.

The 18 workspace packages, under `crates/`, are:

| Package | Start here for |
| --- | --- |
| `hp2-abi` | Shared ABI value types |
| `hp2-audio` | Audio subsystem and OpenAL |
| `hp2-bootstrap-paths` | Data-root and overlay path policy |
| `hp2-codecs` | EA-XA, DXT1 and Vorbis codecs |
| `hp2-core` | Core objects, archives, configuration and VM |
| `hp2-engine` | Engine, world, input and runtime owners |
| `hp2-ffi-support` | FFI guard and logging support |
| `hp2-file-hash` | File SHA-256 and byte count |
| `hp2-fire` | Fire texture behavior |
| `hp2-game` | Game entry point and boot host |
| `hp2-import` | Game-data importer |
| `hp2-ini` | Importer INI generation |
| `hp2-launcher` | Boot sequence policy, settings store (no launcher UI) |
| `hp2-platform` | Window, input and platform services |
| `hp2-render` | Scene, canvas and text rendering logic |
| `hp2-ucc` | UCC tool entry point |
| `hp2-ui` | Reactive in-game UI toolkit (signals, layout, widgets, draw list); no engine dependency |
| `hp2-wgpu` | GPU renderer and driver policy |

A policy decides ordering and behavior; an owner holds or controls actual state and resource lifetimes. An adapter translates between these and a host or SDK. Names alone do not prove ownership: trace the production call path and resource teardown. In particular, `crates/hp2-platform/src/lib.rs` and `crates/hp2-wgpu/src/lib.rs` describe native SDK leaves and retained state; a policy test does not prove that a production owner was replaced. The current port still contains native interfaces and compatibility constraints; do not assume all historical shell descriptions describe the current build graph.

## Platforms and features

`CMakePresets.json` contains macOS arm64, Linux arm64 and Linux x64 presets. Configuration and platform prerequisites are in [BUILDING.md](BUILDING.md); verify the relevant preset and installed SDK. Cargo features are not interchangeable with linked runtime support:

- `hp2-engine` exposes `sdl`, `openal`, `renderer-wgpu`, `text-coretext` and `text-freetype`; `hp2-game` currently requests `sdl`, `openal`, `renderer-wgpu` and `text-coretext` in its manifest. Inspect platform conditionals and actual linkage before claiming a provider works on a platform.
- `hp2-platform` features `sdl` and `nfd` require their native libraries; `sdl-internal-tests` enables private SDL-symbol tests. `hp2-audio`'s `openal` and `hp2-render`'s `coretext` / `freetype` similarly expose SDK bindings. `hp2-import`'s `archive` needs libarchive linkage. CMake supplies configured native dependencies. A bare Cargo test does not establish SDK-backed behavior; `--all-features` is not automatically a portable linked test.
- The CI web job uses the `web-cpp-freeze` checkout, not the current Rust port (`.github/workflows/ci.yml`).

## Domain to test navigation

These are registered names in `Build/CMake/HP2Targets.cmake`, not claims that they passed. Inspect their commands and prerequisites before choosing them.

| Change area | Starting CTest names |
| --- | --- |
| Core configuration | `core_config_safety` |
| Input and world collision | `input_binding_runtime`, `engine_world_collision_contracts` |
| Audio | `audio_contracts` |
| Import | `importer_parity` |
| Canvas/text | `canvas_native_text_scalar_contracts`, `canvas_compatibility_contracts` |
| Renderer / process | `renderer_smoke_wgpu`, `game_test_contract` |

Some tests are registered only when data or other prerequisites exist. Check the configured test list and labels; absence is a configuration gap, not a pass. The test wrapper sets `HP2_ARTIFACT_DIR` and isolated environment variables. Report the data profile and exact artifact path.

## Documentation drift

The earlier Rust rewrite was removed; the present `crates/` is the in-place port. Historical references to `hp2-ffi`, `hp2rs`, or `dist/macos-arm64-rs` must not be treated as current products. `Docs/OPERATIONS.md` contains historical build and platform descriptions; the current `AGENTS.md` points at this map. The native-text gate descriptions also disagree: `Build/feature-gates.json` says the overall gate is experimental and default-disabled, while historical guidance retained in `Docs/OPERATIONS.md` describes it as runtime-verified and default-enabled. This document does not resolve that disagreement or change any default or gate policy. Check the relevant source, configuration and evidence, and report the conflict.
