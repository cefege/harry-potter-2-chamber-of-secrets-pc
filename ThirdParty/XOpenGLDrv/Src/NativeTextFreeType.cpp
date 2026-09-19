/*=============================================================================
	NativeTextFreeType.cpp: Driver-neutral FreeType/HarfBuzz/Fontconfig Canvas
	layout and glyph rasterization core shared by every render driver with a
	native text leg.  Non-Apple twin of NativeTextShared.cpp: same contract,
	shaping performed with HarfBuzz instead of CoreText, family resolution and
	per-codepoint fallback performed with Fontconfig instead of AppKit/UIKit.
=============================================================================*/
// UnRenDev.h (pulled in by NativeTextShared.h) is not self-contained: the
// engine umbrella must come first, exactly like every driver TU and the
// contract tests include it.
#include "Engine.h"

#include "NativeTextShared.h"

#include "NativeText.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_set>
#include <utility>
#include <vector>

#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_OUTLINE_H
#include <hb.h>
#include <hb-ft.h>
#include <fontconfig/fontconfig.h>

#if !HP2_HAS_NATIVE_TEXT_BACKEND
#error NativeTextFreeType.cpp must only be compiled when HP2_HAS_NATIVE_TEXT_BACKEND is enabled.
#endif

// The concrete face handle behind the header's opaque FNativeTextFace
// forward declaration.  Faces are cached forever in a process-global map
// (see GFaceCache below) and are never released, mirroring the implicit
// global state CoreText keeps behind CTFontRef; every caller in this TU
// therefore treats FNativeTextFace* as a stable, non-owning borrow.
struct FNativeTextFace
{
	std::string Identity;
	FT_Face Face{};
	hb_font_t* Font{};
	double PointSize{};
};

namespace Hp2NativeText
{
	namespace
	{
		// UTF-16 code unit type used only inside this TU, mirroring
		// CoreFoundation's UniChar on the Apple twin.
		using UniChar = std::uint16_t;

		// ---------------------------------------------------------------
		// Process-global font stack.  Initialized lazily, never torn down.
		// This module is only ever driven from the single render thread
		// that owns FNativeTextPlatformBackend, so no locking is used --
		// the same assumption the CoreText path makes implicitly.
		// ---------------------------------------------------------------
		std::once_flag GFontStackInitFlag;
		bool GFontStackOk = false;
		FT_Library GFtLibrary{};
		std::map<std::string, std::unique_ptr<FNativeTextFace>> GFaceCache;

		void EnsureFontStack()
		{
			std::call_once(GFontStackInitFlag, []()
			{
				GFontStackOk = (FcInit() != FcFalse) && FT_Init_FreeType(&GFtLibrary) == 0;
			});
		}

		static ENativeTextRuntimeSmokeState GNativeTextRuntimeSmokeState = NativeTextRuntimeSmokeIdle;
		static ENativeTextRuntimeSmokeStage GNativeTextRuntimeSmokeStage = NativeTextRuntimeSmokeStageNone;

		static void AppendUTF16Scalar(uint32_t Scalar, std::vector<UniChar>& OutText)
		{
			if (Scalar > 0x10ffffu || (Scalar >= 0xd800u && Scalar <= 0xdfffu))
				Scalar = 0xfffdu;

			if (Scalar <= 0xffffu)
			{
				OutText.push_back(static_cast<UniChar>(Scalar));
				return;
			}

			Scalar -= 0x10000u;
			OutText.push_back(static_cast<UniChar>(0xd800u + (Scalar >> 10)));
			OutText.push_back(static_cast<UniChar>(0xdc00u + (Scalar & 0x3ffu)));
		}

		static void AppendUTF16Scalar(uint32_t Scalar, std::vector<UniChar>& OutText, std::vector<INT>& OutSourceEnds, INT SourceEnd)
		{
			const size_t Before = OutText.size();
			AppendUTF16Scalar(Scalar, OutText);
			OutSourceEnds.insert(OutSourceEnds.end(), OutText.size() - Before, SourceEnd);
		}

		static std::vector<UniChar> UTF16FromTCHAR(const TCHAR* Text, INT TextLength)
		{
			std::vector<UniChar> Result;
			if (!Text)
				return Result;

			for (INT Index = 0; Index < TextLength; ++Index)
			{
				if (!Text[Index])
					break;
				AppendUTF16Scalar(static_cast<uint32_t>(static_cast<TCHARU>(Text[Index])), Result);
			}
			return Result;
		}

		static std::string Utf8FromTChar(const TCHAR* Text)
		{
			std::string Result;
			if (!Text)
				return Result;
			for (; *Text; ++Text)
			{
				uint32_t Scalar = static_cast<uint32_t>(static_cast<TCHARU>(*Text));
				if (Scalar > 0x10ffffu || (Scalar >= 0xd800u && Scalar <= 0xdfffu))
					Scalar = 0xfffdu;

				if (Scalar <= 0x7fu)
				{
					Result.push_back(static_cast<char>(Scalar));
				}
				else if (Scalar <= 0x7ffu)
				{
					Result.push_back(static_cast<char>(0xc0u | (Scalar >> 6)));
					Result.push_back(static_cast<char>(0x80u | (Scalar & 0x3fu)));
				}
				else if (Scalar <= 0xffffu)
				{
					Result.push_back(static_cast<char>(0xe0u | (Scalar >> 12)));
					Result.push_back(static_cast<char>(0x80u | ((Scalar >> 6) & 0x3fu)));
					Result.push_back(static_cast<char>(0x80u | (Scalar & 0x3fu)));
				}
				else
				{
					Result.push_back(static_cast<char>(0xf0u | (Scalar >> 18)));
					Result.push_back(static_cast<char>(0x80u | ((Scalar >> 12) & 0x3fu)));
					Result.push_back(static_cast<char>(0x80u | ((Scalar >> 6) & 0x3fu)));
					Result.push_back(static_cast<char>(0x80u | (Scalar & 0x3fu)));
				}
			}
			return Result;
		}

		static UBOOL NameEquals(const TCHAR* Name, const TCHAR* Expected)
		{
			return Name && Expected && appStricmp(Name, Expected) == 0;
		}

		static INT RolePointSize(ENativeTextRole Role)
		{
			switch (Role)
			{
				case NTROLE_Tiny:         return 10;
				case NTROLE_Small:        return 12;
				case NTROLE_Body:
				case NTROLE_BodyBold:
				case NTROLE_Console:      return 14;
				case NTROLE_Heading:
				case NTROLE_HPMenuMedium:
				case NTROLE_Subtitle:     return 18;
				case NTROLE_HeadingBold:
				case NTROLE_HPMenuLarge:  return 24;
				default:                   return 14;
			}
		}

		static UBOOL RoleIsBold(ENativeTextRole Role)
		{
			return Role == NTROLE_BodyBold || Role == NTROLE_HeadingBold;
		}

		static INT MaxPageGlyphHeight(const UFont* Font)
		{
			INT Height = 0;
			if (!Font)
				return Height;

			for (INT PageIndex = 0; PageIndex < Font->Pages.Num(); ++PageIndex)
			{
				const FFontPage& Page = Font->Pages(PageIndex);
				for (INT CharacterIndex = 0; CharacterIndex < Page.Characters.Num(); ++CharacterIndex)
					Height = Max(Height, Page.Characters(CharacterIndex).VSize);
			}
			return Height;
		}

