/*=============================================================================
	UnScrTex.h: Unreal scripted texture class
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Jack Porter
=============================================================================*/

class ENGINE_API  UScriptedTexture : public UTexture
{
	DECLARE_CLASS(UScriptedTexture,UTexture,0,Engine)

	// Constructors. Executed on both instantation and reloading of an object.
	UScriptedTexture();

	// UObject interface.	
	void PostLoad();
	void PostEditChange() {PostLoad();}
	void Destroy();

	// UTexture interface.
	void Init( INT InUSize, INT InVSize );
	void Tick(FLOAT DeltaSeconds);

	// Variables.
	AActor*			NotifyActor;
    UTexture*		SourceTexture;

	// Natives.
	DECLARE_FUNCTION(execDrawText)
	DECLARE_FUNCTION(execDrawTile)
	DECLARE_FUNCTION(execDrawColoredText)
	DECLARE_FUNCTION(execReplaceTexture)
	DECLARE_FUNCTION(execTextSize)

private:
	friend struct Hp2ScrTexAccess;
	UTexture*		OldSourceTex;
	BYTE*			LocalSourceBitmap;
	void*			PaletteMap;		// Rust-owned palette cache (hp2_engine::scr_tex::PaletteCache)
	FLOAT			LocalTime;
};


/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
