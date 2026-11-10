# Building from source

Players do not need any of this — see the [README](../README.md) to install and
play. This page is for building, testing, and hacking on the engine.

## Overview

The current desktop application is a Rust workspace for a UE1-derived engine.
Supported targets are **macOS 15+ on Apple silicon (arm64)** and **Linux arm64/x64**.
CMake imports the Rust executables through Corrosion and builds their native SDK
dependencies. The former first-party C++ engine and `hp2-ffi` static library are
not the current build graph. See [ARCHITECTURE.md](ARCHITECTURE.md) for crate owners.

- **wgpu** (`crates/hp2-wgpu`) supplies the GPU backend. The Engine runtime owner
  is `crates/hp2-engine/src/render_device_owner.rs`; the older
  `HarryPotter2/Unreal/WgpuDrv/` tree is reference material, not this Rust source root.
- **SDL2** supplies native window and input services through `crates/hp2-platform`.
- **Text providers** include SDK-linked CoreText and FreeType configurations.
  Provider compilation does not establish activation or qualification. Historical
  native-text default claims disagree with `Build/feature-gates.json`; this page
  does not resolve that disagreement or change defaults. The bitmap/page-backed
  `[Display] NativeText=False` compatibility path remains supported.

## Requirements

### System

**macOS**
- macOS 15.0 or later
- Apple silicon (arm64 / Apple M1, M2, M3, M4, M5, etc.)
- Xcode (Command Line Tools or full IDE)
- CMake 3.24+

**Linux**
- arm64 (including Raspberry Pi 4/5 with 64-bit Raspberry Pi OS) and x86_64
- CMake 3.24+, Git, `pkg-config`, and Clang
- `make` for the Linux presets
- OpenGL development files and the SDL2-probed X11, Wayland, ALSA, and
  PulseAudio development headers
- GTK3 development files for the native launcher file picker
- FreeType, HarfBuzz, and Fontconfig development files for the Linux native
  text provider. Missing provider dependencies leave the intentional bitmap
  compatibility path available.

SDL2, OpenAL Soft, Ogg, Vorbis, Squish, the importer codecs, Dear ImGui, and
nativefiledialog-extended are fetched at pinned commits and built in-tree.
Linux nativefiledialog-extended uses GTK3 (`pkg-config gtk+-3.0`).

Omarchy / Arch (Hyprland or X11):

```sh
sudo pacman -S --needed base-devel clang make cmake pkgconf git python libarchive
sudo pacman -S --needed libglvnd mesa libx11 libxext libxrandr libxcursor \
  libxfixes libxi libxss wayland libxkbcommon alsa-lib libpulse \
  freetype harfbuzz fontconfig gtk3
```

Debian / Ubuntu:

```sh
sudo apt install build-essential clang make cmake ninja-build pkg-config \
  git python3 libarchive-tools libgtk-3-dev libgl1-mesa-dev libegl1-mesa-dev \
  libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev \
  libxi-dev libxss-dev libwayland-dev libxkbcommon-dev libasound2-dev \
  libpulse-dev libfreetype-dev libharfbuzz-dev libfontconfig-dev
```

Fedora:

```sh
sudo dnf install clang make cmake ninja-build pkgconf-pkg-config git \
  python3 bsdtar gtk3-devel mesa-libGL-devel libX11-devel libXext-devel \
  libXrandr-devel libXcursor-devel libXfixes-devel libXi-devel \
  libXScrnSaver-devel wayland-devel wayland-protocols-devel \
  libxkbcommon-devel alsa-lib-devel pulseaudio-libs-devel freetype-devel \
  harfbuzz-devel fontconfig-devel
```

`bsdtar` is not used by the runtime importer: the Rust `hp2-import` crate's
`archive` feature uses linked libarchive for ZIP, TAR, and 7z extraction. The test suite still uses
`/usr/bin/bsdtar` in `verify_prototype_archive.py`; install the package above
(`libarchive-tools` on Debian/Ubuntu, `bsdtar` on Fedora) when running the full
Linux test suite.

## Build

For a verification-only build, use the explicit target after configuration:

```sh
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_verification_binaries
ctest --preset macos-arm64 --show-only=json-v1
```

Run each applicable behavioral test by its exact name, with `--no-tests=error`.
A listing or successful compilation is not behavioral qualification. Check the
configured data profile and required SDKs. Packaging and native-oracle checks have
additional provenance/path-separation requirements; do not clear a failing guard
or use a Rust executable as an independent native oracle. Consult
[AGENTS.md](../AGENTS.md) before running the general build commands below.

```sh
# macOS
cmake --preset macos-arm64
cmake --build --preset macos-arm64

# Linux arm64 / Raspberry Pi OS 64-bit
cmake --preset linux-arm64
cmake --build --preset linux-arm64

# Linux x86_64
cmake --preset linux-x64
cmake --build --preset linux-x64
```

