# Notices

This project's own code is licensed under the MIT license (`LICENSE`). The
repository ships zero game data: players import retail media they own via
`Build/prepare_retail_data.py` (`RETAIL_IMPORT.md`). Maps, textures, sounds,
music, and compiled UnrealScript packages remain copyrighted by their original
publishers and are excluded from version control.

Third-party components are vendored or fetched exactly as recorded in
`ThirdParty/sources.json` and keep their own licenses. They are not covered by
the MIT license above.

## Vendored in-tree

| Component | License | Upstream |
| --- | --- | --- |
| `ThirdParty/UT469eSDK` | Epic retail-derived personal non-profit terms | OldUnreal/UnrealTournamentPatches v469e SDK |
| `ThirdParty/XOpenGLDrv` | BSD-3-Clause-style; bundles glad (MIT) and GLM (MIT, `glm/copying.txt`) | OldUnreal/XOpenGLDrv |
| `ThirdParty/UT99VulkanDrv` | zlib; carries bundled ZVulkan, Vulkan-Headers, VMA, volk, and glslang notices | dpjudas/UT99VulkanDrv |
| `ThirdParty/vgmstream` | ISC-style | vgmstream/vgmstream |

## Fetched at configure time (FetchContent, commit-pinned)

| Component | License | Upstream |
| --- | --- | --- |
| SDL2 | zlib | libsdl-org/SDL |
| OpenAL Soft | LGPL-2.0-or-later, with BSD-3-Clause and PFFFT notices | kcat/openal-soft |
| libogg | Xiph BSD-3-Clause-style | xiph/ogg |
| libvorbis | Xiph BSD-3-Clause-style | xiph/vorbis |
| libSquish | MIT | svn2github/libsquish |

OpenAL Soft is linked as a replaceable shared library and is redistributed in
`dist/macos-arm64/HarryPotter2.app/Contents/Frameworks` under the terms of the
LGPL.

## Additional pinned sources

`ThirdParty/sources.json` also pins MoltenVK (Apache-2.0,
KhronosGroup/MoltenVK) for Vulkan-over-Metal experiments and records
SurrealEngine (dpjudas/SurrealEngine, zlib-style license) as a reference-only
behavioral oracle. Neither is part of the default build graph, and no code was
copied from SurrealEngine.
