/*=============================================================================
	UnLevel.cpp: Level-related functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"

#include <cstdint>
#include <stdlib.h>

#if defined(__GNUC__)
extern void HP2ActorSlotDumpBeginRawLevelActorTable( ULevelBase* Level ) __attribute__((weak));
extern void HP2ActorSlotDumpRawLevelActor( ULevelBase* Level, INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported ) __attribute__((weak));
extern void HP2ActorSlotDumpEndRawLevelActorTable( ULevelBase* Level ) __attribute__((weak));
#else
extern void HP2ActorSlotDumpBeginRawLevelActorTable( ULevelBase* Level );
extern void HP2ActorSlotDumpRawLevelActor( ULevelBase* Level, INT Slot, AActor* Actor, INT ReferenceOffsetBefore, INT ReferenceOffsetAfter, INT CompactPackageIndex, UBOOL CompactPackageIndexCaptured, UBOOL CompactPackageIndexSupported );
extern void HP2ActorSlotDumpEndRawLevelActorTable( ULevelBase* Level );
#endif

/*-----------------------------------------------------------------------------
	ULevelBase implementation.
-----------------------------------------------------------------------------*/

ULevelBase::ULevelBase( UEngine* InEngine, const FURL& InURL )
:	URL( InURL )
,	Engine( InEngine )
,	Actors( this )
,	DemoRecDriver( NULL )
{}

void ULevelBase::Serialize( FArchive& Ar )
{
	guard(ULevelBase::Serialize);
	Super::Serialize(Ar);
	if( Ar.IsTrans() )
	{
		Ar << Actors;
	}
	else
	{
		//oldver Old-format actor list.
		INT DbNum=Actors.Num(), DbMax=DbNum;
		Actors.CountBytes( Ar );
		Ar << DbNum << DbMax;
		if( Ar.IsLoading() )
		{
			Actors.Empty( DbNum );
			Actors.Add( DbNum );
		}
		const char* ActorSlotDumpPath = getenv( "HP2_ACTOR_SLOT_DUMP" );
		const UBOOL CaptureRawActorSlots = Ar.IsLoading()
			&& ActorSlotDumpPath && ActorSlotDumpPath[0]
			&& HP2ActorSlotDumpBeginRawLevelActorTable && HP2ActorSlotDumpRawLevelActor && HP2ActorSlotDumpEndRawLevelActorTable;
		if( CaptureRawActorSlots )
			HP2ActorSlotDumpBeginRawLevelActorTable( this );
		for( INT i=0; i<Actors.Num(); i++ )
		{
			FActorSlotCompactIndexTrace CompactIndexTrace;
			CompactIndexTrace.Index       = INDEX_NONE;
			CompactIndexTrace.OffsetAfter = INDEX_NONE;
			CompactIndexTrace.Captured    = 0;
			const INT ReferenceOffsetBefore = CaptureRawActorSlots ? Ar.Tell() : INDEX_NONE;
			const UBOOL CaptureCompactPackageIndex = CaptureRawActorSlots && Ar.SupportsActorSlotCompactIndexTrace();
			FActorSlotCompactIndexTrace* PreviousCompactIndexTrace = NULL;
			if( CaptureCompactPackageIndex )
				PreviousCompactIndexTrace = Ar.SetActorSlotCompactIndexTrace( &CompactIndexTrace );
			Ar << Actors(i);
			if( CaptureCompactPackageIndex )
				Ar.SetActorSlotCompactIndexTrace( PreviousCompactIndexTrace );
			if( CaptureRawActorSlots )
				HP2ActorSlotDumpRawLevelActor( this, i, Actors(i), ReferenceOffsetBefore, CompactIndexTrace.OffsetAfter, CompactIndexTrace.Index, CompactIndexTrace.Captured, CaptureCompactPackageIndex );
		}
		if( CaptureRawActorSlots )
			HP2ActorSlotDumpEndRawLevelActorTable( this );
	}
	
	// Level variables.
	Ar << URL;
	if( !Ar.IsLoading() && !Ar.IsSaving() )
	{
		Ar << NetDriver;
		Ar << DemoRecDriver;
	}
	unguard;
}
void ULevelBase::Destroy()
{
	guard(ULevelBase::Destroy);
	if( NetDriver )
	{
		delete NetDriver;
		NetDriver = NULL;
	}
	if( DemoRecDriver)
	{
		delete DemoRecDriver;
		DemoRecDriver = NULL;
	}
	Super::Destroy();
	unguard;
}
void ULevelBase::NotifyProgress( const TCHAR* Str1, const TCHAR* Str2, FLOAT Seconds )
{
	guard(ULevelBase::NotifyProgress);
	Engine->SetProgress( Str1, Str2, Seconds );
	unguard;
}
IMPLEMENT_CLASS(ULevelBase);

/*-----------------------------------------------------------------------------
	Level creation & emptying.
-----------------------------------------------------------------------------*/

//
//	Create a new level and allocate all objects needed for it.
//	Call with Editor=1 to allocate editor structures for it, also.
//
ULevel::ULevel( UEngine* InEngine, UBOOL InRootOutside )
:	ULevelBase( InEngine )
{
	guard(ULevel::ULevel);

	// Allocate subobjects.
	SetFlags( RF_Transactional );
	Model = new( GetOuter() )UModel( NULL, InRootOutside );
	Model->SetFlags( RF_Transactional );

	// Spawn the level info.
	SpawnActor( ALevelInfo::StaticClass() );
	check(GetLevelInfo());

	// Spawn the default brush.
	ABrush* Temp = SpawnBrush();
	check(Temp==Actors(1));
	Temp->Brush = new( GetOuter(), TEXT("Brush") )UModel( Temp, 1 );
	Temp->SetFlags( RF_NotForClient | RF_NotForServer | RF_Transactional );
	Temp->Brush->SetFlags( RF_NotForClient | RF_NotForServer | RF_Transactional );

	unguard;
}


// *******************************************************************
//
//	HP2 Specific level functions
//
// *******************************************************************

void ULevel::ScreenActorsByGameState( void )
{
	AActor*			pAct;
	APlayerPawn*	pPPawn = NULL;
	FString			CurGState, buffer, token;
	bool			bInCurGState, bHasGState;
	int				i, j, len;

	// *** Get PlayerPawn
	// because the playerPawn has the CurrentState
	for(i=0; i<Actors.Num(); ++i )
	{
		pAct = Actors(i);
		if( pAct && pAct->IsPlayer() )
		{
			pPPawn		= static_cast<APlayerPawn*>(pAct);
			CurGState	= pPPawn->CurrentGameState.Caps();
			break;
		}
	}
	
	// *** Error checking

	// if the current state is not set then return
	if( CurGState == TEXT("NONE") )
		return;

	// If we didn't find the player or the player's CurGState is not valid then return
	if( pPPawn == NULL || (pPPawn->GameStateMasterList.InStr( CurGState ) < 0))
	{
		if( pPPawn == NULL)
			debugf( NAME_Log, TEXT("**** Can't find a valid player!!!! ") );
		else
			debugf( NAME_Log, TEXT("**** Caution: The player pawn has an Invalid CurrentGameState for this package Load."));
		return;
	}
	
	// *** Screen Actors
	// screen them according to the current GameState
	for( i=0; i<Actors.Num(); ++i )
	{
		pAct = Actors(i);
		if( !pAct )
			continue;
		
		// Reset our local vars for the next token
		buffer		 = *pAct->Group;
		buffer		 = buffer.Caps();
		bHasGState	 = false;
		bInCurGState = false;
		
		do
		{
			// See if this actor has an excludeGameState list, and if so see if the CurGState is in it.
			if( pAct->ExcludeGameStates.Len() )
			{
				FString ExcludeGameStates = pAct->ExcludeGameStates.Caps();
				if( ExcludeGameStates.InStr( CurGState ) >= 0 )
				{
					// The CurGState is in the Actors *exclude* GameState list, so bInCurGState = false;
					bHasGState   = true;
					bInCurGState = false;
					break;
				}
			}

			// See if this actor has a GState that is in the Master GState list
			// Is the GState *keyword* in the group name?
			j = buffer.InStr( TEXT("GSTATE") );
			if( j < 0 )
				break;

			// This actor has a GState
			bHasGState = true;
			
			// If we are in the editor reset our bHiddenEd boolean
			if( GIsEditor )
				pAct->bHiddenEd = false;

			// Get our string to start with the "GS" string
			buffer = buffer.Mid( j );
			
			// Get our current token
			len = buffer.InStr(TEXT(","));
			if( len < 0 )
				len = buffer.Len();
			token = buffer.Mid( 0, len );

			if( token.Len() == pPPawn->GameStateTokenLen &&
				pPPawn->GameStateMasterList.InStr( token ) >= 0 )
			{
				// See if this actor's GState is the *current* GState
				// 
				if( token == CurGState &&
					pAct->ExcludeGameStates.InStr( CurGState ) < 0 )
				{
					bInCurGState = true; // This actor belongs in *this* GameState
					break;
				}
			}
			else
			{
				// ERROR: The GState wasn't in the master list (is this a misspelled GState? )
				buffer = pAct->GetName();
				debugf( NAME_Error, TEXT("*** GameState ERROR Actor: %s has an invalid GameState Token: %s!!!"), *buffer, *token );
			}
			
			// Cut out the token
			buffer = buffer.Mid( token.Len() );
		
		}while( 1 );
		
		// Test to see if this actor has a GState and its in the currentGState
		if( bHasGState )
		{
			// Set the bInCurrentGameState boolean
			pAct->bInCurrentGameState = bInCurGState;
			
			// If we are in the game then call the resolveGameState function.
			if( !GIsEditor )
			{
				// Call this actor's OnResolveGameState function so it can handle itself accordingly
				pAct->eventOnResolveGameState();
			}
		}

	}// end for AllActors
}