		// ---------------------------------------------------------------
		// Face loading.  LoadFaceFromFile is the single point that owns
		// GFaceCache; AcquireFace (family-based) and the per-codepoint
		// fallback path (Fontconfig charset match) both funnel through it.
		// ---------------------------------------------------------------
		static FNativeTextFace* LoadFaceFromFile(const std::string& File, int FaceIndex, double PointSize)
		{
			if (!GFontStackOk || !std::isfinite(PointSize) || PointSize <= 0 ||
				PointSize > static_cast<double>(std::numeric_limits<uint32_t>::max()) / 64.0)
				return NULL;

			const uint32_t RasterPointSize64 = static_cast<uint32_t>(std::llround(PointSize * 64.0));
			const std::string CacheKey = File + ":" + std::to_string(FaceIndex) + "@" + std::to_string(RasterPointSize64);
			const auto Existing = GFaceCache.find(CacheKey);
			if (Existing != GFaceCache.end())
				return Existing->second.get();

			FT_Face FtFace = NULL;
			if (FT_New_Face(GFtLibrary, File.c_str(), FaceIndex, &FtFace) != 0 || !FtFace)
				return NULL;
			if (FT_Set_Char_Size(FtFace, 0, static_cast<FT_F26Dot6>(std::llround(PointSize * 64.0)), 72, 72) != 0)
			{
				FT_Done_Face(FtFace);
				return NULL;
			}
			hb_font_t* HbFont = hb_ft_font_create_referenced(FtFace);
			if (!HbFont)
			{
				FT_Done_Face(FtFace);
				return NULL;
			}

			std::unique_ptr<FNativeTextFace> NewFace(new FNativeTextFace);
			NewFace->Identity = File + ":" + std::to_string(FaceIndex) + "|" + std::to_string(RasterPointSize64);
			NewFace->Face = FtFace;
			NewFace->Font = HbFont;
			NewFace->PointSize = PointSize;
			FNativeTextFace* Result = NewFace.get();
			GFaceCache.emplace(CacheKey, std::move(NewFace));
			return Result;
		}

		static FNativeTextFace* AcquireFace(const char* Family, bool Bold, double PointSize)
		{
			EnsureFontStack();
			if (!GFontStackOk || !Family || !std::isfinite(PointSize) || PointSize <= 0 ||
				PointSize > static_cast<double>(std::numeric_limits<uint32_t>::max()) / 64.0)
				return NULL;

			FcPattern* Pattern = FcPatternCreate();
			if (!Pattern)
				return NULL;
			FcPatternAddString(Pattern, FC_FAMILY, reinterpret_cast<const FcChar8*>(Family));
			FcPatternAddInteger(Pattern, FC_WEIGHT, Bold ? FC_WEIGHT_BOLD : FC_WEIGHT_REGULAR);
			FcPatternAddDouble(Pattern, FC_SIZE, PointSize);
			FcConfigSubstitute(NULL, Pattern, FcMatchPattern);
			FcDefaultSubstitute(Pattern);

			FcResult MatchResult = FcResultNoMatch;
			FcPattern* Matched = FcFontMatch(NULL, Pattern, &MatchResult);
			FcPatternDestroy(Pattern);
			if (!Matched)
				return NULL;

			FcChar8* FileValue = NULL;
			int FaceIndex = 0;
			if (FcPatternGetString(Matched, FC_FILE, 0, &FileValue) != FcResultMatch || !FileValue)
			{
				FcPatternDestroy(Matched);
				return NULL;
			}
			FcPatternGetInteger(Matched, FC_INDEX, 0, &FaceIndex);
			const std::string File(reinterpret_cast<const char*>(FileValue));
			FcPatternDestroy(Matched);

			return LoadFaceFromFile(File, FaceIndex, PointSize);
		}

		// Deliberate policy difference from CoreText: Fontconfig's own
		// substitution *is* the platform font policy on Linux, so whatever
		// family it resolves to is accepted outright -- no .LastResort-style
		// rejection.  This is what makes "Times" and "Helvetica" resolve to
		// distinct files (Nimbus Roman / Nimbus Sans on this host).
		static FNativeTextFace* CreateRoleFont(ENativeTextRole Role, double PointSize)
		{
			if (!std::isfinite(PointSize) || PointSize <= 0 ||
				PointSize > static_cast<double>(std::numeric_limits<uint32_t>::max()) / 64.0)
				return NULL;

			const bool Bold = RoleIsBold(Role) != 0;
			FNativeTextFace* Face = AcquireFace("Times New Roman", Bold, PointSize);
			if (!Face)
				Face = AcquireFace("serif", Bold, PointSize);
			if (!Face)
				Face = AcquireFace("sans-serif", Bold, PointSize);
			return Face;
		}

		static FNativeTextFace* CreateRequestFont(const FCanvasTextLayoutRequest& Request, ENativeTextRole Role)
		{
			if (!Request.Font)
				return NULL;

			const double Scale = Request.TextScale > 0.f ? static_cast<double>(Request.TextScale) : 1.0;
			if (Request.Font->FontName.Len() > 0)
			{
				const INT RequestedHeight = Request.Font->FontHeight > 0 ? Request.Font->FontHeight : RolePointSize(NTROLE_Console);
				const std::string FamilyUtf8 = Utf8FromTChar(*Request.Font->FontName);
				FNativeTextFace* Face = AcquireFace(FamilyUtf8.c_str(), false, static_cast<double>(RequestedHeight) * Scale);
				if (Face)
					return Face;
			}

			const TCHAR* Name = Request.Font->GetName();
			const bool IsKnownPackageFont =
				NameEquals(Name, TEXT("TinyInkFont")) || NameEquals(Name, TEXT("SmallInkFont")) ||
				NameEquals(Name, TEXT("MedInkFont")) || NameEquals(Name, TEXT("BigInkFont")) ||
				NameEquals(Name, TEXT("HugeInkFont")) || NameEquals(Name, TEXT("Tahoma10")) ||
				NameEquals(Name, TEXT("TahomaB10")) || NameEquals(Name, TEXT("Tahoma20")) ||
				NameEquals(Name, TEXT("TahomaB20")) || NameEquals(Name, TEXT("Tahoma30")) ||
				NameEquals(Name, TEXT("TahomaB30")) || NameEquals(Name, TEXT("FontHPMenuMedium")) ||
				NameEquals(Name, TEXT("FontHPMenuLarge"));

			INT PointSize = RolePointSize(Role);
			if (Request.Font->FontName.Len() == 0 && !IsKnownPackageFont)
			{
				PointSize = MaxPageGlyphHeight(Request.Font);
				if (PointSize <= 0)
					PointSize = RolePointSize(NTROLE_Body);
			}
			return CreateRoleFont(Role, static_cast<double>(PointSize) * Scale);
		}

