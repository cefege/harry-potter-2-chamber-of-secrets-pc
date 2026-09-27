# Harry Potter 2: Chamber of Secrets

A modernized C++ runtime for *Harry Potter and the Chamber of Secrets*, originally developed for Unreal Engine 1.

## Overview

This is a faithful recreation of the classic 2002 game engine for **macOS 15+ on Apple silicon (arm64)** and **Linux (arm64)**. It uses:

- **XOpenGL** (`ThirdParty/XOpenGLDrv`) — OpenGL renderer adapted from UE1
- **SDL2** — window management and input handling  
- **Native text rendering** — optional experimental backend for improved font fidelity (CoreText on macOS, FreeType on Linux)

The codebase is legacy-era UE1-derived C++ with modern platform support.

## See It In Action

Watch gameplay in action on LinkedIn: [https://www.linkedin.com/feed/update/urn:li:activity:7498760430992113665/](https://www.linkedin.com/feed/update/urn:li:activity:7498760430992113665/)

## Play it on macOS

You do not need the source code. Two routes, both ending in the same app:

**Disk image (smallest download)**

Grab the `.dmg` from
[Releases](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/releases),
drag `HarryPotter2.app` into Applications, then double-click
`Import Game Data.command` in the image and point it at your retail copy.

**Homebrew**

```sh
brew tap cefege/hp2 https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
brew trust cefege/hp2
brew install --cask cefege/hp2/harry-potter-2
```

The `brew trust` line answers the one-time prompt Homebrew shows for any
third-party tap. This repository *is* the tap — the URL is required, because
Homebrew would otherwise look for a separate `homebrew-hp2` repository. Note
that tapping clones this repo (~330 MB) to read the cask; you never have to
build from it, and the disk image above fetches only the app.

The importer needs Python 3, which macOS does not install by default. If it
says `python3 was not found`, run `xcode-select --install` once.

> **macOS may ask before the first launch.** This build is not signed with an
> Apple Developer ID, so Gatekeeper quarantines downloaded apps. Right-click
> the app in Applications → **Open** → **Open** again. One time, and expected
> on both routes until releases are signed and notarized.

Full instructions: [Docs/PLAYING.md](Docs/PLAYING.md), which ships in the
disk image.

## Requirements

### System

**macOS**
- macOS 15.0 or later
- Apple silicon (arm64 / Apple M1, M2, M3, M4, M5, etc.)
- Xcode (Command Line Tools or full IDE)
- CMake 3.24+

**Linux**
- arm64
- Ninja, GCC or Clang, SDL2 development headers
- CMake 3.24+

The toolchain (Xcode, Ninja, CMake) is only needed to build from source.
Installing a prebuilt app needs none of it.

### Game Data
**You must own a retail copy of *Harry Potter and the Chamber of Secrets* for Windows or macOS.** This repository does not include game data; you will import it from your own installation.

## Quick Start

### 1. Build from Source

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

### 2. Import Game Data

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

**For detailed import instructions, security notes, and validation steps, see [RETAIL_IMPORT.md](RETAIL_IMPORT.md).**

### 3. Run

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

**For complete test documentation, see [Docs/BEHAVIOR_MATRIX.md](Docs/BEHAVIOR_MATRIX.md).**

## Documentation

- **[Docs/OPERATIONS.md](Docs/OPERATIONS.md)** — Authoritative build, test, and launch reference
- **[Docs/BEHAVIOR_MATRIX.md](Docs/BEHAVIOR_MATRIX.md)** — Every test, its invariant, and expected behavior
- **[RETAIL_IMPORT.md](RETAIL_IMPORT.md)** — Game data import security and validation
- **[NOTICE.md](NOTICE.md)** — Third-party software attribution

## License

This source code is provided under the license in [LICENSE](LICENSE).

**Game assets** (maps, textures, sounds, scripts) remain copyrighted by their original publishers. You must own a legal copy of the original retail game to use them.

## Troubleshooting

### "Game Data not found"

Ensure you've imported your retail installation to the path shown in the launcher's **Game Data** settings. The import script prints the exact destination; verify the folder exists and contains `System/`, `Maps/`, `Textures/`, etc.

### Build fails with "CMake not found"

Install CMake via Homebrew (macOS) or your distro package manager (Linux):
```sh
brew install cmake        # macOS
sudo apt install cmake    # Debian/Ubuntu
```

### App crashes on launch

**macOS**: check `~/Library/Application Support/Harry Potter 2/User/Launcher.log`.
**Linux**: check `~/.local/share/Harry Potter 2/User/Launcher.log`.

## Contributing

Contributions are welcome, and PRs get merged fast. Two specific asks:

**Bring it to a platform nobody has tested.** macOS arm64 and Linux arm64 are
covered. **Windows is not**, and there is no x86-64 (Intel/AMD) build for
either platform. If you have a machine the project does not and want Harry
Potter 2 running on it, do the work and open a PR. This game is small and old
enough that it should run on almost anything with patience.

**Use your AI coding agents.** This repo is set up for them and that is
encouraged, not a shortcut. `AGENTS.md` and `Docs/OPERATIONS.md` are the
contract for agents in this codebase: subsystem map, verification discipline,
exact commands. Point your agent at them.

**No need to fork.** Commit to a branch and open a PR. Reviews are quick and
you keep the credit.

Bug reports and compatibility improvements are also welcome — please include
reproduction steps and platform details.

---

**Enjoy the game!** 🧙‍♂️