namespace
{
	static_assert(sizeof(INT) == 4, "Persistent actor counts require 32-bit wire integers");
	static_assert(sizeof(UNICHAR) == 2, "Persistent actor strings require 16-bit wire code units");

	enum { PERSISTENT_WIRE_NAME_CAPACITY = 256 };

	static UBOOL EncodePersistentWireString
	(
		const TCHAR* Source,
		UNICHAR* Wire,
		INT WireCapacity,
		INT& WireBytes
	)
	{
		if( !Source || !Wire || WireCapacity <= 0 )
			return 0;

		INT WireIndex = 0;
		while( *Source )
		{
			if( sizeof(TCHAR) == 2 )
			{
				const UNICHAR Unit = static_cast<UNICHAR>(*Source++);
				if( Unit >= 0xd800 && Unit <= 0xdbff )
				{
					const UNICHAR Low = static_cast<UNICHAR>(*Source++);
					if( Low < 0xdc00 || Low > 0xdfff || WireIndex >= WireCapacity - 2 )
						return 0;
					Wire[WireIndex++] = Unit;
					Wire[WireIndex++] = Low;
				}
				else
				{
					if( (Unit >= 0xdc00 && Unit <= 0xdfff) || WireIndex >= WireCapacity - 1 )
						return 0;
					Wire[WireIndex++] = Unit;
				}
			}
			else if( sizeof(TCHAR) == 4 )
			{
				const std::uint32_t CodePoint = static_cast<std::uint32_t>(*Source++);
				if( CodePoint > 0x10ffff || (CodePoint >= 0xd800 && CodePoint <= 0xdfff) )
					return 0;
				if( CodePoint <= 0xffff )
				{
					if( WireIndex >= WireCapacity - 1 )
						return 0;
					Wire[WireIndex++] = static_cast<UNICHAR>(CodePoint);
				}
				else
				{
					if( WireIndex >= WireCapacity - 2 )
						return 0;
					const std::uint32_t Supplement = CodePoint - 0x10000;
					Wire[WireIndex++] = static_cast<UNICHAR>(0xd800 + (Supplement >> 10));
					Wire[WireIndex++] = static_cast<UNICHAR>(0xdc00 + (Supplement & 0x3ff));
				}
			}
			else
			{
				if( WireIndex >= WireCapacity - 1 )
					return 0;
				Wire[WireIndex++] = ToUnicode(*Source++);
			}
		}

		if( WireIndex + 1 > MAXINT / static_cast<INT>(sizeof(UNICHAR)) )
			return 0;
		Wire[WireIndex++] = 0;
		WireBytes = WireIndex * static_cast<INT>(sizeof(UNICHAR));
		return 1;
	}

	static UBOOL DecodePersistentWireString
	(
		const UNICHAR* Wire,
		INT WireUnits,
		TCHAR* Dest,
		INT DestCapacity
	)
	{
		if( !Wire || WireUnits <= 0 || Wire[WireUnits - 1] != 0 || !Dest || DestCapacity <= 0 )
			return 0;

		INT DestIndex = 0;
		for( INT WireIndex = 0; WireIndex < WireUnits - 1; ++WireIndex )
		{
			const UNICHAR Unit = Wire[WireIndex];
			if( Unit == 0 )
				return 0;

			std::uint32_t CodePoint = Unit;
			if( Unit >= 0xd800 && Unit <= 0xdbff )
			{
				if( ++WireIndex >= WireUnits - 1 )
					return 0;
				const UNICHAR Low = Wire[WireIndex];
				if( Low < 0xdc00 || Low > 0xdfff )
					return 0;
				CodePoint = 0x10000
					+ ((static_cast<std::uint32_t>(Unit) - 0xd800) << 10)
					+ (static_cast<std::uint32_t>(Low) - 0xdc00);
			}
			else if( Unit >= 0xdc00 && Unit <= 0xdfff )
			{
				return 0;
			}

			if( sizeof(TCHAR) == 4 )
			{
				if( DestIndex >= DestCapacity - 1 )
					return 0;
				Dest[DestIndex++] = static_cast<TCHAR>(CodePoint);
			}
			else if( sizeof(TCHAR) == 2 )
			{
				if( CodePoint <= 0xffff )
				{
					if( DestIndex >= DestCapacity - 1 )
						return 0;
					Dest[DestIndex++] = static_cast<TCHAR>(CodePoint);
				}
				else
				{
					if( DestIndex >= DestCapacity - 2 )
						return 0;
					const std::uint32_t Supplement = CodePoint - 0x10000;
					Dest[DestIndex++] = static_cast<TCHAR>(0xd800 + (Supplement >> 10));
					Dest[DestIndex++] = static_cast<TCHAR>(0xdc00 + (Supplement & 0x3ff));
				}
			}
			else
			{
				if( DestIndex >= DestCapacity - 1 )
					return 0;
				Dest[DestIndex++] = FromUnicode(static_cast<UNICHAR>(CodePoint));
			}
		}

		Dest[DestIndex] = 0;
		return 1;
	}

	static UBOOL CanReadPersistentBytes( FArchive& Ar, INT ByteCount )
	{
		const INT Position = Ar.Tell();
		const INT Total = Ar.TotalSize();
		return ByteCount >= 0 && Position >= 0 && Total >= Position && ByteCount <= Total - Position;
	}

	static UBOOL WritePersistentWireUnits( FArchive& Ar, const UNICHAR* Wire, INT WireUnits )
	{
		for( INT Index = 0; Index < WireUnits; ++Index )
		{
			UNICHAR Unit = Wire[Index];
			Ar.ByteOrderSerialize(&Unit, static_cast<INT>(sizeof(Unit)));
			if( Ar.GetError() )
				return 0;
		}
		return 1;
	}

	static UBOOL ReadPersistentWireUnits( FArchive& Ar, UNICHAR* Wire, INT WireUnits )
	{
		if( WireUnits < 0 || WireUnits > MAXINT / static_cast<INT>(sizeof(UNICHAR)) )
			return 0;
		const INT WireBytes = WireUnits * static_cast<INT>(sizeof(UNICHAR));
		if( !CanReadPersistentBytes(Ar, WireBytes) )
			return 0;

		for( INT Index = 0; Index < WireUnits; ++Index )
		{
			Ar.ByteOrderSerialize(&Wire[Index], static_cast<INT>(sizeof(Wire[Index])));
			if( Ar.GetError() )
				return 0;
		}
		return 1;
	}

	static UBOOL WritePersistentWireString
	(
		FArchive& Ar,
		const TCHAR* Source,
		UNICHAR* Wire,
		INT WireCapacity
	)
	{
		INT WireBytes = 0;
		if( !EncodePersistentWireString(Source, Wire, WireCapacity, WireBytes) )
			return 0;
		Ar << WireBytes;
		return !Ar.GetError()
			&& WritePersistentWireUnits(Ar, Wire, WireBytes / static_cast<INT>(sizeof(UNICHAR)));
	}

