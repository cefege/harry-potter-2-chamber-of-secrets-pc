/*=============================================================================
	NativeText.cpp: CoreText-backed Canvas layout and XOpenGL glyph atlas.
=============================================================================*/

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>

#include "XOpenGLDrv.h"
#include "XOpenGL.h"

#if !HP2_HAS_NATIVE_TEXT_BACKEND
#error NativeText.cpp must only be compiled when HP2_HAS_NATIVE_TEXT_BACKEND is enabled.
#endif

#include <CoreGraphics/CoreGraphics.h>
#include <CoreText/CoreText.h>

namespace
{
	static ENativeTextRuntimeSmokeState GNativeTextRuntimeSmokeState = NativeTextRuntimeSmokeIdle;
	static ENativeTextRuntimeSmokeStage GNativeTextRuntimeSmokeStage = NativeTextRuntimeSmokeStageNone;
	constexpr INT NativeTextAtlasPageLimit = 4;
	constexpr INT NativeTextAtlasPadding = 1;
	constexpr DWORD NativeTextLayoutMagic = 0x4E545854; // NTXT

	struct FNativeTextGlyphKey
	{
		std::string FontIdentity;
		CGGlyph Glyph{};
		uint32_t RasterPointSize64{};
		UBOOL Antialias{};
		UBOOL Subpixel{};

		bool operator==(const FNativeTextGlyphKey& Other) const
		{
			return Glyph == Other.Glyph &&
				RasterPointSize64 == Other.RasterPointSize64 &&
				Antialias == Other.Antialias &&
				Subpixel == Other.Subpixel &&
				FontIdentity == Other.FontIdentity;
		}
	};

