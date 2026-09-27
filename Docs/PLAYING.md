# Harry Potter 2: Chamber of Secrets

A modernized runtime for *Harry Potter and the Chamber of Secrets* (2002). This
disk image contains the application; it does **not** contain the game itself.
You need your own retail copy of the game for the maps, textures, sounds, and
scripts.

## 1. Install

Drag **HarryPotter2.app** into your **Applications** folder.

If macOS refuses to open it the first time, macOS is blocking an app that was
not signed by an Apple Developer account. Either:

- right-click the app in Applications and choose **Open**, then confirm **Open**
  again; or
- run this once in Terminal:

  ```sh
  xattr -dr com.apple.quarantine /Applications/HarryPotter2.app
  ```

Installing with Homebrew (`brew install --cask cefege/hp2/harry-potter-2`)
already handles this for you.

## 2. Import your game data

The runtime reads data files only. It never launches the original installer or
the old game executable.

Double-click **Import Game Data.command**. It opens a Terminal window and asks
for the location of your retail installation. You can give it either:

- the folder you installed the game into, or
- an archive (`.zip`, `.tar`, `.tar.gz`, `.7z`) of that folder.

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
