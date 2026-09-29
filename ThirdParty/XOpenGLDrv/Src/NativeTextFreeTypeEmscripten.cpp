/*=============================================================================
	NativeTextFreeTypeEmscripten.cpp: font resolution for the wasm build.

	The FreeType provider needs Fontconfig for exactly two things: turn a
	family name into a font file, and pick a different file when a code point
	is missing from the face already chosen. Fontconfig is a native library
	with its own font cache, so it does not exist in WebAssembly.

	Everything else in the provider is already portable: Emscripten ships
	FreeType and HarfBuzz as ports, so shaping, cluster boundaries, fallback
	runs, the glyph cache and the atlas are the same code the Linux build
	runs. Only these two lookups need replacing, and the browser already
	knows both answers -- CSS resolution through document.fonts, and
	character coverage by asking whether the family renders the character at
	all.

	The font file itself is fetched by the page and handed to FreeType
	through FT_New_Memory_Face, because a page has no filesystem to open a
	path from.

	Every call into JavaScript uses EM_ASM with a braced body, which is the
	form the rest of this tree already uses; EM_JS wants a single string
	literal and is easy to get subtly wrong.

	Compiled only for Emscripten. Every other platform keeps using Fontconfig,
	so the provider's behaviour is otherwise identical, which is what lets one
	typography contract cover all three builds.
=============================================================================*/
#include "Engine.h"

#include "NativeTextShared.h"

#include "NativeText.h"

#if !defined(__EMSCRIPTEN__)
#error NativeTextFreeTypeEmscripten.cpp is only for the Emscripten build.
#endif

#include <emscripten/emscripten.h>

// FreeType is needed here, not just in the provider: the coverage probe opens
// each staged face to ask FT_Get_Char_Index whether it has a glyph.
#include <ft2build.h>
#include FT_FREETYPE_H

#include <cstdint>
#include <string>
#include <vector>

namespace Hp2NativeTextEmscripten
{
	namespace
	{
		// One fetch, kept for the life of the page: a font file is tens of
		// kilobytes and the provider asks for the same handful repeatedly.
		struct FFontFile
		{
			std::string Url;
			std::vector<unsigned char> Bytes;
		};

		std::vector<FFontFile> GLoadedFonts;

		// Font bytes come from the module's own filesystem, not over HTTP. The
		// import puts the game's own TrueType face (Simyou.ttf) there, so there
		// is nothing to publish from the page and nothing to license.
		//
		// Two Emscripten details this has to get right:
		//   - an EM_ASM body cannot write a C++ out-parameter by assigning to
		//     it; that only rebinds a JavaScript local and the C++ side stays
		//     zero, so the result has to come back through setValue.
		//   - the web preset is MEMORY64, so a pointer arriving in JavaScript is
		//     a BigInt and every HEAP/UTF8 helper needs it coerced with Number
		//     first. Casting it away in C++ would not fix that.
		static int ReadFileImpl(const char* Path, unsigned char* OutBytes, int Capacity)
		{
			int Size = 0;
			EM_ASM({

				try {
					var resolved = UTF8ToString($0);
					var buffer = Number($1);
					var capacity = $2;
					if (Module.FS && capacity > 0) {
						var stat = Module.FS.stat(resolved);
						if (stat && stat.size > 0 && stat.size <= capacity) {
							// No encoding option: Emscripten returns a
							// Uint8Array for a raw read, and a string only for
							// an encoding it knows. Asking for 'binary'
							// yields an array whose charCodeAt is undefined, so
							// every byte would have been written as zero.
							var bytes = Module.FS.readFile(resolved);
							var view = bytes instanceof Uint8Array ? bytes : null;
							if (view) {
								HEAPU8.set(view, buffer);
								setValue($3, view.length, 'i32');
							}
						}
					}
				} catch (error) {
					setValue($3, 0, 'i32');
				}

			}, Path, OutBytes, Capacity, &Size);
			return Size;
		}

	}

	// Map a CSS family to a font file inside the module's filesystem. The
	// game's own TrueType face is the answer for every role: it is the face the
	// game shipped for its own text, so using it is faithful rather than a
	// substitution, and it needs no download and no licence decision.
	// Where the shell stages the face. It is a separate directory from the data
	// root on purpose: the engine's bootstrap checks /hp2data for Default.ini,
	// and a font appearing there would make an empty data root look imported.
	// The game's own face first: it is what the text was authored against, so
	// using it keeps the look faithful. The vendored open-licence faces behind
	// it are what make the browser build independent -- the game's data supplies
	// one face, and without these anything outside it would be a missing-glyph
	// box with no way to recover. Liberation Sans is a wide Latin/Greek/Cyrillic
	// cover; Noto Sans SC is the CJK fallback; Liberation Mono covers the
	// console text that is otherwise monospaced in the original.
	const char* const kFontPath = "/hp2fonts/Simyou.ttf";
	const char* const kFallbackSans = "/hp2fonts/LiberationSans-Regular.ttf";
	const char* const kFallbackCJK = "/hp2fonts/NotoSansSC-Regular.otf";
	const char* const kFallbackMono = "/hp2fonts/LiberationMono-Regular.ttf";
	// A sanity bound only: the largest realistic face is far under this, and
	// anything larger is a corrupt stat rather than a font.
	const long long kMaxFontBytes = 64ll * 1024ll * 1024ll;