	struct FNativeTextGlyphKeyHash
	{
		size_t operator()(const FNativeTextGlyphKey& Key) const
		{
			size_t Result = std::hash<std::string>{}(Key.FontIdentity);
			Result ^= static_cast<size_t>(Key.Glyph) + 0x9e3779b9u + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.RasterPointSize64) + 0x9e3779b9u + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.Antialias) + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.Subpixel) + (Result << 6) + (Result >> 2);
			return Result;
		}
	};

	struct FNativeTextGlyphPlacement
	{
		CTFontRef Font{};
		CGGlyph Glyph{};
		CGPoint Position{};
		CGSize Advance{};
		CGRect Bounds{};
		CFRange SourceRange{};
		FNativeTextGlyphKey Key;
		UBOOL bDrawable{};
	};

	struct FNativeTextUnderline
	{
		CGFloat Start{};
		CGFloat End{};
		CGFloat Baseline{};
		CGFloat Position{};
		CGFloat Thickness{};
	};

	struct FNativeTextCluster
	{
		CFRange SourceRange{};
		INT SourceEnd{};
	};

	struct FNativeTextAtlasGlyph
	{
		INT PageIndex{INDEX_NONE};
		INT X{};
		INT Y{};
		INT Width{};
		INT Height{};
	};

	struct FNativeTextAtlasPage
	{
		GLuint Texture{};
		GLuint Sampler{};
		GLuint64 BindlessTextureHandle{};
		INT Size{};
		INT NextX{};
		INT NextY{};
		INT RowHeight{};
		uint64_t LastUse{};
	};

	struct FRasterizedGlyph
	{
		INT Width{};
		INT Height{};
		std::vector<BYTE> Alpha;
	};

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

		for (INT Index = 0; TextLength < 0 || Index < TextLength; ++Index)
		{
			if (!Text[Index])
				break;
			AppendUTF16Scalar(static_cast<uint32_t>(static_cast<TCHARU>(Text[Index])), Result);
		}
		return Result;
	}

	static std::string UTF8FromCFString(CFStringRef String)
	{
		if (!String)
			return std::string();

		const CFIndex Length = CFStringGetLength(String);
		if (Length == 0)
			return std::string();

		const CFIndex Capacity = CFStringGetMaximumSizeForEncoding(Length, kCFStringEncodingUTF8) + 1;
		std::vector<char> Buffer(static_cast<size_t>(Capacity));
		if (!CFStringGetCString(String, Buffer.data(), Capacity, kCFStringEncodingUTF8))
			return std::string();
		return std::string(Buffer.data());
	}

	static std::string FontIdentity(CTFontRef Font)
	{
		CTFontDescriptorRef Descriptor = CTFontCopyFontDescriptor(Font);
		CFTypeRef DescriptorName = Descriptor ? CTFontDescriptorCopyAttribute(Descriptor, kCTFontNameAttribute) : NULL;
		CFStringRef PostScriptName = CTFontCopyPostScriptName(Font);
		const CGAffineTransform Transform = CTFontGetMatrix(Font);

		std::string Identity = UTF8FromCFString(PostScriptName);
		Identity += "|";
		if (DescriptorName && CFGetTypeID(DescriptorName) == CFStringGetTypeID())
			Identity += UTF8FromCFString(static_cast<CFStringRef>(DescriptorName));
		Identity += "|" + std::to_string(CTFontGetSize(Font));
		Identity += "|" + std::to_string(Transform.a);
		Identity += "," + std::to_string(Transform.b);
		Identity += "," + std::to_string(Transform.c);
		Identity += "," + std::to_string(Transform.d);
		Identity += "," + std::to_string(Transform.tx);
		Identity += "," + std::to_string(Transform.ty);

		if (PostScriptName)
			CFRelease(PostScriptName);
		if (DescriptorName)
			CFRelease(DescriptorName);
		if (Descriptor)
			CFRelease(Descriptor);
		return Identity;
	}

	static bool MakeGlyphKey(CTFontRef Font, CGGlyph Glyph, FNativeTextGlyphKey& OutKey)
	{
		const double RasterPointSize64 = static_cast<double>(CTFontGetSize(Font)) * 64.0;
		if (!std::isfinite(RasterPointSize64) || RasterPointSize64 <= 0.0 ||
			RasterPointSize64 > static_cast<double>(std::numeric_limits<uint32_t>::max()) - 0.5)
			return false;

		OutKey.FontIdentity = FontIdentity(Font);
		OutKey.Glyph = Glyph;
		OutKey.RasterPointSize64 = static_cast<uint32_t>(std::llround(RasterPointSize64));
		OutKey.Antialias = 1;
		OutKey.Subpixel = 1;
		return true;
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

	static CTFontRef CreateNamedFont(const TCHAR* FamilyName, CGFloat PointSize)
	{
		if (!std::isfinite(PointSize) || PointSize <= 0 ||
			PointSize > static_cast<CGFloat>(std::numeric_limits<uint32_t>::max()) / 64)
			return NULL;

		const std::vector<UniChar> UTF16Name = UTF16FromTCHAR(FamilyName, -1);
		if (UTF16Name.empty())
			return NULL;

		CFStringRef Name = CFStringCreateWithCharacters(kCFAllocatorDefault, UTF16Name.data(), static_cast<CFIndex>(UTF16Name.size()));
		if (!Name)
			return NULL;
		CTFontRef Font = CTFontCreateWithName(Name, PointSize, NULL);
		CFRelease(Name);
		if (!Font)
			return NULL;

		CFStringRef PostScriptName = CTFontCopyPostScriptName(Font);
		const bool IsLastResort = PostScriptName && CFStringCompare(PostScriptName, CFSTR(".LastResort"), 0) == kCFCompareEqualTo;
		if (PostScriptName)
			CFRelease(PostScriptName);
		if (IsLastResort)
		{
			CFRelease(Font);
			return NULL;
		}
		return Font;
	}

	static CTFontRef CreateRoleFont(ENativeTextRole Role, CGFloat PointSize)
	{
		if (!std::isfinite(PointSize) || PointSize <= 0 ||
			PointSize > static_cast<CGFloat>(std::numeric_limits<uint32_t>::max()) / 64)
			return NULL;

		const bool Bold = RoleIsBold(Role);
		CTFontRef Font = CreateNamedFont(Bold ? TEXT("Times New Roman Bold") : TEXT("Times New Roman"), PointSize);
		if (!Font)
			Font = CreateNamedFont(Bold ? TEXT("Times-Bold") : TEXT("Times-Roman"), PointSize);
		if (!Font)
			Font = CTFontCreateUIFontForLanguage(Bold ? kCTFontUIFontEmphasizedSystem : kCTFontUIFontSystem, PointSize, NULL);
		return Font;
	}

	static CTFontRef CreateRequestFont(const FCanvasTextRequest& Request, ENativeTextRole Role)
	{
		if (!Request.Font)
			return NULL;

		const CGFloat Scale = Request.TextScale > 0.f ? static_cast<CGFloat>(Request.TextScale) : 1.f;
		if (Request.Font->FontName.Len() > 0)
		{
			const INT RequestedHeight = Request.Font->FontHeight > 0 ? Request.Font->FontHeight : RolePointSize(NTROLE_Console);
			CTFontRef Font = CreateNamedFont(*Request.Font->FontName, static_cast<CGFloat>(RequestedHeight) * Scale);
			if (Font)
				return Font;
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
		return CreateRoleFont(Role, static_cast<CGFloat>(PointSize) * Scale);
	}

	static bool IsSafeNativeTextFloat(FLOAT Value)
	{
		const double WideValue = static_cast<double>(Value);
		return std::isfinite(WideValue) &&
			WideValue >= static_cast<double>(std::numeric_limits<INT>::min()) &&
			WideValue <= static_cast<double>(std::numeric_limits<INT>::max());
	}

	static bool IsSafeNativeTextRequest(const FCanvasTextRequest& Request)
	{
		return IsSafeNativeTextFloat(Request.TextScale) &&
			IsSafeNativeTextFloat(Request.SpaceX) &&
			IsSafeNativeTextFloat(Request.SpaceY) &&
			IsSafeNativeTextFloat(Request.OriginX) &&
			IsSafeNativeTextFloat(Request.OriginY) &&
			IsSafeNativeTextFloat(Request.ClipX) &&
			IsSafeNativeTextFloat(Request.ClipY) &&
			IsSafeNativeTextFloat(Request.Color.X) &&
			IsSafeNativeTextFloat(Request.Color.Y) &&
			IsSafeNativeTextFloat(Request.Color.Z) &&
			IsSafeNativeTextFloat(Request.Color.W);
	}

	static bool MakePreprocessedText
	(
		const FCanvasTextRequest& Request,
		std::vector<UniChar>& OutText,
		std::vector<INT>& OutSourceEnds,
		std::vector<CFRange>& OutUnderlines
	)
	{
		OutText.clear();
		OutSourceEnds.clear();
		OutUnderlines.clear();
		if (Request.TextLength == std::numeric_limits<INT>::min() ||
			!IsSafeNativeTextRequest(Request))
			return false;
		if (!Request.Text || !Request.Font)
			return false;

		const INT TextLength = Request.TextLength < 0 ? -Request.TextLength : Request.TextLength;
		for (INT Index = 0; Index < TextLength && Request.Text[Index]; ++Index)
		{
			const uint32_t Scalar = static_cast<uint32_t>(static_cast<TCHARU>(Request.Text[Index]));
			if (Request.bHandleAmpersand && Scalar == static_cast<uint32_t>('&'))
			{
				if (Index + 1 >= TextLength || !Request.Text[Index + 1])
					break;

				const uint32_t NextScalar = static_cast<uint32_t>(static_cast<TCHARU>(Request.Text[Index + 1]));
				const CFIndex OutputStart = static_cast<CFIndex>(OutText.size());
				AppendUTF16Scalar(NextScalar, OutText, OutSourceEnds, Index + 2);
				if (NextScalar != static_cast<uint32_t>('&'))
					OutUnderlines.push_back(CFRangeMake(OutputStart, static_cast<CFIndex>(OutText.size()) - OutputStart));
				++Index;
				continue;
			}

			AppendUTF16Scalar(Scalar, OutText, OutSourceEnds, Index + 1);
		}
		return true;
	}

	static void ReleaseGlyphFonts(std::vector<FNativeTextGlyphPlacement>& Glyphs)
	{
		for (FNativeTextGlyphPlacement& Glyph : Glyphs)
		{
			if (Glyph.Font)
				CFRelease(Glyph.Font);
			Glyph.Font = NULL;
		}
	}
}

struct FCanvasTextLayout
{
	DWORD Magic{NativeTextLayoutMagic};
	INT Width{};
	INT Height{};
	CGFloat Ascent{};
	CGFloat Descent{};
	DWORD PolyFlags{};
	FPlane Color{};
	FLOAT OriginX{};
	FLOAT OriginY{};
	FLOAT ClipX{};
	FLOAT ClipY{};
	INT StartX{};
	INT StartY{};
	UBOOL bClip{};
	UBOOL bCenter{};
	uint64_t AtlasGeneration{};
	std::vector<FNativeTextGlyphPlacement> Glyphs;
	std::vector<FNativeTextUnderline> Underlines;
	std::vector<FNativeTextCluster> Clusters;
};

namespace
{
	static CFMutableAttributedStringRef CreateNativeTextAttributes(CFStringRef Source, CTFontRef Font, FLOAT SpaceX)
	{
		CFMutableAttributedStringRef Attributes = CFAttributedStringCreateMutable(kCFAllocatorDefault, 0);
		if (!Attributes)
			return NULL;

		const CFRange FullRange = CFRangeMake(0, CFStringGetLength(Source));
		CFAttributedStringReplaceString(Attributes, CFRangeMake(0, 0), Source);
		if (FullRange.length)
			CFAttributedStringSetAttribute(Attributes, FullRange, kCTFontAttributeName, Font);
		if (FullRange.length && SpaceX != 0.f)
		{
			const CGFloat Kerning = static_cast<CGFloat>(SpaceX);
			CFNumberRef KerningNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberCGFloatType, &Kerning);
			if (KerningNumber)
			{
				CFAttributedStringSetAttribute(Attributes, FullRange, kCTKernAttributeName, KerningNumber);
				CFRelease(KerningNumber);
			}
		}
		return Attributes;
	}

	static void AddNativeTextRange(std::vector<CFRange>& Ranges, CFStringRef Source, CFRange LineRange, CFRange Candidate)
	{
		const CFIndex LineEnd = LineRange.location + LineRange.length;
		CFIndex Start = Max(LineRange.location, Candidate.location);
		CFIndex End = Min(LineEnd, Candidate.location + Candidate.length);
		if (Start >= End)
			return;

		const CFRange StartComponent = CFStringGetRangeOfComposedCharactersAtIndex(Source, Start);
		const CFRange EndComponent = CFStringGetRangeOfComposedCharactersAtIndex(Source, End - 1);
		if (StartComponent.location < LineRange.location || EndComponent.location + EndComponent.length > LineEnd)
			return;

		Start = StartComponent.location;
		End = EndComponent.location + EndComponent.length;
		Ranges.push_back(CFRangeMake(Start, End - Start));
	}

	static bool CollectNativeTextLineClusters(CFStringRef Source, CTLineRef Line, CFRange LineRange, std::vector<CFRange>& OutClusters)
	{
		OutClusters.clear();
		const CFIndex LineEnd = LineRange.location + LineRange.length;
		for (CFIndex Index = LineRange.location; Index < LineEnd; )
		{
			const CFRange Component = CFStringGetRangeOfComposedCharactersAtIndex(Source, Index);
			if (Component.length <= 0 || Component.location < LineRange.location || Component.location + Component.length > LineEnd)
				return false;
			OutClusters.push_back(Component);
			Index = Component.location + Component.length;
		}

		const CFArrayRef Runs = CTLineGetGlyphRuns(Line);
		const CFIndex RunCount = Runs ? CFArrayGetCount(Runs) : 0;
		for (CFIndex RunIndex = 0; RunIndex < RunCount; ++RunIndex)
		{
			CTRunRef Run = static_cast<CTRunRef>(CFArrayGetValueAtIndex(Runs, RunIndex));
			const CFRange RunRange = CTRunGetStringRange(Run);
			const CFIndex RunEnd = RunRange.location + RunRange.length;
			if (RunRange.length <= 0 || RunRange.location < LineRange.location || RunEnd > LineEnd)
				continue;

			const CFIndex GlyphCount = CTRunGetGlyphCount(Run);
			std::vector<CFIndex> SourceIndices;
			SourceIndices.reserve(static_cast<size_t>(GlyphCount) + 2);
			SourceIndices.push_back(RunRange.location);
			if (GlyphCount > 0)
			{
				std::vector<CFIndex> GlyphIndices(static_cast<size_t>(GlyphCount));
				CTRunGetStringIndices(Run, CFRangeMake(0, 0), GlyphIndices.data());
				SourceIndices.insert(SourceIndices.end(), GlyphIndices.begin(), GlyphIndices.end());
			}
			SourceIndices.push_back(RunEnd);
			std::sort(SourceIndices.begin(), SourceIndices.end());
			SourceIndices.erase(std::unique(SourceIndices.begin(), SourceIndices.end()), SourceIndices.end());
			for (size_t Index = 1; Index < SourceIndices.size(); ++Index)
				AddNativeTextRange(OutClusters, Source, LineRange, CFRangeMake(SourceIndices[Index - 1], SourceIndices[Index] - SourceIndices[Index - 1]));
		}

		std::sort(OutClusters.begin(), OutClusters.end(), [](const CFRange& A, const CFRange& B)
		{
			return A.location != B.location ? A.location < B.location : A.length < B.length;
		});

		std::vector<CFRange> Merged;
		for (const CFRange& Range : OutClusters)
		{
			if (Merged.empty())
			{
				Merged.push_back(Range);
				continue;
			}
			CFRange& Previous = Merged.back();
			const CFIndex PreviousEnd = Previous.location + Previous.length;
			const CFIndex RangeEnd = Range.location + Range.length;
			if (Range.location < PreviousEnd)
				Previous.length = Max(PreviousEnd, RangeEnd) - Previous.location;
			else if (Range.location > PreviousEnd || RangeEnd > PreviousEnd)
				Merged.push_back(Range);
		}
		OutClusters.swap(Merged);
		return true;
	}

	static void AppendNativeTextClusters(FCanvasTextLayout& Layout, const std::vector<CFRange>& Ranges, const std::vector<INT>& SourceEnds)
	{
		for (const CFRange& Range : Ranges)
		{
			const CFIndex End = Range.location + Range.length;
			if (Range.length <= 0 || End > static_cast<CFIndex>(SourceEnds.size()))
				continue;
			if (!Layout.Clusters.empty())
			{
				const FNativeTextCluster& Previous = Layout.Clusters.back();
				if (Previous.SourceRange.location == Range.location && Previous.SourceRange.length == Range.length)
					continue;
			}
			FNativeTextCluster Cluster;
			Cluster.SourceRange = Range;
			Cluster.SourceEnd = SourceEnds[static_cast<size_t>(End - 1)];
			Layout.Clusters.push_back(Cluster);
		}
	}

	static const FNativeTextCluster* NativeTextClusterAt(const std::vector<FNativeTextCluster>& Clusters, CFIndex SourceIndex)
	{
		for (const FNativeTextCluster& Cluster : Clusters)
		{
			const CFIndex End = Cluster.SourceRange.location + Cluster.SourceRange.length;
			if (SourceIndex >= Cluster.SourceRange.location && SourceIndex < End)
				return &Cluster;
		}
		return NULL;
	}

	static bool AppendNativeTextGlyphs(FCanvasTextLayout& Layout, CTLineRef Line, const std::vector<FNativeTextCluster>& LineClusters, CGFloat LineOriginX, CGFloat Baseline)
	{
		const CFArrayRef Runs = CTLineGetGlyphRuns(Line);
		const CFIndex RunCount = Runs ? CFArrayGetCount(Runs) : 0;
		for (CFIndex RunIndex = 0; RunIndex < RunCount; ++RunIndex)
		{
			CTRunRef Run = static_cast<CTRunRef>(CFArrayGetValueAtIndex(Runs, RunIndex));
			const CFDictionaryRef RunAttributes = CTRunGetAttributes(Run);
			CTFontRef RunFont = RunAttributes ? static_cast<CTFontRef>(CFDictionaryGetValue(RunAttributes, kCTFontAttributeName)) : NULL;
			const CFIndex GlyphCount = CTRunGetGlyphCount(Run);
			if (!RunFont || GlyphCount <= 0)
				continue;

			std::vector<CGGlyph> GlyphIDs(static_cast<size_t>(GlyphCount));
			std::vector<CGPoint> Positions(static_cast<size_t>(GlyphCount));
			std::vector<CGSize> Advances(static_cast<size_t>(GlyphCount));
			std::vector<CFIndex> SourceIndices(static_cast<size_t>(GlyphCount));
			CTRunGetGlyphs(Run, CFRangeMake(0, 0), GlyphIDs.data());
			CTRunGetPositions(Run, CFRangeMake(0, 0), Positions.data());
			CTRunGetAdvances(Run, CFRangeMake(0, 0), Advances.data());
			CTRunGetStringIndices(Run, CFRangeMake(0, 0), SourceIndices.data());

			for (CFIndex GlyphIndex = 0; GlyphIndex < GlyphCount; ++GlyphIndex)
			{
				const FNativeTextCluster* Cluster = NativeTextClusterAt(LineClusters, SourceIndices[static_cast<size_t>(GlyphIndex)]);
				if (!Cluster)
					return false;

				FNativeTextGlyphPlacement Placement;
				Placement.Font = static_cast<CTFontRef>(CFRetain(RunFont));
				Placement.Glyph = GlyphIDs[static_cast<size_t>(GlyphIndex)];
				Placement.Position = CGPointMake(LineOriginX + Positions[static_cast<size_t>(GlyphIndex)].x, Baseline + Positions[static_cast<size_t>(GlyphIndex)].y);
				Placement.Advance = Advances[static_cast<size_t>(GlyphIndex)];
				Placement.SourceRange = Cluster->SourceRange;
				CTFontGetBoundingRectsForGlyphs(Placement.Font, kCTFontOrientationDefault, &Placement.Glyph, &Placement.Bounds, 1);

				const bool FinitePlacement =
					std::isfinite(Placement.Position.x) && std::isfinite(Placement.Position.y) &&
					std::isfinite(Placement.Advance.width) && std::isfinite(Placement.Advance.height);
				const bool NullBounds = CGRectIsNull(Placement.Bounds);
				const bool FiniteBounds = NullBounds ||
					(std::isfinite(Placement.Bounds.origin.x) && std::isfinite(Placement.Bounds.origin.y) &&
					 std::isfinite(Placement.Bounds.size.width) && std::isfinite(Placement.Bounds.size.height) &&
					 Placement.Bounds.size.width >= 0 && Placement.Bounds.size.height >= 0);
				if (!FinitePlacement || !FiniteBounds)
				{
					CFRelease(Placement.Font);
					return false;
				}

				Placement.bDrawable = !NullBounds && Placement.Bounds.size.width > 0 && Placement.Bounds.size.height > 0;
				if (Placement.bDrawable && !MakeGlyphKey(Placement.Font, Placement.Glyph, Placement.Key))
				{
					CFRelease(Placement.Font);
					return false;
				}
				Layout.Glyphs.push_back(std::move(Placement));
			}
		}
		return true;
	}

	static void AppendNativeTextUnderlines(FCanvasTextLayout& Layout, CTLineRef Line, CFRange LineRange, const std::vector<CFRange>& UnderlineRanges, CTFontRef BaseFont, CGFloat LineOriginX, CGFloat Baseline)
	{
		const CFIndex LineEnd = LineRange.location + LineRange.length;
		for (const CFRange& Range : UnderlineRanges)
		{
			const CFIndex Start = Max(LineRange.location, Range.location);
			const CFIndex End = Min(LineEnd, Range.location + Range.length);
			if (Start >= End)
				continue;

			const CGFloat OffsetA = CTLineGetOffsetForStringIndex(Line, Start, NULL);
			const CGFloat OffsetB = CTLineGetOffsetForStringIndex(Line, End, NULL);
			if (!std::isfinite(OffsetA) || !std::isfinite(OffsetB) || OffsetA == OffsetB)
				continue;

			FNativeTextUnderline Underline;
			Underline.Start = LineOriginX + Min(OffsetA, OffsetB);
			Underline.End = LineOriginX + Max(OffsetA, OffsetB);
			Underline.Baseline = Baseline;
			Underline.Position = CTFontGetUnderlinePosition(BaseFont);
			Underline.Thickness = Max(1.f, static_cast<FLOAT>(CTFontGetUnderlineThickness(BaseFont)));
			Layout.Underlines.push_back(Underline);
		}
	}

	static bool AppendNativeTextLine(FCanvasTextLayout& Layout, CTTypesetterRef Typesetter, CFStringRef Source, CFRange LineRange, const std::vector<INT>& SourceEnds, const std::vector<CFRange>& UnderlineRanges, CTFontRef BaseFont, const FCanvasTextRequest& Request, CGFloat& InOutTop, UBOOL bCaptureGlyphs)
	{
		if (LineRange.length == 0)
		{
			const CGFloat Ascent = CTFontGetAscent(BaseFont);
			const CGFloat Descent = CTFontGetDescent(BaseFont);
			const CGFloat Leading = CTFontGetLeading(BaseFont);
			const CGFloat LineHeight = Ascent + Descent + Leading + static_cast<CGFloat>(Request.SpaceY);
			const CGFloat NextTop = InOutTop + Max(static_cast<CGFloat>(0), LineHeight);
			if (!std::isfinite(Ascent) || !std::isfinite(Descent) || !std::isfinite(Leading) ||
				!std::isfinite(LineHeight) || !std::isfinite(NextTop) ||
				NextTop > static_cast<CGFloat>(std::numeric_limits<INT>::max()))
				return false;
			Layout.Ascent = Max(Layout.Ascent, Ascent);
			Layout.Descent = Max(Layout.Descent, Descent);
			InOutTop = NextTop;
			return true;
		}

		CTLineRef Line = CTTypesetterCreateLine(Typesetter, LineRange);
		if (!Line)
			return false;

		CGFloat Ascent = 0;
		CGFloat Descent = 0;
		CGFloat Leading = 0;
		const CGFloat Advance = CTLineGetTypographicBounds(Line, &Ascent, &Descent, &Leading);
		const CGFloat LineHeight = Ascent + Descent + Leading + static_cast<CGFloat>(Request.SpaceY);
		const CGFloat NextTop = InOutTop + Max(static_cast<CGFloat>(0), LineHeight);
		if (!std::isfinite(Advance) || !std::isfinite(Ascent) || !std::isfinite(Descent) || !std::isfinite(Leading) ||
			!std::isfinite(LineHeight) || !std::isfinite(NextTop) ||
			Advance > static_cast<CGFloat>(std::numeric_limits<INT>::max()) ||
			NextTop > static_cast<CGFloat>(std::numeric_limits<INT>::max()))
		{
			CFRelease(Line);
			return false;
		}

		std::vector<CFRange> Ranges;
		if (!CollectNativeTextLineClusters(Source, Line, LineRange, Ranges))
		{
			CFRelease(Line);
			return false;
		}

		std::vector<FNativeTextCluster> LineClusters;
		LineClusters.reserve(Ranges.size());
		for (const CFRange& Range : Ranges)
		{
			const CFIndex End = Range.location + Range.length;
			if (End > static_cast<CFIndex>(SourceEnds.size()))
			{
				CFRelease(Line);
				return false;
			}
			FNativeTextCluster Cluster;
			Cluster.SourceRange = Range;
			Cluster.SourceEnd = SourceEnds[static_cast<size_t>(End - 1)];
			LineClusters.push_back(Cluster);
		}

		const CGFloat LineOriginX = Request.bCenter ? -Advance * 0.5 : 0;
		const CGFloat Baseline = InOutTop + Ascent;
		if (bCaptureGlyphs && !AppendNativeTextGlyphs(Layout, Line, LineClusters, LineOriginX, Baseline))
		{
			CFRelease(Line);
			return false;
		}
		AppendNativeTextUnderlines(Layout, Line, LineRange, UnderlineRanges, BaseFont, LineOriginX, Baseline);
		AppendNativeTextClusters(Layout, Ranges, SourceEnds);
		const INT LineWidth = Advance > 0 ? static_cast<INT>(std::ceil(Advance)) : 0;
		Layout.Width = Max(Layout.Width, LineWidth);
		Layout.Ascent = Max(Layout.Ascent, Ascent);
		Layout.Descent = Max(Layout.Descent, Descent);
		InOutTop = NextTop;
		CFRelease(Line);
		return true;
	}

	static bool BuildNativeTextLayout(FCanvasTextLayout& Layout, const FCanvasTextRequest& Request, const std::vector<UniChar>& Text, const std::vector<INT>& SourceEnds, const std::vector<CFRange>& UnderlineRanges, CTFontRef BaseFont, UBOOL bCaptureGlyphs)
	{
		if (Text.empty())
			return true;

		CFStringRef Source = CFStringCreateWithCharacters(kCFAllocatorDefault, Text.data(), static_cast<CFIndex>(Text.size()));
		if (!Source)
			return false;
		CFMutableAttributedStringRef Attributes = CreateNativeTextAttributes(Source, BaseFont, Request.SpaceX);
		if (!Attributes)
		{
			CFRelease(Source);
			return false;
		}
		CTTypesetterRef Typesetter = CTTypesetterCreateWithAttributedString(Attributes);
		if (!Typesetter)
		{
			CFRelease(Attributes);
			CFRelease(Source);
			return false;
		}

		const CFIndex TextLength = static_cast<CFIndex>(Text.size());
		// Canvas uses a negative TextLength only for its private wrapped-layout
		// request.  Keep bClip exclusively for local quad clipping.
		const bool bWrap = Request.TextLength < 0;
		const CGFloat AvailableWidth = Request.bCenter
			? static_cast<CGFloat>(Request.ClipX)
			: static_cast<CGFloat>(Request.ClipX) - static_cast<CGFloat>(Request.StartX);
		UBOOL Success = 1;
		CGFloat Top = 0;
		CFIndex LineStart = 0;
		while (Success && LineStart < TextLength)
		{
			CFIndex ParagraphEnd = LineStart;
			while (ParagraphEnd < TextLength)
			{
				const UniChar Character = Text[static_cast<size_t>(ParagraphEnd)];
				if (Character == '\n' || Character == '\r')
					break;
				++ParagraphEnd;
			}

			CFIndex Cursor = LineStart;
			if (Cursor == ParagraphEnd)
				Success = AppendNativeTextLine(Layout, Typesetter, Source, CFRangeMake(Cursor, 0), SourceEnds, UnderlineRanges, BaseFont, Request, Top, bCaptureGlyphs);
			while (Success && Cursor < ParagraphEnd)
			{
				CFIndex Length = ParagraphEnd - Cursor;
				if (bWrap)
				{
					if (AvailableWidth <= 0)
					{
						Success = 0;
						break;
					}
					CFIndex SuggestedLength = CTTypesetterSuggestLineBreak(Typesetter, Cursor, AvailableWidth);
					if (SuggestedLength <= 0)
						SuggestedLength = CTTypesetterSuggestClusterBreak(Typesetter, Cursor, AvailableWidth);
					if (SuggestedLength <= 0)
					{
						Success = 0;
						break;
					}
					Length = Min(Length, SuggestedLength);
				}
				Success = AppendNativeTextLine(Layout, Typesetter, Source, CFRangeMake(Cursor, Length), SourceEnds, UnderlineRanges, BaseFont, Request, Top, bCaptureGlyphs);
				Cursor += Length;
			}

			if (Success && ParagraphEnd < TextLength)
			{
				CFIndex DelimiterEnd = ParagraphEnd + 1;
				if (Text[static_cast<size_t>(ParagraphEnd)] == '\r' && DelimiterEnd < TextLength && Text[static_cast<size_t>(DelimiterEnd)] == '\n')
					++DelimiterEnd;
				std::vector<CFRange> Delimiter;
				Delimiter.push_back(CFRangeMake(ParagraphEnd, DelimiterEnd - ParagraphEnd));
				AppendNativeTextClusters(Layout, Delimiter, SourceEnds);
				LineStart = DelimiterEnd;
				if (LineStart == TextLength)
					Success = AppendNativeTextLine(Layout, Typesetter, Source, CFRangeMake(LineStart, 0), SourceEnds, UnderlineRanges, BaseFont, Request, Top, bCaptureGlyphs);
			}
			else
				LineStart = ParagraphEnd;
		}

		CFRelease(Typesetter);
		CFRelease(Attributes);
		CFRelease(Source);
		if (Success)
			Layout.Height = Max(0, static_cast<INT>(std::ceil(Top)));
		return Success != 0;
	}

	static CFIndex NativeTextVisibleEnd(const FCanvasTextLayout& Layout, INT VisibleSourceCharacters, CFIndex FullLength)
	{
		if (VisibleSourceCharacters <= 0)
			return FullLength;

		CFIndex AllowedLength = 0;
		for (const FNativeTextCluster& Cluster : Layout.Clusters)
		{
			const CFIndex ClusterEnd = Cluster.SourceRange.location + Cluster.SourceRange.length;
			if (Cluster.SourceRange.location > AllowedLength || Cluster.SourceEnd > VisibleSourceCharacters)
				break;
			AllowedLength = Max(AllowedLength, ClusterEnd);
		}
		return AllowedLength;
	}
}

	static INT NativeTextAtlasPageSize(const UXOpenGLRenderDevice& Renderer)
	{
		return Min(1024, Renderer.MaxTextureSize);
	}

	struct FNativeTextRasterRectangle
	{
		CGFloat MinX{};
		CGFloat MinY{};
		INT Width{};
		INT Height{};
	};

	static bool CalculateNativeTextRasterRectangle(const FNativeTextGlyphPlacement& Placement, INT PageSize, FNativeTextRasterRectangle& OutRectangle)
	{
		if (PageSize <= NativeTextAtlasPadding * 2 || !Placement.bDrawable ||
			!std::isfinite(Placement.Bounds.origin.x) || !std::isfinite(Placement.Bounds.origin.y) ||
			!std::isfinite(Placement.Bounds.size.width) || !std::isfinite(Placement.Bounds.size.height) ||
			Placement.Bounds.size.width <= 0 || Placement.Bounds.size.height <= 0)
			return false;
		const INT MaxGlyphDimension = PageSize - NativeTextAtlasPadding * 2;

		const CGFloat MinX = std::floor(CGRectGetMinX(Placement.Bounds));
		const CGFloat MinY = std::floor(CGRectGetMinY(Placement.Bounds));
		const CGFloat MaxX = std::ceil(CGRectGetMaxX(Placement.Bounds));
		const CGFloat MaxY = std::ceil(CGRectGetMaxY(Placement.Bounds));
		const CGFloat Width = MaxX - MinX;
		const CGFloat Height = MaxY - MinY;
		if (!std::isfinite(MinX) || !std::isfinite(MinY) ||
			!std::isfinite(MaxX) || !std::isfinite(MaxY) ||
			!std::isfinite(Width) || !std::isfinite(Height) ||
			Width <= 0 || Height <= 0 ||
			Width > static_cast<CGFloat>(MaxGlyphDimension) ||
			Height > static_cast<CGFloat>(MaxGlyphDimension))
			return false;

		OutRectangle.MinX = MinX;
		OutRectangle.MinY = MinY;
		OutRectangle.Width = static_cast<INT>(Width);
		OutRectangle.Height = static_cast<INT>(Height);
		return true;
	}

	class FNativeTextAtlas
	{
	public:
		void Reset(UXOpenGLRenderDevice& Renderer)
		{
			Renderer.FlushNativeTextTileBatch();
			for (FNativeTextAtlasPage& Page : Pages)
			{
				if (Page.BindlessTextureHandle)
					glMakeTextureHandleNonResidentARB(Page.BindlessTextureHandle);
				if (Page.Sampler)
					glDeleteSamplers(1, &Page.Sampler);
				if (Page.Texture)
					glDeleteTextures(1, &Page.Texture);
			}
			Pages.clear();
			Glyphs.clear();
			++Generation;
		}

		uint64_t GetGeneration() const
		{
			return Generation;
		}

		INT PageCount() const
		{
			return static_cast<INT>(Pages.size());
		}

		void FailNextAllocationForRuntimeSmoke()
		{
			FailNextAllocation = true;
			ForcedAllocationFailureObserved = false;
		}

		UBOOL DidForceAllocationFailure() const
		{
			return ForcedAllocationFailureObserved ? 1 : 0;
		}

		void PinCachedGlyph(const FNativeTextGlyphKey& Key, std::vector<INT>& PinnedPages)
		{
			const auto Existing = Glyphs.find(Key);
			if (Existing != Glyphs.end())
				Pin(PinnedPages, Existing->second.PageIndex);
		}

		bool PrepareGlyph(UXOpenGLRenderDevice& Renderer, const FNativeTextGlyphPlacement& Placement, std::vector<INT>& PinnedPages, FNativeTextAtlasGlyph& OutGlyph)
		{
			if (!Placement.bDrawable)
				return false;

			auto Existing = Glyphs.find(Placement.Key);
			if (Existing != Glyphs.end())
			{
				TouchPage(Existing->second.PageIndex);
				Pin(PinnedPages, Existing->second.PageIndex);
				OutGlyph = Existing->second;
				return true;
			}

			FRasterizedGlyph Raster;
			if (!RasterizeGlyph(Placement, NativeTextAtlasPageSize(Renderer), Raster))
				return false;

			if (FailNextAllocation)
			{
				FailNextAllocation = false;
				ForcedAllocationFailureObserved = true;
				return false;
			}

			const INT OuterWidth = Raster.Width + NativeTextAtlasPadding * 2;
			const INT OuterHeight = Raster.Height + NativeTextAtlasPadding * 2;
			const INT PageIndex = FindPageForRectangle(Renderer, OuterWidth, OuterHeight, PinnedPages);
			if (PageIndex == INDEX_NONE)
				return false;

			FNativeTextAtlasPage& Page = Pages[static_cast<size_t>(PageIndex)];
			const INT SavedNextX = Page.NextX;
			const INT SavedNextY = Page.NextY;
			const INT SavedRowHeight = Page.RowHeight;
			INT SlotX = 0;
			INT SlotY = 0;
			if (!Allocate(Page, OuterWidth, OuterHeight, SlotX, SlotY))
			{
				Page.NextX = SavedNextX;
				Page.NextY = SavedNextY;
				Page.RowHeight = SavedRowHeight;
				return false;
			}

			std::vector<BYTE> Packed(static_cast<size_t>(OuterWidth) * static_cast<size_t>(OuterHeight) * 4u, 0);
			for (INT Row = 0; Row < Raster.Height; ++Row)
			{
				for (INT Column = 0; Column < Raster.Width; ++Column)
				{
					const BYTE Coverage = Raster.Alpha[static_cast<size_t>(Row) * static_cast<size_t>(Raster.Width) + static_cast<size_t>(Column)];
					const size_t Pixel = (static_cast<size_t>(Row + NativeTextAtlasPadding) * static_cast<size_t>(OuterWidth) + static_cast<size_t>(Column + NativeTextAtlasPadding)) * 4u;
					Packed[Pixel + 0] = 255;
					Packed[Pixel + 1] = 255;
					Packed[Pixel + 2] = 255;
					Packed[Pixel + 3] = Coverage;
				}
			}
			if (!UploadRectangle(Renderer, Page, SlotX, SlotY, OuterWidth, OuterHeight, Packed.data()))
			{
				Page.NextX = SavedNextX;
				Page.NextY = SavedNextY;
				Page.RowHeight = SavedRowHeight;
				return false;
			}

			OutGlyph.PageIndex = PageIndex;
			OutGlyph.X = SlotX + NativeTextAtlasPadding;
			OutGlyph.Y = SlotY + NativeTextAtlasPadding;
			OutGlyph.Width = Raster.Width;
			OutGlyph.Height = Raster.Height;
			Glyphs.emplace(Placement.Key, OutGlyph);
			TouchPage(PageIndex);
			Pin(PinnedPages, PageIndex);
			return true;
		}

		bool PrepareSolidPixel(UXOpenGLRenderDevice& Renderer, std::vector<INT>& PinnedPages, FNativeTextAtlasGlyph& OutGlyph)
		{
			if (Pages.empty() && CreatePage(Renderer) == INDEX_NONE)
				return false;
			Pin(PinnedPages, 0);
			TouchPage(0);
			OutGlyph.PageIndex = 0;
			OutGlyph.X = 0;
			OutGlyph.Y = 0;
			OutGlyph.Width = 1;
			OutGlyph.Height = 1;
			return true;
		}

		bool FindGlyph(const FNativeTextGlyphKey& Key, FNativeTextAtlasGlyph& OutGlyph) const
		{
			const auto Existing = Glyphs.find(Key);
			if (Existing == Glyphs.end())
				return false;
			OutGlyph = Existing->second;
			return true;
		}

		const FNativeTextAtlasPage* GetPage(INT PageIndex) const
		{
			return PageIndex >= 0 && PageIndex < static_cast<INT>(Pages.size()) ? &Pages[static_cast<size_t>(PageIndex)] : NULL;
		}

	private:
		static bool RasterizeGlyph(const FNativeTextGlyphPlacement& Placement, INT PageSize, FRasterizedGlyph& OutRaster)
		{
			FNativeTextRasterRectangle Rectangle;
			if (!CalculateNativeTextRasterRectangle(Placement, PageSize, Rectangle))
				return false;
			OutRaster.Width = Rectangle.Width;
			OutRaster.Height = Rectangle.Height;

			std::vector<BYTE> Pixels(static_cast<size_t>(OutRaster.Width) * static_cast<size_t>(OutRaster.Height) * 4u, 0);
			CGColorSpaceRef ColorSpace = CGColorSpaceCreateDeviceRGB();
			if (!ColorSpace)
				return false;
			CGContextRef Context = CGBitmapContextCreate(
				Pixels.data(),
				static_cast<size_t>(OutRaster.Width),
				static_cast<size_t>(OutRaster.Height),
				8,
				static_cast<size_t>(OutRaster.Width) * 4u,
				ColorSpace,
				kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
			CGColorSpaceRelease(ColorSpace);
			if (!Context)
				return false;

			CGContextSetShouldAntialias(Context, true);
			CGContextSetShouldSmoothFonts(Context, true);
			CGContextSetShouldSubpixelPositionFonts(Context, true);
			CGContextSetShouldSubpixelQuantizeFonts(Context, false);
			CGContextSetTextDrawingMode(Context, kCGTextFill);
			CGContextSetRGBFillColor(Context, 1.f, 1.f, 1.f, 1.f);
			CGContextTranslateCTM(Context, -Rectangle.MinX, -Rectangle.MinY);
			const CGPoint Origin = CGPointMake(0, 0);
			CTFontDrawGlyphs(Placement.Font, &Placement.Glyph, &Origin, 1, Context);
			CGContextRelease(Context);

			OutRaster.Alpha.resize(static_cast<size_t>(OutRaster.Width) * static_cast<size_t>(OutRaster.Height));
			for (size_t Index = 0; Index < OutRaster.Alpha.size(); ++Index)
				OutRaster.Alpha[Index] = Pixels[Index * 4u + 3u];
			return true;
		}

		static bool UploadRectangle(UXOpenGLRenderDevice& Renderer, const FNativeTextAtlasPage& Page, INT X, INT Y, INT Width, INT Height, const BYTE* Pixels)
		{
			while (glGetError() != GL_NO_ERROR)
			{
			}
			GLint PreviousUnpackAlignment = 4;
			glGetIntegerv(GL_UNPACK_ALIGNMENT, &PreviousUnpackAlignment);
			Renderer.PrepareNativeTextTextureMutation(Page.Texture);
			glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
			glTexSubImage2D(GL_TEXTURE_2D, 0, X, Y, Width, Height, GL_RGBA, GL_UNSIGNED_BYTE, Pixels);
			const GLenum MutationError = glGetError();
			glPixelStorei(GL_UNPACK_ALIGNMENT, PreviousUnpackAlignment);
			return MutationError == GL_NO_ERROR;
		}

		INT CreatePage(UXOpenGLRenderDevice& Renderer)
		{
			if (static_cast<INT>(Pages.size()) >= NativeTextAtlasPageLimit)
				return INDEX_NONE;

			const INT PageSize = NativeTextAtlasPageSize(Renderer);
			if (PageSize <= NativeTextAtlasPadding * 2 + 1)
				return INDEX_NONE;

			FNativeTextAtlasPage Page;
			Page.Size = PageSize;
			Page.NextX = 1; // (0,0) is the permanent white underline texel.
			Page.NextY = 0;
			Page.RowHeight = 1;
			glGenTextures(1, &Page.Texture);
			glGenSamplers(1, &Page.Sampler);
			if (!Page.Texture || !Page.Sampler)
			{
				if (Page.Sampler)
					glDeleteSamplers(1, &Page.Sampler);
				if (Page.Texture)
					glDeleteTextures(1, &Page.Texture);
				return INDEX_NONE;
			}

			Renderer.PrepareNativeTextTextureMutation(Page.Texture);
			glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
			glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
			glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
			glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
			glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, PageSize, PageSize, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
			glSamplerParameteri(Page.Sampler, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
			glSamplerParameteri(Page.Sampler, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
			glSamplerParameteri(Page.Sampler, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
			glSamplerParameteri(Page.Sampler, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

			const BYTE SolidPixel[4] = {255, 255, 255, 255};
			if (!UploadRectangle(Renderer, Page, 0, 0, 1, 1, SolidPixel))
			{
				glDeleteSamplers(1, &Page.Sampler);
				glDeleteTextures(1, &Page.Texture);
				return INDEX_NONE;
			}

			if (Renderer.UsingBindlessTextures)
			{
				Page.BindlessTextureHandle = glGetTextureSamplerHandleARB(Page.Texture, Page.Sampler);
				if (!Page.BindlessTextureHandle)
				{
					glDeleteSamplers(1, &Page.Sampler);
					glDeleteTextures(1, &Page.Texture);
					return INDEX_NONE;
				}
				glMakeTextureHandleResidentARB(Page.BindlessTextureHandle);
			}

			Pages.push_back(Page);
			return static_cast<INT>(Pages.size()) - 1;
		}

		static bool Allocate(FNativeTextAtlasPage& Page, INT Width, INT Height, INT& OutX, INT& OutY)
		{
			if (Width > Page.Size || Height > Page.Size)
				return false;
			if (Page.NextX + Width > Page.Size)
			{
				Page.NextX = 0;
				Page.NextY += Page.RowHeight;
				Page.RowHeight = 0;
			}
			if (Page.NextY + Height > Page.Size)
				return false;

			OutX = Page.NextX;
			OutY = Page.NextY;
			Page.NextX += Width;
			Page.RowHeight = Max(Page.RowHeight, Height);
			return true;
		}

		INT FindPageForRectangle(UXOpenGLRenderDevice& Renderer, INT Width, INT Height, const std::vector<INT>& PinnedPages)
		{
			for (INT PageIndex = 0; PageIndex < static_cast<INT>(Pages.size()); ++PageIndex)
			{
				const FNativeTextAtlasPage& Page = Pages[static_cast<size_t>(PageIndex)];
				INT TestX = Page.NextX;
				INT TestY = Page.NextY;
				if (TestX + Width > Page.Size)
				{
					TestX = 0;
					TestY += Page.RowHeight;
				}
				if (TestY + Height <= Page.Size)
					return PageIndex;
			}

			const INT NewPage = CreatePage(Renderer);
			if (NewPage != INDEX_NONE)
				return NewPage;

			INT EvictionCandidate = INDEX_NONE;
			uint64_t OldestUse = std::numeric_limits<uint64_t>::max();
			for (INT PageIndex = 0; PageIndex < static_cast<INT>(Pages.size()); ++PageIndex)
			{
				if (std::find(PinnedPages.begin(), PinnedPages.end(), PageIndex) != PinnedPages.end())
					continue;
				const FNativeTextAtlasPage& Page = Pages[static_cast<size_t>(PageIndex)];
				if (Page.LastUse < OldestUse)
				{
					OldestUse = Page.LastUse;
					EvictionCandidate = PageIndex;
				}
			}
			if (EvictionCandidate == INDEX_NONE)
				return INDEX_NONE;

			Renderer.FlushNativeTextTileBatch();
			for (auto It = Glyphs.begin(); It != Glyphs.end(); )
			{
				if (It->second.PageIndex == EvictionCandidate)
					It = Glyphs.erase(It);
				else
					++It;
			}
			FNativeTextAtlasPage& Page = Pages[static_cast<size_t>(EvictionCandidate)];
			Page.NextX = 1;
			Page.NextY = 0;
			Page.RowHeight = 1;
			Page.LastUse = ++UseSerial;
			++Generation;
			return EvictionCandidate;
		}

		void TouchPage(INT PageIndex)
		{
			if (PageIndex >= 0 && PageIndex < static_cast<INT>(Pages.size()))
				Pages[static_cast<size_t>(PageIndex)].LastUse = ++UseSerial;
		}

		static void Pin(std::vector<INT>& PinnedPages, INT PageIndex)
		{
			if (std::find(PinnedPages.begin(), PinnedPages.end(), PageIndex) == PinnedPages.end())
				PinnedPages.push_back(PageIndex);
		}

		std::vector<FNativeTextAtlasPage> Pages;
		std::unordered_map<FNativeTextGlyphKey, FNativeTextAtlasGlyph, FNativeTextGlyphKeyHash> Glyphs;
		uint64_t Generation{1};
		uint64_t UseSerial{};
		bool FailNextAllocation{};
		bool ForcedAllocationFailureObserved{};
	};

	class FNativeTextCoreTextBackend final : public FNativeTextPlatformBackend
	{
	public:
		UBOOL CreateLayout(const FCanvasTextRequest& Request, FCanvasTextLayout*& OutLayout) override
		{
			OutLayout = NULL;
			std::vector<UniChar> Text;
			std::vector<INT> SourceEnds;
			std::vector<CFRange> UnderlineRanges;
			if (!MakePreprocessedText(Request, Text, SourceEnds, UnderlineRanges))
				return 0;

			const ENativeTextRole Role = NativeTextRoleForFont(Request.Font);
			CTFontRef BaseFont = CreateRequestFont(Request, Role);
			if (!BaseFont)
				return 0;

			if (!Text.empty() && Request.VisibleSourceCharacters > 0 && !SourceEnds.empty() &&
				Request.VisibleSourceCharacters < SourceEnds.back())
			{
				FCanvasTextLayout Probe;
				if (!BuildNativeTextLayout(Probe, Request, Text, SourceEnds, UnderlineRanges, BaseFont, 0))
				{
					CFRelease(BaseFont);
					return 0;
				}
				const CFIndex VisibleEnd = NativeTextVisibleEnd(Probe, Request.VisibleSourceCharacters, static_cast<CFIndex>(Text.size()));
				if (VisibleEnd < static_cast<CFIndex>(Text.size()))
				{
					Text.resize(static_cast<size_t>(VisibleEnd));
					SourceEnds.resize(static_cast<size_t>(VisibleEnd));
					UnderlineRanges.erase(
						std::remove_if(UnderlineRanges.begin(), UnderlineRanges.end(), [VisibleEnd](const CFRange& Range)
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
			Layout->OriginX = Request.OriginX;
			Layout->OriginY = Request.OriginY;
			Layout->ClipX = Request.ClipX;
			Layout->ClipY = Request.ClipY;
			Layout->StartX = Request.StartX;
			Layout->StartY = Request.StartY;
			Layout->bClip = Request.bClip;
			Layout->bCenter = Request.bCenter;

			const UBOOL Success = BuildNativeTextLayout(*Layout, Request, Text, SourceEnds, UnderlineRanges, BaseFont, 1);
			CFRelease(BaseFont);
			if (!Success)
			{
				ReleaseGlyphFonts(Layout->Glyphs);
				delete Layout;
				return 0;
			}
			OutLayout = Layout;
			return 1;
		}

		void DestroyLayout(FCanvasTextLayout* Layout) override
		{
			if (!IsLayout(Layout))
				return;
			ReleaseGlyphFonts(Layout->Glyphs);
			Layout->Magic = 0;
			delete Layout;
		}

		UBOOL MeasureLayout(FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight) override
		{
			OutWidth = 0;
			OutHeight = 0;
			if (!IsLayout(Layout))
				return 0;
			OutWidth = Layout->Width;
			OutHeight = Layout->Height;
			return 1;
		}

		UBOOL DrawLayout(UXOpenGLRenderDevice& Renderer, FSceneNode* Frame, FCanvasTextLayout* Layout) override
		{
			if (!Frame || !IsLayout(Layout))
				return 0;

			// Reject every pathological glyph before atlas lookup/allocation so a
			// failed native call cannot leave a partially populated persistent atlas.
			const INT PageSize = NativeTextAtlasPageSize(Renderer);
			for (const FNativeTextGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (!Placement.bDrawable)
					continue;
				FNativeTextRasterRectangle Rectangle;
				if (!CalculateNativeTextRasterRectangle(Placement, PageSize, Rectangle))
					return 0;
			}

			std::vector<INT> PinnedPages;
			for (const FNativeTextGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (Placement.bDrawable)
					Atlas.PinCachedGlyph(Placement.Key, PinnedPages);
			}
			for (const FNativeTextGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (!Placement.bDrawable)
					continue;
				FNativeTextAtlasGlyph PreparedGlyph;
				if (!Atlas.PrepareGlyph(Renderer, Placement, PinnedPages, PreparedGlyph))
					return 0;
			}

			FNativeTextAtlasGlyph SolidPixel;
			if (!Layout->Underlines.empty() && !Atlas.PrepareSolidPixel(Renderer, PinnedPages, SolidPixel))
				return 0;

			// Layouts retain CoreText placements and glyph-cache keys, never atlas
			// page pointers.  Every page eviction therefore invalidates references by
			// construction and the next draw reacquires its glyphs by key.
			Layout->AtlasGeneration = Atlas.GetGeneration();
			// Preflight the complete batch through the exact clipping and resource
			// validation used for submission.  Canvas may fall back to bitmap text
			// when this method fails, so no native prefix may have been queued first.
			for (const FNativeTextGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (!Placement.bDrawable)
					continue;
				FNativeTextAtlasGlyph Glyph;
				if (!Atlas.FindGlyph(Placement.Key, Glyph) || !DrawAtlasRect(Renderer, Frame, *Layout, Glyph,
					static_cast<FLOAT>(Placement.Position.x + std::floor(CGRectGetMinX(Placement.Bounds))),
					static_cast<FLOAT>(Placement.Position.y - std::ceil(CGRectGetMaxY(Placement.Bounds))),
					static_cast<FLOAT>(Glyph.Width), static_cast<FLOAT>(Glyph.Height), 0))
					return 0;
			}
			for (const FNativeTextUnderline& Underline : Layout->Underlines)
			{
				const FLOAT Width = static_cast<FLOAT>(Underline.End - Underline.Start);
				if (Width <= 0.f)
					continue;
				if (!DrawAtlasRect(Renderer, Frame, *Layout, SolidPixel,
					static_cast<FLOAT>(Underline.Start),
					static_cast<FLOAT>(Underline.Baseline - Underline.Position - Underline.Thickness * 0.5),
					Width, static_cast<FLOAT>(Underline.Thickness), 0))
					return 0;
			}


			for (const FNativeTextGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (!Placement.bDrawable)
					continue;
				FNativeTextAtlasGlyph Glyph;
				if (!Atlas.FindGlyph(Placement.Key, Glyph) || !DrawAtlasRect(Renderer, Frame, *Layout, Glyph,
					static_cast<FLOAT>(Placement.Position.x + std::floor(CGRectGetMinX(Placement.Bounds))),
					static_cast<FLOAT>(Placement.Position.y - std::ceil(CGRectGetMaxY(Placement.Bounds))),
					static_cast<FLOAT>(Glyph.Width), static_cast<FLOAT>(Glyph.Height), 1))
					return 0;
			}

			for (const FNativeTextUnderline& Underline : Layout->Underlines)
			{
				const FLOAT Width = static_cast<FLOAT>(Underline.End - Underline.Start);
				if (Width <= 0.f)
					continue;
				if (!DrawAtlasRect(Renderer, Frame, *Layout, SolidPixel,
					static_cast<FLOAT>(Underline.Start),
					static_cast<FLOAT>(Underline.Baseline - Underline.Position - Underline.Thickness * 0.5),
					Width, static_cast<FLOAT>(Underline.Thickness), 1))
					return 0;
			}
			return 1;
		}

		UBOOL RunRuntimeSmoke(UXOpenGLRenderDevice& Renderer, FSceneNode* Frame) override
		{
			if (!Frame || !Frame->Viewport || !Frame->Viewport->Canvas)
				return 0;

			UViewport* Viewport = Frame->Viewport;
			UCanvas* Canvas = Viewport->Canvas;
			UClient* Client = Viewport->GetOuterUClient();
			if (!Client)
				return 0;
			for (INT ErrorDrain = 0; ErrorDrain < 16 && glGetError() != GL_NO_ERROR; ++ErrorDrain)
			{
			}

			// This must come from the extracted game data.  A transient fixture
			// would not exercise the page-backed package-font branch.
			UFont* PageFont = LoadObject<UFont>(NULL, TEXT("UWindowFonts.Tahoma10"), NULL,
				LOAD_NoWarn | LOAD_Quiet, NULL);
			UFont* NativeFont = Viewport->CreateNativeFont(TEXT("Times"), 24);
			SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageFonts);
			UFont* LargeNativeFont = Viewport->CreateNativeFont(TEXT("Times"), 256);
			if (!PageFont || PageFont->Pages.Num() == 0 || !NativeFont || !LargeNativeFont)
			{
				debugf(TEXT("Native text smoke: font fixture unavailable (page=%p pages=%i native=%p large=%p)"), PageFont, PageFont ? PageFont->Pages.Num() : 0, NativeFont, LargeNativeFont);
				return 0;
			}

			const UBOOL SavedNativeText = Client->NativeText;
			Client->NativeText = 1;

			FCanvasTextRequest Request = {};
			Request.TextScale = 1.f;
			Request.OriginX = 0;
			Request.OriginY = 0;
			Request.ClipX = Max(1, Viewport->SizeX);
			Request.ClipY = Max(1, Viewport->SizeY);
			Request.PolyFlags = PF_Translucent | PF_TwoSided;
			// Canvas colors commonly carry zero source alpha; legacy DrawTile
			// normalizes it to opaque before texture coverage is applied.
			Request.Color = FPlane(1.f, 1.f, 1.f, 0.f);

			auto Draw = [&](UFont* Font, const TCHAR* Text, INT X, INT Y) -> UBOOL
			{
				Request.Font = Font;
				Request.Text = Text;
				Request.TextLength = appStrlen(Text);
				Request.StartX = X;
				Request.StartY = Y;
				FCanvasTextLayout* Layout = NULL;
				INT Width = 0;
				INT Height = 0;
				const UBOOL Result = CreateLayout(Request, Layout)
					&& MeasureLayout(Layout, Width, Height)
					&& Width > 0
					&& Height > 0
					&& DrawLayout(Renderer, Frame, Layout);
				if (Layout)
					DestroyLayout(Layout);
				return Result;
			};

			SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStagePageDraw);
			UBOOL Success = Draw(PageFont, TEXT("PAGE FONT"), 8, 8);
			if( !Success ) debugf(TEXT("Native text smoke: page-font draw failed"));
			if( Success )
			{
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageNativeDraw);
				Success = Draw(NativeFont, TEXT("NATIVE FONT"), 8, 32);
			}
			if( !Success ) debugf(TEXT("Native text smoke: native-font draw failed"));

			// 36 distinct, 256-point glyphs cannot fit a single 1024² page,
			// but fit within the four-page production atlas limit.
			if (Success)
			{
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageAtlas);
				Success = Draw(LargeNativeFont, TEXT("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"), 0, 96)
					&& Atlas.PageCount() > 1;
				if( !Success ) debugf(TEXT("Native text smoke: atlas multipage draw failed (pages=%i)"), Atlas.PageCount());
			}
			Renderer.FlushNativeTextTileBatch();
			SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageGLError);
			for (INT ErrorDrain = 0; ErrorDrain < 16 && glGetError() != GL_NO_ERROR; ++ErrorDrain)
				Success = 0;

			// The forced failure is injected after layout/rasterization but before
			// atlas placement.  Canvas therefore sees a whole-call native failure
			// and must execute its existing page-font bitmap fallback.
			const FLOAT SavedCurX = Canvas->CurX;
			const FLOAT SavedCurY = Canvas->CurY;
			const FLOAT SavedCurYL = Canvas->CurYL;
			const FLOAT SavedClipX = Canvas->ClipX;
			FSceneNode* SavedCanvasFrame = Canvas->Frame;
			const FLOAT SavedClipY = Canvas->ClipY;
			const UFont* SavedFont = Canvas->Font;
			Canvas->CurX = 8.f;
			Canvas->CurY = 8.f;
			Canvas->CurYL = 0.f;
			Canvas->ClipX = Max(Canvas->ClipX, 64.f);
			Canvas->Frame = Frame;
			Canvas->ClipY = Max(Canvas->ClipY, 64.f);
			Canvas->Font = PageFont;
			TCHAR FallbackText[2] = { 0, 0 };
			for (INT Candidate = 33; Candidate < 127 && !FallbackText[0]; ++Candidate)
			{
				if (Candidate == 'P' || Candidate == 'A' || Candidate == 'G' || Candidate == 'E' ||
					Candidate == 'F' || Candidate == 'O' || Candidate == 'N' || Candidate == 'T')
					continue;
				const INT Remapped = (TCHARU)PageFont->RemapChar((TCHAR)Candidate);
				const INT PageIndex = Remapped / PageFont->CharactersPerPage;
				const INT CharacterIndex = Remapped - PageIndex * PageFont->CharactersPerPage;
				if (PageIndex >= 0 && PageIndex < PageFont->Pages.Num() &&
					PageFont->Pages(PageIndex).Texture &&
					CharacterIndex >= 0 && CharacterIndex < PageFont->Pages(PageIndex).Characters.Num())
				{
					const FFontCharacter& Character = PageFont->Pages(PageIndex).Characters(CharacterIndex);
					if (Character.USize > 0 && Character.VSize > 0)
						FallbackText[0] = (TCHAR)Candidate;
				}
			}
			if (Success)
			{
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageFallbackAllocation);
				Success = FallbackText[0] != 0;
				if (Success)
				{
					Atlas.Reset(Renderer);
					Atlas.FailNextAllocationForRuntimeSmoke();
					Canvas->WrappedPrintf(PageFont, 0, FallbackText);
					Success = Atlas.DidForceAllocationFailure();
				}
			}
			if (Success)
			{
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageFallbackCursor);
				Success = Canvas->CurY > 8.f;
			}
			Canvas->CurX = SavedCurX;
			Canvas->CurY = SavedCurY;
			Canvas->CurYL = SavedCurYL;
			Canvas->ClipX = SavedClipX;
			Canvas->ClipY = SavedClipY;
			Canvas->Font = const_cast<UFont*>(SavedFont);
			Canvas->Frame = SavedCanvasFrame;

			// Renderer flush/recreation has to release every GL page and leave
			// no cached placement pointing at a deleted texture.  Re-drawing
			// immediately after Reset proves lazy recreation remains valid.
			if (Success)
			{
				const uint64_t GenerationBeforeReset = Atlas.GetGeneration();
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageReset);
				Atlas.Reset(Renderer);
				Success = Atlas.PageCount() == 0 && Atlas.GetGeneration() > GenerationBeforeReset;
			}
			if (Success)
			{
				Request.Font = NativeFont;
				Request.Text = TEXT("RECREATED");
				Request.TextLength = appStrlen(Request.Text);
				Request.StartX = 8;
				Request.StartY = 56;
				FCanvasTextLayout* Recreated = NULL;
				INT RecreatedWidth = 0;
				INT RecreatedHeight = 0;
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageRecreateLayout);
				Success = CreateLayout(Request, Recreated);
				if (Success)
				{
					SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageRecreateMeasure);
					Success = MeasureLayout(Recreated, RecreatedWidth, RecreatedHeight) && RecreatedWidth > 0 && RecreatedHeight > 0;
				}
				if (Success)
				{
					SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageRecreateSubmit);
					Success = DrawLayout(Renderer, Frame, Recreated);
				}
				if (Recreated)
					DestroyLayout(Recreated);
				if (Success)
				{
					SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageRecreatePage);
					Success = Atlas.PageCount() > 0;
				}
			}
			if (Success)
			{
				Renderer.FlushNativeTextTileBatch();
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageFinalReset);
				Atlas.Reset(Renderer);
				Success = Atlas.PageCount() == 0;
			}
			if (Success)
			{
				SetNativeTextRuntimeSmokeStage(NativeTextRuntimeSmokeStageGLError);
				for (INT ErrorDrain = 0; ErrorDrain < 16 && glGetError() != GL_NO_ERROR; ++ErrorDrain)
					Success = 0;
			}

			Client->NativeText = SavedNativeText;
			return Success;
		}

		void Reset(UXOpenGLRenderDevice& Renderer) override
		{
			Atlas.Reset(Renderer);
		}

	private:
		static bool IsLayout(const FCanvasTextLayout* Layout)
		{
			return Layout && Layout->Magic == NativeTextLayoutMagic;
		}

		bool DrawAtlasRect(UXOpenGLRenderDevice& Renderer, FSceneNode* Frame, const FCanvasTextLayout& Layout, const FNativeTextAtlasGlyph& Glyph, FLOAT OffsetX, FLOAT OffsetY, FLOAT Width, FLOAT Height, UBOOL bSubmit)
		{
			const FNativeTextAtlasPage* Page = Atlas.GetPage(Glyph.PageIndex);
			if (!Page || !Page->Texture || !Page->Sampler || Page->Size <= 0 ||
				(Renderer.UsingBindlessTextures && !Page->BindlessTextureHandle) ||
				Width <= 0.f || Height <= 0.f)
				return false;
			FLOAT SourceWidth = static_cast<FLOAT>(Glyph.Width);
			FLOAT SourceHeight = static_cast<FLOAT>(Glyph.Height);
			FLOAT X = static_cast<FLOAT>(Layout.StartX) + OffsetX;
			FLOAT Y = static_cast<FLOAT>(Layout.StartY) + OffsetY;
			// Per-line centering is captured in shaped glyph offsets.
			FLOAT U = static_cast<FLOAT>(Glyph.X);
			FLOAT V = static_cast<FLOAT>(Glyph.Y);
			if (Layout.bClip)
			{
				if (X < 0.f)
				{
					const FLOAT Delta = Min(-X, Width);
					const FLOAT SourceDelta = Delta * SourceWidth / Width;
					X += Delta; Width -= Delta; U += SourceDelta; SourceWidth -= SourceDelta;
				}
				if (Y < 0.f)
				{
					const FLOAT Delta = Min(-Y, Height);
					const FLOAT SourceDelta = Delta * SourceHeight / Height;
					Y += Delta; Height -= Delta; V += SourceDelta; SourceHeight -= SourceDelta;
				}
				if (X + Width > Layout.ClipX)
				{
					const FLOAT Keep = Max(0.f, Layout.ClipX - X);
					SourceWidth *= Keep / Width; Width = Keep;
				}
				if (Y + Height > Layout.ClipY)
				{
					const FLOAT Keep = Max(0.f, Layout.ClipY - Y);
					SourceHeight *= Keep / Height; Height = Keep;
				}
			}
			if (Width <= 0.f || Height <= 0.f || SourceWidth <= 0.f || SourceHeight <= 0.f)
				return true;
			const DWORD TextPolyFlags = (Layout.PolyFlags & ~PF_Masked) | PF_Highlighted | PF_Translucent | PF_TwoSided;
			FPlane TextColor = Layout.Color;
			TextColor.W = 1.f;
			const FLOAT DrawX = Layout.OriginX + X;
			const FLOAT DrawY = Layout.OriginY + Y;
			if (!IsSafeNativeTextFloat(DrawX) || !IsSafeNativeTextFloat(DrawY) ||
				!IsSafeNativeTextFloat(Width) || !IsSafeNativeTextFloat(Height) ||
				!IsSafeNativeTextFloat(U) || !IsSafeNativeTextFloat(V) ||
				!IsSafeNativeTextFloat(SourceWidth) || !IsSafeNativeTextFloat(SourceHeight))
				return false;
			if (!bSubmit)
				return !Renderer.NoDrawTile;
			return Renderer.DrawNativeTextTile(
				Frame, Page->Texture, Page->Sampler, Page->BindlessTextureHandle, Page->Size, Page->Size,
				DrawX, DrawY, Width, Height, U, V, SourceWidth, SourceHeight, 1.f, TextColor, TextPolyFlags
			) != 0;
		}

		FNativeTextAtlas Atlas;
	};

