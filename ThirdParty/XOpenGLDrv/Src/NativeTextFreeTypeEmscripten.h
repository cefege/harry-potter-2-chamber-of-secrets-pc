/*=============================================================================
	NativeTextFreeTypeEmscripten.h: font resolution for the wasm build.

	Declared here so NativeTextFreeType.cpp can share one seam across
	platforms: the FreeType/HarfBuzz shaping code is identical everywhere, and
	only the family -> font-file lookup differs. Compiled only for Emscripten.
=============================================================================*/
#ifndef NATIVETEXTFREETYPEEMSCRIPTEN_H
#define NATIVETEXTFREETYPEEMSCRIPTEN_H

#include <cstdint>
#include <string>
#include <vector>

namespace Hp2NativeTextEmscripten
{
	// Resolve a CSS family to a font file inside the module filesystem. Returns false
	// when nothing resolves, which is the caller's signal to try
	// the next candidate -- the same fallthrough the Fontconfig path uses.
	bool ResolveFamilyFile(const std::string& Family, bool Bold, std::string& OutPath);

	// Whether the family has real coverage for a code point, used to pick a
	// different face for characters the primary one lacks.
	bool FamilyHasCodePoint(const std::string& Family, std::uint32_t CodePoint);

	// Fetch a font file's bytes. Cached for the life of the page.
	bool LoadFontFile(const std::string& Path, std::vector<unsigned char>& OutBytes);

	// Whether the shell has staged a face yet. The browser build probes for a
	// font when the render device is created, which can be before the player
	// has imported anything; the provider uses this to retry rather than
	// caching that first failure for the life of the page.
	bool FontIsPresent();

}

#endif