		// Fontconfig charset match for a single missing code point.  Cached
		// per CreateLayout call (the map lives on the caller's stack), not
		// process-globally: a global cache keyed only by code point would go
		// stale across requests that resolve the same code point at a
		// different point size.
		static FNativeTextFace* FallbackFaceFor(uint32_t CodePoint, bool Bold, double PointSize,
			std::map<uint32_t, FNativeTextFace*>& Cache, FNativeTextFace* Primary)
		{
			const auto Existing = Cache.find(CodePoint);
			if (Existing != Cache.end())
				return Existing->second;

			FNativeTextFace* Result = Primary;
			FcCharSet* CharSet = FcCharSetCreate();
			if (CharSet && FcCharSetAddChar(CharSet, static_cast<FcChar32>(CodePoint)))
			{
				FcPattern* Pattern = FcPatternCreate();
				if (Pattern)
				{
					FcPatternAddCharSet(Pattern, FC_CHARSET, CharSet);
					FcPatternAddString(Pattern, FC_FAMILY, reinterpret_cast<const FcChar8*>("serif"));
					FcPatternAddInteger(Pattern, FC_WEIGHT, Bold ? FC_WEIGHT_BOLD : FC_WEIGHT_REGULAR);
					FcPatternAddDouble(Pattern, FC_SIZE, PointSize);
					FcConfigSubstitute(NULL, Pattern, FcMatchPattern);
					FcDefaultSubstitute(Pattern);

					FcResult MatchResult = FcResultNoMatch;
					FcPattern* Matched = FcFontMatch(NULL, Pattern, &MatchResult);
					if (Matched)
					{
						FcCharSet* MatchedCharSet = NULL;
						if (FcPatternGetCharSet(Matched, FC_CHARSET, 0, &MatchedCharSet) == FcResultMatch &&
							MatchedCharSet && FcCharSetHasChar(MatchedCharSet, static_cast<FcChar32>(CodePoint)))
						{
							FcChar8* FileValue = NULL;
							int FaceIndex = 0;
							if (FcPatternGetString(Matched, FC_FILE, 0, &FileValue) == FcResultMatch && FileValue)
							{
								FcPatternGetInteger(Matched, FC_INDEX, 0, &FaceIndex);
								FNativeTextFace* Loaded = LoadFaceFromFile(reinterpret_cast<const char*>(FileValue), FaceIndex, PointSize);
								if (Loaded)
									Result = Loaded;
							}
						}
						FcPatternDestroy(Matched);
					}
					FcPatternDestroy(Pattern);
				}
			}
			if (CharSet)
				FcCharSetDestroy(CharSet);

			Cache.emplace(CodePoint, Result);
			return Result;
		}

		// ---------------------------------------------------------------
		// Coverage segmentation: splits a line/paragraph range into runs of
		// one face, keeping combining marks/variation selectors/ZWJ glued
		// to their base code point so a run switch never separates them.
		// ---------------------------------------------------------------
		static uint32_t DecodeUtf16At(const std::vector<UniChar>& Text, std::ptrdiff_t Index, std::ptrdiff_t& OutUnitLength)
		{
			const UniChar Unit = Text[static_cast<size_t>(Index)];
			if (Unit >= 0xd800u && Unit <= 0xdbffu && Index + 1 < static_cast<std::ptrdiff_t>(Text.size()))
			{
				const UniChar Low = Text[static_cast<size_t>(Index + 1)];
				if (Low >= 0xdc00u && Low <= 0xdfffu)
				{
					OutUnitLength = 2;
					return 0x10000u + ((static_cast<uint32_t>(Unit) - 0xd800u) << 10) + (static_cast<uint32_t>(Low) - 0xdc00u);
				}
			}
			OutUnitLength = 1;
			return static_cast<uint32_t>(Unit);
		}

		static bool IsAttachmentScalar(uint32_t Scalar)
		{
			if (Scalar >= 0x0300u && Scalar <= 0x036fu) return true;
			if (Scalar >= 0x1ab0u && Scalar <= 0x1affu) return true;
			if (Scalar >= 0x1dc0u && Scalar <= 0x1dffu) return true;
			if (Scalar >= 0x20d0u && Scalar <= 0x20ffu) return true;
			if (Scalar >= 0xfe00u && Scalar <= 0xfe0fu) return true;
			if (Scalar >= 0xfe20u && Scalar <= 0xfe2fu) return true;
			if (Scalar == 0x200du) return true;
			if (Scalar >= 0xe0100u && Scalar <= 0xe01efu) return true;
			return false;
		}

		struct FTextRun
		{
			std::ptrdiff_t Start{};
			std::ptrdiff_t Length{};
			FNativeTextFace* Face{};
		};

		static std::vector<FTextRun> SegmentIntoRuns(const std::vector<UniChar>& Text, FTextRange Range,
			FNativeTextFace* PrimaryFace, bool Bold, double PointSize, std::map<uint32_t, FNativeTextFace*>& FallbackCache)
		{
			std::vector<FTextRun> Runs;
			const std::ptrdiff_t End = Range.location + Range.length;
			std::ptrdiff_t Cursor = Range.location;
			while (Cursor < End)
			{
				std::ptrdiff_t UnitLength = 0;
				const uint32_t BaseScalar = DecodeUtf16At(Text, Cursor, UnitLength);
				std::ptrdiff_t UnitEnd = Cursor + UnitLength;
				while (UnitEnd < End)
				{
					std::ptrdiff_t NextLength = 0;
					const uint32_t NextScalar = DecodeUtf16At(Text, UnitEnd, NextLength);
					if (!IsAttachmentScalar(NextScalar))
						break;
					UnitEnd += NextLength;
				}

				FNativeTextFace* Face = PrimaryFace;
				if (!PrimaryFace || FT_Get_Char_Index(PrimaryFace->Face, static_cast<FT_ULong>(BaseScalar)) == 0)
					Face = FallbackFaceFor(BaseScalar, Bold, PointSize, FallbackCache, PrimaryFace);

				if (!Runs.empty() && Runs.back().Face == Face)
					Runs.back().Length += (UnitEnd - Cursor);
				else
					Runs.push_back(FTextRun{Cursor, UnitEnd - Cursor, Face});

				Cursor = UnitEnd;
			}
			return Runs;
		}

		// ---------------------------------------------------------------
		// Shaping.
		// ---------------------------------------------------------------
		struct FShapedRun
		{
			FNativeTextFace* Face{};
			std::vector<hb_glyph_info_t> Infos;
			std::vector<hb_glyph_position_t> Positions;
		};

		static bool ShapeRun(const std::vector<UniChar>& Text, const FTextRun& Run, FShapedRun& OutShaped)
		{
			if (!Run.Face)
				return false;
			hb_buffer_t* Buffer = hb_buffer_create();
			if (!Buffer)
				return false;
			hb_buffer_add_utf16(Buffer, reinterpret_cast<const uint16_t*>(Text.data()), static_cast<int>(Text.size()),
				static_cast<unsigned>(Run.Start), static_cast<int>(Run.Length));
			hb_buffer_guess_segment_properties(Buffer);
			hb_shape(Run.Face->Font, Buffer, NULL, 0);

			unsigned GlyphCount = 0;
			hb_glyph_info_t* Infos = hb_buffer_get_glyph_infos(Buffer, &GlyphCount);
			hb_glyph_position_t* Positions = hb_buffer_get_glyph_positions(Buffer, &GlyphCount);
			if (GlyphCount > 0 && (!Infos || !Positions))
			{
				hb_buffer_destroy(Buffer);
				return false;
			}

			OutShaped.Face = Run.Face;
			OutShaped.Infos.assign(Infos, Infos + GlyphCount);
			OutShaped.Positions.assign(Positions, Positions + GlyphCount);
			hb_buffer_destroy(Buffer);
			return true;
		}

		static void CollectRunClusterBoundaries(const std::vector<hb_glyph_info_t>& Infos, std::ptrdiff_t RunEnd, std::vector<FTextRange>& OutRanges)
		{
			std::vector<std::ptrdiff_t> Sorted;
			Sorted.reserve(Infos.size());
			for (const hb_glyph_info_t& Info : Infos)
				Sorted.push_back(static_cast<std::ptrdiff_t>(Info.cluster));
			std::sort(Sorted.begin(), Sorted.end());
			Sorted.erase(std::unique(Sorted.begin(), Sorted.end()), Sorted.end());
			for (size_t Index = 0; Index < Sorted.size(); ++Index)
			{
				const std::ptrdiff_t Start = Sorted[Index];
				const std::ptrdiff_t End = (Index + 1 < Sorted.size()) ? Sorted[Index + 1] : RunEnd;
				if (End > Start)
					OutRanges.push_back(FTextRange{Start, End - Start});
			}
		}