	static UBOOL ReadPersistentWireString
	(
		FArchive& Ar,
		UNICHAR* Wire,
		INT WireCapacity,
		TCHAR* Dest,
		INT DestCapacity
	)
	{
		INT WireBytes = 0;
		Ar << WireBytes;
		if( Ar.GetError() || WireBytes < static_cast<INT>(sizeof(UNICHAR))
			|| (WireBytes & 1) != 0 )
			return 0;

		const INT WireUnits = WireBytes / static_cast<INT>(sizeof(UNICHAR));
		if( WireUnits > WireCapacity || !ReadPersistentWireUnits(Ar, Wire, WireUnits) )
			return 0;
		return DecodePersistentWireString(Wire, WireUnits, Dest, DestCapacity);
	}

	static UBOOL WritePersistentIdentifier( FArchive& Ar )
	{
		const UNICHAR Identifier[] = { 'P', 'A', '0', 0 };
		return WritePersistentWireUnits(Ar, Identifier, static_cast<INT>(ARRAY_COUNT(Identifier)));
	}

	static UBOOL ReadPersistentIdentifier( FArchive& Ar )
	{
		UNICHAR Identifier[4];
		return ReadPersistentWireUnits(Ar, Identifier, static_cast<INT>(ARRAY_COUNT(Identifier)))
			&& Identifier[0] == 'P'
			&& Identifier[1] == 'A'
			&& (Identifier[2] == '0' || Identifier[2] == '1')
			&& Identifier[3] == 0;
	}
}



#define IF_TRUE_MSG_AND_BREAK(_b,_s)		if((_b)){debugf(NAME_Error,(_s));break;}

bool ULevel::SavePersistentActors( const FString& MapName )
{
	AActor*				pAct;
	FArchive*			pActAr = NULL;
	TArray<AActor*>		ActorsToSave;
	UNICHAR				wireName[PERSISTENT_WIRE_NAME_CAPACITY];
	FString				FinalMapName;
	FString				MapFilename;
	FString				PAFilename;
	SQWORD				FileTime;
	INT					i, iPosPreProp, iPosPostProp, iPosSize, iToSave, iPersistentActors;
	UBOOL				bWireError = 0;
	bool				bError = false;
	
	// Make sure we are NOT in the editor
	if( GIsEditor )
		return false;
	
	//************** CONSTRUCT CORRECT FILENAMES ********************
	// Make sure the final mapName is just that.. the name ex: "startup.unr"
	FinalMapName = appPathLeaf(*MapName);
	if( FinalMapName.InStr( URL.DefaultMapExt ) < 0 )
	{
		FinalMapName += TEXT(".");
		FinalMapName += URL.DefaultMapExt;
	}
	
	// Get the MAP filename
	MapFilename = GSys->MapPath * FinalMapName;
	
	// Get the PA filename 
	PAFilename  = GSys->SaveSlotPath * TEXT("cache") * FinalMapName.Left( FinalMapName.InStr(URL.DefaultMapExt)-1 ) + 
				  TEXT("_pa.") + URL.DefaultSaveExt;
	//****************************************************************
	
	do
	{	
		// go through the level's actor list and find out how many need to be saved
		for(i=0; i<Actors.Num(); ++i )
		{
			pAct = Actors(i);
			if( pAct && pAct->bPersistent )
				ActorsToSave.AddItem( pAct );
		}
		
		// if there are no actors to save then break out
		if( ActorsToSave.Num() == 0 )
			break;
		
		// open our save pa (persistent actors) file
		pActAr = (FArchive*)GFileManager->CreateFileWriter( *PAFilename );
		if( pActAr == NULL || pActAr->GetError() )
		{
			debugf(NAME_Log, TEXT("*** Could not Save Persistent Actor file %s, CreateFileWriter had an error!!!!."), *PAFilename );
			break; // goto cleanUp and return
		}
		
		debugf(NAME_Log, TEXT("*** Starting to Save Persistent Actors for map %s"), *FinalMapName);
		
		// Block 1a.) The first token is a PA file identifier ( "PA0" == unverified PA file, "PA1" == verified PA file )
		if( !WritePersistentIdentifier(*pActAr) )
		{
			debugf(NAME_Error,TEXT("ULevelBase::SavePersistentActors ERROR: Saving map PA file identifier!"));
			bError = true;
			break;
		}
		
		// Block 1b.) The second token in the save file will be the size of the map name then the map name
		if( !WritePersistentWireString(*pActAr, *FinalMapName, wireName, static_cast<INT>(ARRAY_COUNT(wireName))) )
		{
			debugf(NAME_Error,TEXT("ULevelBase::SavePersistentActors ERROR: Saving map name as UTF-16LE!"));
			bError = true;
			break;
		}
		
		// Block 1c.) Save the level's file size
		iToSave = GFileManager->FileSize( *MapFilename );
		(*pActAr) << iToSave;
		IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::SavePersistentActors ERROR: Saving map FileSize that this PA file is for!") ) 
		
		// Block 1d.) Save the level's file time
		FileTime = GFileManager->GetGlobalTime( *MapFilename );
		(*pActAr) << FileTime;
		IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::SavePersistentActors ERROR: Saving map FileTime that this PA file is for!") ) 		
		
		// Block 2.) Save the number of persistent Actors
		iPersistentActors = ActorsToSave.Num();
		(*pActAr) << iPersistentActors;
		IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::SavePersistentActors ERROR: Saving Number of persistent Actors!") ) 
			
		// Block 3.) Save all persistant actors
		for(i=0; i<ActorsToSave.Num(); ++i )
		{
			pAct = ActorsToSave(i);
			
			// Block 3.1i )	Save the size of the name then the name of the actor
			if( !WritePersistentWireString(*pActAr, pAct->GetName(), wireName, static_cast<INT>(ARRAY_COUNT(wireName))) )
			{
				debugf(NAME_Error,TEXT("ULevelBase::SavePersistentActors ERROR: Saving actor name as UTF-16LE!"));
				bError = true;
				bWireError = 1;
				break;
			}
			
			
			// Block 3.2i ) Save a DUMMY var for the size of the taggedProperties
			iPosSize = pActAr->Tell();
			(*pActAr) << iToSave;
			IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::SavePersistentActors ERROR: Saving temp size of taggedProperties!") ) 
			
			
			// Block 3.3i ) Save the Persistence Properties
			iPosPreProp  = pActAr->Tell();
			
			pAct->SerializePersistence( *pActAr );
			IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::SavePersistentActors ERROR: Saving tagged properties of actor!") ) 
						
			iPosPostProp = pActAr->Tell();
			
			
			// Block 3.2i ) Save the REAL size of the tagged Properties
			// We need to seek to the position BEFORE we saved the tagged properties.
			// That way when we load the taggedProperties we will know the size before we load it.
			// We need that information if we wish to skip that actor when loading.
			pActAr->Seek( iPosSize );
			iToSave = iPosPostProp - iPosPreProp;
			(*pActAr) << iToSave;
			pActAr->Seek( iPosPostProp );
			IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::SavePersistentActors ERROR: Saving real size of taggedProperties!") ) 
			
		}
		if( bWireError )
			break;
		
		
		// Block 4.) Save an EOF token
		if( !WritePersistentWireString(*pActAr, TEXT("EOF"), wireName, static_cast<INT>(ARRAY_COUNT(wireName))) )
		{
			debugf(NAME_Error,TEXT("ULevelBase::SavePersistentActors ERROR: Saving EOF token as UTF-16LE!"));
			bError = true;
			break;
		}
		
		debugf(NAME_Log, TEXT("*** Finished Saving Persistent Actors for map %s"), *FinalMapName);

	}while(0);
	
	
	
	// *** clean up and return
	ActorsToSave.Empty();
	
	if( pActAr ) 
	{
		if( pActAr->GetError() ) 
			bError = true;
		delete pActAr;
	}
	
	return bError;
}


