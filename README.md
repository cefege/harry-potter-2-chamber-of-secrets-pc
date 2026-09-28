# Harry Potter and the Chamber of Secrets — on your Mac

The 2002 PC game, running natively on Apple silicon Macs. Not an emulator, not a
wrapper — the original engine, rebuilt for today.

▶️ **[Watch it running](https://www.linkedin.com/feed/update/urn:li:activity:7498760430992113665/)**

## What you need

- A Mac with **Apple silicon** (M1 or newer) running **macOS 15 Sequoia or later**
- **Your own copy of the original game** — the folder it installed into, or a
  `.zip` / `.7z` archive of that folder. This project doesn't include the game's
  maps, sounds, or textures; they come from your copy.

That's all. No coding tools, no building.

## Step 1 — Install the app

Open **Terminal** (press ⌘ Space, type *Terminal*, press Return) and paste these
three lines:

```sh
brew tap cefege/hp2 https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
brew trust cefege/hp2
brew install --cask cefege/hp2/harry-potter-2
```

**HarryPotter2** is now in your Applications folder.

Don't have Homebrew? Install it from [brew.sh](https://brew.sh) first (one line
to paste), or use the download below.

<details>
<summary><b>No Homebrew? Download the app directly</b></summary>

1. Download **HarryPotter2-0.1.0-macos-arm64.dmg** from
   [the latest release](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/releases/latest).
2. Open it and drag **HarryPotter2** onto **Applications**.
3. In Terminal, paste this once so macOS lets the app open:

   ```sh
   xattr -dr com.apple.quarantine /Applications/HarryPotter2.app
   ```

</details>

## Step 2 — Add your game files

In Terminal, paste:

```sh
"/Applications/HarryPotter2.app/Contents/Resources/Import Game Data.command"
```

It asks two questions:

1. **Where is your game?** Drag your game folder (or `.zip` / `.7z`) from Finder
   into the Terminal window — that types its location for you — then press Return.
2. **Where should it go?** Just press Return to accept the default.

Wait for **Import complete.** It only copies files; it never runs anything from
the old game. You only do this once.

> **macOS asks to install "command line developer tools"?** Click **Install**,
> wait for it to finish, then paste the command above again. The importer needs
> Python, and that's how macOS provides it.

## Step 3 — Play

Open **HarryPotter2** from Applications. In the window that appears:

- **Game Data** already points at the files you imported
- **New Game** starts the story
- **Continue** picks up a save

## Something not working?

**"HarryPotter2" Not Opened / Apple could not verify it.** This happens if you
used the download instead of Homebrew and skipped the Terminal line in Step 1.
Either paste that line, or: try opening the app once, then go to
**System Settings → Privacy & Security**, scroll down, click **Open Anyway**,
and confirm. (The old right-click → Open trick no longer works on macOS 15.)

**The import says something is missing.** Point it at the folder the game was
*installed* into (it contains folders like `System`, `Maps`, `Textures`), not
the CD or the installer. Running the import again is safe.

**The game won't start or crashes.** The details are in
`~/Library/Application Support/Harry Potter 2/User/Launcher.log` — include that
file if you [report a problem](https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc/issues).

**Where are my saves?** In `~/Library/Application Support/Harry Potter 2/User`.
Uninstalling the app (`brew uninstall --cask harry-potter-2`, or dragging it to
the Trash) leaves your saves and game files alone.

## Linux

Linux on **arm64** works — tested on **Omarchy** (Arch-based, Hyprland), and
any other distro should work too. There's no download yet, so you build it
yourself. First the tools and libraries the build needs:

**Omarchy / Arch / EndeavourOS:**

```sh
sudo pacman -S --needed base-devel clang make cmake pkgconf git python libarchive
sudo pacman -S --needed libglvnd mesa libx11 libxext libxrandr libxcursor \
  libxfixes libxi libxss wayland libxkbcommon alsa-lib libpulse \
  freetype harfbuzz fontconfig
```

**Debian / Ubuntu:**

```sh
sudo apt install build-essential clang make cmake ninja-build pkg-config \
  git python3 libarchive-tools libgl1-mesa-dev libegl1-mesa-dev \
  libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxfixes-dev \
  libxi-dev libxss-dev libwayland-dev libxkbcommon-dev libasound2-dev \
  libpulse-dev libfreetype-dev libharfbuzz-dev libfontconfig-dev
```

`clang` is not optional: the `linux-arm64` preset pins it as the compiler, so
the GCC-only toolchain groups above won't do.

You do **not** install SDL2, OpenAL, or the Vorbis libraries yourself — the
build fetches pinned copies and compiles them. Both Wayland (Hyprland) and
X11 work; it uses whichever your session provides.

Then build, add your game files, and play:
```sh
git clone https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc
cd harry-potter-2-chamber-of-secrets-pc
cmake --preset linux-arm64 && cmake --build --preset linux-arm64

python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/your/game" \
  --output "$HOME/.local/share/harry-potter-2/Data/Retail" \
  --profile retail-only --link-mode copy

./dist/linux-arm64/bin/hp2_game
```

You'll need an **arm64** machine and Git. (Any arm64 Linux works — a
Raspberry Pi 4 or 5 running 64-bit Raspberry Pi OS counts, though expect a
long build.) Logs are in
`~/.local/share/harry-potter-2/User/Launcher.log`. More in
[Docs/BUILDING.md](Docs/BUILDING.md).

## Help it run everywhere

Windows and Intel/AMD (x86-64) machines aren't supported yet — only because
nobody with one has done the port. If you have one, you're very welcome to make
it work and open a pull request. **Using AI coding agents is encouraged**: point
yours at [AGENTS.md](AGENTS.md). No need to fork; reviews are quick and you keep
the credit.

Building and testing the engine: [Docs/BUILDING.md](Docs/BUILDING.md).

## Legal

The code is [MIT licensed](LICENSE); third-party credits are in
[NOTICE.md](NOTICE.md). The game's maps, sounds, textures, and scripts belong to
their original publishers and are not included — you need your own copy of the
original game.
