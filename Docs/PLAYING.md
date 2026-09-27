# Harry Potter 2: Chamber of Secrets

A modernized runtime for *Harry Potter and the Chamber of Secrets* (2002). This
disk image contains the application; it does **not** contain the game itself.
You need your own retail copy of the game for the maps, textures, sounds, and
scripts.

## 1. Install

Drag **HarryPotter2.app** into your **Applications** folder.

If macOS shows **"HarryPotter2" Not Opened** and says it could not verify the
app is free of malware, that is Gatekeeper. This build is not signed with an
Apple Developer ID, so macOS quarantines anything downloaded and then asks you
to confirm. Either:

- right-click the app in Applications and choose **Open**, then confirm **Open**
  again (one time only); or
- run this once in Terminal:

  ```sh
  xattr -dr com.apple.quarantine /Applications/HarryPotter2.app
  ```

Homebrew users get the same one-time check: the cask clears the quarantine
attribute during install, so a successful `brew install` normally opens
straight away. If your Mac still prompts, use the right-click **Open** above.
Until the project has a Developer ID certificate and notarized releases, this
is expected on both routes.

## 2. Import your game data

The runtime reads data files only. It never launches the original installer or
the old game executable.

Double-click **Import Game Data.command** (in this disk image, or inside the
installed app). It opens a Terminal window and asks for the location of your
retail installation. You can give it either:

- the folder you installed the game into, or
- an archive (`.zip`, `.tar`, `.tar.gz`, `.7z`) of that folder.

The importer is a Python 3 script, and macOS does not install Python 3 by
default. If you see "python3 was not found", install it once:

```sh
xcode-select --install      # Apple's Command Line Tools
# or
brew install python         # Homebrew
```

Press Return to accept each prompt. The importer copies the data to:

```text
~/Library/Application Support/Harry Potter 2/Data/Retail
```

and validates it. You can also run it from Terminal at any time:

```sh
"/Applications/HarryPotter2.app/Contents/Resources/Import Game Data.command" \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy
```

The importer prints a summary and exits non-zero if anything is missing, so it
is safe to re-run.

## 3. Play

Open **HarryPotter2** from Applications. The launcher window opens with:

- **Game Data** — the imported retail folder is preselected. Use
  **Choose Folder…** if you keep your data somewhere else.
- **New Game** — start a fresh campaign.
- **Continue** — resume from a save.

Your saves, settings, and logs live in:

```text
~/Library/Application Support/Harry Potter 2/User
```

If something goes wrong, that directory holds `Launcher.log` and the engine
log with the details.

## Requirements

- macOS 15.0 or later
- Apple silicon (arm64)

## Uninstalling

Drag **HarryPotter2.app** out of Applications. Your imported game data and
saves are left alone in `~/Library/Application Support/Harry Potter 2` — that
directory is yours, and deleting it is a separate, deliberate act.

## Source, issues, and more platforms

<https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc>