bool ULevel::LoadPersistentActors( const FString& MapName )
{
	AActor*		pAct;
	AActor**	ppAct;
	FArchive*	pActAr = NULL;
	FString		FinalMapName;
	FString		MapFilename;
	FString		PAFilename;	
	SQWORD		FileTime;
	INT			i, iPActorsInFile, iToLoad, iPropSize;
	UNICHAR		wireName[PERSISTENT_WIRE_NAME_CAPACITY];
	TCHAR		strName[PERSISTENT_WIRE_NAME_CAPACITY];
	UBOOL		bWireError = 0;
	bool		bError = false;

	TMultiMap<FName,AActor*> ActorsToLoad;

	// Make sure we are NOT in the editor
	if( GIsEditor )
		return false;

	//************** CONSTRUCT CORRECT FILENAMES ********************
	// Make sure the final mapName is just that.. the name ex: "startup.unr"
	FinalMapName = appPathLeaf(*MapName);
	if( FinalMapName.InStr( URL.DefaultMapExt ) < 0 )
	{
		FinalMapName += TEXT(".");
		FinalMapName += URL.DefaultMapExt;
	}
	
	// Get the MAP filename
	MapFilename = GSys->MapPath * FinalMapName;
	
	// Get the PA filename
	// NOTE: only load pa files that have been verified (pa files that don't have "pa_new", but just "_pa" )
	PAFilename  = GSys->SaveSlotPath * TEXT("cache") * FinalMapName.Left( FinalMapName.InStr(URL.DefaultMapExt)-1 ) + 
				  TEXT("_pa.") + URL.DefaultSaveExt;
	//****************************************************************
	
	do
	{
		// Open our actors file (if we have one)
		pActAr = (FArchive*)GFileManager->CreateFileReader( *PAFilename );	
		if( pActAr == NULL || pActAr->GetError() || pActAr->TotalSize() == 0 )
		{
			debugf(NAME_Log, TEXT("*** Could not load Persistent Actor file %s (file is probably missing)."), *PAFilename );
			break; // goto cleanUp and return
		}
		
		debugf(NAME_Log, TEXT("*** Starting to Load Persistent Actors for map %s"), *FinalMapName);
		
		// Create a map of persistent actors we will need to load
		iToLoad = 0;
		for(i=0; i<Actors.Num(); ++i )
		{
			pAct = Actors(i);
			if( pAct && pAct->bPersistent )
			{
				ActorsToLoad.Add( pAct->GetName(), pAct );
				iToLoad++;
			}
		}
		
		// If we have no persistent actors to load then break out
		if( iToLoad == 0 )
			break;

		// Block 1a.) Get the PA file identifier ( "PA0" == unverified PA file, "PA1" == verified PA file )
		if( !ReadPersistentIdentifier(*pActAr) )
		{
			debugf(NAME_Error,TEXT("ULevelBase::LoadPersistentActors ERROR: Invalid or truncated PA file identifier!"));
			bError = true;
			break;
		}
		
		
		// Block 1b.) Get the persistant actor archive's map name that it should affect
		if( !ReadPersistentWireString(*pActAr, wireName, static_cast<INT>(ARRAY_COUNT(wireName)), strName, static_cast<INT>(ARRAY_COUNT(strName))) )
		{
			debugf(NAME_Error,TEXT("ULevelBase::LoadPersistentActors ERROR: Invalid map name UTF-16LE string!"));
			bError = true;
			break;
		}
		
		// Error check the map name
		IF_TRUE_MSG_AND_BREAK(FinalMapName!=FString(strName),TEXT("ULevelBase::LoadPersistentActors ERROR: FinalMapName != Saved map name!") )
		
		// Block 1c.) Load and compare the level's file size
		(*pActAr) << iToLoad;
		IF_TRUE_MSG_AND_BREAK(iToLoad != GFileManager->FileSize(*MapFilename),TEXT("ULevelBase::LoadPersistentActors ERROR: Loading map FileSize that this PA file is for!") ) 
		
		// Block 1d.) Load and compare the level's file time
		(*pActAr) << FileTime;
		IF_TRUE_MSG_AND_BREAK(FileTime != GFileManager->GetGlobalTime(*MapFilename), TEXT("ULevelBase::LoadPersistentActors ERROR: Saving map FileTime that this PA file is for!") ) 
		
		
		// Block 2.) Get the number of actors we will need to load.
		(*pActAr) << iPActorsInFile;
		IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::LoadPersistentActors ERROR: Loading total number of persistent actors!") ) 
		
		
		// ******************************************************************************
		// Table of Conditions/Actions for actors that have bPersistant == true
		// 
		//	X = actor found
		//  O = actor missing
		//
		//	#	level.unr	level_pa.usa	Action
		//	-----------------------------------------------------------------------------
		//	1	X			X				Override the non-transient properites
		//	2	X			O				Destroy actor in level.unr
		//	3	O			X				Create actor from level_a.usa
		//	4	O			O				Do nothing
		//
		// ******************************************************************************
		
		// Block 3.) Go through each actor in the file and sync it with the actor in our level
		for(i=0; i < iPActorsInFile; ++i )
		{
			// Block 3.1i ) Get the Persistent Actor's Name size then the name
			if( !ReadPersistentWireString(*pActAr, wireName, static_cast<INT>(ARRAY_COUNT(wireName)), strName, static_cast<INT>(ARRAY_COUNT(strName))) )
			{
				debugf(NAME_Error,TEXT("ULevelBase::LoadPersistentActors ERROR: Invalid actor name UTF-16LE string!"));
				bError = true;
				bWireError = 1;
				break;
			}
			
			
			// Block 3.2i ) Get the size of this actor's taggedProperties ( because we may need to skip it )
			(*pActAr) << iPropSize;
			IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::LoadPersistentActors ERROR: Loading size of tagged properties!") ) 
			
			
			// Use the map of persistent actors to find out if this actor is currently in the level
			ppAct = ActorsToLoad.Find( strName );
			pAct  = ( ppAct ) ? *ppAct : NULL;
			
			if( pAct == NULL )
			{
				// *********
				// Case # 3: Actor is NOT in the level.unr but is in level_pa.usa
				//
				// Action: Create actor from level_a.usa
				// *********
				debugf(NAME_Log, TEXT("*** Actor is NOT in level %s but is in pa file %s! \n. "), *FinalMapName, *PAFilename );
				
/*				// This commented out code will atempt to spawn an actor that is suppose to be in the level
				
				// A good example of when this code is needed is when a chest spawns some beans that need to be
				// there when you come back into the level

				// Have the actor's properteis placed in this TempActor var
				AActor TempActor;
				TempActor.SerializePersistence( *pActAr );
				IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::LoadPersistentActors ERROR: Loading tagged properties!") ) 
				
				// Spawn this actor based upon information given in our TempActor
				pAct = SpawnActor( TempActor.GetClass(),		// UClass*	Class, 
							FName(TempActor.GetName()),	// FName	InName, 
							TempActor.Owner,			// AActor*	Owner = NULL, 
							TempActor.Instigator,		// APawn*	Instigator = NULL, 
							TempActor.Location,			// FVector	Location,
							TempActor.Rotation,			// FRotator Rotation,
							NULL, // use actors default	// AActor*	Template,
							true );//,					// bool		bNoCollisionFail,
												// bool		bRemoteOwned );
				
				// Destroy our temp actor
				TempActor.Destroy();
*/				
				// Seek past the actor in the level_pa.usa file
				pActAr->Seek( pActAr->Tell() + iPropSize );
				continue;
			}
			
			// Check to make sure our names match
			IF_TRUE_MSG_AND_BREAK( appStrcmp(pAct->GetName(),strName) != 0, 
				TEXT("Level actor's name != persistent actor's name!!!!!") ) 
		
			// Remove this actor from the ActorsToLoad map
			ActorsToLoad.Remove( strName );
			
			// *********
			// Case # 1: Actor is in the level.unr as well as level_a.usa
			//
			// Action: Override the non-transient properites
			// 
			// *********
			// Block 3.3i ) Load the Persistence Properties
			pAct->SerializePersistence( *pActAr );
			IF_TRUE_MSG_AND_BREAK(pActAr->GetError(),TEXT("ULevelBase::LoadPersistentActors ERROR: Loading tagged properties!") ) 
			
			// bScriptInitialized should be set to false so that the actor will be placed 
			// in there proper collision zone during level init at at UGameEngine::LoadMap
			pAct->bScriptInitialized = false;
		}
		if( bWireError )
			break;
		
		
		// Block 4.) Get the EOF token
		if( !ReadPersistentWireString(*pActAr, wireName, static_cast<INT>(ARRAY_COUNT(wireName)), strName, static_cast<INT>(ARRAY_COUNT(strName))) )
		{
			debugf(NAME_Error,TEXT("ULevelBase::LoadPersistentActors ERROR: Invalid EOF UTF-16LE string!"));
			bError = true;
			break;
		}
		
		// Check the EOF token
		IF_TRUE_MSG_AND_BREAK(appStrcmp(TEXT("EOF"),strName)!=0,
			TEXT("The EOF token is corrupt in the persistent actor file!!!!!") ) 
		
		// See if we have any Case # 2 actors. 
		for( TMultiMap<FName,AActor*>::TIterator It(ActorsToLoad); It; ++It )
		{
			// *********
			// Case # 2: Actor is in the level.unr but NOT in level_pa.usa
			//
			// Action: Destroy actor in level.unr
			// *********
			pAct = It.Value();
			if( pAct )
				DestroyActor( pAct );
		}
		
		debugf(NAME_Log, TEXT("*** Finished Loading Persistent Actors for map %s"), *FinalMapName);

	}while(0);

	// *** clean up and return
	ActorsToLoad.Empty();
	
	if( pActAr ) 
	{
		if( pActAr->GetError() ) 
			bError = true;
		delete pActAr;
	}
	
	return bError;
}