	static int StatSizeImpl(const char* Path, long long& OutSize)
	{
		int Present = 0;
		EM_ASM({
			try {
				if (Module.FS) {
					var s = Module.FS.stat(UTF8ToString($0));
					if (s) { setValue($1, 1, 'i32'); setValue($2, s.size, 'i64'); }
				}
			} catch (e) { }
		}, Path, &Present, &OutSize);
		return Present;
	}

	bool FontIsPresent()
	{
		long long Size = 0;
		return StatSizeImpl(kFontPath, Size) != 0 && Size > 0;
	}


	// A family name carries no information here, so the face is chosen by
	// coverage instead: the first chain entry that has the requested glyph.
	// Requesting the game's own face is the default so a caller with no code
	// point to check still gets the authored look.
	// Both defined below; the coverage probe needs the loader and they are
	// mutually recursive through the chain.
	bool LoadFontFile(const std::string& Path, std::vector<unsigned char>& OutBytes);
	bool FaceCoversPath(const std::string& Path, std::uint32_t CodePoint);

	// Chain in preference order: the game's face, then the vendored ones.
	const char* const kChain[] = {
		kFontPath, kFallbackSans, kFallbackCJK, kFallbackMono
	};

	bool FaceCoversPath(const std::string& Path, std::uint32_t CodePoint)
	{
		std::vector<unsigned char> Bytes;
		if (!LoadFontFile(Path, Bytes))
			return false;
		// Probe through FreeType itself rather than an index lookup, so the
		// answer reflects what the rasterizer would actually draw.
		FT_Library Library = NULL;
		if (FT_Init_FreeType(&Library) != 0)
			return false;
		FT_Face Face = NULL;
		bool Covers = false;
		if (FT_New_Memory_Face(Library, Bytes.data(),
				static_cast<FT_Long>(Bytes.size()), 0, &Face) == 0 && Face)
		{
			Covers = FT_Get_Char_Index(Face, static_cast<FT_ULong>(CodePoint)) != 0;
			FT_Done_Face(Face);
		}
		FT_Done_FreeType(Library);
		return Covers;
	}
	bool ResolveFamilyFile(const std::string& /*Family*/, bool /*Bold*/,
		std::string& OutPath, std::uint32_t CodePoint, bool bCheckCoverage)
	{
		OutPath = kFontPath;
		if (!bCheckCoverage)
			return true;
		for (const char* Candidate : kChain)
		{
			if (FaceCoversPath(Candidate, CodePoint))
			{
				OutPath = Candidate;
				return true;
			}
		}
		return true;
	}

	// The real question the Linux build answers with a Fontconfig charset
	// match. Here the answer is "which of the staged faces has this glyph",
	// which is exactly FT_Get_Char_Index against each one. Nothing here
	// guesses from a family name, so a face is only credited with coverage it
	// really has.

	bool FamilyHasCodePoint(const std::string& /*Family*/, std::uint32_t CodePoint)
	{
		for (const char* Candidate : kChain)
		{
			if (FaceCoversPath(Candidate, CodePoint))
				return true;
		}
		return false;
	}

	bool LoadFontFile(const std::string& Path, std::vector<unsigned char>& OutBytes)
	{
		for (const FFontFile& Cached : GLoadedFonts)
		{
			if (Cached.Url == Path)
			{
				OutBytes = Cached.Bytes;
				return !OutBytes.empty();
			}
		}

		// Size the buffer from the file rather than guessing a ceiling. The
		// game's own face is about 6.8 MB, which a fixed 4 MB cap rejected
		// before FreeType ever saw it -- a font-size assumption, not a limit.
		long long Present = 0;
		if (!StatSizeImpl(Path.c_str(), Present) || Present <= 0 ||
			Present > static_cast<long long>(kMaxFontBytes))
		{
			return false;
		}
		std::vector<unsigned char> Buffer(static_cast<size_t>(Present));
		const int Size = ReadFileImpl(Path.c_str(), Buffer.data(), static_cast<int>(Present));
		if (Size <= 0)
		{
			return false;
		}
		Buffer.resize(static_cast<size_t>(Size));
		OutBytes = std::move(Buffer);
		if (OutBytes.empty())
			return false;

		FFontFile& Entry = GLoadedFonts.emplace_back();
		Entry.Url = Path;
		Entry.Bytes = OutBytes;
		return true;
	}
}