		static void AppendNativeTextClusters(FCanvasTextLayout& Layout, const std::vector<FTextRange>& Ranges, const std::vector<INT>& SourceEnds)
		{
			for (const FTextRange& Range : Ranges)
			{
				const std::ptrdiff_t End = Range.location + Range.length;
				if (Range.length <= 0 || End > static_cast<std::ptrdiff_t>(SourceEnds.size()))
					continue;
				if (!Layout.Clusters.empty())
				{
					const FCluster& Previous = Layout.Clusters.back();
					if (Previous.SourceRange.location == Range.location && Previous.SourceRange.length == Range.length)
						continue;
				}
				FCluster Cluster;
				Cluster.SourceRange = Range;
				Cluster.SourceEnd = SourceEnds[static_cast<size_t>(End - 1)];
				Layout.Clusters.push_back(Cluster);
			}
		}

		static const FCluster* NativeTextClusterAt(const std::vector<FCluster>& Clusters, std::ptrdiff_t SourceIndex)
		{
			for (const FCluster& Cluster : Clusters)
			{
				const std::ptrdiff_t End = Cluster.SourceRange.location + Cluster.SourceRange.length;
				if (SourceIndex >= Cluster.SourceRange.location && SourceIndex < End)
					return &Cluster;
			}
			return NULL;
		}

		static void AppendShapedUnderlines(FCanvasTextLayout& Layout, FTextRange LineRange, const std::vector<FTextRange>& UnderlineRanges,
			const std::map<std::ptrdiff_t, double>& PenAtSource, double Baseline, FNativeTextFace* BaseFace)
		{
			if (!BaseFace || !BaseFace->Face || !BaseFace->Face->size)
				return;
			const std::ptrdiff_t LineEnd = LineRange.location + LineRange.length;
			const double UnderlinePosition = static_cast<double>(FT_MulFix(BaseFace->Face->underline_position, BaseFace->Face->size->metrics.y_scale)) / 64.0;
			const double UnderlineThickness = Max(1.0, static_cast<double>(FT_MulFix(BaseFace->Face->underline_thickness, BaseFace->Face->size->metrics.y_scale)) / 64.0);

			for (const FTextRange& Range : UnderlineRanges)
			{
				const std::ptrdiff_t Start = Max(LineRange.location, Range.location);
				const std::ptrdiff_t End = Min(LineEnd, Range.location + Range.length);
				if (Start >= End)
					continue;

				const auto ItA = PenAtSource.lower_bound(Start);
				const auto ItB = PenAtSource.lower_bound(End);
				if (ItA == PenAtSource.end() || ItB == PenAtSource.end())
					continue;
				const double OffsetA = ItA->second;
				const double OffsetB = ItB->second;
				if (!std::isfinite(OffsetA) || !std::isfinite(OffsetB) || OffsetA == OffsetB)
					continue;

				FUnderline Underline;
				Underline.Start = Min(OffsetA, OffsetB);
				Underline.End = Max(OffsetA, OffsetB);
				Underline.Baseline = Baseline;
				Underline.Position = UnderlinePosition;
				Underline.Thickness = UnderlineThickness;
				Layout.Underlines.push_back(Underline);
			}
		}

		// Fills a glyph's Bounds/bDrawable/Key from a freshly rendered
		// FreeType glyph slot.  Rendering here (rather than just measuring)
		// is deliberate: it guarantees CalculateRasterRectangle's floor/ceil
		// agrees pixel-for-pixel with the bitmap RasterizeGlyph produces
		// later, because both read the same rendered ink box.
		static bool FillPlacementFromRender(FGlyphPlacement& Placement, FNativeTextFace* Face, hb_codepoint_t Glyph)
		{
			if (FT_Load_Glyph(Face->Face, Glyph, FT_LOAD_DEFAULT | FT_LOAD_RENDER) != 0)
				return false;
			const FT_GlyphSlot Slot = Face->Face->glyph;
			const bool NullBounds = (Slot->bitmap.width == 0 || Slot->bitmap.rows == 0);
			if (!NullBounds)
			{
				Placement.Bounds.MinX = static_cast<double>(Slot->bitmap_left);
				Placement.Bounds.MaxY = static_cast<double>(Slot->bitmap_top);
				Placement.Bounds.MaxX = Placement.Bounds.MinX + static_cast<double>(Slot->bitmap.width);
				Placement.Bounds.MinY = Placement.Bounds.MaxY - static_cast<double>(Slot->bitmap.rows);
			}

			const bool FiniteBounds = NullBounds ||
				(std::isfinite(Placement.Bounds.MinX) && std::isfinite(Placement.Bounds.MinY) &&
				 std::isfinite(Placement.Bounds.MaxX) && std::isfinite(Placement.Bounds.MaxY) &&
				 Placement.Bounds.MaxX >= Placement.Bounds.MinX && Placement.Bounds.MaxY >= Placement.Bounds.MinY);
			if (!FiniteBounds)
				return false;

			Placement.bDrawable = !NullBounds;
			if (Placement.bDrawable)
			{
				const double RasterPointSize64 = Face->PointSize * 64.0;
				if (!std::isfinite(RasterPointSize64) || RasterPointSize64 <= 0.0 ||
					RasterPointSize64 > static_cast<double>(std::numeric_limits<uint32_t>::max()) - 0.5)
					return false;
				Placement.Key.FontIdentity = Face->Identity;
				Placement.Key.Glyph = static_cast<uint32_t>(Glyph);
				Placement.Key.RasterPointSize64 = static_cast<uint32_t>(std::llround(RasterPointSize64));
				Placement.Key.Antialias = 1;
				Placement.Key.Subpixel = 0;
			}
			return true;
		}

