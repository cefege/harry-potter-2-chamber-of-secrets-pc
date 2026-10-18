/*=============================================================================
	NativeTextFontsBundled.cpp: font resolution against the shipped font set.
=============================================================================*/
#include "Engine.h"
#include "NativeTextShared.h"
#include "NativeText.h"
#include "NativeTextFontSeam.h"

#include <ft2build.h>
#include FT_FREETYPE_H
#include <SDL.h>

#include <cctype>
#include <fstream>
#include <string>

namespace Hp2NativeTextFontSeam
{
namespace
{
std::string FontPath(const char* Name)
{
	char* Base = SDL_GetBasePath();
	if (!Base)
		return std::string();
	std::string Path(Base);
	SDL_free(Base);
	while (!Path.empty() && (Path.back() == '/' || Path.back() == '\\'))
		Path.pop_back();
	return Path + "/fonts/" + Name;
}

bool Covers(const std::string& Path, std::uint32_t CodePoint)
{
	FT_Library Library = nullptr;
	if (FT_Init_FreeType(&Library) != 0)
		return false;
	FT_Face Face = nullptr;
	const bool Result = FT_New_Face(Library, Path.c_str(), 0, &Face) == 0 && Face &&
		FT_Get_Char_Index(Face, static_cast<FT_ULong>(CodePoint)) != 0;
	if (Face)
		FT_Done_Face(Face);
	FT_Done_FreeType(Library);
	return Result;
}

bool IsMonospace(const std::string& Family)
{
	std::string Lower = Family;
	for (char& Character : Lower)
		Character = static_cast<char>(std::tolower(static_cast<unsigned char>(Character)));
	return Lower.find("mono") != std::string::npos || Lower.find("courier") != std::string::npos;
}
}

bool ResolveFamilyFile(const std::string& Family, bool Bold, std::string& OutPath,
	std::uint32_t CodePoint, bool bCheckCoverage)
{
	const bool bFallback = bCheckCoverage && CodePoint > 0x7f;
	const std::string Candidate = FontPath(bFallback ? "NotoSansSC-Regular.otf" :
		(IsMonospace(Family) ? "LiberationMono-Regular.ttf" :
		(Bold ? "LiberationSans-Bold.ttf" : "LiberationSans-Regular.ttf")));

	std::ifstream File(Candidate, std::ios::binary);
	if (!File)
		return false;
	if (bCheckCoverage && !Covers(Candidate, CodePoint))
	{
		if (bFallback)
			return false;
		const std::string Fallback = FontPath("NotoSansSC-Regular.otf");
		std::ifstream FallbackFile(Fallback, std::ios::binary);
		if (!FallbackFile || !Covers(Fallback, CodePoint))
			return false;
		OutPath = Fallback;
		return true;
	}
	OutPath = Candidate;
	return true;
}

bool FamilyHasCodePoint(const std::string& Family, std::uint32_t CodePoint)
{
	std::string Path;
	return ResolveFamilyFile(Family, false, Path, CodePoint, true);
}

bool LoadFontFile(const std::string&, std::vector<unsigned char>&)
{
	return false;
}

bool FontIsPresent()
{
	std::string Path;
	return ResolveFamilyFile("Liberation Sans", false, Path, 0, false);
}
}
