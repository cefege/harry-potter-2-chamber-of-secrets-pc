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

Open **HarryPotter2**. On a first run the launcher asks you to **Choose your
game folder**: pick the installed folder, or click **I have a .zip or .7z of the
game instead** for an archive of it:

- `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz2`, `.tar.xz`, `.txz`,
  or `.7z`.

The importer copies the data to:

```text
~/Library/Application Support/Harry Potter 2/Data/Retail
```

and validates it. A progress bar and cancel control stay in the launcher while
the import runs; when it completes, **Start playing** becomes available.

The importer runs in-process in the launcher. It does not require Python or
another external importer. Folder and supported archive imports use the same
validation and can be run again safely.

## 3. Play

Open **HarryPotter2** from Applications. Once the game files are installed the
launcher leads with one big button: **Start playing** on a fresh install, or
**Continue** (with the save's name and date) once you have a save; **New game**
sits below it. **Settings** holds sound, controls and graphics; **Game files**
re-imports the game, and its **Advanced** section selects Retail or Prototype
data (**Choose prototype folder…** assigns an external validated prototype tree).

Your saves, settings, and logs live in:

```text
~/Library/Application Support/Harry Potter 2/User
```

If something goes wrong, that directory holds `Launcher.log` and the engine
log with the details.

In **Graphics**, **Native text** selects the smooth CoreText renderer. The
original 2002 bitmap-font path remains available for stock-font fidelity;
turn **Native text** off, or set `NativeText=False` in the `[Display]` section
of `Game.ini` in the user directory above.

## Requirements

- macOS 15.0 or later
- Apple silicon (arm64)

## Uninstalling

Drag **HarryPotter2.app** out of Applications. Your imported game data and
saves are left alone in `~/Library/Application Support/Harry Potter 2` — that
directory is yours, and deleting it is a separate, deliberate act.

## Source, issues, and more platforms

<https://github.com/cefege/harry-potter-2-chamber-of-secrets-pc>
