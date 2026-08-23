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
#include "NativeTextShared.h"

#if !HP2_HAS_NATIVE_TEXT_BACKEND
#error NativeText.cpp must only be compiled when HP2_HAS_NATIVE_TEXT_BACKEND is enabled.
#endif

#include <CoreGraphics/CoreGraphics.h>
#include <CoreText/CoreText.h>

constexpr INT NativeTextAtlasPageLimit = 4;

	static INT NativeTextAtlasPageSize(const UXOpenGLRenderDevice& Renderer)
	{
		return Min(Hp2NativeText::PageSize, Renderer.MaxTextureSize);
	}

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

	struct FNativeTextAtlasGlyph
	{
		INT PageIndex{INDEX_NONE};
		INT X{};
		INT Y{};
		INT Width{};
		INT Height{};
	};

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

		void PinCachedGlyph(const Hp2NativeText::FGlyphKey& Key, std::vector<INT>& PinnedPages)
		{
			const auto Existing = Glyphs.find(Key);
			if (Existing != Glyphs.end())
				Pin(PinnedPages, Existing->second.PageIndex);
		}

		bool PrepareGlyph(UXOpenGLRenderDevice& Renderer, const Hp2NativeText::FGlyphPlacement& Placement, std::vector<INT>& PinnedPages, FNativeTextAtlasGlyph& OutGlyph)
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

			Hp2NativeText::FRasterizedGlyph Raster;
			if (!Hp2NativeText::RasterizeGlyph(Placement, NativeTextAtlasPageSize(Renderer), Raster))
				return false;

			if (FailNextAllocation)
			{
				FailNextAllocation = false;
				ForcedAllocationFailureObserved = true;
				return false;
			}

			const INT OuterWidth = Raster.Width + Hp2NativeText::Padding * 2;
			const INT OuterHeight = Raster.Height + Hp2NativeText::Padding * 2;
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
					const size_t Pixel = (static_cast<size_t>(Row + Hp2NativeText::Padding) * static_cast<size_t>(OuterWidth) + static_cast<size_t>(Column + Hp2NativeText::Padding)) * 4u;
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
			OutGlyph.X = SlotX + Hp2NativeText::Padding;
			OutGlyph.Y = SlotY + Hp2NativeText::Padding;
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

		bool FindGlyph(const Hp2NativeText::FGlyphKey& Key, FNativeTextAtlasGlyph& OutGlyph) const
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
			if (PageSize <= Hp2NativeText::Padding * 2 + 1)
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
		std::unordered_map<Hp2NativeText::FGlyphKey, FNativeTextAtlasGlyph, Hp2NativeText::FGlyphKeyHash> Glyphs;
		uint64_t Generation{1};
		uint64_t UseSerial{};
		bool FailNextAllocation{};
		bool ForcedAllocationFailureObserved{};
	};

	class FNativeTextCoreTextBackend final : public FNativeTextPlatformBackend
	{
	public:
		UBOOL CreateLayout(const FCanvasTextLayoutRequest& Request, FCanvasTextLayout*& OutLayout) override
		{
			OutLayout = Hp2NativeText::CreateLayout(Request);
			return OutLayout ? 1 : 0;
		}

		void DestroyLayout(FCanvasTextLayout* Layout) override
		{
			Hp2NativeText::DestroyLayout(Layout);
		}

		UBOOL MeasureLayout(FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight) override
		{
			return Hp2NativeText::MeasureLayout(Layout, OutWidth, OutHeight) ? 1 : 0;
		}

		UBOOL DrawLayout(UXOpenGLRenderDevice& Renderer, FSceneNode* Frame, FCanvasTextLayout* Layout) override
		{
			if (!Frame || !Hp2NativeText::IsLayout(Layout))
				return 0;

			// Reject every pathological glyph before atlas lookup/allocation so a
			// failed native call cannot leave a partially populated persistent atlas.
			const INT PageSize = NativeTextAtlasPageSize(Renderer);
			for (const Hp2NativeText::FGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (!Placement.bDrawable)
					continue;
				Hp2NativeText::FRasterRectangle Rectangle;
				if (!Hp2NativeText::CalculateRasterRectangle(Placement, PageSize, Rectangle))
					return 0;
			}

			std::vector<INT> PinnedPages;
			for (const Hp2NativeText::FGlyphPlacement& Placement : Layout->Glyphs)
			{
				if (Placement.bDrawable)
					Atlas.PinCachedGlyph(Placement.Key, PinnedPages);
			}
			for (const Hp2NativeText::FGlyphPlacement& Placement : Layout->Glyphs)
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
			for (const Hp2NativeText::FGlyphPlacement& Placement : Layout->Glyphs)
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
			for (const Hp2NativeText::FUnderline& Underline : Layout->Underlines)
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


			for (const Hp2NativeText::FGlyphPlacement& Placement : Layout->Glyphs)
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

			for (const Hp2NativeText::FUnderline& Underline : Layout->Underlines)
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

			FCanvasTextLayoutRequest Request = FCanvasTextLayoutRequest::Computed(NULL, NULL, 0);
			Request.TextScale = 1.f;
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
			if (!Hp2NativeText::SafeFloat(DrawX) || !Hp2NativeText::SafeFloat(DrawY) ||
				!Hp2NativeText::SafeFloat(Width) || !Hp2NativeText::SafeFloat(Height) ||
				!Hp2NativeText::SafeFloat(U) || !Hp2NativeText::SafeFloat(V) ||
				!Hp2NativeText::SafeFloat(SourceWidth) || !Hp2NativeText::SafeFloat(SourceHeight))
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

FNativeTextPlatformBackend* CreateNativeTextPlatformBackend(FNativeTextBackendStatus& OutStatus)
{
	const char* ReasonCode = "text.internal_error";
	if (!Hp2NativeText::ProbeAvailability(ReasonCode))
	{
		// Unavailable platform text stack: hand back no backend at all so every
		// per-draw call site keeps its existing Boolean fallback path.
		OutStatus.Available = false;
		OutStatus.ReasonCode = ReasonCode;
		return NULL;
	}

	OutStatus.Available = true;
	OutStatus.ReasonCode = ReasonCode;
	return new FNativeTextCoreTextBackend;
}
FNativeTextPlatformBackend* CreateNativeTextPlatformBackend()
{
	FNativeTextBackendStatus Status;
	return CreateNativeTextPlatformBackend(Status);
}

UBOOL UXOpenGLRenderDevice::CreateCanvasTextLayout(const FCanvasTextRequest& Request, FCanvasTextLayout*& OutLayout)
{
	OutLayout = NULL;
	if (!NativeTextBackend)
		return 0;
	// Draw-side state: the Canvas origin is validated here and applied to the
	// opaque layout only after a successful all-or-nothing creation.
	if (!Hp2NativeText::SafeFloat(Request.OriginX) || !Hp2NativeText::SafeFloat(Request.OriginY))
		return 0;
	FCanvasTextLayoutRequest LayoutRequest;
	if (!ProjectCanvasTextLayoutRequest(Request, LayoutRequest))
		return 0;
	if (!NativeTextBackend->CreateLayout(LayoutRequest, OutLayout) || !OutLayout)
		return 0;
	OutLayout->OriginX = Request.OriginX;
	OutLayout->OriginY = Request.OriginY;
	return 1;
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