		// ---------------------------------------------------------------
		// One line: segment -> shape every run -> place glyphs (optional) ->
		// underlines -> clusters -> advance the running Top cursor.
		// ---------------------------------------------------------------
		static bool AppendShapedLine(FCanvasTextLayout& Layout, const std::vector<UniChar>& Text, FTextRange LineRange,
			const std::vector<INT>& SourceEnds, const std::vector<FTextRange>& UnderlineRanges,
			FNativeTextFace* BaseFace, bool Bold, double PointSize, const FCanvasTextLayoutRequest& Request,
			double& InOutTop, UBOOL bCaptureGlyphs, std::map<uint32_t, FNativeTextFace*>& FallbackCache)
		{
			if (LineRange.length == 0)
			{
				if (!BaseFace || !BaseFace->Face || !BaseFace->Face->size)
					return false;
				const FT_Size_Metrics& Metrics = BaseFace->Face->size->metrics;
				const double Ascent = Metrics.ascender / 64.0;
				const double Descent = -Metrics.descender / 64.0;
				const double Leading = Max(0.0, Metrics.height / 64.0 - (Ascent + Descent));
				const double LineHeight = Ascent + Descent + Leading + static_cast<double>(Request.SpaceY);
				const double NextTop = InOutTop + Max(0.0, LineHeight);
				if (!std::isfinite(Ascent) || !std::isfinite(Descent) || !std::isfinite(Leading) ||
					!std::isfinite(LineHeight) || !std::isfinite(NextTop) ||
					NextTop > static_cast<double>(std::numeric_limits<INT>::max()))
					return false;
				Layout.Ascent = Max(Layout.Ascent, Ascent);
				Layout.Descent = Max(Layout.Descent, Descent);
				InOutTop = NextTop;
				return true;
			}

			const std::vector<FTextRun> Runs = SegmentIntoRuns(Text, LineRange, BaseFace, Bold, PointSize, FallbackCache);
			if (Runs.empty())
				return false;

			std::vector<FShapedRun> ShapedRuns;
			ShapedRuns.reserve(Runs.size());
			for (const FTextRun& Run : Runs)
			{
				FShapedRun Shaped;
				if (!ShapeRun(Text, Run, Shaped))
					return false;
				ShapedRuns.push_back(std::move(Shaped));
			}

			double TotalAdvance = 0.0;
			std::vector<FTextRange> ClusterRanges;
			for (size_t RunIndex = 0; RunIndex < Runs.size(); ++RunIndex)
			{
				CollectRunClusterBoundaries(ShapedRuns[RunIndex].Infos, Runs[RunIndex].Start + Runs[RunIndex].Length, ClusterRanges);
				for (const hb_glyph_position_t& Pos : ShapedRuns[RunIndex].Positions)
					TotalAdvance += Pos.x_advance / 64.0 + static_cast<double>(Request.SpaceX);
			}
			std::sort(ClusterRanges.begin(), ClusterRanges.end(), [](const FTextRange& A, const FTextRange& B)
			{
				return A.location != B.location ? A.location < B.location : A.length < B.length;
			});

			if (!std::isfinite(TotalAdvance) || TotalAdvance > static_cast<double>(std::numeric_limits<INT>::max()))
				return false;

			double Ascent = 0.0, Descent = 0.0, Leading = 0.0;
			for (const FShapedRun& Shaped : ShapedRuns)
			{
				if (!Shaped.Face || !Shaped.Face->Face || !Shaped.Face->Face->size)
					return false;
				const FT_Size_Metrics& Metrics = Shaped.Face->Face->size->metrics;
				const double RunAscent = Metrics.ascender / 64.0;
				const double RunDescent = -Metrics.descender / 64.0;
				Ascent = Max(Ascent, RunAscent);
				Descent = Max(Descent, RunDescent);
				Leading = Max(Leading, Max(0.0, Metrics.height / 64.0 - (RunAscent + RunDescent)));
			}
			const double LineHeight = Ascent + Descent + Leading + static_cast<double>(Request.SpaceY);
			const double NextTop = InOutTop + Max(0.0, LineHeight);
			if (!std::isfinite(Ascent) || !std::isfinite(Descent) || !std::isfinite(Leading) ||
				!std::isfinite(LineHeight) || !std::isfinite(NextTop) ||
				NextTop > static_cast<double>(std::numeric_limits<INT>::max()))
				return false;

			std::vector<FCluster> LineClusters;
			LineClusters.reserve(ClusterRanges.size());
			for (const FTextRange& Range : ClusterRanges)
			{
				const std::ptrdiff_t End = Range.location + Range.length;
				if (End > static_cast<std::ptrdiff_t>(SourceEnds.size()))
					return false;
				FCluster Cluster;
				Cluster.SourceRange = Range;
				Cluster.SourceEnd = SourceEnds[static_cast<size_t>(End - 1)];
				LineClusters.push_back(Cluster);
			}

			const double LineOriginX = Request.bCenter ? -TotalAdvance * 0.5 : 0.0;
			const double Baseline = InOutTop + Ascent;

			if (bCaptureGlyphs)
			{
				double Pen = 0.0;
				std::map<std::ptrdiff_t, double> PenAtSource;
				for (const FShapedRun& Shaped : ShapedRuns)
				{
					for (size_t Index = 0; Index < Shaped.Infos.size(); ++Index)
					{
						const hb_glyph_info_t& Info = Shaped.Infos[Index];
						const hb_glyph_position_t& Pos = Shaped.Positions[Index];
						const FCluster* Cluster = NativeTextClusterAt(LineClusters, static_cast<std::ptrdiff_t>(Info.cluster));
						if (!Cluster)
							return false;

						PenAtSource.emplace(Cluster->SourceRange.location, LineOriginX + Pen);

						FGlyphPlacement Placement;
						Placement.Face = Shaped.Face;
						Placement.Glyph = static_cast<uint32_t>(Info.codepoint);
						const double XAdvance = Pos.x_advance / 64.0 + static_cast<double>(Request.SpaceX);
						const double XOffset = Pos.x_offset / 64.0;
						const double YOffset = Pos.y_offset / 64.0;
						Placement.Position = FTextPoint{LineOriginX + Pen + XOffset, Baseline + YOffset};
						Placement.Advance = FTextSize{XAdvance, 0.0};
						Placement.SourceRange = Cluster->SourceRange;

						const bool FinitePlacement =
							std::isfinite(Placement.Position.x) && std::isfinite(Placement.Position.y) &&
							std::isfinite(Placement.Advance.width) && std::isfinite(Placement.Advance.height);
						if (!FinitePlacement)
							return false;

						if (!FillPlacementFromRender(Placement, Shaped.Face, Info.codepoint))
							return false;

						Layout.Glyphs.push_back(Placement);
						Pen += XAdvance;
					}
				}
				PenAtSource[LineRange.location + LineRange.length] = LineOriginX + Pen;
				AppendShapedUnderlines(Layout, LineRange, UnderlineRanges, PenAtSource, Baseline, BaseFace);
			}

			AppendNativeTextClusters(Layout, ClusterRanges, SourceEnds);

			const INT LineWidth = TotalAdvance > 0 ? static_cast<INT>(std::ceil(TotalAdvance)) : 0;
			Layout.Width = Max(Layout.Width, LineWidth);
			Layout.Ascent = Max(Layout.Ascent, Ascent);
			Layout.Descent = Max(Layout.Descent, Descent);
			InOutTop = NextTop;
			return true;
		}

		// ---------------------------------------------------------------
		// Greedy word-wrap line breaking (replaces CTTypesetterSuggestLine-
		// Break/SuggestClusterBreak).  Shapes the candidate range fresh on
		// every call so cumulative widths are always relative to Cursor.
		// ---------------------------------------------------------------
		struct FBreakCandidate
		{
			std::ptrdiff_t ClusterStart{};
			std::ptrdiff_t ClusterEnd{};
			double CumulativeWidth{};
			bool bWhitespace{};
		};

		static bool ComputeBreakCandidates(const std::vector<UniChar>& Text, FTextRange Range,
			FNativeTextFace* BaseFace, bool Bold, double PointSize, const FCanvasTextLayoutRequest& Request,
			std::map<uint32_t, FNativeTextFace*>& FallbackCache, std::vector<FBreakCandidate>& OutCandidates)
		{
			OutCandidates.clear();
			const std::vector<FTextRun> Runs = SegmentIntoRuns(Text, Range, BaseFace, Bold, PointSize, FallbackCache);
			if (Runs.empty())
				return Range.length == 0;

			struct FGlyphSample { std::ptrdiff_t Cluster; double XAdvance; };
			std::vector<FGlyphSample> Samples;
			for (const FTextRun& Run : Runs)
			{
				FShapedRun Shaped;
				if (!ShapeRun(Text, Run, Shaped))
					return false;
				for (size_t Index = 0; Index < Shaped.Infos.size(); ++Index)
					Samples.push_back(FGlyphSample{
						static_cast<std::ptrdiff_t>(Shaped.Infos[Index].cluster),
						Shaped.Positions[Index].x_advance / 64.0 + static_cast<double>(Request.SpaceX)});
			}
			if (Samples.empty())
				return true;

			std::sort(Samples.begin(), Samples.end(), [](const FGlyphSample& A, const FGlyphSample& B)
			{
				return A.Cluster < B.Cluster;
			});

			std::vector<std::ptrdiff_t> ClusterStarts;
			ClusterStarts.reserve(Samples.size());
			for (const FGlyphSample& Sample : Samples)
				if (ClusterStarts.empty() || ClusterStarts.back() != Sample.Cluster)
					ClusterStarts.push_back(Sample.Cluster);

			double Cumulative = 0.0;
			size_t SampleIndex = 0;
			const std::ptrdiff_t RangeEnd = Range.location + Range.length;
			for (size_t ClusterIndex = 0; ClusterIndex < ClusterStarts.size(); ++ClusterIndex)
			{
				const std::ptrdiff_t Start = ClusterStarts[ClusterIndex];
				const std::ptrdiff_t End = (ClusterIndex + 1 < ClusterStarts.size()) ? ClusterStarts[ClusterIndex + 1] : RangeEnd;
				double ClusterWidth = 0.0;
				while (SampleIndex < Samples.size() && Samples[SampleIndex].Cluster == Start)
				{
					ClusterWidth += Samples[SampleIndex].XAdvance;
					++SampleIndex;
				}
				Cumulative += ClusterWidth;
				const UniChar BaseUnit = Text[static_cast<size_t>(Start)];
				FBreakCandidate Candidate;
				Candidate.ClusterStart = Start;
				Candidate.ClusterEnd = End;
				Candidate.CumulativeWidth = Cumulative;
				Candidate.bWhitespace = (BaseUnit == ' ' || BaseUnit == '\t');
				OutCandidates.push_back(Candidate);
			}
			return true;
		}

