/*=============================================================================
	NativeTextShared.cpp: Driver-neutral CoreText Canvas layout and glyph
	rasterization core shared by every render driver with a native text leg.
=============================================================================*/
// UnRenDev.h (pulled in by NativeTextShared.h) is not self-contained: the
// engine umbrella must come first, exactly like every driver TU and the
// contract tests include it.
#include "Engine.h"

#include "NativeTextShared.h"

#include "NativeText.h"

#include <algorithm>
#include <cmath>
#include <limits>
#include <string>
#include <unordered_set>
#include <utility>

#if !HP2_HAS_NATIVE_TEXT_BACKEND
#error NativeTextShared.cpp must only be compiled when HP2_HAS_NATIVE_TEXT_BACKEND is enabled.
#endif

namespace Hp2NativeText
{
	namespace
	{
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

		static bool MakeGlyphKey(CTFontRef Font, CGGlyph Glyph, FGlyphKey& OutKey)
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

			const std::vector<UniChar> UTF16Name = UTF16FromTCHAR(FamilyName, appStrlen(FamilyName));
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

		static CTFontRef CreateRequestFont(const FCanvasTextLayoutRequest& Request, ENativeTextRole Role)
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
			std::vector<CFRange>& OutUnderlines
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

		static void ReleaseGlyphFonts(std::vector<FGlyphPlacement>& Glyphs)
		{
			for (FGlyphPlacement& Glyph : Glyphs)
			{
				if (Glyph.Font)
					CFRelease(Glyph.Font);
				Glyph.Font = NULL;
			}
		}

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

		static const FCluster* NativeTextClusterAt(const std::vector<FCluster>& Clusters, CFIndex SourceIndex)
		{
			for (const FCluster& Cluster : Clusters)
			{
				const CFIndex End = Cluster.SourceRange.location + Cluster.SourceRange.length;
				if (SourceIndex >= Cluster.SourceRange.location && SourceIndex < End)
					return &Cluster;
			}
			return NULL;
		}

		static bool AppendNativeTextGlyphs(FCanvasTextLayout& Layout, CTLineRef Line, const std::vector<FCluster>& LineClusters, CGFloat LineOriginX, CGFloat Baseline)
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
					const FCluster* Cluster = NativeTextClusterAt(LineClusters, SourceIndices[static_cast<size_t>(GlyphIndex)]);
					if (!Cluster)
						return false;

					FGlyphPlacement Placement;
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

				FUnderline Underline;
				Underline.Start = LineOriginX + Min(OffsetA, OffsetB);
				Underline.End = LineOriginX + Max(OffsetA, OffsetB);
				Underline.Baseline = Baseline;
				Underline.Position = CTFontGetUnderlinePosition(BaseFont);
				Underline.Thickness = Max(1.f, static_cast<FLOAT>(CTFontGetUnderlineThickness(BaseFont)));
				Layout.Underlines.push_back(Underline);
			}
		}

		static bool AppendNativeTextLine(FCanvasTextLayout& Layout, CTTypesetterRef Typesetter, CFStringRef Source, CFRange LineRange, const std::vector<INT>& SourceEnds, const std::vector<CFRange>& UnderlineRanges, CTFontRef BaseFont, const FCanvasTextLayoutRequest& Request, CGFloat& InOutTop, UBOOL bCaptureGlyphs)
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

			std::vector<FCluster> LineClusters;
			LineClusters.reserve(Ranges.size());
			for (const CFRange& Range : Ranges)
			{
				const CFIndex End = Range.location + Range.length;
				if (End > static_cast<CFIndex>(SourceEnds.size()))
				{
					CFRelease(Line);
					return false;
				}
				FCluster Cluster;
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

		static bool BuildNativeTextLayout(FCanvasTextLayout& Layout, const FCanvasTextLayoutRequest& Request, const std::vector<UniChar>& Text, const std::vector<INT>& SourceEnds, const std::vector<CFRange>& UnderlineRanges, CTFontRef BaseFont, UBOOL bCaptureGlyphs)
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
			const bool bWrap = Request.Mode == CanvasLayout_Wrapped;
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
			for (const FCluster& Cluster : Layout.Clusters)
			{
				const CFIndex ClusterEnd = Cluster.SourceRange.location + Cluster.SourceRange.length;
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
		CFArrayRef FamilyNames = CTFontManagerCopyAvailableFontFamilyNames();
		if (!FamilyNames || CFArrayGetCount(FamilyNames) == 0)
		{
			if (FamilyNames)
				CFRelease(FamilyNames);
			OutReasonCode = "text.fonts_unavailable";
			return false;
		}
		CFRelease(FamilyNames);

		CTFontRef RoleFont = CreateRoleFont(NTROLE_Body, 12);
		if (!RoleFont)
		{
			OutReasonCode = "text.fonts_unavailable";
			return false;
		}
		CFRelease(RoleFont);

		CGColorSpaceRef ColorSpace = CGColorSpaceCreateDeviceRGB();
		if (!ColorSpace)
		{
			OutReasonCode = "text.rasterizer_unavailable";
			return false;
		}
		BYTE Pixel[4] = {0, 0, 0, 0};
		CGContextRef Context = CGBitmapContextCreate(
			Pixel,
			1,
			1,
			8,
			4,
			ColorSpace,
			kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
		CGColorSpaceRelease(ColorSpace);
		if (!Context)
		{
			OutReasonCode = "text.rasterizer_unavailable";
			return false;
		}
		CGContextRelease(Context);

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
		std::vector<CFRange> UnderlineRanges;
		if (!MakePreprocessedText(Request, Text, SourceEnds, UnderlineRanges))
			return NULL;

		const ENativeTextRole Role = RoleForFont(Request.Font);
		CTFontRef BaseFont = CreateRequestFont(Request, Role);
		if (!BaseFont)
			return NULL;

		if (!Text.empty() && Request.VisibleSourceCharacters > 0 && !SourceEnds.empty() &&
			Request.VisibleSourceCharacters < SourceEnds.back())
		{
			FCanvasTextLayout Probe;
			if (!BuildNativeTextLayout(Probe, Request, Text, SourceEnds, UnderlineRanges, BaseFont, 0))
			{
				CFRelease(BaseFont);
				return NULL;
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
			return NULL;
		}
		return Layout;
	}

	void DestroyLayout(FCanvasTextLayout* Layout)
	{
		if (!IsLayout(Layout))
			return;
		ReleaseGlyphFonts(Layout->Glyphs);
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
			!std::isfinite(Placement.Bounds.origin.x) || !std::isfinite(Placement.Bounds.origin.y) ||
			!std::isfinite(Placement.Bounds.size.width) || !std::isfinite(Placement.Bounds.size.height) ||
			Placement.Bounds.size.width <= 0 || Placement.Bounds.size.height <= 0)
			return false;
		const INT MaxGlyphDimension = PageSize - Padding * 2;

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

	bool RasterizeGlyph(const FGlyphPlacement& Placement, int PageSize, FRasterizedGlyph& OutRaster)
	{
		FRasterRectangle Rectangle;
		if (!CalculateRasterRectangle(Placement, PageSize, Rectangle))
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

	void GlyphDrawOffset(const FGlyphPlacement& Placement, double& OutLeft, double& OutTop)
	{
		OutLeft = Placement.Position.x + std::floor(CGRectGetMinX(Placement.Bounds));
		OutTop  = Placement.Position.y - std::ceil(CGRectGetMaxY(Placement.Bounds));
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
	const std::vector<UniChar> Converted = Hp2NativeText::UTF16FromTCHAR(Text, TextLength);
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
