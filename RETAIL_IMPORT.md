# Importing a retail Harry Potter 2 installation

The native runtime does not bundle the copyrighted retail game data. Use this workflow with a retail installation you own. The importer reads data files only: it never launches the old installer, game executable, DLLs, Wine, Rosetta, or a virtual machine.

## Supported input

Provide either:

- an extracted retail installation directory with `System`, `Maps`, `Textures`, `Sounds`, `Music`, and `Help` below it; or
- a ZIP, TAR-family, or 7z archive containing exactly one such installation tree.

For 7z input, the importer uses the trusted macOS `/usr/bin/bsdtar`. Archive paths and entry types are validated before extraction. Windows executables, DLLs, installers, scripts, source-control metadata, symbolic links, and disguised PE binaries are excluded or rejected.

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

In the native launcher, **Game Data** initially displays
`~/Library/Application Support/Harry Potter 2/Data/Retail` for **Retail**,
even before you import data there. Use **Choose Folder…** to assign that folder
or any other validated retail root. The launcher does not automatically select
`out/retail-data` or a `full` retail-plus-prototype output as either named
source.

**Prototype / Beta** is optional and uses the separate external location
`~/Library/Application Support/Harry Potter 2/Data/Prototype`. It is not the
repository checkout: choose that folder after placing a valid prototype/beta
tree there, or use **Choose Folder…** to assign a different validated custom
root. Choose **New Game** for a clean campaign or **Continue** for a listed
save.

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
