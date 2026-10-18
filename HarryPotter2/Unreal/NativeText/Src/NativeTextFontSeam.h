/*=============================================================================
	NativeTextFontSeam.h: font resolution supplied by non-Fontconfig providers.
=============================================================================*/
#ifndef NATIVETEXTFONTSEAM_H
#define NATIVETEXTFONTSEAM_H

#include <cstdint>
#include <string>
#include <vector>

namespace Hp2NativeTextFontSeam
{
	bool ResolveFamilyFile(const std::string& Family, bool Bold,
		std::string& OutPath, std::uint32_t CodePoint, bool bCheckCoverage);
	bool FamilyHasCodePoint(const std::string& Family, std::uint32_t CodePoint);
	bool LoadFontFile(const std::string& Path, std::vector<unsigned char>& OutBytes);
	bool FontIsPresent();
}

#endif
