/*=============================================================================
	UnGame.h: Unreal game class.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "UnReplay.h"

/*-----------------------------------------------------------------------------
	Unreal game engine.
-----------------------------------------------------------------------------*/
// Parse and consume the startup -LOAD slot at most once per process.
ENGINE_API UBOOL appConsumeCommandLineLoadSlot( const TCHAR* CmdLine, INT& Slot );
// Build the relative FURL option used for both startup and in-game save loads.
ENGINE_API UBOOL appFormatLoadGameURL( INT Slot, TCHAR* Out, INT OutCapacity );


//
// The Unreal game engine.
//
class ENGINE_API UGameEngine : public UEngine
{
	DECLARE_CLASS(UGameEngine,UEngine,CLASS_Config|CLASS_Transient,Engine)

	// Variables.
	ULevel*			GLevel;
	ULevel*			GEntry;
	UPendingLevel*	GPendingLevel;
	FURL			LastURL;
	TArray<FString> ServerActors;
	TArray<FString> ServerPackages;
	FLOAT			FrameRateLimit;
	UViewport*		GViewport;
	FReplay		    Replay;
	bool			GIsDrawing;

	// Constructors.
	UGameEngine();
	void StaticConstructor();
	static FLOAT GetConsoleUIScale( FLOAT Width, FLOAT Height, FLOAT UserScale );

	// UObject interface.
	void Serialize( FArchive& Ar );
	void Destroy();

	// UEngine interface.
	void Init();
	void Exit();
	void Tick( FLOAT DeltaSeconds );
	void Draw( UViewport* Viewport, UBOOL Blit=1, BYTE* HitData=NULL, INT* HitSize=NULL );
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar=*GLog );
	UBOOL Key( UViewport* Viewport, EInputKey Key );
	UBOOL InputEvent( UViewport* Viewport, EInputKey iKey, EInputAction State, FLOAT Delta );
	void MouseDelta( UViewport*, DWORD, FLOAT, FLOAT );
	void MousePosition( class UViewport*, DWORD, FLOAT, FLOAT );
	void Click( UViewport*, DWORD, FLOAT, FLOAT );
	void SetClientTravel( UPlayer* Viewport, const TCHAR* NextURL, UBOOL bItems, ETravelType TravelType );
	FLOAT GetMaxTickRate();
	INT ChallengeResponse( INT Challenge );
	void SetProgress( const TCHAR* Str1, const TCHAR* Str2, FLOAT Seconds );

	// UGameEngine interface.
	virtual UBOOL Browse( FURL URL, const TMap<FString,FString>* TravelInfo, FString& Error );
	virtual ULevel* LoadMap( const FURL& URL, UPendingLevel* Pending, const TMap<FString,FString>* TravelInfo, FString& Error );
	virtual void SaveGame( INT Position );

	virtual void CancelPending();
	virtual void PaintProgress();
	virtual void UpdateConnectingMessage();
	virtual void BuildServerMasterMap( UNetDriver* NetDriver, ULevel* InLevel );
	virtual void NotifyLevelChange();
	void FixUpLevel();
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
