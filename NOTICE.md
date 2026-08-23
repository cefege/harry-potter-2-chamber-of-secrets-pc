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
| `ThirdParty/XOpenGLDrv` | BSD-3-Clause | OldUnreal/XOpenGLDrv | Bundles glad (MIT, generated OpenGL loader) and GLM (MIT, `glm/copying.txt`); renders through SDL2 GL |
| `ThirdParty/UT99VulkanDrv` | zlib | dpjudas/UT99VulkanDrv | Carries bundled ZVulkan, Vulkan-Headers, VMA, volk, and glslang license notices |
| `ThirdParty/vgmstream` | ISC | vgmstream/vgmstream | Includes EA-XA decoder coefficient data; see its attribution notice |
| `ThirdParty/stb` | Public domain / MIT | nothings/stb | `stb_image_write.h` used for end-of-frame PNG capture |

## Fetched at configure time (FetchContent, hash-pinned)

| Component | License | Upstream | Notes |
| --- | --- | --- | --- |
| SDL2 | zlib | libsdl-org/SDL | Window/input backend for the Linux and macOS clients |
| OpenAL Soft | LGPL-2.1-or-later | kcat/openal-soft | Also carries BSD-3-Clause and PFFFT notices; used by ALAudio |
| libogg | Xiph BSD-3-Clause-style | xiph/ogg | |
| libvorbis | Xiph BSD-3-Clause-style | xiph/vorbis | |
| libSquish | MIT | svn2github/libsquish | DXT compression |
| MoltenVK | Apache-2.0 | KhronosGroup/MoltenVK | Vulkan-over-Metal runtime for the macOS Vulkan driver |

## Reference-only projects

- SurrealEngine (dpjudas/SurrealEngine, zlib-style license) was used as an
  unread oracle reference for engine behavior questions. No code was copied
  from it.
