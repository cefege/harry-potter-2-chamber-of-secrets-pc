/*=============================================================================
	Hp2ScrTexServices.cpp: Engine callbacks used by the Rust UScriptedTexture port
	(crates/hp2-engine/src/scr_tex.rs). Each entry is the original expression from
	UnScrTex.cpp. Rust owns the control flow. UScriptedTexture's private members
	are reached through friend struct Hp2ScrTexAccess (declared in UnScrTex.h).
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2ScrTexServices.h"

// Colours cross the boundary as FColor::Int4, so the union must be exactly 4 bytes.
static_assert( sizeof(FColor) == 4, "Hp2ScrTexServices: FColor must be 4 bytes" );

struct Hp2ScrTexAccess
{
	// UFont
	static int32_t FontCharsPerPage( void* Font )
	{
		return ((UFont*)Font)->CharactersPerPage;
	}
	static int32_t FontRemapChar( void* Font, int32_t Ch )
	{
		return (TCHARU)((UFont*)Font)->RemapChar( (TCHAR)Ch );
	}
	static int32_t FontPageCount( void* Font )
	{
		return ((UFont*)Font)->Pages.Num();
	}
	static void* FontPageTexture( void* Font, int32_t Page )
	{
		return ((UFont*)Font)->Pages(Page).Texture;
	}
	static int32_t FontPageCharCount( void* Font, int32_t Page )
	{
		return ((UFont*)Font)->Pages(Page).Characters.Num();
	}
	static Hp2FontChar FontPageChar( void* Font, int32_t Page, int32_t Index )
	{
		const FFontCharacter& Glyph = ((UFont*)Font)->Pages(Page).Characters(Index);
		Hp2FontChar Result = { Glyph.StartU, Glyph.StartV, Glyph.USize, Glyph.VSize };
		return Result;
	}

	// Text (TCHAR array; Rust stops at the first zero)
	static int32_t TextChar( const void* Text, int32_t Index )
	{
		return ((const TCHAR*)Text)[Index];
	}

	// UTexture (any texture, including locked sources)
	static int32_t TexUSize( void* Tex )
	{
		return ((UTexture*)Tex)->USize;
	}
	static int32_t TexVSize( void* Tex )
	{
		return ((UTexture*)Tex)->VSize;
	}
	static int32_t TexUBits( void* Tex )
	{
		return ((UTexture*)Tex)->UBits;
	}
	static int32_t TexVBits( void* Tex )
	{
		return ((UTexture*)Tex)->VBits;
	}
	static void* TexMip0( void* Tex )
	{
		return &((UTexture*)Tex)->Mips(0).DataArray(0);
	}
	static void TexPaletteColors( void* Tex, void* Out )
	{
		FColor* Colors = (FColor*)Out;
		for( INT i=0; i<256; i++ )
			Colors[i] = ((UTexture*)Tex)->Palette->Colors(i);
	}
	static void* TexGetLocal( void* Obj, void* Tex )
	{
		return ((UTexture*)Tex)->Get( ((UScriptedTexture*)Obj)->LocalTime );
	}
	static void TexLockLocal( void* Obj, void* Tex, void* Info )
	{
		((UTexture*)Tex)->Lock( *(FTextureInfo*)Info, ((UScriptedTexture*)Obj)->LocalTime, 0, NULL );
	}
	static void TexLockNow( void* Tex, void* Info )
	{
		((UTexture*)Tex)->Lock( *(FTextureInfo*)Info, appSeconds(), 0, NULL );
	}
	static void TexUnlock( void* Tex, void* Info )
	{
		((UTexture*)Tex)->Unlock( *(FTextureInfo*)Info );
	}
	static void* InfoMip0( void* Info )
	{
		return ((FTextureInfo*)Info)->Mips[0]->DataPtr;
	}

	// UScriptedTexture
	static int32_t SelfUSize( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->USize;
	}
	static int32_t SelfVSize( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->VSize;
	}
	static int32_t SelfUBits( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->UBits;
	}
	static int32_t SelfVBits( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->VBits;
	}
	static void* SelfMip0( void* Obj )
	{
		return &((UScriptedTexture*)Obj)->Mips(0).DataArray(0);
	}
	static void* SelfLocalBitmap( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->LocalSourceBitmap;
	}
	// Scalar delete of new[] is kept from the original PostLoad (undefined behaviour, see scr_tex.rs).
	static void* SelfAllocLocalBitmap( void* Obj, int32_t Bytes )
	{
		UScriptedTexture* ScrTex = (UScriptedTexture*)Obj;
		if( ScrTex->LocalSourceBitmap )
			delete ScrTex->LocalSourceBitmap;
		ScrTex->LocalSourceBitmap = new BYTE[Bytes];
		return ScrTex->LocalSourceBitmap;
	}
	static void* SelfSourceTexture( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->SourceTexture;
	}
	static void* SelfOldSource( void* Obj )
	{
		return ((UScriptedTexture*)Obj)->OldSourceTex;
	}
	static void SelfSetOldSource( void* Obj, void* Tex )
	{
		((UScriptedTexture*)Obj)->OldSourceTex = (UTexture*)Tex;
	}
	static void SelfCopyPaletteFrom( void* Obj, void* Tex )
	{
		((UScriptedTexture*)Obj)->Palette = ((UTexture*)Tex)->Palette;
	}
	static void SelfPaletteColors( void* Obj, void* Out )
	{
		FColor* Colors = (FColor*)Out;
		UPalette* Pal = ((UScriptedTexture*)Obj)->Palette;
		for( INT i=0; i<256; i++ )
			Colors[i] = Pal->Colors(i);
	}
	static uint8_t SelfPaletteBestMatch( void* Obj, uint32_t Color )
	{
		FColor C;
		C.Int4 = Color;
		return ((UScriptedTexture*)Obj)->Palette->BestMatch( C, 0 );
	}

	// (*PalMap)(Index) from DrawTile's palette path. Rust passes a null map on the
	// bUseColor=1, bMasked=0 path, so this faults at the original dereference.
	static uint8_t PaletteMapLookup( void* Map, uint8_t Index )
	{
		TArray<BYTE>* PalMap = (TArray<BYTE>*)Map;
		return (*PalMap)( Index );
	}

	static void SelfNotifyRender( void* Obj )
	{
		UScriptedTexture* ScrTex = (UScriptedTexture*)Obj;
		if( ScrTex->NotifyActor )
			ScrTex->NotifyActor->eventRenderTexture( ScrTex );
	}

	// Script log. The message text is byte-identical to the original Stack.Logf calls.
	static void Log( void* Stack, uint32_t Code )
	{
		FFrame& S = *(FFrame*)Stack;
		switch( Code )
		{
			case HP2_SCR_TEX_LOG_DRAW_TEXT_NO_FONT:
				S.Logf( TEXT("DrawText: No font") );
				break;
			case HP2_SCR_TEX_LOG_REPLACE_MISSING_TEXTURE:
				S.Logf( TEXT("ReplaceTexture: Missing Texture") );
				break;
			case HP2_SCR_TEX_LOG_REPLACE_DIMENSIONS:
				S.Logf( TEXT("ReplaceTexture: Dimensions are different") );
				break;
			case HP2_SCR_TEX_LOG_DRAW_TILE_MISSING_TEXTURE:
				S.Logf( TEXT("DrawTile: Missing Texture") );
				break;
			case HP2_SCR_TEX_LOG_TEXT_SIZE_NO_FONT:
				S.Logf( TEXT("TextSize: No font") );
				break;
			default:
				appErrorf( TEXT("Hp2ScrTexServices: unknown log code %u"), Code );
		}
	}
};

const Hp2ScrTexServices GHp2ScrTexServices =
{
	Hp2ScrTexAccess::FontCharsPerPage,
	Hp2ScrTexAccess::FontRemapChar,
	Hp2ScrTexAccess::FontPageCount,
	Hp2ScrTexAccess::FontPageTexture,
	Hp2ScrTexAccess::FontPageCharCount,
	Hp2ScrTexAccess::FontPageChar,
	Hp2ScrTexAccess::TextChar,
	Hp2ScrTexAccess::TexUSize,
	Hp2ScrTexAccess::TexVSize,
	Hp2ScrTexAccess::TexUBits,
	Hp2ScrTexAccess::TexVBits,
	Hp2ScrTexAccess::TexMip0,
	Hp2ScrTexAccess::TexPaletteColors,
	Hp2ScrTexAccess::TexGetLocal,
	Hp2ScrTexAccess::TexLockLocal,
	Hp2ScrTexAccess::TexLockNow,
	Hp2ScrTexAccess::TexUnlock,
	Hp2ScrTexAccess::InfoMip0,
	Hp2ScrTexAccess::SelfUSize,
	Hp2ScrTexAccess::SelfVSize,
	Hp2ScrTexAccess::SelfUBits,
	Hp2ScrTexAccess::SelfVBits,
	Hp2ScrTexAccess::SelfMip0,
	Hp2ScrTexAccess::SelfLocalBitmap,
	Hp2ScrTexAccess::SelfAllocLocalBitmap,
	Hp2ScrTexAccess::SelfSourceTexture,
	Hp2ScrTexAccess::SelfOldSource,
	Hp2ScrTexAccess::SelfSetOldSource,
	Hp2ScrTexAccess::SelfCopyPaletteFrom,
	Hp2ScrTexAccess::SelfPaletteColors,
	Hp2ScrTexAccess::SelfPaletteBestMatch,
	Hp2ScrTexAccess::SelfNotifyRender,
	Hp2ScrTexAccess::PaletteMapLookup,
	Hp2ScrTexAccess::Log,
};
