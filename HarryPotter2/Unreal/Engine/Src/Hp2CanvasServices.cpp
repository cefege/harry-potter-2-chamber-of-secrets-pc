/*=============================================================================
	Hp2CanvasServices.cpp: Engine callbacks for the Rust UCanvas/UFont text and
	tile port (crates/hp2-engine/src/canvas_text.rs). Rust decides the logic;
	these bodies perform only the engine operations it asks for.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "Hp2CanvasServices.h"

// Defined in UnCanvas.cpp: one native text backend attempt (TryNativeCanvasText).
UBOOL CanvasTryNativeTextForServices( UCanvas* Canvas, const Hp2NativeTextRequest& Req, UFont* Font, const TCHAR* Text, FPlane Color, INT& OutWidth, INT& OutHeight );

static FPlane CanvasToFPlane( Hp2Plane P )
{
	return FPlane( P.x, P.y, P.z, P.w );
}

// Rust sends NUL-terminated UNICHAR text; the engine takes TCHAR.
static void CanvasTextFromUnits( const UNICHAR* Units, TArray<TCHAR>& Text )
{
	INT Len = 0;
	while( Units[Len] )
		Len++;
	Text.Add( Len + 1 );
	for( INT i=0; i<Len; i++ )
		Text(i) = FromUnicode( Units[i] );
	Text(Len) = 0;
}

static FLOAT CanvasUIScale( void* Canvas )
{
	UCanvas* C = (UCanvas*)Canvas;
	if( C->Viewport && C->Viewport->GetOuterUClient() )
		return C->Viewport->GetOuterUClient()->GetUIScale();
	return 1.f;
}

static BYTE CanvasLanguageIsInt()
{
	return appStricmp( UObject::GetLanguage(), TEXT("INT") ) == 0;
}

static void CanvasViewportDrawString( void* Canvas, uint32_t Flags, void* Font, INT* X, INT* Y, const UNICHAR* Text, Hp2Plane Color )
{
	UCanvas* C = (UCanvas*)Canvas;
	TArray<TCHAR> Buffer;
	CanvasTextFromUnits( Text, Buffer );
	C->Viewport->DrawString( Flags, (UFont*)Font, *X, *Y, &Buffer(0), CanvasToFPlane(Color) );
}

// Copies the fields TryNativeCanvasText reads from the Rust view into the canvas.
// Font is not copied: TryNativeCanvasText takes it as a parameter.
static BYTE CanvasTryNativeText( void* Canvas, const Hp2CanvasView* View, const Hp2NativeTextRequest* Req, void* Font, const UNICHAR* Text, Hp2Plane Color, Hp2CanvasSize* Out )
{
	UCanvas* C = (UCanvas*)Canvas;
	C->SpaceX = View->space_x;
	C->SpaceY = View->space_y;
	C->OrgX   = View->org_x;
	C->OrgY   = View->org_y;
	C->ClipX  = View->clip_x;
	C->ClipY  = View->clip_y;
	C->CurX   = View->cur_x;
	C->CurY   = View->cur_y;
	C->CurYL  = View->cur_yl;

	TArray<TCHAR> Buffer;
	CanvasTextFromUnits( Text, Buffer );
	INT Width = 0;
	INT Height = 0;
	const UBOOL bDrawn = CanvasTryNativeTextForServices( C, *Req, (UFont*)Font, &Buffer(0), CanvasToFPlane(Color), Width, Height );
	Out->w = Width;
	Out->h = Height;
	return bDrawn ? 1 : 0;
}

static INT CanvasFontCharsPerPage( void* Font )
{
	return ((UFont*)Font)->CharactersPerPage;
}

static UNICHAR CanvasFontRemapChar( void* Font, UNICHAR Ch )
{
	return ToUnicode( ((UFont*)Font)->RemapChar( FromUnicode(Ch) ) );
}

static INT CanvasFontPageCount( void* Font )
{
	return ((UFont*)Font)->Pages.Num();
}

static void* CanvasFontPageTexture( void* Font, INT Page )
{
	return ((UFont*)Font)->Pages(Page).Texture;
}

static INT CanvasFontPageCharCount( void* Font, INT Page )
{
	return ((UFont*)Font)->Pages(Page).Characters.Num();
}

static Hp2FontChar CanvasFontPageChar( void* Font, INT Page, INT Index )
{
	const FFontCharacter& Glyph = ((UFont*)Font)->Pages(Page).Characters(Index);
	Hp2FontChar R = { Glyph.StartU, Glyph.StartV, Glyph.USize, Glyph.VSize };
	return R;
}

static BYTE CanvasFontHasName( void* Font )
{
	return ((UFont*)Font)->FontName.Len() > 0;
}

static void CanvasSlotReset( void* Slots )
{
	FTextureInfo* S = (FTextureInfo*)Slots;
	for( INT i=0; i<HP2_CANVAS_TEXT_SLOTS; i++ )
		S[i].Texture = NULL;
}

static void* CanvasSlotTexture( void* Slots, INT Slot )
{
	return ((FTextureInfo*)Slots)[Slot].Texture;
}

static void CanvasSlotLock( void* Canvas, void* Slots, INT Slot, void* Tex )
{
	UCanvas* C = (UCanvas*)Canvas;
	FTextureInfo* S = (FTextureInfo*)Slots;
	((UTexture*)Tex)->Lock( S[Slot], C->Viewport->CurrentTime, 0, C->Viewport->RenDev );
}

static void CanvasSlotUnlock( void* Slots, INT Slot )
{
	FTextureInfo* S = (FTextureInfo*)Slots;
	if( S[Slot].Texture )
		S[Slot].Texture->Unlock( S[Slot] );
}

static void CanvasDrawTileSlot( void* Canvas, void* Slots, INT Slot, const Hp2CanvasTileRect* Rect, Hp2Plane Color, uint32_t Flags )
{
	UCanvas* C = (UCanvas*)Canvas;
	FTextureInfo* S = (FTextureInfo*)Slots;
	C->Viewport->RenDev->DrawTile( C->Frame, S[Slot], Rect->x, Rect->y, Rect->xl, Rect->yl, Rect->u, Rect->v, Rect->ul, Rect->vl, NULL, C->Z, CanvasToFPlane(Color), FPlane(0,0,0,0), Flags );
}

static void CanvasDrawTileTexture( void* Canvas, const Hp2CanvasTileDraw* Draw )
{
	UCanvas* C = (UCanvas*)Canvas;
	C->DrawTile( (UTexture*)Draw->tex, Draw->rect.x, Draw->rect.y, Draw->rect.xl, Draw->rect.yl, Draw->rect.u, Draw->rect.v, Draw->rect.ul, Draw->rect.vl, (FSpanBuffer*)Draw->span, Draw->z, CanvasToFPlane(Draw->color), CanvasToFPlane(Draw->fog), Draw->flags );
}

static Hp2CanvasSize CanvasTextureSize( void* Tex )
{
	Hp2CanvasSize R = { ((UTexture*)Tex)->USize, ((UTexture*)Tex)->VSize };
	return R;
}

// Log text is byte-identical to the original TEXT() literals.
static void CanvasLog( void* Stack, uint32_t Code )
{
	FFrame& S = *(FFrame*)Stack;
	switch( Code )
	{
		case HP2_CANVAS_LOG_DRAW_TEXT_NO_FONT:           S.Logf( TEXT("DrawText: No font") ); break;
		case HP2_CANVAS_LOG_DRAW_TEXT_CLIPPED_NO_FONT:   S.Logf( TEXT("DrawTextClipped: No font") ); break;
		case HP2_CANVAS_LOG_TEXT_SIZE_NO_FONT:           S.Logf( TEXT("TextSize: No font") ); break;
		default: appErrorf( TEXT("Hp2CanvasServices: unknown log code %u"), Code ); break;
	}
}

const Hp2CanvasServices GHp2CanvasServices =
{
	CanvasUIScale,
	CanvasLanguageIsInt,
	CanvasViewportDrawString,
	CanvasTryNativeText,
	CanvasFontCharsPerPage,
	CanvasFontRemapChar,
	CanvasFontPageCount,
	CanvasFontPageTexture,
	CanvasFontPageCharCount,
	CanvasFontPageChar,
	CanvasFontHasName,
	CanvasSlotReset,
	CanvasSlotTexture,
	CanvasSlotLock,
	CanvasSlotUnlock,
	CanvasDrawTileSlot,
	CanvasDrawTileTexture,
	CanvasTextureSize,
	CanvasLog,
};