void ULevel::ShrinkLevel()
{
	guard(ULevel::Shrink);

	Model->ShrinkModel();
	ReachSpecs.Shrink();

	unguard;
}
void ULevel::DetailChange( UBOOL NewDetail )
{
	guard(ULevel::DetailChange);
	GetLevelInfo()->bHighDetailMode = NewDetail;
	if( GetLevelInfo()->Game )
		GetLevelInfo()->Game->eventDetailChange();
	unguard;
}

/*-----------------------------------------------------------------------------
	Level locking and unlocking.
-----------------------------------------------------------------------------*/

//
// Modify this level.
//
void ULevel::Modify( UBOOL DoTransArrays )
{
	guard(ULevel::Modify);
	UObject::Modify();
	Model->Modify();
	unguard;
}
void ULevel::PostLoad()
{
	guard(ULevel::PostLoad);
	Super::PostLoad();
#if ENGINE_VERSION>230
	for( TObjectIterator<AActor> It; It; ++It )
		if( It->GetOuter()==GetOuter() )
			It->XLevel = this;
#endif
	unguard;
}
void ULevel::SetActorCollision( UBOOL bCollision )
{
	guard(ULevel::SetActorCollision);

	// Init collision if first time through.
	if( bCollision && !Hash )
	{
		FAppRandTracePhaseScope StartupPhaseTrace( "collision_hash" );
		// Init hash.
		guard(StartCollision);
		Hash = GNewCollisionHash();
		for( INT i=0; i<Actors.Num(); i++ )
			if( Actors(i) && Actors(i)->bCollideActors )
				Hash->AddActor( Actors(i) );
		unguard;
	}
	else if( Hash && !bCollision )
	{
		// Destroy hash.
		guard(EndCollision);
		for( INT i=0; i<Actors.Num(); i++ )
			if( Actors(i) && Actors(i)->bCollideActors )
				Hash->RemoveActor( Actors(i), true );
		delete Hash;
		Hash = NULL;
		unguard;
	}

	unguard;
}

/*-----------------------------------------------------------------------------
	Level object implementation.
-----------------------------------------------------------------------------*/

void ULevel::Serialize( FArchive& Ar )
{
	guard(ULevel::Serialize);
	Super::Serialize( Ar );

	FLOAT ApproxTime = TimeSeconds.GetFloat();
	Ar << Model;
	Ar << ReachSpecs;
	Ar << ApproxTime;
	Ar << FirstDeleted;
	for( INT i=0; i<NUM_LEVEL_TEXT_BLOCKS; i++ )
		Ar << TextBlocks[i];
	if( Ar.Ver()>62 )//oldver
	{
		Ar << TravelInfo;
	}
	else if( Ar.Ver()>=61 )
	{
		TArray<FString> Names, Items;
		Ar << Names << Items;
		TravelInfo = TMap<FString,FString>();
		for( INT i=0; i<Names.Num(); i++ )
			TravelInfo.Set( *Names(i), *Items(i) );
	}
	if( Model && !Ar.IsTrans() )
		Ar.Preload( Model );
	if( BrushTracker )
		BrushTracker->CountBytes( Ar );

	unguard;
}


void ULevel::Destroy()
{
	guard(ULevel::Destroy);

	// Free allocated stuff.
	if( Hash )
	{
		delete Hash;
		Hash = NULL; /* Required because actors may try to unhash themselves. */
	}
	if( BrushTracker )
	{
		delete BrushTracker;
		BrushTracker = NULL; /* Required because brushes may clean themselves up. */
	}

	Super::Destroy();
	unguard;
}
IMPLEMENT_CLASS(ULevel);

/*-----------------------------------------------------------------------------
	Reconcile actors and Viewports after loading or creating a new level.

	These functions provide the basic mechanism by which UnrealEd associates
	Viewports and actors together, even when new maps are loaded which contain
	an entirely different set of actors which must be mapped onto the existing 
	Viewports.
-----------------------------------------------------------------------------*/

//
// Remember actors.
//
void ULevel::RememberActors()
{
	guard(ULevel::RememberActors);
	if( Engine->Client )
	{
		for( INT i=0; i<Engine->Client->Viewports.Num(); i++ )
		{
			UViewport* Viewport			= Engine->Client->Viewports(i);
			Viewport->SavedOrthoZoom	= Viewport->Actor->OrthoZoom;
			Viewport->SavedFovAngle		= Viewport->Actor->FovAngle;
			Viewport->SavedShowFlags	= Viewport->Actor->ShowFlags;
			Viewport->SavedRendMap		= Viewport->Actor->RendMap;
			Viewport->SavedMisc1		= Viewport->Actor->Misc1;
			Viewport->SavedMisc2		= Viewport->Actor->Misc2;
			Viewport->Actor				= NULL;
		}
	}
	unguard;
}

//
// Reconcile actors.  This is called after loading a level.
// It attempts to match each existing Viewport to an actor in the newly-loaded
// level.  If no decent match can be found, creates a new actor for the Viewport.
//
void ULevel::ReconcileActors()
{
	guard(ULevel::ReconcileActors);
	check(GIsEditor);

	// Dissociate all actor Viewports and remember their view properties.
	for( INT i=0; i<Actors.Num(); i++ )
		if( Actors(i) && Actors(i)->IsA(APlayerPawn::StaticClass()) )
			if( ((APlayerPawn*)Actors(i))->Player )
				((APlayerPawn*)Actors(i))->Player = NULL;

	// Match Viewports and Viewport-actors with identical names.
	INT i;
	guard(MatchIdentical);
	for( i=0; i<Engine->Client->Viewports.Num(); i++ )
	{
		UViewport* Viewport = Engine->Client->Viewports(i);
		check(Viewport->Actor==NULL);
		for( INT j=0; j<Actors.Num(); j++ )
		{
			AActor* Actor = Actors(j);
			if( Actor && Actor->IsA(ACamera::StaticClass()) && appStricmp(*Actor->Tag,Viewport->GetName())==0 )
			{
				debugf( NAME_Log, TEXT("Matched Viewport %s"), Viewport->GetName() );
				Viewport->Actor         = (APlayerPawn *)Actor;
				Viewport->Actor->Player = Viewport;
				break;
			}
		}
	}
	unguard;

	// Match up all remaining Viewports to actors.
	guard(MatchEditorOther);
	for( i=0; i<Engine->Client->Viewports.Num(); i++ )
	{
		// Hook Viewport up to an existing actor or createa a new one.
		UViewport* Viewport = Engine->Client->Viewports(i);
		if( !Viewport->Actor )
			SpawnViewActor( Viewport );
	}
	unguard;

	// Handle remaining unassociated view actors.
	guard(KillViews);
	for( i=0; i<Actors.Num(); i++ )
	{
		ACamera* View = Cast<ACamera>(Actors(i));
		if( View )
		{
			UViewport* Viewport = Cast<UViewport>(View->Player);
			if( Viewport )
			{
				UViewport* Viewport	= (UViewport*)View->Player;
				View->ClearFlags( RF_Transactional );
				View->OrthoZoom		= Viewport->SavedOrthoZoom;	
				View->FovAngle		= Viewport->SavedFovAngle;
				View->ShowFlags		= Viewport->SavedShowFlags;
				View->RendMap		= Viewport->SavedRendMap;
				View->Misc1			= Viewport->SavedMisc1;
				View->Misc2			= Viewport->SavedMisc2;
			}
			else DestroyActor( View );
		}
	}
	unguard;

	unguard;
}

