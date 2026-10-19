/*=============================================================================
	UnScrTex.cpp: Unreal scripted texture class
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Jack Porter
============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "Hp2ScrTexServices.h"

/*-----------------------------------------------------------------------------
	UScriptedTexture.
	The logic (font drawing, tile blitting, glyph sizing, texture replacement,
	PostLoad resampling, Tick copy) is in crates/hp2-engine/src/scr_tex.rs. This
	file keeps the UObject shell and the exec thunks; engine state is reached
	through GHp2ScrTexServices (Src/Hp2ScrTexServices.cpp).
-----------------------------------------------------------------------------*/

// Builds the context the Rust side receives for one call. Stack is NULL outside
// natives, Info is NULL where no texture is locked.
static Hp2ScrTexCtx ScrTexCtx( UScriptedTexture* ScrTex, void* Stack, void* Info, void* PaletteMap )
{
	Hp2ScrTexCtx Ctx = { &GHp2ScrTexServices, ScrTex, Stack, Info, PaletteMap };
	return Ctx;
}

UScriptedTexture::UScriptedTexture()
:	UTexture()
{	
	guard(UScriptedTexture::UScriptedTexture);

	// Set appropriate texture flags
	bParametric = 1;
	bRealtime   = 1;

	SourceTexture = NULL;
	OldSourceTex = NULL;
	LocalSourceBitmap = NULL;
	NotifyActor = NULL;
	PaletteMap = hp2_rs_engine_scr_tex_palette_map_new();

	unguard;
}


void UScriptedTexture::Destroy()
{
	guard(UScriptedTexture::Destroy);
	hp2_rs_engine_scr_tex_palette_map_free( PaletteMap );
	if( LocalSourceBitmap )
		delete LocalSourceBitmap;

	Super::Destroy();
	
	unguard;
}

void UScriptedTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UScriptedTexture::Init);

	// Init base class.
	UTexture::Init( InUSize, InVSize );

    unguard;
}

void UScriptedTexture::PostLoad()
{
	guard(UScriptedTexture::PostLoad);

	UTexture::PostLoad();

	FTextureInfo Info;
	Hp2ScrTexCtx Ctx = ScrTexCtx( this, NULL, &Info, PaletteMap );
	hp2_rs_engine_scr_tex_post_load( &Ctx );

	unguard;
}

void UScriptedTexture::Tick(FLOAT DeltaSeconds)
{
	BYTE* DestBitmap;

	guard(UScriptedTexture::Tick);

	LocalTime = appSeconds().GetFloat();

	// Init base class.
	Super::Tick(DeltaSeconds);
	DestBitmap = &Mips(0).DataArray(0);

	check(DestBitmap);

	Hp2ScrTexCtx Ctx = ScrTexCtx( this, NULL, NULL, PaletteMap );
	hp2_rs_engine_scr_tex_tick( &Ctx );

	unguard;
}

/*-----------------------------------------------------------------------------
	Natives.
-----------------------------------------------------------------------------*/

void UScriptedTexture::execDrawText( FFrame& Stack, RESULT_DECL )
{
	guard(UScriptedTexture::execDrawText);
	P_GET_FLOAT(X);
	P_GET_FLOAT(Y);
	P_GET_STR(InText);
	P_GET_OBJECT(UFont,Font);
	P_FINISH;

	FTextureInfo Info;
	Hp2ScrTexCtx Ctx = ScrTexCtx( this, &Stack, &Info, PaletteMap );
	hp2_rs_engine_scr_tex_draw_text( &Ctx, X, Y, (void*)*InText, Font );

	unguardexec;
}
IMPLEMENT_FUNCTION( UScriptedTexture, 472, execDrawText );

void UScriptedTexture::execDrawTile( FFrame& Stack, RESULT_DECL )
{
	guard(UScriptedTexture::execDrawTile);
	P_GET_FLOAT(X);
	P_GET_FLOAT(Y);
	P_GET_FLOAT(XL);
	P_GET_FLOAT(YL);
	P_GET_FLOAT(U);
	P_GET_FLOAT(V);
	P_GET_FLOAT(UL);
	P_GET_FLOAT(VL);
	P_GET_OBJECT(UTexture,Tex);
	P_GET_UBOOL(bMasked);
	P_FINISH;

	FTextureInfo Info;
	Hp2ScrTexCtx Ctx = ScrTexCtx( this, &Stack, &Info, PaletteMap );
	hp2_rs_engine_scr_tex_draw_tile( &Ctx, X, Y, XL, YL, U, V, UL, VL, Tex, bMasked );

	unguardexec;
}
IMPLEMENT_FUNCTION( UScriptedTexture, 473, execDrawTile );

void UScriptedTexture::execDrawColoredText( FFrame& Stack, RESULT_DECL )
{
	// Label kept from the original file (it was copy-pasted from execDrawText).
	guard(UScriptedTexture::execDrawText);
	P_GET_FLOAT(X);
	P_GET_FLOAT(Y);
	P_GET_STR(InText);
	P_GET_OBJECT(UFont,Font);
	P_GET_STRUCT(FColor,Color);
	P_FINISH;

	FTextureInfo Info;
	Hp2ScrTexCtx Ctx = ScrTexCtx( this, &Stack, &Info, PaletteMap );
	hp2_rs_engine_scr_tex_draw_colored_text( &Ctx, X, Y, (void*)*InText, Font, GET_COLOR_DWORD(Color) );

	unguardexec;
}
IMPLEMENT_FUNCTION( UScriptedTexture, 474, execDrawColoredText );

void UScriptedTexture::execReplaceTexture( FFrame& Stack, RESULT_DECL )
{
	guard(UScriptedTexture::execReplaceTexture);
	P_GET_OBJECT(UTexture,Tex);
	P_FINISH;

	FTextureInfo Info;
	Hp2ScrTexCtx Ctx = ScrTexCtx( this, &Stack, &Info, PaletteMap );
	hp2_rs_engine_scr_tex_replace_texture( &Ctx, Tex );

	unguardexec;
}
IMPLEMENT_FUNCTION( UScriptedTexture, 475, execReplaceTexture );

void UScriptedTexture::execTextSize( FFrame& Stack, RESULT_DECL )
{
	guard(UScriptedTexture::execTextSize);
	P_GET_STR(InText);
	P_GET_FLOAT_REF(XL);
	P_GET_FLOAT_REF(YL);
	P_GET_OBJECT(UFont,Font);
	P_FINISH;

	Hp2ScrTexCtx Ctx = ScrTexCtx( this, &Stack, NULL, PaletteMap );
	hp2_rs_engine_scr_tex_text_size( &Ctx, (void*)*InText, Font, XL, YL );

	unguardexec;
}
IMPLEMENT_FUNCTION( UScriptedTexture, 476, execTextSize );

IMPLEMENT_CLASS(UScriptedTexture);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
