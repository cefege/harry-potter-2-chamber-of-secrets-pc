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
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal" \
  --profile retail-only \
  --link-mode copy
```

From an extracted installation:

```sh
python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal" \
  --profile retail-only \
  --link-mode copy
```

`copy` is the portable choice and leaves the source dump independent. `auto` may hard-link files when source and destination are on the same filesystem.

Validate the completed import without changing it:

```sh
python3 Build/prepare_retail_data.py \
  --retail-root "/path/to/Harry Potter and the Chamber of Secrets" \
  --output "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal" \
  --profile retail-only \
  --link-mode copy \
  --check
```

For archive-based validation, use the same `--archive` argument instead of `--retail-root`.

The importer writes `overlay-manifest.json` with per-file SHA-256 hashes, provenance, classifications, and profile metadata. A malformed or incomplete retail tree fails before launch rather than silently borrowing prototype files.

## Run

After a successful import, open:

```text
dist/macos-arm64/HarryPotter2.app
```

A normal Finder double-click discovers the data at `~/Library/Application Support/Harry Potter 2/Data/Unreal` and opens the native launcher. Choose **New Game** for a clean retail campaign or **Continue** for a listed save.

For an alternate validated output directory, launch explicitly:

```sh
open -n dist/macos-arm64/HarryPotter2.app --args \
  -datadir="/absolute/path/to/retail-data"
```

Writable settings, logs, cache, persistent actors, and saves remain separate under `~/Library/Application Support/Harry Potter 2/User`. Re-importing data does not overwrite that user directory. The launcher reads saves without modifying them and commits settings only after **New Game** or **Continue** is chosen.