/*-----------------------------------------------------------------------------
	ULevel command-line.
-----------------------------------------------------------------------------*/

UBOOL ULevel::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(ULevel::Exec);
	if( NetDriver && NetDriver->Exec( Cmd, Ar ) )
	{
		return 1;
	}
	else if( DemoRecDriver && DemoRecDriver->Exec( Cmd, Ar ) )
	{
		return 1;
	}
	else if( ParseCommand( &Cmd, TEXT("DEMOREC") ) )
	{
		FURL URL;
		if( ParseToken( Cmd, URL.Map, 0 ) )
		{
			if( URL.Map.Right(4)!=TEXT(".dem") )
				URL.Map += TEXT(".dem");
			debugf( TEXT("Attempting to record demo %s"), *URL.Map );
			UClass* DemoDriverClass = StaticLoadClass( UNetDriver::StaticClass(), NULL, TEXT("ini:Engine.Engine.DemoRecordingDevice"), NULL, LOAD_NoFail, NULL );
			DemoRecDriver           = ConstructObject<UNetDriver>( DemoDriverClass );
			FString Error;
			if( !DemoRecDriver->InitListen( this, URL, Error ) )
			{
				Ar.Logf( TEXT("Demo recording failed: %s"), *Error );//!!localize!!
				delete DemoRecDriver;
				DemoRecDriver = NULL;
			}
			else
				Ar.Logf( TEXT("Demo recording started to %s"), *URL.Map );
		}
		else
			Ar.Log( TEXT("You must specify a filename") );//!!localize!!
		return 1;
	}
	else if( ParseCommand( &Cmd, TEXT("DEMOPLAY") ) )
	{
		FString Temp;
		if( ParseToken( Cmd, Temp, 0 ) )
		{
			INT i = Temp.Caps().InStr(TEXT(".DEM"));
			if( i != -1)
				Temp = Temp.Left(i) + Temp.Mid(i+4);
			FURL URL(NULL, *Temp, TRAVEL_Absolute);
			URL.Map += TEXT(".dem");
			debugf( TEXT("Attempting to play demo %s"), *URL.Map );
			UGameEngine* GameEngine = CastChecked<UGameEngine>( Engine );
			if( GameEngine->GPendingLevel )
				GameEngine->CancelPending();
			GameEngine->GPendingLevel = new UDemoPlayPendingLevel( GameEngine, URL );
			if( !GameEngine->GPendingLevel->DemoRecDriver )
			{
				Ar.Logf( TEXT("Demo playback failed: %s"), *GameEngine->GPendingLevel->Error );//!!localize!!
				delete GameEngine->GPendingLevel;
				GameEngine->GPendingLevel = NULL;
			}
		}
		else Ar.Log( TEXT("You must specify a filename") );//!!localize!!
		return 1;
	}
	else return 0;
	unguard;
}

/*-----------------------------------------------------------------------------
	ULevel networking related functions.
-----------------------------------------------------------------------------*/

//
// Start listening for connections.
//
UBOOL ULevel::Listen( FString& Error )
{
	guard(ULevel::Listen);
	if( NetDriver )
	{
		Error = LocalizeError("NetAlready");
		return 0;
	}
	if( !GetLinker() )
	{
		Error = LocalizeError("NetListen");
		return 0;
	}

	// Create net driver.
	UClass* NetDriverClass = StaticLoadClass( UNetDriver::StaticClass(), NULL, TEXT("ini:Engine.Engine.NetworkDevice"), NULL, LOAD_NoFail, NULL );
	NetDriver = (UNetDriver*)StaticConstructObject( NetDriverClass );
	if( !NetDriver->InitListen( this, URL, Error ) )
	{
		debugf( TEXT("Failed to listen: %s"), *Error );
		delete NetDriver;
		NetDriver=NULL;
		return 0;
	}

	// Load everything required for network server support.
	UGameEngine* GameEngine = CastChecked<UGameEngine>( Engine );
	GameEngine->BuildServerMasterMap( NetDriver, this );

	// Spawn network server support.
	for( INT i=0; i<GameEngine->ServerActors.Num(); i++ )
	{
		TCHAR Str[240];
		const TCHAR* Ptr = *GameEngine->ServerActors(i);
		if( ParseToken( Ptr, Str, ARRAY_COUNT(Str), 1 ) )
		{
			debugf( TEXT("Spawning: %s"), Str );
			UClass* HelperClass = StaticLoadClass( AActor::StaticClass(), NULL, Str, NULL, LOAD_NoFail, NULL );
			AActor* Actor = SpawnActor( HelperClass );
			while( Actor && ParseToken(Ptr,Str,ARRAY_COUNT(Str),1) )
			{
				TCHAR* Value = appStrchr(Str,'=');
				if( Value )
				{
					*Value++ = 0;
					for( TFieldIterator<UProperty> It(Actor->GetClass()); It; ++It )
						if
						(	appStricmp(It->GetName(),Str)==0
						&&	(It->PropertyFlags & CPF_Config) )
							It->ImportText( Value, (BYTE*)Actor + It->Offset, 0 );
				}
			}
		}
	}

	// Set LevelInfo properties.
	GetLevelInfo()->NetMode = Engine->Client ? NM_ListenServer : NM_DedicatedServer;
	GetLevelInfo()->NextSwitchCountdown = NetDriver->ServerTravelPause;

	return 1;
	unguard;
}

//
// Return whether this level is a server.
//
UBOOL ULevel::IsServer()
{
	guardSlow(ULevel::IsServer);
	return (!NetDriver || !NetDriver->ServerConnection) && (!DemoRecDriver || !DemoRecDriver->ServerConnection);
	unguardSlow;
}

/*-----------------------------------------------------------------------------
	ULevel network notifys.
-----------------------------------------------------------------------------*/

//
// The network driver is about to accept a new connection attempt by a
// connectee, and we can accept it or refuse it.
//
EAcceptConnection ULevel::NotifyAcceptingConnection()
{
	guard(ULevel::NotifyAcceptingConnection);
	check(NetDriver);
	if( NetDriver->ServerConnection )
	{
		// We are a client and we don't welcome incoming connections.
		debugf( NAME_DevNet, TEXT("NotifyAcceptingConnection: Client refused") );
		return ACCEPTC_Reject;
	}
	else if( GetLevelInfo()->NextURL!=TEXT("") )
	{
		// Server is switching levels.
		debugf( NAME_DevNet, TEXT("NotifyAcceptingConnection: Server %s refused"), GetName() );
		return ACCEPTC_Ignore;
	}
	else
	{
		// Server is up and running.
		debugf( NAME_DevNet, TEXT("NotifyAcceptingConnection: Server %s accept"), GetName() );
		return ACCEPTC_Accept;
	}
	unguard;
}

//
// This server has accepted a connection.
//
void ULevel::NotifyAcceptedConnection( UNetConnection* Connection )
{
	guard(ULevel::NotifyAcceptedConnection);
	check(NetDriver!=NULL);
	check(NetDriver->ServerConnection==NULL);
	debugf( NAME_NetComeGo, TEXT("Open %s %s %s"), GetName(), appTimestamp(), *Connection->LowLevelGetRemoteAddress() );
	unguard;
}

