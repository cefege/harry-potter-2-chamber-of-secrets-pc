# Building from source

Players do not need any of this — see the [README](../README.md) to install and
play. This page is for building, testing, and hacking on the engine.

## Overview

This is a faithful recreation of the classic 2002 game engine for **macOS 15+ on Apple silicon (arm64)** and **Linux (arm64)**. It uses:

- **XOpenGL** (`ThirdParty/XOpenGLDrv`) — OpenGL renderer adapted from UE1
- **SDL2** — window management and input handling  
- **Native text rendering** — optional experimental backend for improved font fidelity (CoreText on macOS, FreeType on Linux)

The codebase is legacy-era UE1-derived C++ with modern platform support.

## Requirements

### System

**macOS**
- macOS 15.0 or later
- Apple silicon (arm64 / Apple M1, M2, M3, M4, M5, etc.)
- Xcode (Command Line Tools or full IDE)
- CMake 3.24+

**Linux**
- arm64
- CMake 3.24+, Git, `pkg-config`, and **Clang** — `linux-arm64-base` pins
  `CMAKE_C_COMPILER=clang` / `CMAKE_CXX_COMPILER=clang++`
  (`CMakePresets.json:100-101`), so a GCC-only toolchain cannot run the
  documented preset
- `make` — that preset's generator is `Unix Makefiles`
  (`CMakePresets.json:98`), so Ninja is not what drives the Linux build
- OpenGL (`find_package(OpenGL REQUIRED)`)
- The window and audio development headers SDL2 probes for: X11 (`X11`,
  `Xext`, `Xrandr`, `Xcursor`, `Xfixes`, `Xi`, `Xss`), Wayland
  (`wayland-client`, `xkbcommon`), ALSA and PulseAudio
- FreeType, HarfBuzz and Fontconfig — required for the native text backend,
  whose condition is `NOT APPLE AND HP2_FREETYPE_FOUND AND HP2_HARFBUZZ_FOUND
  AND HP2_FONTCONFIG_FOUND` (`Build/CMake/HP2Dependencies.cmake:145`).
  Without all three the backend stays off and the game falls back to the
  bitmap fonts; macOS never hits this because CoreText ships with the system.

  One caveat, which is a bug and not a requirement: when test data *is*
  present (`HP2_TEST_DATA_PRESENT`, i.e. `System/Default.ini` exists under
  `HarryPotter2/Unreal`) **and** the backend is off, `HP2Targets.cmake:876`
  calls `set_tests_properties` on `native_typography_contracts` — a test only
  registered when the backend exists (`:777`) — so configure dies with "Can
  not find test to add properties to". A fresh clone has no
  `System/Default.ini` (`.gitignore:55` excludes the whole directory), so the
  crash only reproduces on a checkout that has game data imported. Keep the
  three packages; the guard is what should change.

SDL2, OpenAL Soft, Ogg, Vorbis and Squish are **not** system packages here —
`Build/CMake/HP2Dependencies.cmake` fetches each at a pinned commit
(`ThirdParty/sources.json`) and builds it in-tree. There is no system SDL2 to
install, and the "SDL2 development headers" this page used to list were never
read by the build.

Omarchy / Arch (Hyprland or X11):

```sh
sudo pacman -S --needed base-devel clang make cmake pkgconf git python libarchive
sudo pacman -S --needed libglvnd mesa libx11 libxext libxrandr libxcursor \
  libxfixes libxi libxss wayland libxkbcommon alsa-lib libpulse \
  freetype harfbuzz fontconfig
```

Debian / Ubuntu:

```sh
sudo apt install build-essential clang make cmake ninja-build pkg-config \
  git python3 libarchive-tools libgl1-mesa-dev libegl1-mesa-dev \
  libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev \
  libxi-dev libxss-dev libwayland-dev libxkbcommon-dev libasound2-dev \
  libpulse-dev libfreetype-dev libharfbuzz-dev libfontconfig-dev
```

Fedora:

```sh
sudo dnf install clang make cmake ninja-build pkgconf-pkg-config git \
  python3 bsdtar mesa-libGL-devel libX11-devel libXext-devel \
  libXrandr-devel libXcursor-devel libXfixes-devel libXi-devel \
  libXScrnSaver-devel wayland-devel wayland-protocols-devel \
  libxkbcommon-devel alsa-lib-devel pulseaudio-libs-devel freetype-devel \
  harfbuzz-devel fontconfig-devel
```

