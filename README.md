# Harry Potter 2 (modernized)

UE1-derived *Harry Potter and the Chamber of Secrets* runtime, modernized for
macOS 15 on Apple silicon (arm64). The renderer runs through XOpenGL
(`ThirdParty/XOpenGLDrv`), input/windowing through SDL2, with native text
rendering as an experimental opt-in backend.

The repository does not bundle retail game data. Prototype/beta data lives at
`HarryPotter2/Unreal`; importing a retail installation you own is documented in
[RETAIL_IMPORT.md](RETAIL_IMPORT.md).

## Quick start

```sh
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_verification_binaries
ctest --preset macos-arm64
```

Presets: `macos-arm64`, `macos-arm64-asan-ubsan`, `macos-arm64-tsan`,
`macos-arm64-full-smoke`. See [Docs/OPERATIONS.md](Docs/OPERATIONS.md) for the
authoritative command reference, artifact layout, smoke tiers, offline builds,
and label taxonomy.

## Verification

Every registered test, its invariant, data profile, and artifacts are listed in
[Docs/BEHAVIOR_MATRIX.md](Docs/BEHAVIOR_MATRIX.md). Renderer smoke tiers require
the packaged app (`dist/macos-arm64/HarryPotter2.app`) plus prototype data; a
missing bundle is a configuration gap, not a skipped test.
