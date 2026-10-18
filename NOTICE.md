# Notices

This project's own code is licensed under the MIT license (`LICENSE`). The
runtime ships zero game data: players import retail media they own via
`Build/prepare_retail_data.py` (`RETAIL_IMPORT.md`). The repository tree
additionally carries prototype reference assets (maps, packages, music) that
serve internal development and testing only; any publication export excludes
them.
Third-party components are vendored or fetched exactly as recorded in
`ThirdParty/sources.json` and keep their own licenses. They are not covered by
the MIT license above.

## Vendored in-tree

| Component | License | Upstream | Notes |
| --- | --- | --- | --- |
| `ThirdParty/vgmstream` | ISC | vgmstream/vgmstream | Includes EA-XA decoder coefficient data; see its attribution notice |

## Fetched at configure time (FetchContent, hash-pinned)

| Component | License | Upstream | Notes |
| --- | --- | --- | --- |
| SDL2 | zlib | libsdl-org/SDL | Window/input backend for the Linux and macOS clients |
| OpenAL Soft | LGPL-2.1-or-later | kcat/openal-soft | Also carries BSD-3-Clause and PFFFT notices; used by ALAudio |
| libogg | Xiph BSD-3-Clause-style | xiph/ogg | |
| libvorbis | Xiph BSD-3-Clause-style | xiph/vorbis | |
| libSquish | MIT | svn2github/libsquish | DXT compression |

## Reference-only projects

- SurrealEngine (dpjudas/SurrealEngine, zlib-style license) was used as an
  unread oracle reference for engine behavior questions. No code was copied
  from it.
