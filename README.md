# Harry Potter and the Chamber of Secrets PC: Mac/Linux/Windows/Web Native

Play the 2002 game with a modern launcher on macOS, Windows, Linux, and Raspberry Pi. A browser build is also available for compatible browsers.

**You need your own copy of the original game.** This project does not include the game's maps, sounds, textures, or other copyrighted game data. The first time you launch, the game asks you to import your installed game folder. Nothing is installed from the original CD or installer.

## macOS

1. Download the latest **macOS arm64 DMG** from [Releases](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/releases/latest).
2. Open the DMG and drag **Chamber of Secrets** into **Applications**.
3. Open **Chamber of Secrets**. If macOS blocks the app, open **System Settings → Privacy & Security**, then choose **Open Anyway**.
4. In the launcher, choose **Game Data → Import retail game…** and select the installed game folder (the folder containing `Maps`, `System`, and `Textures`). You can also choose **Import retail archive…** for a `.zip`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz`, or `.7z` file.
5. Choose **New Game** or **Continue**.

The app also installs through Homebrew:

```sh
brew tap cefege/hp2 https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
brew install --cask cefege/hp2/harry-potter-2
```

## Windows 10/11 (64-bit)

The published `v0.1.0` release currently has the macOS download only. The Windows ZIP will be added to the next release. After it appears on [Releases](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/releases/latest):

1. Download the **Windows x64 ZIP** and choose **Extract All**. Keep all extracted files together.
2. Double-click **HarryPotter2.exe**.
3. Import your installed game folder from **Game Data → Import retail game…**, or choose **Import retail archive…** for a supported archive.
4. Choose **New Game** or **Continue**.

This is a portable app; there is no installer. Your game data and saves are kept separately under your Windows user profile. The ZIP contains the runtime files it needs, but not the original game data.

## Linux

There is currently no Linux installer in Releases. Build the game once on your Linux computer. The commands below are for Debian or Ubuntu; Raspberry Pi OS is covered in the next section.

1. Open Terminal and install the required packages:

   ```sh
   sudo apt update
   sudo apt install -y build-essential clang make cmake ninja-build pkg-config git python3 \
     libarchive-tools libgtk-3-dev libgl1-mesa-dev libegl1-mesa-dev \
     libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev \
     libxi-dev libxss-dev libwayland-dev libxkbcommon-dev libasound2-dev \
     libpulse-dev libfreetype-dev libharfbuzz-dev libfontconfig-dev
   ```

2. Download the project and build it:

   ```sh
   git clone https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc.git
   cd harry-potter-2-chamber-of-secrets-pc
   ```

   For a typical Intel/AMD 64-bit PC:

   ```sh
   cmake --preset linux-x64
   cmake --build --preset linux-x64
   ./dist/linux-x64/bin/hp2_game
   ```

   For a 64-bit ARM Linux computer, use `linux-arm64` instead of `linux-x64` in each command.

3. Import your game folder in the launcher, then choose **New Game** or **Continue**.

Other Linux distributions need the equivalent development packages listed in [the build guide](Docs/BUILDING.md).

## Raspberry Pi 4 or 5

Use **64-bit Raspberry Pi OS with the desktop**. The game is not packaged as a Pi installer yet; build it on the Pi:

1. Open Terminal and install the Debian/Ubuntu packages listed in the Linux section above.
2. Download the project and build the Pi version:

   ```sh
   git clone https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc.git
   cd harry-potter-2-chamber-of-secrets-pc
   cmake --preset linux-arm64
   cmake --build --preset linux-arm64
   ./dist/linux-arm64/bin/hp2_game
   ```

3. Import your own game folder in the launcher. Choose **New Game** to play.

The first launch can take a while while the Pi builds the game. A 32-bit Raspberry Pi OS installation is not supported.

## Browser mode: play on a phone or tablet

Browser mode runs on a computer that hosts the game and sends a secure web link to your phone or tablet. **Android devices with a current Chrome or Firefox browser are the supported mobile option. iPhone and iPad browsers currently cannot run this WebAssembly build.** Check [browser support for WebAssembly Memory64](https://caniuse.com/wf-wasm-memory64) before trying another device.

You need a computer to build and host the browser version. This is not a public game website or a ready-to-download mobile app.

### 1. Build the browser version

On a Mac or Linux computer, install **Git, CMake, Ninja, and Python 3**. From the project folder, install the Emscripten SDK version used by this project and activate it in this terminal:

```sh
git clone --branch 6.0.10 --depth 1 https://github.com/emscripten-core/emsdk.git ../emsdk
../emsdk/emsdk install 6.0.10
../emsdk/emsdk activate 6.0.10
source ../emsdk/emsdk_env.sh
```

Now build the browser version:

```sh
cmake --preset web-wasm64
cmake --build --preset web-wasm64
```

### 2. Start the local web server

In the same project folder, run:

```sh
python3 Build/serve_web.py --bind 127.0.0.1 --port 8080
```

Leave that terminal open. The game is now available on the host computer at <http://127.0.0.1:8080>.

### 3. Open it on your Android phone or tablet

The browser requires HTTPS for its shared-memory feature. For a simple temporary HTTPS link, install [Cloudflare Tunnel (`cloudflared`)](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/) on the host computer. Open a second terminal and run:

```sh
cloudflared tunnel --url http://127.0.0.1:8080
```

Open the `https://…trycloudflare.com` link printed in that terminal on the phone or tablet. Keep both terminals and the host computer running while you play. The temporary link can be opened by anyone who has it, so close the tunnel when you're done. **Do not add `--data-root`** to the server command; that option is for development and exposes files from a folder on the host.

On the device, choose **Game Data** and select your extracted game-data folder when asked. The browser saves imported data and game saves in that browser's storage on that device. Archives must be extracted before browser import.

## Troubleshooting

- **The launcher says game data is missing:** select the installed game folder containing `Maps`, `System`, and `Textures`; don't select the CD drive or installer.
- **A browser says it cannot start:** make sure you opened the HTTPS tunnel link, not the host's plain `http://` address. Use a browser that supports WebAssembly Memory64, WebGL2, and cross-origin isolation.
- **Where are saves stored?** The desktop game keeps saves in your normal user data folder. Browser saves stay in the browser's storage on the device where you played.
- **Need help?** Open an [issue](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/issues) and include your operating system and the steps that caused the problem. Do not upload copyrighted game files.

## Contributions

Contributions are welcome, including AI-assisted and AI-generated contributions. Please describe what changed, disclose AI assistance, and include the checks you ran. Follow [AGENTS.md](AGENTS.md) and preserve the game's and third-party projects' licenses.

## License

The code is [MIT licensed](LICENSE). Third-party software credits are in [NOTICE.md](NOTICE.md). Original game content is not included; you must supply your own copy.
