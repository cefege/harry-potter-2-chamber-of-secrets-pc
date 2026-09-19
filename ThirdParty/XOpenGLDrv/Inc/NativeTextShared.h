/*=============================================================================
	NativeTextShared.h: Driver-neutral CoreText Canvas layout and glyph
	rasterization core shared by every render driver with a native text leg
	(XOpenGLDrv, UT99VulkanDrv).
=============================================================================*/
#pragma once

#include <cstdint>
#include <string>
#include <vector>

#include "UnRenDev.h"

#if defined(__APPLE__)
#include <CoreGraphics/CoreGraphics.h>
#include <CoreText/CoreText.h>
#endif


enum ENativeTextRole
{
	NTROLE_Tiny,
	NTROLE_Small,
	NTROLE_Body,
	NTROLE_BodyBold,
	NTROLE_Heading,
	NTROLE_HeadingBold,
	NTROLE_HPMenuMedium,
	NTROLE_HPMenuLarge,
	NTROLE_Console,
	NTROLE_Subtitle,
};

enum ENativeTextRuntimeSmokeState
{
	NativeTextRuntimeSmokeIdle,
	NativeTextRuntimeSmokePending,
	NativeTextRuntimeSmokePassed,
	NativeTextRuntimeSmokeFailed,
};
enum ENativeTextRuntimeSmokeStage
{
	NativeTextRuntimeSmokeStageNone,
	NativeTextRuntimeSmokeStageFonts,
	NativeTextRuntimeSmokeStagePageDraw,
	NativeTextRuntimeSmokeStageNativeDraw,
	NativeTextRuntimeSmokeStageAtlas,
	NativeTextRuntimeSmokeStageFallback,
	NativeTextRuntimeSmokeStageReset,
	NativeTextRuntimeSmokeStageRecreate,
	NativeTextRuntimeSmokeStageFinalReset,
	NativeTextRuntimeSmokeStageGLError,
	NativeTextRuntimeSmokeStageRecreateLayout,
	NativeTextRuntimeSmokeStageRecreateMeasure,
	NativeTextRuntimeSmokeStageRecreateSubmit,
	NativeTextRuntimeSmokeStageRecreatePage,
	NativeTextRuntimeSmokeStageFallbackAllocation,
	NativeTextRuntimeSmokeStageFallbackCursor,
};

#if defined(__APPLE__)
namespace Hp2NativeText
{
	constexpr DWORD LayoutMagic = 0x4E545854; // NTXT
	constexpr int PageSize = 1024;
	constexpr int Padding = 1;

	struct FGlyphKey
	{
		std::string FontIdentity;
		CGGlyph Glyph{};
		uint32_t RasterPointSize64{};
		UBOOL Antialias{};
		UBOOL Subpixel{};

		bool operator==(const FGlyphKey& Other) const
		{
			return Glyph == Other.Glyph &&
				RasterPointSize64 == Other.RasterPointSize64 &&
				Antialias == Other.Antialias &&
				Subpixel == Other.Subpixel &&
				FontIdentity == Other.FontIdentity;
		}
	};

	struct FGlyphKeyHash
	{
		size_t operator()(const FGlyphKey& Key) const
		{
			size_t Result = std::hash<std::string>{}(Key.FontIdentity);
			Result ^= static_cast<size_t>(Key.Glyph) + 0x9e3779b9u + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.RasterPointSize64) + 0x9e3779b9u + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.Antialias) + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.Subpixel) + (Result << 6) + (Result >> 2);
			return Result;
		}
	};

	struct FGlyphPlacement
	{
		CTFontRef Font{};
		CGGlyph Glyph{};
		CGPoint Position{};
		CGSize Advance{};
		CGRect Bounds{};
		CFRange SourceRange{};
		FGlyphKey Key;
		UBOOL bDrawable{};
	};

	struct FUnderline
	{
		CGFloat Start{};
		CGFloat End{};
		CGFloat Baseline{};
		CGFloat Position{};
		CGFloat Thickness{};
	};

	struct FCluster
	{
		CFRange SourceRange{};
		INT SourceEnd{};
	};

	struct FRasterizedGlyph
	{
		INT Width{};
		INT Height{};
		std::vector<BYTE> Alpha;
	};

	struct FAtlasSlot
	{
		INT X{};
		INT Y{};
		INT Width{};
		INT Height{};
	};

	struct FRasterRectangle
	{
		CGFloat MinX{};
		CGFloat MinY{};
		INT Width{};
		INT Height{};
	};
}

struct FCanvasTextLayout
{
	DWORD Magic{Hp2NativeText::LayoutMagic};
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
	std::vector<Hp2NativeText::FGlyphPlacement> Glyphs;
	std::vector<Hp2NativeText::FUnderline> Underlines;
	std::vector<Hp2NativeText::FCluster> Clusters;
};

namespace Hp2NativeText
{
	ENativeTextRole RoleForFont(const UFont* Font);
	bool ProbeAvailability(const char*& OutReasonCode);
	bool SafeFloat(FLOAT Value);

