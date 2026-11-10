# Harry Potter and the Chamber of Secrets PC: Mac/Linux/Windows/Web Native

Play the 2002 PC game on a modern Mac. The engine is a complete Rust reimplementation of the
game's Unreal-based engine: no original C++ engine code remains. It renders with Metal through
`wgpu` and boots straight into the game's own menus.

**You need your own copy of the original game.** This project does not include the game's maps,
sounds, textures, or other copyrighted data. You import your installed copy once (see below).

## What is new

- **Fast.** Every retail level runs above 60 fps on Apple silicon; saving no longer pauses the game
  (the save is written in the background) and loading a save takes about half as long.
- **Rebuilt menus and HUD.** All menus and the subtitle bar are drawn by a new Rust UI with the
  original artwork: reliable clicks, real word wrapping, exact mouse-speed entry, mouse smoothing
  toggle, and every setting in the in-game **Options** pages.
- **Better image quality** (Options → Sound/Video → Advanced): mipmaps (no more shimmering
  distant textures), Edge AA for foliage and fences, smoother lighting, MSAA, anisotropic
  filtering, VSync and frame-rate limits, UI scale, and a 3D scale from 50% up to 200%
  supersampling.

## macOS (Apple silicon, macOS 15 or later)

1. Download the latest **macOS arm64 DMG** from
   [Releases](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/releases/latest),
   or install with Homebrew:

   ```sh
   brew tap cefege/hp2 https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
   brew install --cask cefege/hp2/harry-potter-2
   ```

2. Drag **HarryPotter2** into **Applications**. If macOS blocks it, open
   **System Settings → Privacy & Security** and choose **Open Anyway**.
3. **Import your game once.** Open Terminal and run (use the folder that contains `Maps`,
   `System` and `Textures`):

   ```sh
   /Applications/HarryPotter2.app/Contents/MacOS/hp2_import \
     --retail-root "/path/to/your game folder" \
     --profile retail-only \
     --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail"
   ```

   A `.zip`, `.tar.*` or `.7z` of that folder works too: use `--archive "/path/to/file.zip"`
   instead of `--retail-root`. The game finds the data in that `--output` folder. If you open
   the game before importing, it shows this command with your paths filled in.
4. Open **HarryPotter2** and play. Settings are in the game's own Options pages.

## Building from source

The shipping executables (`hp2_game_rust`, `hp2_import`, `hp2_ucc_rust`) are Rust crates built
through CMake. On macOS:

```sh
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_verification_binaries
./out/macos-arm64/hp2_game_rust
```

Prerequisites and the Linux presets (`linux-x64`, `linux-arm64`) are described in
[Docs/BUILDING.md](Docs/BUILDING.md). This release was built and tested on macOS only; the Linux
presets exist but were not re-verified for it. The older browser build lives on the frozen
`web-cpp-freeze` branch and is not part of this release.

## Troubleshooting

- **The game says to import your game:** run the `hp2_import` command above with the folder that
  contains `Maps`, `System` and `Textures` (not the CD drive or the installer).
- **Where are saves?** `~/Library/Application Support/Harry Potter 2/User/Save`.
- **Need the original look?** `HP2_STOCK_MENUS=1` and `HP2_STOCK_HUD=1` bring back the original
  menu and HUD drawing; the Advanced options can turn each image-quality feature off.
- **Need help?** Open an [issue](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/issues)
  with your macOS version and the steps that caused the problem. Do not upload game files.

## Contributions

Contributions are welcome, including AI-assisted ones. Describe what changed, disclose AI
assistance, include the checks you ran, and follow [AGENTS.md](AGENTS.md).

## License

The code is [MIT licensed](LICENSE). Third-party software credits are in [NOTICE.md](NOTICE.md).
Original game content is not included; you must supply your own copy.