		static std::ptrdiff_t SuggestLineBreakLength(const std::vector<FBreakCandidate>& Candidates, std::ptrdiff_t Cursor, double AvailableWidth)
		{
			if (Candidates.empty())
				return 0;

			if (Candidates.back().CumulativeWidth <= AvailableWidth)
				return Candidates.back().ClusterEnd - Cursor;

			std::ptrdiff_t LastFitIndex = -1;
			std::ptrdiff_t LastWhitespaceFitIndex = -1;
			for (size_t Index = 0; Index < Candidates.size(); ++Index)
			{
				if (Candidates[Index].CumulativeWidth > AvailableWidth)
					break;
				LastFitIndex = static_cast<std::ptrdiff_t>(Index);
				if (Candidates[Index].bWhitespace)
					LastWhitespaceFitIndex = static_cast<std::ptrdiff_t>(Index);
			}

			if (LastFitIndex < 0)
				// Not even the first cluster fits: forced break after it so
				// the line-building loop always makes forward progress.
				return Candidates[0].ClusterEnd - Cursor;

			if (LastWhitespaceFitIndex >= 0)
				return Candidates[static_cast<size_t>(LastWhitespaceFitIndex)].ClusterEnd - Cursor;

			return Candidates[static_cast<size_t>(LastFitIndex)].ClusterEnd - Cursor;
		}

		static bool IsSafeNativeTextRequest(const FCanvasTextLayoutRequest& Request)
		{
			// Draw-side Canvas origin never enters the layout request; it is
			// validated at the canvas-request boundary and applied by the device.
			return SafeFloat(Request.TextScale) &&
				SafeFloat(Request.SpaceX) &&
				SafeFloat(Request.SpaceY) &&
				SafeFloat(Request.ClipX) &&
				SafeFloat(Request.ClipY) &&
				SafeFloat(Request.Color.X) &&
				SafeFloat(Request.Color.Y) &&
				SafeFloat(Request.Color.Z) &&
				SafeFloat(Request.Color.W);
		}

		static bool MakePreprocessedText
		(
			const FCanvasTextLayoutRequest& Request,
			std::vector<UniChar>& OutText,
			std::vector<INT>& OutSourceEnds,
			std::vector<FTextRange>& OutUnderlines
		)
		{
			OutText.clear();
			OutSourceEnds.clear();
			OutUnderlines.clear();
			if (Request.TextLength < 0 || !IsSafeNativeTextRequest(Request))
				return false;
			if (!Request.Text || !Request.Font)
				return false;

			const INT TextLength = Request.TextLength;
			for (INT Index = 0; Index < TextLength && Request.Text[Index]; ++Index)
			{
				const uint32_t Scalar = static_cast<uint32_t>(static_cast<TCHARU>(Request.Text[Index]));
				if (Request.bHandleAmpersand && Scalar == static_cast<uint32_t>('&'))
				{
					if (Index + 1 >= TextLength || !Request.Text[Index + 1])
						break;

					const uint32_t NextScalar = static_cast<uint32_t>(static_cast<TCHARU>(Request.Text[Index + 1]));
					const std::ptrdiff_t OutputStart = static_cast<std::ptrdiff_t>(OutText.size());
					AppendUTF16Scalar(NextScalar, OutText, OutSourceEnds, Index + 2);
					if (NextScalar != static_cast<uint32_t>('&'))
						OutUnderlines.push_back(FTextRange{OutputStart, static_cast<std::ptrdiff_t>(OutText.size()) - OutputStart});
					++Index;
					continue;
				}

				AppendUTF16Scalar(Scalar, OutText, OutSourceEnds, Index + 1);
			}
			return true;
		}

		static bool BuildShapedLayout(FCanvasTextLayout& Layout, const FCanvasTextLayoutRequest& Request,
			const std::vector<UniChar>& Text, const std::vector<INT>& SourceEnds, const std::vector<FTextRange>& UnderlineRanges,
			FNativeTextFace* BaseFace, bool Bold, double PointSize, UBOOL bCaptureGlyphs)
		{
			if (Text.empty())
				return true;

			std::map<uint32_t, FNativeTextFace*> FallbackCache;

			const std::ptrdiff_t TextLength = static_cast<std::ptrdiff_t>(Text.size());
			const bool bWrap = Request.Mode == CanvasLayout_Wrapped;
			const double AvailableWidth = Request.bCenter
				? static_cast<double>(Request.ClipX)
				: static_cast<double>(Request.ClipX) - static_cast<double>(Request.StartX);

			UBOOL Success = 1;
			double Top = 0.0;
			std::ptrdiff_t LineStart = 0;
			while (Success && LineStart < TextLength)
			{
				std::ptrdiff_t ParagraphEnd = LineStart;
				while (ParagraphEnd < TextLength)
				{
					const UniChar Character = Text[static_cast<size_t>(ParagraphEnd)];
					if (Character == '\n' || Character == '\r')
						break;
					++ParagraphEnd;
				}

				std::ptrdiff_t Cursor = LineStart;
				if (Cursor == ParagraphEnd)
					Success = AppendShapedLine(Layout, Text, FTextRange{Cursor, 0}, SourceEnds, UnderlineRanges, BaseFace, Bold, PointSize, Request, Top, bCaptureGlyphs, FallbackCache);
				while (Success && Cursor < ParagraphEnd)
				{
					std::ptrdiff_t Length = ParagraphEnd - Cursor;
					if (bWrap)
					{
						if (AvailableWidth <= 0)
						{
							Success = 0;
							break;
						}
						std::vector<FBreakCandidate> Candidates;
						if (!ComputeBreakCandidates(Text, FTextRange{Cursor, ParagraphEnd - Cursor}, BaseFace, Bold, PointSize, Request, FallbackCache, Candidates))
						{
							Success = 0;
							break;
						}
						const std::ptrdiff_t SuggestedLength = SuggestLineBreakLength(Candidates, Cursor, AvailableWidth);
						if (SuggestedLength <= 0)
						{
							Success = 0;
							break;
						}
						Length = Min(Length, SuggestedLength);
					}
					Success = AppendShapedLine(Layout, Text, FTextRange{Cursor, Length}, SourceEnds, UnderlineRanges, BaseFace, Bold, PointSize, Request, Top, bCaptureGlyphs, FallbackCache);
					Cursor += Length;
				}

				if (Success && ParagraphEnd < TextLength)
				{
					std::ptrdiff_t DelimiterEnd = ParagraphEnd + 1;
					if (Text[static_cast<size_t>(ParagraphEnd)] == '\r' && DelimiterEnd < TextLength && Text[static_cast<size_t>(DelimiterEnd)] == '\n')
						++DelimiterEnd;
					std::vector<FTextRange> Delimiter;
					Delimiter.push_back(FTextRange{ParagraphEnd, DelimiterEnd - ParagraphEnd});
					AppendNativeTextClusters(Layout, Delimiter, SourceEnds);
					LineStart = DelimiterEnd;
					if (LineStart == TextLength)
						Success = AppendShapedLine(Layout, Text, FTextRange{LineStart, 0}, SourceEnds, UnderlineRanges, BaseFace, Bold, PointSize, Request, Top, bCaptureGlyphs, FallbackCache);
				}
				else
					LineStart = ParagraphEnd;
			}

			if (Success)
				Layout.Height = Max(0, static_cast<INT>(std::ceil(Top)));
			return Success != 0;
		}