	// All-or-nothing layout creation: preprocesses the request text, resolves
	// the role font, trims to the visible source span, then shapes the whole
	// layout.  Returns NULL on any failure without leaking shaped state.
	FCanvasTextLayout* CreateLayout(const FCanvasTextLayoutRequest& Request);
	void DestroyLayout(FCanvasTextLayout* Layout);
	bool MeasureLayout(const FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight);
	bool IsLayout(const FCanvasTextLayout* Layout);

	bool RasterizeGlyph(const FGlyphPlacement& Placement, int PageSize, FRasterizedGlyph& OutRaster);
	bool CalculateRasterRectangle(const FGlyphPlacement& Placement, int PageSize, FRasterRectangle& OutRectangle);

	// Top-left pixel corner of a glyph's ink box in layout space (y grows down).
	void GlyphDrawOffset(const FGlyphPlacement& Placement, double& OutLeft, double& OutTop);
}
#else

struct FNativeTextFace;            // opaque, owned by the FreeType core

namespace Hp2NativeText
{
	constexpr DWORD LayoutMagic = 0x4E545854; // NTXT
	constexpr int PageSize = 1024;
	constexpr int Padding = 1;

	struct FTextPoint { double x{}; double y{}; };
	struct FTextSize  { double width{}; double height{}; };
	struct FTextRect  { double MinX{}; double MinY{}; double MaxX{}; double MaxY{}; };
	struct FTextRange { std::ptrdiff_t location{}; std::ptrdiff_t length{}; };

	struct FGlyphKey
	{
		std::string FontIdentity;
		std::uint32_t Glyph{};
		std::uint32_t RasterPointSize64{};
		UBOOL Antialias{};
		UBOOL Subpixel{};

		bool operator==(const FGlyphKey& Other) const
		{
			return Glyph == Other.Glyph &&
				RasterPointSize64 == Other.RasterPointSize64 &&
				Antialias == Other.Antialias &&
				Subpixel == Other.Subpixel &&
				FontIdentity == Other.FontIdentity;
		}
	};

	struct FGlyphKeyHash
	{
		size_t operator()(const FGlyphKey& Key) const
		{
			size_t Result = std::hash<std::string>{}(Key.FontIdentity);
			Result ^= static_cast<size_t>(Key.Glyph) + 0x9e3779b9u + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.RasterPointSize64) + 0x9e3779b9u + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.Antialias) + (Result << 6) + (Result >> 2);
			Result ^= static_cast<size_t>(Key.Subpixel) + (Result << 6) + (Result >> 2);
			return Result;
		}
	};

	struct FGlyphPlacement
	{
		const FNativeTextFace* Face{};   // borrowed; faces live for the process
		std::uint32_t Glyph{};
		FTextPoint Position{};
		FTextSize  Advance{};
		FTextRect  Bounds{};              // integer pixel ink box, y-up, relative to the pen
		FTextRange SourceRange{};
		FGlyphKey  Key;
		UBOOL bDrawable{};
	};

	struct FUnderline
	{
		double Start{};
		double End{};
		double Baseline{};
		double Position{};
		double Thickness{};
	};

	struct FCluster
	{
		FTextRange SourceRange{};
		INT SourceEnd{};
	};

	struct FRasterizedGlyph
	{
		INT Width{};
		INT Height{};
		std::vector<BYTE> Alpha;
	};

	struct FAtlasSlot
	{
		INT X{};
		INT Y{};
		INT Width{};
		INT Height{};
	};

	struct FRasterRectangle
	{
		double MinX{};
		double MinY{};
		INT Width{};
		INT Height{};
	};
}

struct FCanvasTextLayout
{
	DWORD Magic{Hp2NativeText::LayoutMagic};
	INT Width{};
	INT Height{};
	double Ascent{};
	double Descent{};
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
	std::vector<Hp2NativeText::FGlyphPlacement> Glyphs;
	std::vector<Hp2NativeText::FUnderline> Underlines;
	std::vector<Hp2NativeText::FCluster> Clusters;
};

namespace Hp2NativeText
{
	ENativeTextRole RoleForFont(const UFont* Font);
	bool ProbeAvailability(const char*& OutReasonCode);
	bool SafeFloat(FLOAT Value);

	// All-or-nothing layout creation: preprocesses the request text, resolves
	// the role font, trims to the visible source span, then shapes the whole
	// layout.  Returns NULL on any failure without leaking shaped state.
	FCanvasTextLayout* CreateLayout(const FCanvasTextLayoutRequest& Request);
	void DestroyLayout(FCanvasTextLayout* Layout);
	bool MeasureLayout(const FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight);
	bool IsLayout(const FCanvasTextLayout* Layout);

	bool RasterizeGlyph(const FGlyphPlacement& Placement, int PageSize, FRasterizedGlyph& OutRaster);
	bool CalculateRasterRectangle(const FGlyphPlacement& Placement, int PageSize, FRasterRectangle& OutRectangle);

	// Top-left pixel corner of a glyph's ink box in layout space (y grows down).
	void GlyphDrawOffset(const FGlyphPlacement& Placement, double& OutLeft, double& OutTop);
}

#endif