//
// The network interface is notifying this level of a new channel-open
// attempt by a connectee, and we can accept or refuse it.
//
UBOOL ULevel::NotifyAcceptingChannel( UChannel* Channel )
{
	guard(ULevel::NotifyAcceptingChannel);
	
	check(Channel);
	check(Channel->Connection);
	check(Channel->Connection->Driver);
	UNetDriver* Driver = Channel->Connection->Driver;

	if( Driver->ServerConnection )
	{
		// We are a client and the server has just opened up a new channel.
		//debugf( "NotifyAcceptingChannel %i/%i client %s", Channel->ChIndex, Channel->ChType, GetName() );
		if( Channel->ChType==CHTYPE_Actor )
		{
			// Actor channel.
			//debugf( "Client accepting actor channel" );
			return 1;
		}
		else
		{
			// Unwanted channel type.
			debugf( NAME_DevNet, TEXT("Client refusing unwanted channel of type %i"), Channel->ChType );
			return 0;
		}
	}
	else
	{
		// We are the server.
		if( Channel->ChIndex==0 && Channel->ChType==CHTYPE_Control )
		{
			// The client has opened initial channel.
			debugf( NAME_DevNet, TEXT("NotifyAcceptingChannel Control %i server %s: Accepted"), Channel->ChIndex, GetFullName() );
			return 1;
		}
		else if( Channel->ChType==CHTYPE_File )
		{
			// The client is going to request a file.
			debugf( NAME_DevNet, TEXT("NotifyAcceptingChannel File %i server %s: Accepted"), Channel->ChIndex, GetFullName() );
			return 1;
		}
		else
		{
			// Client can't open any other kinds of channels.
			debugf( NAME_DevNet, TEXT("NotifyAcceptingChannel %i %i server %s: Refused"), Channel->ChType, Channel->ChIndex, GetFullName() );
			return 0;
		}
	}
	unguard;
}

//
// Welcome a new player joining this server.
//
void ULevel::WelcomePlayer( UNetConnection* Connection, const TCHAR* Optional )
{
	guard(ULevel::WelcomePlayer);

	Connection->PackageMap->Copy( Connection->Driver->MasterMap );
	Connection->SendPackageMap();
	if( Optional[0] )
		Connection->Logf( TEXT("WELCOME LEVEL=%s LONE=%i %s"), GetOuter()->GetName(), GetLevelInfo()->bLonePlayer, Optional );
	else
		Connection->Logf( TEXT("WELCOME LEVEL=%s LONE=%i"), GetOuter()->GetName(), GetLevelInfo()->bLonePlayer );
	Connection->FlushNet();

	unguard;
}

//
// Received text on the control channel.
//
void ULevel::NotifyReceivedText( UNetConnection* Connection, const TCHAR* Text )
{
	guard(ULevel::NotifyReceivedText);
	if( ParseCommand(&Text,TEXT("USERFLAG")) )
	{
		Connection->UserFlags = appAtoi(Text);
	}
	else if( NetDriver->ServerConnection )
	{
		// We are the client.
		debugf( NAME_DevNet, TEXT("Level client received: %s"), Text );
		if( ParseCommand(&Text,TEXT("FAILURE")) )
		{
			// Return to entry.
			check(Engine->Client->Viewports.Num());
			Engine->SetClientTravel( Engine->Client->Viewports(0), TEXT("?failed"), 0, TRAVEL_Absolute );
		}
	}
	else
	{
		// We are the server.
		debugf( NAME_DevNet, TEXT("Level server received: %s"), Text );
		if( ParseCommand(&Text,TEXT("HELLO")) )
		{
			// Versions.
			INT RemoteMinVer=219, RemoteVer=219;
			Parse( Text, TEXT("MINVER="), RemoteMinVer );
			Parse( Text, TEXT("VER="),    RemoteVer    );
			Connection->RemoteVersion = RemoteVer;
			if( RemoteVer<ENGINE_MIN_NET_VERSION || RemoteMinVer>ENGINE_VERSION )
			{
				Connection->Logf( TEXT("UPGRADE MINVER=%i VER=%i"), ENGINE_MIN_NET_VERSION, ENGINE_VERSION );
				Connection->Channels[0]->Close();
				Connection->FlushNet();
				if( Connection->RemoteVersion <= 420 ) //!!oldver
					Connection->State = USOCK_Closed;
				return;
			}
			Connection->NegotiatedVer = Min(RemoteVer,ENGINE_VERSION);

			// Get byte limit.
			INT Stats = GetLevelInfo()->Game->bWorldLog;
			Connection->Challenge = appCycles();
			Connection->Logf( TEXT("CHALLENGE VER=%i CHALLENGE=%i STATS=%i"), Connection->NegotiatedVer, Connection->Challenge, Stats );
			Connection->FlushNet();
		}
		else if( ParseCommand(&Text,TEXT("NETSPEED")) )
		{
			INT Rate = appAtoi(Text);
			if( Rate>=2000 )
				Connection->CurrentNetSpeed = Clamp( Rate, 2000, NetDriver->MaxClientRate );
			debugf( TEXT("Client netspeed is %i"), Connection->CurrentNetSpeed );
		}
		else if( ParseCommand(&Text,TEXT("HAVE")) )
		{
			// Client specifying his generation.
			FGuid Guid(0,0,0,0);
			Parse( Text, TEXT("GUID=" ), Guid );
			for( TArray<FPackageInfo>::TIterator It(Connection->PackageMap->List); It; ++It )
				if( It->Guid==Guid )
					Parse( Text, TEXT("GEN=" ), It->RemoteGeneration );
		}
		else if( ParseCommand( &Text, TEXT("SKIP") ) )
		{
			FGuid Guid(0,0,0,0);
			Parse( Text, TEXT("GUID=" ), Guid );
			if( Connection->PackageMap )
			{
				for( INT i=0;i<Connection->PackageMap->List.Num();i++ )
					if( Connection->PackageMap->List(i).Guid == Guid )
					{
						debugf( TEXT("User skipped download of '%s'"), *Connection->PackageMap->List(i).URL );
						Connection->PackageMap->List.Remove( i );
						break;
					}
			}
		}
		else if( ParseCommand(&Text,TEXT("LOGIN")) )
		{
			// Admit or deny the player here.
			INT Response=0;
			if
			(	!Parse(Text,TEXT("RESPONSE="),Response)
			||	!Engine->ChallengeResponse(Connection->Challenge)==Response )
			{
				Connection->Logf( TEXT("FAILURE CHALLENGE") );
				Connection->Channels[0]->Close();
				Connection->FlushNet();
				if( Connection->RemoteVersion <= 420 ) //!!oldver
					Connection->State = USOCK_Closed;
				return;
			}
			TCHAR Str[1024]=TEXT("");
			FString Error, FailCode;
			Parse( Text, TEXT("URL="), Str, ARRAY_COUNT(Str) );
			Connection->RequestURL = Str;
			debugf( NAME_DevNet, TEXT("Login request: %s"), *Connection->RequestURL );
			const TCHAR* Tmp=Str;
			for( ; *Tmp && *Tmp!='?'; Tmp++ );
			GetLevelInfo()->Game->eventPreLogin( Tmp, Connection->LowLevelGetRemoteAddress(), Error, FailCode );
			if( Error!=TEXT("") )
			{
				debugf( NAME_DevNet, TEXT("PreLogin failure: %s (%s)"), *Error, *FailCode );
				Connection->Logf( TEXT("FAILURE %s"), *Error );
				if( (*FailCode)[0] )
					Connection->Logf( TEXT("FAILCODE %s"), *FailCode );
				Connection->Channels[0]->Close();
				Connection->FlushNet();
				if( Connection->RemoteVersion <= 420 ) //!!oldver
					Connection->State = USOCK_Closed;
				return;
			}
			WelcomePlayer( Connection );
		}
		else if( ParseCommand(&Text,TEXT("JOIN")) && !Connection->Actor )
		{
			// Finish computing the package map.
			Connection->PackageMap->Compute();

			// Spawn the player-actor for this network player.
			FString Error;
			debugf( NAME_DevNet, TEXT("Join request: %s"), *Connection->RequestURL );
			if( !SpawnPlayActor( Connection, ROLE_AutonomousProxy, FURL(NULL,*Connection->RequestURL,TRAVEL_Absolute), Error ) )
			{
				// Failed to connect.
				debugf( NAME_DevNet, TEXT("Join failure: %s"), *Error );
				Connection->Logf( TEXT("FAILURE %s"), *Error );
				Connection->Channels[0]->Close();
				Connection->FlushNet();
				if( Connection->RemoteVersion <= 420 ) //!!oldver
					Connection->State = USOCK_Closed;
			}
			else
			{
				// Successfully in game.
				debugf( NAME_DevNet, TEXT("Join succeeded: %s"), *Connection->Actor->PlayerReplicationInfo->PlayerName );
			}
		}
	}
	unguard;
}