		static std::ptrdiff_t NativeTextVisibleEnd(const FCanvasTextLayout& Layout, INT VisibleSourceCharacters, std::ptrdiff_t FullLength)
		{
			if (VisibleSourceCharacters <= 0)
				return FullLength;

			std::ptrdiff_t AllowedLength = 0;
			for (const FCluster& Cluster : Layout.Clusters)
			{
				const std::ptrdiff_t ClusterEnd = Cluster.SourceRange.location + Cluster.SourceRange.length;
				if (Cluster.SourceRange.location > AllowedLength || Cluster.SourceEnd > VisibleSourceCharacters)
					break;
				AllowedLength = Max(AllowedLength, ClusterEnd);
			}
			return AllowedLength;
		}
	}

	bool SafeFloat(FLOAT Value)
	{
		const double WideValue = static_cast<double>(Value);
		return std::isfinite(WideValue) &&
			WideValue >= static_cast<double>(std::numeric_limits<INT>::min()) &&
			WideValue <= static_cast<double>(std::numeric_limits<INT>::max());
	}

	bool ProbeAvailability(const char*& OutReasonCode)
	{
		EnsureFontStack();
		if (!GFontStackOk)
		{
			OutReasonCode = "text.fonts_unavailable";
			return false;
		}

		FNativeTextFace* RoleFace = CreateRoleFont(NTROLE_Body, 12.0);
		if (!RoleFace)
		{
			OutReasonCode = "text.fonts_unavailable";
			return false;
		}

		const FT_UInt GlyphIndex = FT_Get_Char_Index(RoleFace->Face, static_cast<FT_ULong>('A'));
		if (GlyphIndex == 0 || FT_Load_Glyph(RoleFace->Face, GlyphIndex, FT_LOAD_DEFAULT | FT_LOAD_RENDER) != 0)
		{
			OutReasonCode = "text.rasterizer_unavailable";
			return false;
		}

		OutReasonCode = "text.ready";
		return true;
	}

	ENativeTextRole RoleForFont(const UFont* Font)
	{
		if (!Font)
			return NTROLE_Body;

		const TCHAR* Name = Font->GetName();
		if (NameEquals(Name, TEXT("TinyInkFont")))
			return NTROLE_Tiny;
		if (NameEquals(Name, TEXT("SmallInkFont")) || NameEquals(Name, TEXT("Tahoma10")))
			return NTROLE_Small;
		if (NameEquals(Name, TEXT("MedInkFont")))
			return NTROLE_Body;
		if (NameEquals(Name, TEXT("BigInkFont")) || NameEquals(Name, TEXT("Tahoma20")) || NameEquals(Name, TEXT("Tahoma30")))
			return NTROLE_Heading;
		if (NameEquals(Name, TEXT("HugeInkFont")) || NameEquals(Name, TEXT("TahomaB20")) || NameEquals(Name, TEXT("TahomaB30")))
			return NTROLE_HeadingBold;
		if (NameEquals(Name, TEXT("TahomaB10")))
			return NTROLE_BodyBold;
		if (NameEquals(Name, TEXT("FontHPMenuMedium")))
			return NTROLE_HPMenuMedium;
		if (NameEquals(Name, TEXT("FontHPMenuLarge")))
			return NTROLE_HPMenuLarge;
		return Font->FontName.Len() > 0 ? NTROLE_Console : NTROLE_Body;
	}

	FCanvasTextLayout* CreateLayout(const FCanvasTextLayoutRequest& Request)
	{
		std::vector<UniChar> Text;
		std::vector<INT> SourceEnds;
		std::vector<FTextRange> UnderlineRanges;
		if (!MakePreprocessedText(Request, Text, SourceEnds, UnderlineRanges))
			return NULL;

		const ENativeTextRole Role = RoleForFont(Request.Font);
		FNativeTextFace* BaseFace = CreateRequestFont(Request, Role);
		if (!BaseFace)
			return NULL;
		const bool Bold = RoleIsBold(Role) != 0;
		const double PointSize = BaseFace->PointSize;

		if (!Text.empty() && Request.VisibleSourceCharacters > 0 && !SourceEnds.empty() &&
			Request.VisibleSourceCharacters < SourceEnds.back())
		{
			FCanvasTextLayout Probe;
			if (!BuildShapedLayout(Probe, Request, Text, SourceEnds, UnderlineRanges, BaseFace, Bold, PointSize, 0))
				return NULL;
			const std::ptrdiff_t VisibleEnd = NativeTextVisibleEnd(Probe, Request.VisibleSourceCharacters, static_cast<std::ptrdiff_t>(Text.size()));
			if (VisibleEnd < static_cast<std::ptrdiff_t>(Text.size()))
			{
				Text.resize(static_cast<size_t>(VisibleEnd));
				SourceEnds.resize(static_cast<size_t>(VisibleEnd));
				UnderlineRanges.erase(
					std::remove_if(UnderlineRanges.begin(), UnderlineRanges.end(), [VisibleEnd](const FTextRange& Range)
					{
						return Range.location + Range.length > VisibleEnd;
					}),
					UnderlineRanges.end()
				);
			}
		}

		FCanvasTextLayout* Layout = new FCanvasTextLayout;
		Layout->PolyFlags = Request.PolyFlags;
		Layout->Color = Request.Color;
		Layout->ClipX = Request.ClipX;
		Layout->ClipY = Request.ClipY;
		Layout->StartX = Request.StartX;
		Layout->StartY = Request.StartY;
		Layout->bClip = Request.bClip;
		Layout->bCenter = Request.bCenter;

		const UBOOL Success = BuildShapedLayout(*Layout, Request, Text, SourceEnds, UnderlineRanges, BaseFace, Bold, PointSize, 1);
		if (!Success)
		{
			delete Layout;
			return NULL;
		}
		return Layout;
	}

	void DestroyLayout(FCanvasTextLayout* Layout)
	{
		if (!IsLayout(Layout))
			return;
		// Faces are process-owned (GFaceCache); only the layout is freed.
		Layout->Magic = 0;
		delete Layout;
	}

	bool MeasureLayout(const FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight)
	{
		OutWidth = 0;
		OutHeight = 0;
		if (!IsLayout(Layout))
			return false;
		OutWidth = Layout->Width;
		OutHeight = Layout->Height;
		return true;
	}

	bool IsLayout(const FCanvasTextLayout* Layout)
	{
		return Layout && Layout->Magic == LayoutMagic;
	}

