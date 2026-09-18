# Harry Potter 2: Chamber of Secrets

A modernized C++ runtime for *Harry Potter and the Chamber of Secrets*, originally developed for Unreal Engine 1.

## Overview

This is a faithful recreation of the classic 2002 game engine for macOS 15+ on Apple silicon (arm64). It uses:

- **XOpenGL** (`ThirdParty/XOpenGLDrv`) — OpenGL renderer adapted from UE1
- **SDL2** — window management and input handling  
- **OpenAL Soft** — audio mixing, streaming Ogg music, and EA-XA decoding

The codebase is legacy-era UE1-derived C++ with modern platform support.

## See It In Action

Watch gameplay in action on LinkedIn: [https://www.linkedin.com/feed/update/urn:li:activity:7498760430992113665/](https://www.linkedin.com/feed/update/urn:li:activity:7498760430992113665/)

## Requirements

### System
- **macOS 15.0 or later**
- **Apple silicon** (arm64 / Apple M1, M2, M3, M4, M5, etc.)
- **Xcode** (Command Line Tools or full IDE)
- **CMake 3.24+**

### Game Data
**You must own a retail copy of *Harry Potter and the Chamber of Secrets* for Windows or macOS.** This repository does not include game data; you will import it from your own installation.

## Quick Start

### 1. Build from Source

```sh
# Configure
cmake --preset macos-arm64

# Build
cmake --build --preset macos-arm64
```

The packaged app lands at:
```
dist/macos-arm64/HarryPotter2.app
```

### 2. Import Game Data

If you own a retail installation, import it using the provided script:

```sh
python3 Build/prepare_retail_data.py \
  --archive "/path/to/Harry Potter and the Chamber of Secrets.7z" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal" \
  --profile retail-only \
  --link-mode copy
```

Or from an extracted installation directory:

```sh
python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal" \
  --profile retail-only \
  --link-mode copy
```

**For detailed import instructions, security notes, and validation steps, see [RETAIL_IMPORT.md](RETAIL_IMPORT.md).**

### 3. Run

Open the packaged app:

```sh
open dist/macos-arm64/HarryPotter2.app
```

In the launcher window:
- **Game Data** → select the imported retail folder (or **Choose Folder…** to browse)
- **New Game** → start a fresh campaign
- **Continue** → resume from a save

## Build Presets

| Preset | Purpose |
|--------|---------|
| `macos-arm64` | Release build with optimizations |
| `macos-arm64-asan-ubsan` | AddressSanitizer + UndefinedBehaviorSanitizer |
| `macos-arm64-tsan` | ThreadSanitizer |

Example:
```sh
cmake --preset macos-arm64-asan-ubsan
cmake --build --preset macos-arm64-asan-ubsan
ctest --preset macos-arm64-asan-ubsan
```

## Verification & Tests

Run the full test suite:

```sh
ctest --preset macos-arm64
```

Run a single test:

```sh
ctest --preset macos-arm64 -R <test_name>
```

The suite has 14 tests; `ctest --preset macos-arm64 -N` lists them. Nine run
without game data; `native_registration`, `package79_manifest`,
`spell_interaction_manifest`, `spell_runtime_contracts`, and `audio_lifecycle`
read imported retail packages and fail until step 2 is complete.

Commonly useful tests:
- `game_test_contract` — baseline game behavior
- `abi_widths` — engine struct layout and property offsets
- `package79_manifest` — data serialization
- `native_launcher_contract` — launcher state, data-root selection, and save discovery

## Documentation

- **[RETAIL_IMPORT.md](RETAIL_IMPORT.md)** — Game data import security and validation
- **[NOTICE.md](NOTICE.md)** — Third-party software attribution

## License

This source code is provided under the license in [LICENSE](LICENSE).

**Game assets** (maps, textures, sounds, scripts) remain copyrighted by their original publishers. You must own a legal copy of the original retail game to use them.

## Troubleshooting

### "Game Data not found"

Ensure you've imported your retail installation to the path shown in the launcher's **Game Data** settings. The import script prints the exact destination; verify the folder exists and contains `System/`, `Maps/`, `Textures/`, etc.

### Build fails with "CMake not found"

Install CMake via Homebrew:
```sh
brew install cmake
```

### App crashes on launch

Check `~/Library/Application Support/Harry Potter 2/User/Launcher.log` for detailed error messages.

## Contributing

This repository is a historical preservation and modernization effort. Bug reports and compatibility improvements are welcome; please file an issue with reproduction steps and platform details.

---

**Enjoy the game!** 🧙‍♂️