`bsdtar` (Arch `libarchive`, Debian `libarchive-tools`, Fedora `bsdtar`) is
needed in two places, not just the test suite:

- **Importing a retail `.7z` on Linux.** `retail_root_from_archive`
  (`Build/prepare_retail_data.py:1708-1716`) dispatches `.7z` to
  `extract_7z`, which calls `system_bsdtar()` (`:1569-1582`, checking
  `/usr/bin/bsdtar`, `/bin/bsdtar`, then `$PATH`) and hard-errors if none is
  found. macOS always has it, so this only bites Linux. `.zip` and tar
  archives (`.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz2`, `.tar.xz`,
  `.txz`) do not need it — `extract_zip` (`:1518`) and `extract_tar`
  (`:1542`) use the standard library.
- **The test suite.** `Build/verify_prototype_archive.py:31` hardcodes
  `/usr/bin/bsdtar` with no `$PATH` fallback; without it
  `prototype_archive_contract` fails on Linux, and the script does not
  degrade gracefully — it returns no JSON and the test errors out.

## Build

```sh
# macOS
cmake --preset macos-arm64
cmake --build --preset macos-arm64

# Linux
cmake --preset linux-arm64
cmake --build --preset linux-arm64
```

The packaged build lands at:
```
dist/macos-arm64/HarryPotter2.app   # macOS
dist/linux-arm64/                   # Linux (bin/, lib/, share/)
```

## Import game data from a checkout

If you own a retail installation, import it using the provided script:

```sh
python3 Build/prepare_retail_data.py \
  --archive "/path/to/Harry Potter and the Chamber of Secrets.7z" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy
```

Or from an extracted installation directory:

```sh
python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy
```

**For detailed import instructions, security notes, and validation steps, see [RETAIL_IMPORT.md](../RETAIL_IMPORT.md).**

On Linux, use `--output "$HOME/.local/share/harry-potter-2/Data/Retail"`
instead (or `$XDG_DATA_HOME/harry-potter-2/Data/Retail` if you set
XDG_DATA_HOME).

## Run a build

Open the packaged build:

```sh
# macOS
open dist/macos-arm64/HarryPotter2.app

# Linux
./dist/linux-arm64/bin/hp2_game
```

In the launcher window:
- **Game Data** → select the imported retail folder (or **Choose Folder…** to browse)
- **New Game** → start a fresh campaign
- **Continue** → resume from a save

## Build Presets

| Preset | Platform | Purpose |
|--------|----------|---------|
| `macos-arm64` | macOS | Release build with optimizations |
| `macos-arm64-asan-ubsan` | macOS | AddressSanitizer + UndefinedBehaviorSanitizer |
| `macos-arm64-tsan` | macOS | ThreadSanitizer |
| `macos-arm64-full-smoke` | macOS | Full test suite with renderer validation |
| `macos-arm64-vulkan` | macOS | Release build with Vulkan driver |
| `macos-arm64-retail` | macOS | Release build against retail-only test data |
| `linux-arm64` | Linux | Release build with optimizations |
| `linux-arm64-asan-ubsan` | Linux | AddressSanitizer + UndefinedBehaviorSanitizer |
| `linux-arm64-tsan` | Linux | ThreadSanitizer |
| `linux-arm64-full-smoke` | Linux | Full test suite with renderer validation |
| `linux-arm64-retail` | Linux | Release build against retail-only test data |

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
```

Run a single test:

```sh
ctest --preset macos-arm64 -R <test_name>
```

Commonly useful tests:
- `game_test_contract` — baseline game behavior
- `renderer_smoke_xopengl` — renderer correctness
- `package79_manifest` — data serialization

**For complete test documentation, see [BEHAVIOR_MATRIX.md](BEHAVIOR_MATRIX.md).**

## Documentation

- **[OPERATIONS.md](OPERATIONS.md)** — Authoritative build, test, and launch reference
- **[BEHAVIOR_MATRIX.md](BEHAVIOR_MATRIX.md)** — Every test, its invariant, and expected behavior
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
