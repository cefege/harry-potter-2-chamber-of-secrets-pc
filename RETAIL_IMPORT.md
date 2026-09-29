# Importing a retail Harry Potter 2 installation

The native runtime does not bundle the copyrighted retail game data. Use this workflow with a retail installation you own. The importer reads data files only: it never launches the old installer, game executable, DLLs, Wine, Rosetta, or a virtual machine.

## Easiest: the Import Game Data button

Open the app. On a first run the launcher says **No game data yet** — that is
the expected state, not an error — and offers **Import Game Data…**. Click it
and pick your retail folder or archive. The launcher runs the bundled importer,
writes to `~/Library/Application Support/Harry Potter 2/Data/Retail`, and
returns when it finishes. Re-running it is safe.

On Linux the same button appears as **Choose Folder…** / **Choose Archive…** in
the launcher, and the destination is
`$XDG_DATA_HOME/harry-potter-2/Data/Retail` (defaulting to
`~/.local/share/harry-potter-2/Data/Retail`).

The button is hidden when the installation does not bundle the importer, which
keeps a control that would always fail off the screen.

## From Terminal

The same importer ships as a double-clickable script, and is still the way to
re-run an import non-interactively:

```sh
"/Applications/HarryPotter2.app/Contents/Resources/Import Game Data.command"
```

Double-clicking that file from Finder gives the same walkthrough. It prompts
for your retail folder or archive, writes to
`~/Library/Application Support/Harry Potter 2/Data/Retail`, and prints a
summary.

The rest of this document covers the importer's flags and validation rules for
people calling it directly from a source checkout.

## Supported input

Provide either:

- an extracted retail installation directory with `System`, `Maps`, `Textures`, `Sounds`, `Music`, and `Help` below it;
- a ZIP, TAR-family, or 7z archive containing exactly one such installation tree; or
- the original Windows installer (`setup.exe`) itself.

The installer is an InstallShield self-extracting executable. It is unpacked
read-only with two host tools — `7zz` splits the self-extractor and `unshield`
reads the InstallShield cabinet inside it, which 7-Zip cannot do — and the
resulting per-component directories are merged into one game tree. Install
them with `brew install sevenzip unshield` (both are also in Debian, Fedora
and Arch repositories). If either is missing the importer says so and stops
without touching anything.

The components overlap only in one place: each language ships its own
`Default.ini` carrying that language's `Language=` line, which is what the
engine reads to decide its asset suffixes. The merge prefers the neutral
`int` file so importing does not silently pin the game to a language, and
refuses to guess if no neutral file is present.

For 7z input, the importer uses the trusted macOS `/usr/bin/bsdtar`. Archive paths and entry types are validated before extraction. Windows executables, DLLs, scripts, source-control metadata, symbolic links, and disguised PE binaries are excluded or rejected.

A valid retail-only import contains the final 42 retail maps, the complete retail package and localization set, 214 cutscene localization files, retail audio/music, textures, and Help assets. It contains no prototype fallback assets.

## Import for normal app launches

From this repository, run one of the following commands.

From an archive:

```sh
python3 Build/prepare_retail_data.py \
  --archive "/path/to/Harry Potter and the Chamber of Secrets.7z" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy
```

From the Windows installer, with no manual unpacking:

```sh
python3 Build/prepare_retail_data.py \
  --archive "/path/to/HPCoS_Demo1.exe" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy
```

From an extracted installation:

```sh
python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy
```

`copy` is the portable choice and leaves the source dump independent. `auto` may hard-link files when source and destination are on the same filesystem.

Validate the completed import without changing it:

```sh
python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Retail" \
  --profile retail-only \
  --link-mode copy \
  --check
```

For archive-based validation, use the same `--archive` argument instead of `--retail-root`.

The importer writes `overlay-manifest.json` with per-file SHA-256 hashes, provenance, classifications, and profile metadata. A malformed or incomplete retail tree fails before launch rather than silently borrowing prototype files.

## Data identity

`overlay-manifest.json` is written and fully validated at import time: every
file's SHA-256, provenance, classification, and profile metadata are recorded,
and a malformed or incomplete tree fails the import instead of producing a
silent mix of retail and prototype files. Runtime enforcement of these
manifests is landing separately; until it ships, the runtime does not yet
re-validate the manifest at launch, so treat import-time validation as the
current integrity boundary.

## Run

After a successful import, open:

```text
dist/macos-arm64/HarryPotter2.app
```

The launcher discovers the imported retail root without any further
configuration, and **New Game** becomes available. The launcher does not
automatically select `out/retail-data` or a `full` retail-plus-prototype
output as either named source.

**Choose Folder… is not a substitute for importing.** It points the launcher
at an existing data root, so a *raw* retail folder accepted that way keeps its
original `System/Default.ini` and `System/DefUser.ini`. Those still name
`D3D11Drv`/`Editor` and Windows-style paths, so the engine will not render or
resolve assets correctly from it. The importer rewrites those files for
SDL/XOpenGL/OpenAL and portable relative paths; use **Import Game Data…** (or
`prepare_retail_data.py`) to get a playable tree. Choose Folder… is for a root
that was already imported, or for a prototype tree.

**Prototype / Beta** is optional and uses the separate external location
`~/Library/Application Support/Harry Potter 2/Data/Prototype`. It is not the
repository checkout: place a valid prototype/beta tree there, or use
**Choose Folder…** to assign a different validated custom root. Choose
**New Game** for a clean campaign or **Continue** for a listed save.

## Discovery order

With no `-datadir`, the bootstrap and the launcher agree on this order, so a
root the launcher can see is also a root the engine can start from:

1. a repository checkout (`out/retail-data`, then `HarryPotter2/Unreal`);
2. the retail import destination (`Data/Retail` on macOS,
   `$XDG_DATA_HOME/harry-potter-2/Data/Retail` on Linux);
3. the historical hand-copy location (`Data/Unreal`);
4. the prototype tree (`Data/Prototype`, or a checkout's `HarryPotter2/Unreal`).

Finding none of these is not a failure: the launcher opens and offers the
import flow. Only an explicit `-datadir` that fails validation is fatal.

For an alternate validated output directory, launch explicitly:

```sh
open -n dist/macos-arm64/HarryPotter2.app --args \
  -datadir="/absolute/path/to/retail-data"
```

Writable settings, logs, cache, persistent actors, and saves are separated by
edition under:

```text
~/Library/Application Support/Harry Potter 2/User/Profiles/Retail
~/Library/Application Support/Harry Potter 2/User/Profiles/Prototype
```

The common `User/Launcher.ini` stores launcher folder assignments. Re-importing
data does not overwrite profile state. The launcher reads saves without
modifying them and commits settings only after **New Game** or **Continue** is
chosen. `-datadir=` remains a transient command-line override: it uses an
isolated `Profiles/Explicit` state and does not change saved folders.