ENativeTextRole NativeTextRoleForFont(const UFont* Font)
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

FNativeTextPlatformBackend* CreateNativeTextPlatformBackend()
{
	return new FNativeTextCoreTextBackend;
}

void BeginNativeTextRuntimeSmoke()
{
	GNativeTextRuntimeSmokeState = NativeTextRuntimeSmokePending;
	GNativeTextRuntimeSmokeStage = NativeTextRuntimeSmokeStageNone;
}

ENativeTextRuntimeSmokeState GetNativeTextRuntimeSmokeState()
{
	return GNativeTextRuntimeSmokeState;
}
ENativeTextRuntimeSmokeStage GetNativeTextRuntimeSmokeStage()
{
	return GNativeTextRuntimeSmokeStage;
}

void SetNativeTextRuntimeSmokeStage(ENativeTextRuntimeSmokeStage Stage)
{
	GNativeTextRuntimeSmokeStage = Stage;
}

void CompleteNativeTextRuntimeSmoke(UBOOL Success)
{
	GNativeTextRuntimeSmokeState = Success ? NativeTextRuntimeSmokePassed : NativeTextRuntimeSmokeFailed;
}

UBOOL NativeTextCopyUTF16ForTests(const TCHAR* Text, INT TextLength, UNICHAR* OutText, INT OutCapacity, INT& OutLength)
{
	const std::vector<UniChar> Converted = UTF16FromTCHAR(Text, TextLength);
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
	if (!Layout || Layout->Magic != NativeTextLayoutMagic || OutCapacity < static_cast<INT>(Layout->Clusters.size()) ||
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
	for (const FNativeTextCluster& Cluster : Layout->Clusters)
	{
		OutInfo.UTF16Length = Max(OutInfo.UTF16Length, static_cast<INT>(Cluster.SourceRange.location + Cluster.SourceRange.length));
		OutInfo.VisibleSourceEnd = Max(OutInfo.VisibleSourceEnd, Cluster.SourceEnd);
		OutClusterSourceEnds[&Cluster - Layout->Clusters.data()] = Cluster.SourceEnd;
	}
	for (const FNativeTextGlyphPlacement& Glyph : Layout->Glyphs)
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

UBOOL UXOpenGLRenderDevice::CreateCanvasTextLayout(const FCanvasTextRequest& Request, FCanvasTextLayout*& OutLayout)
{
	OutLayout = NULL;
	return NativeTextBackend ? NativeTextBackend->CreateLayout(Request, OutLayout) : 0;
}

void UXOpenGLRenderDevice::DestroyCanvasTextLayout(FCanvasTextLayout* Layout)
{
	if (NativeTextBackend)
		NativeTextBackend->DestroyLayout(Layout);
}

UBOOL UXOpenGLRenderDevice::MeasureCanvasText(FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight)
{
	OutWidth = 0;
	OutHeight = 0;
	return NativeTextBackend ? NativeTextBackend->MeasureLayout(Layout, OutWidth, OutHeight) : 0;
}

UBOOL UXOpenGLRenderDevice::DrawCanvasText(FSceneNode* Frame, FCanvasTextLayout* Layout)
{
	return NativeTextBackend ? NativeTextBackend->DrawLayout(*this, Frame, Layout) : 0;
}

UBOOL UXOpenGLRenderDevice::RunNativeTextRuntimeSmoke(FSceneNode* Frame)
{
	return NativeTextBackend ? NativeTextBackend->RunRuntimeSmoke(*this, Frame) : 0;
}
