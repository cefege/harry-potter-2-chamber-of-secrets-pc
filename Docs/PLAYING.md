# Harry Potter 2: Chamber of Secrets

A modernized runtime for *Harry Potter and the Chamber of Secrets* (2002). This
disk image contains the application; it does **not** contain the game itself.
You need your own retail copy of the game for the maps, textures, sounds, and
scripts.

## 1. Install

Drag **HarryPotter2.app** into your **Applications** folder.

Before opening it the first time, open **Terminal** (⌘ Space, type
*Terminal*) and paste this once:

```sh
xattr -dr com.apple.quarantine /Applications/HarryPotter2.app
```

This build isn't signed with an Apple Developer ID, so macOS blocks it until
you clear that flag. If you already saw **"HarryPotter2" Not Opened**, you can
instead go to **System Settings → Privacy & Security**, scroll down, click
**Open Anyway**, and confirm. (Right-click → Open no longer works on macOS 15.)

Installed with Homebrew? It already did this for you.

## 2. Import your game data

The runtime reads data files only. It never launches the original installer or
the old game executable.

In Terminal, paste:

```sh
"/Applications/HarryPotter2.app/Contents/Resources/Import Game Data.command"
```

It asks where your game is — drag your game folder or archive from Finder into
the Terminal window and press Return. You can give it either:

- the folder you installed the game into, or
- an archive (`.zip`, `.tar`, `.tar.gz`, `.7z`) of that folder.

If macOS asks to install **command line developer tools**, click **Install**,
wait for it to finish, then paste the command again — that's how macOS
provides the Python the importer needs.

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