	bool CalculateRasterRectangle(const FGlyphPlacement& Placement, int PageSize, FRasterRectangle& OutRectangle)
	{
		if (PageSize <= Padding * 2 || !Placement.bDrawable ||
			!std::isfinite(Placement.Bounds.MinX) || !std::isfinite(Placement.Bounds.MinY) ||
			!std::isfinite(Placement.Bounds.MaxX) || !std::isfinite(Placement.Bounds.MaxY))
			return false;
		const INT MaxGlyphDimension = PageSize - Padding * 2;

		const double MinX = std::floor(Placement.Bounds.MinX);
		const double MinY = std::floor(Placement.Bounds.MinY);
		const double MaxX = std::ceil(Placement.Bounds.MaxX);
		const double MaxY = std::ceil(Placement.Bounds.MaxY);
		const double Width = MaxX - MinX;
		const double Height = MaxY - MinY;
		if (!std::isfinite(Width) || !std::isfinite(Height) ||
			Width <= 0 || Height <= 0 ||
			Width > static_cast<double>(MaxGlyphDimension) ||
			Height > static_cast<double>(MaxGlyphDimension))
			return false;

		OutRectangle.MinX = MinX;
		OutRectangle.MinY = MinY;
		OutRectangle.Width = static_cast<INT>(Width);
		OutRectangle.Height = static_cast<INT>(Height);
		return true;
	}

	bool RasterizeGlyph(const FGlyphPlacement& Placement, int PageSize, FRasterizedGlyph& OutRaster)
	{
		FRasterRectangle Rectangle;
		if (!CalculateRasterRectangle(Placement, PageSize, Rectangle))
			return false;
		if (!Placement.Face || !Placement.Face->Face)
			return false;

		const FT_Face FtFace = Placement.Face->Face;
		if (FT_Load_Glyph(FtFace, Placement.Key.Glyph, FT_LOAD_DEFAULT | FT_LOAD_RENDER) != 0)
			return false;
		const FT_GlyphSlot Slot = FtFace->glyph;
		if (Slot->bitmap.pixel_mode != FT_PIXEL_MODE_GRAY)
			return false;

		OutRaster.Width = Rectangle.Width;
		OutRaster.Height = Rectangle.Height;
		if (static_cast<INT>(Slot->bitmap.width) != OutRaster.Width || static_cast<INT>(Slot->bitmap.rows) != OutRaster.Height)
			return false;

		OutRaster.Alpha.resize(static_cast<size_t>(OutRaster.Width) * static_cast<size_t>(OutRaster.Height));
		const int Pitch = Slot->bitmap.pitch;
		const int AbsPitch = Pitch < 0 ? -Pitch : Pitch;
		const BYTE* Base = Slot->bitmap.buffer;
		for (INT Row = 0; Row < OutRaster.Height; ++Row)
		{
			// FreeType's row 0 is the top of the ink box for a positive
			// pitch; a negative pitch means the buffer is stored bottom-up.
			// Either way, output row 0 must be the top -- the orientation
			// the atlas uploader and GlyphDrawOffset's top-left anchor
			// already assume.
			const BYTE* SourceRow = (Pitch >= 0)
				? Base + static_cast<std::ptrdiff_t>(Row) * AbsPitch
				: Base + static_cast<std::ptrdiff_t>(OutRaster.Height - 1 - Row) * AbsPitch;
			BYTE* DestRow = OutRaster.Alpha.data() + static_cast<size_t>(Row) * static_cast<size_t>(OutRaster.Width);
			for (INT Col = 0; Col < OutRaster.Width; ++Col)
				DestRow[Col] = SourceRow[Col];
		}
		return true;
	}

	void GlyphDrawOffset(const FGlyphPlacement& Placement, double& OutLeft, double& OutTop)
	{
		OutLeft = Placement.Position.x + std::floor(Placement.Bounds.MinX);
		OutTop  = Placement.Position.y - std::ceil(Placement.Bounds.MaxY);
	}
}

void BeginNativeTextRuntimeSmoke()
{
	Hp2NativeText::GNativeTextRuntimeSmokeState = NativeTextRuntimeSmokePending;
	Hp2NativeText::GNativeTextRuntimeSmokeStage = NativeTextRuntimeSmokeStageNone;
}

ENativeTextRuntimeSmokeState GetNativeTextRuntimeSmokeState()
{
	return Hp2NativeText::GNativeTextRuntimeSmokeState;
}
ENativeTextRuntimeSmokeStage GetNativeTextRuntimeSmokeStage()
{
	return Hp2NativeText::GNativeTextRuntimeSmokeStage;
}

void SetNativeTextRuntimeSmokeStage(ENativeTextRuntimeSmokeStage Stage)
{
	Hp2NativeText::GNativeTextRuntimeSmokeStage = Stage;
}

void CompleteNativeTextRuntimeSmoke(UBOOL Success)
{
	Hp2NativeText::GNativeTextRuntimeSmokeState = Success ? NativeTextRuntimeSmokePassed : NativeTextRuntimeSmokeFailed;
}

UBOOL NativeTextCopyUTF16ForTests(const TCHAR* Text, INT TextLength, UNICHAR* OutText, INT OutCapacity, INT& OutLength)
{
	const std::vector<std::uint16_t> Converted = Hp2NativeText::UTF16FromTCHAR(Text, TextLength);
	OutLength = static_cast<INT>(Converted.size());
	if (OutCapacity < OutLength || (OutLength > 0 && !OutText))
		return 0;
	for (INT Index = 0; Index < OutLength; ++Index)
		OutText[Index] = static_cast<UNICHAR>(Converted[static_cast<size_t>(Index)]);
	return 1;
}

UBOOL NativeTextInspectLayoutForTests(const FCanvasTextLayout* Layout, FNativeTextLayoutTestInfo& OutInfo, INT* OutClusterSourceEnds, INT OutCapacity)
{
	OutInfo = {};
	if (!Layout || Layout->Magic != Hp2NativeText::LayoutMagic || OutCapacity < static_cast<INT>(Layout->Clusters.size()) ||
		(!OutClusterSourceEnds && !Layout->Clusters.empty()))
		return 0;

	OutInfo.Width = Layout->Width;
	OutInfo.Height = Layout->Height;
	OutInfo.ClusterCount = static_cast<INT>(Layout->Clusters.size());
	OutInfo.GlyphKeyCount = static_cast<INT>(Layout->Glyphs.size());
	OutInfo.UnderlineCount = static_cast<INT>(Layout->Underlines.size());

	std::unordered_set<std::string> Fonts;
	uint64_t Fingerprint = 1469598103934665603ull;
	auto Mix = [&Fingerprint](uint64_t Value)
	{
		for (INT Byte = 0; Byte < 8; ++Byte)
		{
			Fingerprint ^= (Value >> (Byte * 8)) & 0xffu;
			Fingerprint *= 1099511628211ull;
		}
	};
	for (const Hp2NativeText::FCluster& Cluster : Layout->Clusters)
	{
		OutInfo.UTF16Length = Max(OutInfo.UTF16Length, static_cast<INT>(Cluster.SourceRange.location + Cluster.SourceRange.length));
		OutInfo.VisibleSourceEnd = Max(OutInfo.VisibleSourceEnd, Cluster.SourceEnd);
		OutClusterSourceEnds[&Cluster - Layout->Clusters.data()] = Cluster.SourceEnd;
	}
	for (const Hp2NativeText::FGlyphPlacement& Glyph : Layout->Glyphs)
	{
		Fonts.insert(Glyph.Key.FontIdentity);
		for (const char Character : Glyph.Key.FontIdentity)
			Mix(static_cast<unsigned char>(Character));
		Mix(static_cast<uint64_t>(Glyph.Key.Glyph));
		Mix(static_cast<uint64_t>(Glyph.Key.RasterPointSize64));
		Mix(static_cast<uint64_t>(Glyph.Key.Antialias));
		Mix(static_cast<uint64_t>(Glyph.Key.Subpixel));
	}
	Fonts.erase(std::string());
	OutInfo.ResolvedFontCount = static_cast<INT>(Fonts.size());
	OutInfo.GlyphKeyFingerprint = static_cast<QWORD>(Fingerprint);
	return 1;
}