Packaged builds:
```
dist/macos-arm64/HarryPotter2.app   # macOS
dist/linux-arm64/                   # Linux arm64 (bin/, lib/, share/)
dist/linux-x64/                     # Linux x86_64 (bin/, lib/, share/)
```

## Import game data from a checkout

Players import a retail folder or supported archive with the `hp2_import` CLI;
the game has no startup launcher window and exits with guidance when no data is
installed. No Python runtime or external archive utility is needed.

For developer workflows, build and run the native CLI:

```sh
cmake --build --preset linux-arm64 --target hp2_import
out/linux-arm64/hp2_import \
  --archive "/path/to/Harry Potter and the Chamber of Secrets.7z" \
  --output "$HOME/.local/share/harry-potter-2/Data/Retail" \
  --profile retail-only --link-mode copy
```

Use `--retail-root` instead of `--archive` for an extracted installation.
`Build/prepare_retail_data.py` remains the reference oracle used to check
byte-for-byte importer parity; it is not part of any player installation.

**For detailed import instructions, security notes, and validation steps, see [RETAIL_IMPORT.md](../RETAIL_IMPORT.md).**

## Run a build

Open the packaged build:

```sh
# macOS
open dist/macos-arm64/HarryPotter2.app

# Linux
./dist/linux-arm64/bin/hp2_game
./dist/linux-x64/bin/hp2_game
```

The game starts directly in the stock menus (`Startup.unr`). If data is missing it
prints import guidance and exits 1; pass `-datadir=PATH` to select a data root.
Use the stock New Game / Load menus and Options pages in game.

## Build Presets

| Preset | Platform | Purpose |
|--------|----------|---------|
| `macos-arm64` | macOS | Release build with optimizations |
| `macos-arm64-asan-ubsan` | macOS | AddressSanitizer + UndefinedBehaviorSanitizer |
| `macos-arm64-tsan` | macOS | ThreadSanitizer |
| `macos-arm64-full-smoke` | macOS | Full test suite with renderer validation |
| `macos-arm64-vulkan` | macOS | Release build with Vulkan driver |
| `macos-arm64-retail` | macOS | Release build against retail-only test data |
| `linux-arm64` | Linux arm64 | Release build with optimizations |
| `linux-arm64-asan-ubsan` | Linux arm64 | AddressSanitizer + UndefinedBehaviorSanitizer |
| `linux-arm64-tsan` | Linux arm64 | ThreadSanitizer |
| `linux-arm64-full-smoke` | Linux arm64 | Full test suite with renderer validation |
| `linux-arm64-retail` | Linux arm64 | Release build against retail-only test data |
| `linux-x64` | Linux x86_64 | Release build with optimizations |
| `linux-x64-asan-ubsan` | Linux x86_64 | AddressSanitizer + UndefinedBehaviorSanitizer |
| `linux-x64-retail` | Linux x86_64 | Release build against retail-only test data |

Example:
```sh
cmake --preset macos-arm64-asan-ubsan
cmake --build --preset macos-arm64-asan-ubsan
ctest --preset macos-arm64-asan-ubsan
```

## Verification & Tests

Run the full test suite:

```sh
# macOS
ctest --preset macos-arm64

# Linux
ctest --preset linux-arm64

# Linux x86_64
ctest --preset linux-x64
```

Run a single test:

```sh
ctest --preset macos-arm64 -R <test_name>
```

Commonly useful tests:
- `game_test_contract` — baseline game behavior
- `renderer_smoke_wgpu` — renderer correctness
- `package79_manifest` — data serialization

**For complete test documentation, see [BEHAVIOR_MATRIX.md](BEHAVIOR_MATRIX.md).**

## Documentation

- **[OPERATIONS.md](OPERATIONS.md)** — Authoritative build, test, and launch reference
- **[BEHAVIOR_MATRIX.md](BEHAVIOR_MATRIX.md)** — Every registered test, its data profile, class, and source
- **[../RETAIL_IMPORT.md](../RETAIL_IMPORT.md)** — Game data import security and validation
- **[../NOTICE.md](../NOTICE.md)** — Third-party software attribution
- **[../AGENTS.md](../AGENTS.md)** — Contract for AI coding agents working in this repo

## Build troubleshooting

### Build fails with "CMake not found"

Install CMake via Homebrew (macOS) or your distro package manager (Linux):
```sh
brew install cmake        # macOS
sudo apt install cmake    # Debian/Ubuntu
```

## Releasing

Bump `VERSION` in `CMakeLists.txt`, then
`git tag -a vX.Y.Z -m "X.Y.Z" && git push origin vX.Y.Z`.
`.github/workflows/release.yml` builds the DMG, publishes the GitHub release,
and stamps the checksum into `Casks/harry-potter-2.rb` on the default branch.
Never hand-edit the cask's `version` or `sha256`. Full details:
[OPERATIONS.md](OPERATIONS.md#distribution).