//
// Called when a file receive is about to begin.
//
void ULevel::NotifyReceivedFile( UNetConnection* Connection, INT PackageIndex, const TCHAR* Error, UBOOL Skipped )
{
	guard(ULevel::NotifyReceivingFile);
	appErrorf( TEXT("Level received unexpected file") );
	unguard;
}

//
// Called when other side requests a file.
//
UBOOL ULevel::NotifySendingFile( UNetConnection* Connection, FGuid Guid )
{
	guard(ULevel::NotifySendingFile);
	if( NetDriver->ServerConnection )
	{
		// We are the client.
		debugf( NAME_DevNet, TEXT("Server requested file: Refused") );
		return 0;
	}
	else
	{
		// We are the server.
		debugf( NAME_DevNet, TEXT("Client requested file: Allowed") );
		return 1;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Stats.
-----------------------------------------------------------------------------*/

void ULevel::InitStats()
{
	guard(ULevel::InitStats);
	NetTickCycles = NetDiffCycles = ActorTickCycles = AudioTickCycles = FindPathCycles
	= MoveCycles = NumMoves = NumReps = NumPV = GetRelevantCycles = NumRPC = SeePlayer
	= Spawning = Unused = 0;
	GScriptEntryTag = GScriptCycles = 0;
	unguard;
}
void ULevel::GetStats( TCHAR* Result )
{
	guard(ULevel::GetStats);
	appSprintf
	(
		Result,
		TEXT("Script=%05.1f Actor=%04.1f Path=%04.1f See=%04.1f Spawn=%04.1f Audio=%04.1f Un=%04.1f Move=%04.1f (%i) Net=%04.1f"),
		GSecondsPerCycle*1000 * GScriptCycles,
		GSecondsPerCycle*1000 * ActorTickCycles,
		GSecondsPerCycle*1000 * FindPathCycles,
		GSecondsPerCycle*1000 * SeePlayer,
		GSecondsPerCycle*1000 * Spawning,
		GSecondsPerCycle*1000 * AudioTickCycles,
		GSecondsPerCycle*1000 * Unused,
		GSecondsPerCycle*1000 * MoveCycles,
		NumMoves,
		GSecondsPerCycle*1000 * NetTickCycles
	);
	unguard;
}

/*-----------------------------------------------------------------------------
	Clock.
-----------------------------------------------------------------------------*/

void ULevel::UpdateTime(ALevelInfo* Info)
{
	appSystemTime( Info->Year, Info->Month, Info->DayOfWeek, Info->Day, Info->Hour, Info->Minute, Info->Second, Info->Millisecond );
}

/*-----------------------------------------------------------------------------
	Sound Occlusion
-----------------------------------------------------------------------------*/

UBOOL ULevel::IsAudibleAt( FVector SoundLocation, FVector ListenerLocation, AActor* SoundActor, ESoundOcclusion SoundOcclusion )
{
	guard(ULevel::IsAudibleAt);

	FCheckResult Hit;
	switch ( SoundOcclusion )
	{
	case OCCLUSION_StaticMeshes:
		return SingleLineCheck( Hit, SoundActor, SoundLocation, ListenerLocation, TRACE_VisBlocking );
// TG ALPHA		return SingleLineCheck( Hit, SoundActor, SoundLocation, ListenerLocation, TRACE_World | TRACE_StopAtFirstHit );
		break;
	case OCCLUSION_Default:
	case OCCLUSION_BSP:
		return Model->FastLineCheck( SoundLocation, ListenerLocation );
		break;
	case OCCLUSION_None:
	default:
		return 1;
	}

	return 0;
	unguard;
}

#define ZONE_FACTOR 0.85
FLOAT ULevel::CalculateRadiusMultiplier( INT Zone1, INT Zone2 )
{
	guard(AActor::CalculateRadiusMultiplier);

	INT Distance = ZoneDist[Zone1][Zone2];
	return appPow( ZONE_FACTOR, Distance * Distance );

	unguard;
}

/*-----------------------------------------------------------------------------
	Actors relevant to a viewer.
-----------------------------------------------------------------------------*/

void ULevel::TraceVisible
(
	FVector&		vTraceDirection,
	FCheckResult&	Hit,			// Item hit.
	AActor*			SourceActor,	// Source actor, this or its parents is never hit.
	const FVector&	Start,			// Start location.
	DWORD           TraceFlags,		// Trace flags.
	int				iDistance
)
{
	guard(ULevel::TraceVisible);

	const FBspNode*	Node = NULL;
	FCheckResult	FirstHit;
	FCheckResult*	Check;
	FVector			StartTrace,
					End,
					Extent(0,0,0);
	APlayerPawn*	Player = SourceActor->IsA( APlayerPawn::StaticClass() ) ? (APlayerPawn*)SourceActor : NULL;

	// trace the entire distance looking for a "selected" zone
	StartTrace = Start;
	End = Start + iDistance * vTraceDirection;
	while( appRound(FDist(Start, StartTrace)) < iDistance )
	{
		// Get list of hit actors.
		if( SingleLineCheck( FirstHit, SourceActor, End, StartTrace, TraceFlags, Extent ) )
			break;

		// skip owned actors, but return the one nearest actor or level data
		for( Check = &FirstHit; Check != NULL; Check = Check->GetNext() )
		{
			if( !SourceActor || !SourceActor->IsOwnedBy( Check->Actor ) )
			{
				if( Check->Actor->IsA( ALevelInfo::StaticClass() ) )
				{
					// if we're in the rock (node 0), skip the test, then try again
					if( Check->Item == 0 )
						break;

					// make sure node we hit is in a visible zone
					Node = &Check->Actor->XLevel->Model->Nodes( Check->Item );
					if( !Player || Player->IsZoneVisible( Node->iZone[1] ) )
					{
						goto HitSection;
					}

				}
				else if( Check->Actor ) 
				{
					// make sure the actor we hit is a visible zone
					if( !Player || Player->IsZoneVisible( Check->Actor->Region.ZoneNumber ) )
					{
						goto HitSection;
					}
				}
			}
		}

		// found a room, but it's the wrong one, move forward one foot, then try again
		StartTrace = FirstHit.Location + 16 * vTraceDirection;
	}

	// missed section
	Hit.Time = 1.f;
	Hit.Actor = NULL;
	return;

HitSection:
	Hit = *Check;
	return;

	unguard;
}

// Trace a line and return the first actor hit in the selected section that matches the ParentClass
void ULevel::TraceVisibleObjects
(
	UClass*			ParentClass,	
	FVector&		vTraceDirection,
	FCheckResult&	Hit,			// Item hit.
	AActor*			SourceActor,	// Source actor, this or its parents is never hit.
	const FVector&	Start,			// Start location.
	DWORD           TraceFlags,		// Trace flags.
	int				iDistance
)
{
	guard(ULevel::TraceVisibleObjects);

	FCheckResult	FirstHit;
	FCheckResult*	Check;
	FVector			StartTrace,
					End,
					Extent(0,0,0);
	APlayerPawn*	Player = SourceActor->IsA( APlayerPawn::StaticClass() ) ? (APlayerPawn*)SourceActor : NULL;

	// trace the entire distance looking for a matching object in the "selected" zone
	StartTrace = Start;
	End = Start + iDistance * vTraceDirection;
	while( appRound(FDist(Start,StartTrace)<iDistance))
	{
		// Get list of hit actors.
		if( SingleLineCheck( FirstHit, SourceActor, End, StartTrace, TraceFlags, Extent ) )
			break;

		// skip owned actors, but return the one nearest actor matching the ParentClass
		for( Check = &FirstHit; Check != NULL; Check = Check->GetNext() )
		{
			if( !SourceActor || !SourceActor->IsOwnedBy( Check->Actor ) )
			{
				if( Check->Actor->GetClass()->IsChildOf( ParentClass ) )
				{
					// make sure actor we hit is in a visible zone
					if( !Player || Player->IsZoneVisible( Check->Actor->Region.ZoneNumber ) )
					{
						Hit = *Check;
						return;
					}
				}
			}
		}

		// found a room, but it's the wrong one, move forward 1 foot, then try again
		StartTrace = FirstHit.Location + 16 * vTraceDirection;
	}

	// missed section
	Hit.Time = 1.f;
	Hit.Actor = NULL;

	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
