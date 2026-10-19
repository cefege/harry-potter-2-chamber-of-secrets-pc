/*=============================================================================
	UnPawn.cpp: APawn AI implementation

  This contains both C++ methods (movement and reachability), as well as some 
  AI related natives

	Copyright 1997 Epic MegaGames, Inc. This software is a trade secret.

	Revision history:
		* Created by Steven Polge 3/97
=============================================================================*/

#include "EnginePrivate.h"
#include "UnNet.h"
#include "FConfigCacheIni.h"
#include "HP2PawnServices.h"
#include "Hp2PawnReachServices.h"
#include "Hp2PawnSenseServices.h"
#include "Hp2PawnNativeInputServices.h"
#include "Hp2PawnNativeAiServices.h"
#include "Hp2PawnNativeMoveServices.h"
#include "Hp2PawnNativeFilesServices.h"

/*-----------------------------------------------------------------------------
	APawn object implementation.
-----------------------------------------------------------------------------*/

void APlayerPawn::StaticConstructor()
{
	guard(APlayerPawn::StaticConstructor);

	const INT LoadingScreenOffset = static_cast<INT>(__builtin_offsetof(APlayerPawn, TotalGameStateTokens) - sizeof(BITFIELD));
	UBoolProperty* ShowLoadingScreen = new(GetClass(),TEXT("bShowLoadingScreen"),RF_Public)
		UBoolProperty(EC_CppProperty,LoadingScreenOffset,TEXT(""),CPF_Travel);
	ShowLoadingScreen->BitMask = 1;

	new(GetClass(),TEXT("TotalGameStateTokens"),RF_Public)
		UIntProperty(CPP_PROPERTY(TotalGameStateTokens),TEXT(""),0);
	new(GetClass(),TEXT("GameStateTokenLen"),RF_Public)
		UIntProperty(CPP_PROPERTY(GameStateTokenLen),TEXT(""),0);
	new(GetClass(),TEXT("GameStateMasterList"),RF_Public)
		UStrProperty(CPP_PROPERTY(GameStateMasterList),TEXT("GameState"),CPF_Edit|CPF_Const|CPF_EditConst);
	new(GetClass(),TEXT("CurrentGameState"),RF_Public)
		UStrProperty(CPP_PROPERTY(CurrentGameState),TEXT("GameState"),CPF_Edit|CPF_Travel);

	const INT LumosOffset = static_cast<INT>(__builtin_offsetof(APlayerPawn, fLumosRadius) - sizeof(BITFIELD));
	UBoolProperty* LumosOn = new(GetClass(),TEXT("bLumosOn"),RF_Public)
		UBoolProperty(EC_CppProperty,LumosOffset,TEXT(""),0);
	LumosOn->BitMask = 1;
	new(GetClass(),TEXT("fLumosRadius"),RF_Public)
		UFloatProperty(CPP_PROPERTY(fLumosRadius),TEXT(""),0);

	const INT SaveStateOffset = static_cast<INT>(__builtin_offsetof(APlayerPawn, bMouseAltFire) - sizeof(BITFIELD));
	UBoolProperty* QueuedToSaveGame = new(GetClass(),TEXT("bQueuedToSaveGame"),RF_Public)
		UBoolProperty(EC_CppProperty,SaveStateOffset,TEXT(""),0);
	QueuedToSaveGame->BitMask = 1;
	UBoolProperty* IsCaptured = new(GetClass(),TEXT("bIsCaptured"),RF_Public)
		UBoolProperty(EC_CppProperty,SaveStateOffset,TEXT(""),0);
	IsCaptured->BitMask = 2;
	UBoolProperty* ModernThirdPersonControls = new(GetClass(),TEXT("bModernThirdPersonControls"),RF_Public)
		UBoolProperty(EC_CppProperty,SaveStateOffset,TEXT(""),CPF_Config|CPF_GlobalConfig);
	ModernThirdPersonControls->BitMask = 4;
	new(GetClass(),TEXT("bMouseAltFire"),RF_Public)
		UByteProperty(CPP_PROPERTY(bMouseAltFire),TEXT(""),CPF_Input);
	new(GetClass(),TEXT("bKeyAltFire"),RF_Public)
		UByteProperty(CPP_PROPERTY(bKeyAltFire),TEXT(""),CPF_Input);

	unguard;
}

UBOOL APlayerPawn::BeginModernThirdPersonInput
(
	FLOAT& RawBaseX,
	FLOAT& RawStrafe,
	FLOAT& RawCameraYaw,
	FLOAT& RawCameraPitch
)
{
	guard(APlayerPawn::BeginModernThirdPersonInput);

	FLOAT Raw[4] = { RawBaseX, RawStrafe, RawCameraYaw, RawCameraPitch };
	const UBOOL Active = hp2_rs_engine_pawn_native_input_begin( &GHp2PawnNativeInputServices, this, Raw );
	RawBaseX = Raw[0];
	RawStrafe = Raw[1];
	RawCameraYaw = Raw[2];
	RawCameraPitch = Raw[3];
	return Active;

	unguard;
}

void APlayerPawn::FinishModernThirdPersonInput
(
	UBOOL Active,
	FLOAT RawBaseX,
	FLOAT RawStrafe,
	FLOAT RawCameraYaw,
	FLOAT RawCameraPitch,
	FLOAT DeltaSeconds
)
{
	guard(APlayerPawn::FinishModernThirdPersonInput);

	hp2_rs_engine_pawn_native_input_finish( &GHp2PawnNativeInputServices, this, Active, RawBaseX, RawStrafe, RawCameraYaw, RawCameraPitch, DeltaSeconds );

	unguard;
}

void APlayerPawn::ApplyModernThirdPersonMovement()
{
	guard(APlayerPawn::ApplyModernThirdPersonMovement);

	hp2_rs_engine_pawn_native_input_apply( &GHp2PawnNativeInputServices, this );

	unguard;
}


IMPLEMENT_CLASS(APawn);
IMPLEMENT_CLASS(APlayerPawn);

/*-----------------------------------------------------------------------------
	ANavigationPoint functions.
-----------------------------------------------------------------------------*/

enum EAIFunctions
{
	AI_PollMoveTo = 501,
	AI_PollMoveToward = 503,
	AI_PollStrafeTo = 505,
	AI_PollStrafeFacing = 507,
	AI_PollTurnTo = 509,
	AI_PollTurnToward = 511,
	AI_PollWaitForLanding = 528,
};

void AActor::execGetNextInt( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetNextInt);

	P_GET_STR(ClassName);
	P_GET_INT(CurrentInt);
	P_FINISH;

	UClass* TempClass = FindObjectChecked<UClass>( ANY_PACKAGE, *ClassName );

	TArray<FRegistryObjectInfo> List;
	GetRegistryObjects( List, UClass::StaticClass(), TempClass, 0 );

	INT Index = hp2_rs_engine_pawn_native_files_registry_index( List.Num(), CurrentInt );
	*(FString*)Result = (Index >= 0) ? List(Index).Object : TEXT("");

	unguard;
}

void AActor::execGetNextIntDesc( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetNextIntDesc);

	P_GET_STR(ClassName);
	P_GET_INT(CurrentInt);
	P_GET_STR_REF(EntryName);
	P_GET_STR_REF(Description);
	P_FINISH;

	UClass* TempClass = FindObjectChecked<UClass>( ANY_PACKAGE, *ClassName );

	TArray<FRegistryObjectInfo> List;
	GetRegistryObjects( List, UClass::StaticClass(), TempClass, 0 );

	INT Index = hp2_rs_engine_pawn_native_files_registry_index( List.Num(), CurrentInt );
	*EntryName = (Index >= 0) ? List(Index).Object : TEXT("");
	*Description = (Index >= 0) ? List(Index).Description : TEXT("");

	unguard;
}

void AActor::execGetCacheEntry( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetCacheEntry);
	P_GET_INT(Num);
	P_GET_STR_REF(GUID);
	P_GET_STR_REF(Filename);
	P_FINISH;

	*GUID = TEXT("");
	*Filename = TEXT("");
	TCHAR IniName[256];
	FConfigCacheIni CacheIni;
	appSprintf( IniName, TEXT("%s") PATH_SEPARATOR TEXT("cache.ini"), *GSys->CachePath );
	TMultiMap<FString,FString>* Sec = CacheIni.GetSectionPrivate( TEXT("Cache"), 0, 1, IniName );
	if( Sec )
	{
		TArray<FString> Keys;
		TArray<FString> Values;
		TArray<BYTE> Empty;
		for( TMultiMap<FString,FString>::TIterator It(*Sec); It; ++It )
		{
			Keys.AddItem( *It.Key() );
			Values.AddItem( *It.Value() );
			Empty.AddItem( *(*It.Value()) ? 0 : 1 );
		}
		INT Index = hp2_rs_engine_pawn_native_files_cache_entry_index( Num, Empty.Num() ? &Empty(0) : NULL, Empty.Num() );
		if( Index >= 0 )
		{
			*GUID = Keys(Index);
			*Filename = Values(Index);
			*(UBOOL*)Result = 1;
			return;
		}
	}
	*(UBOOL*)Result = 0;
	unguard;
}

void AActor::execMoveCacheEntry( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execMoveCacheEntry);
	P_GET_STR(GUID);
	P_GET_STR_OPTX(NewFilename,TEXT(""));
	P_FINISH;

	TArray<UNICHAR> GuidUnits = Hp2PawnNativeFilesUnits( *GUID );
	TArray<UNICHAR> NewFilenameUnits = Hp2PawnNativeFilesUnits( *NewFilename );
	*(UBOOL*)Result = hp2_rs_engine_pawn_native_files_move_cache_entry( &GHp2PawnNativeFilesServices, GuidUnits.GetData(), NewFilenameUnits.GetData() );
	unguard;
}

// hack check for valid skins for Unreal Tournament
void APawn::execCheckValidSkinPackage( FFrame& Stack, RESULT_DECL )
{
	guard(APawn::execCheckValidSkinPackage);

	P_GET_STR(Pkg);
	P_GET_STR(MeshName);
	P_FINISH;

	TArray<UNICHAR> PkgUnits = Hp2PawnNativeFilesUnits( *Pkg );
	TArray<UNICHAR> MeshUnits = Hp2PawnNativeFilesUnits( *MeshName );
	*(UBOOL*)Result = hp2_rs_engine_pawn_native_files_check_valid_skin_package( &GHp2PawnNativeFilesServices, PkgUnits.GetData(), MeshUnits.GetData() );
	unguard;
}

void APlayerPawn::execUpdateURL( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execUpdateURL);

	P_GET_STR(NewOption);
	P_GET_STR(NewValue);
	P_GET_UBOOL(bSaveDefault);
	P_FINISH;

	TArray<UNICHAR> OptionUnits = Hp2PawnNativeFilesUnits( *NewOption );
	TArray<UNICHAR> ValueUnits = Hp2PawnNativeFilesUnits( *NewValue );
	hp2_rs_engine_pawn_native_files_update_url( &GHp2PawnNativeFilesServices, this, OptionUnits.GetData(), ValueUnits.GetData(), bSaveDefault ? 1 : 0 );
	unguard;
}

void APlayerPawn::execGetDefaultURL( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execGetDefaultURL);

	P_GET_STR(Option);
	P_FINISH;

	TArray<UNICHAR> OptionUnits = Hp2PawnNativeFilesUnits( *Option );
	*(FString*)Result = Hp2PawnNativeFilesString( hp2_rs_engine_pawn_native_files_get_default_url( &GHp2PawnNativeFilesServices, OptionUnits.GetData() ) );
	unguard;
}

void AActor::execGetURLMap( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetURLMap);

	P_FINISH;

	*(FString*)Result = Hp2PawnNativeFilesString( hp2_rs_engine_pawn_native_files_url_map( &GHp2PawnNativeFilesServices, this ) );
	unguard;
}

void AActor::execGetNextSkin( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetNextSkin);

	P_GET_STR(Prefix);
	P_GET_STR(CurrentSkin);
	P_GET_INT(Dir);
	P_GET_STR_REF(SkinName);
	P_GET_STR_REF(SkinDesc);
	P_FINISH;

	TArray<FRegistryObjectInfo> List;
	GetRegistryObjects( List, UTexture::StaticClass(), NULL, 0 );

	// Registry names as NUL-terminated UNICHAR strings for the Rust search.
	TArray<TArray<UNICHAR>> NameUnits;
	for( INT i=0; i<List.Num(); i++ )
		NameUnits.AddItem( Hp2PawnNativeFilesUnits( *List(i).Object ) );
	TArray<const UNICHAR*> NamePtrs;
	for( INT i=0; i<NameUnits.Num(); i++ )
		NamePtrs.AddItem( NameUnits(i).GetData() );
	TArray<UNICHAR> PrefixUnits = Hp2PawnNativeFilesUnits( *Prefix );
	TArray<UNICHAR> CurrentUnits = Hp2PawnNativeFilesUnits( *CurrentSkin );
	INT UseSkin = hp2_rs_engine_pawn_native_files_next_skin_index( NamePtrs.GetData(), NamePtrs.Num(), PrefixUnits.GetData(), CurrentUnits.GetData(), Dir );

	if( UseSkin >= 0 && UseSkin < List.Num() )
	{
		*SkinName = List(UseSkin).Object;
		*SkinDesc = List(UseSkin).Description;
	}
	else
	{
		*SkinName = TEXT("");
		*SkinDesc = TEXT("");
	}

	unguard;
}

void APlayerPawn::execGetEntryLevel( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execGetEntryLevel);
	P_FINISH;

	*(ALevelInfo**)Result = (ALevelInfo*)hp2_rs_engine_pawn_native_files_entry_level( &GHp2PawnNativeFilesServices, this );

	unguard;
}

void APlayerPawn::execResetKeyboard( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execResetKeyboard);

	P_FINISH;

	hp2_rs_engine_pawn_native_files_reset_keyboard( &GHp2PawnNativeFilesServices, this );
	unguard;
}

void APawn::execFindBestInventoryPath( FFrame& Stack, RESULT_DECL )
{
	guard(APawn::execFindBestInventoryPath);

	P_GET_FLOAT_REF(Weight);
	P_GET_UBOOL(bPredictRespawns);
	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	AActor* bestPath = (AActor*)hp2_rs_engine_pawn_native_files_find_best_inventory_path( &GHp2PawnNativeFilesServices, this, *Weight, bPredictRespawns ? 1 : 0, Weight );
	unclock(GetLevel()->FindPathCycles);
	//debugf("Find path to time was %f", GetLevel()->FindPathCycles * MSecPerCycle);

	*(AActor**)Result = bestPath; 
	unguard;
}

void AActor::execGetMapName( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetMapName);

	P_GET_STR(Prefix);
	P_GET_STR(MapName);
	P_GET_INT(Dir);
	P_FINISH;

	TArray<UNICHAR> PrefixUnits = Hp2PawnNativeFilesUnits( *Prefix );
	TArray<UNICHAR> MapUnits = Hp2PawnNativeFilesUnits( *MapName );
	*(FString*)Result = Hp2PawnNativeFilesString( hp2_rs_engine_pawn_native_files_get_map_name( &GHp2PawnNativeFilesServices, PrefixUnits.GetData(), MapUnits.GetData(), Dir ) );
	unguard;
}

void APlayerPawn::execConsoleCommand( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execConsoleCommand);

	P_GET_STR(Command);
	P_FINISH;

	TArray<UNICHAR> CommandUnits = Hp2PawnNativeFilesUnits( *Command );
	*(FString*)Result = Hp2PawnNativeFilesString( hp2_rs_engine_pawn_native_files_console_command( &GHp2PawnNativeFilesServices, this, CommandUnits.GetData() ) );
	unguard;
}

void APlayerPawn::execScreenToWorld( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execScreenToWorld);

	P_GET_VECTOR(S);
	P_FINISH;

	hp2_rs_engine_pawn_native_files_screen_to_world( &GHp2PawnNativeFilesServices, this, S.X, S.Y, S.Z, Result );
	unguard;
}

void APawn::execStopWaiting( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execStopWaiting);

	P_FINISH;

	hp2_rs_engine_pawn_native_ai_stop_waiting( &GHp2PawnNativeAiServices, this );

	unguardSlow;
}

/* CanSee()
returns true if LineOfSightto object and it is within creature's 
peripheral vision
*/

void APawn::execCanSee( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execCanSee);

	P_GET_ACTOR(Other);
	P_FINISH;

	*(DWORD*)Result = LineOfSightTo(Other, 1);
	unguardSlow;
}

/* PlayerCanSeeMe()
	returns true if actor is visible to some player
*/
void AActor::execPlayerCanSeeMe( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::PlayerCanSeeMe);
	P_FINISH;

	*(DWORD*)Result = hp2_rs_engine_pawn_native_ai_player_can_see_me( &GHp2PawnNativeAiServices, this );
	unguard;
}

int AActor::TestCanSeeMe( APlayerPawn *Viewer )
{
	guard(AActor::TestCanSeeMe);

	return hp2_rs_engine_pawn_native_ai_test_can_see_me( &GHp2PawnNativeAiServices, this, Viewer );

	unguard;
}

/* execDescribeSpec - returns spec components to script
*/
void ANavigationPoint::execdescribeSpec( FFrame& Stack, RESULT_DECL )
{
	guardSlow(ANavigationPoint::execdescribeSpec);

	P_GET_INT(iSpec);
	P_GET_ACTOR_REF(Start);
	P_GET_ACTOR_REF(End);
	P_GET_INT_REF(reachFlags);
	P_GET_INT_REF(distance);
	P_FINISH;

	FReachSpec spec = GetLevel()->ReachSpecs(iSpec);
	*Start          = spec.Start;
	*End            = spec.End;
	*reachFlags     = spec.reachFlags;
	*distance       = spec.distance;

	unguardSlow;
}

/*-----------------------------------------------------------------------------
	Pawn related functions.
-----------------------------------------------------------------------------*/

void APawn::execPickTarget( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPickTarget);

	P_GET_FLOAT_REF(bestAim);
	P_GET_FLOAT_REF(bestDist);
	P_GET_VECTOR(FireDir);
	P_GET_VECTOR(projStart);
	P_FINISH;
	*(APawn**)Result = (APawn*)hp2_rs_engine_pawn_native_ai_pick_target( &GHp2PawnNativeAiServices, this, bestAim, bestDist, &FireDir, &projStart );
	unguardSlow;
}

void APawn::execPickAnyTarget( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPickAnyTarget);

	P_GET_FLOAT_REF(bestAim);
	P_GET_FLOAT_REF(bestDist);
	P_GET_VECTOR(FireDir);
	P_GET_VECTOR(projStart);
	P_FINISH;
	*(AActor**)Result = (AActor*)hp2_rs_engine_pawn_native_ai_pick_any_target( &GHp2PawnNativeAiServices, this, bestAim, bestDist, &FireDir, &projStart );
	unguardSlow;
}

void APawn::execAddPawn( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execAddPawn);

	P_FINISH;

	hp2_rs_engine_pawn_native_ai_add_pawn( &GHp2PawnNativeAiServices, this );
	unguardSlow;
}

void APawn::execRemovePawn( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execRemovePawn);

	P_FINISH;

	hp2_rs_engine_pawn_native_ai_remove_pawn( &GHp2PawnNativeAiServices, this );

	unguardSlow;
}

/* execWaitForLanding()
wait until physics is not PHYS_Falling
*/
void APawn::execWaitForLanding( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execWaitForLanding);

	P_FINISH;

	hp2_rs_engine_pawn_native_ai_wait_for_landing( &GHp2PawnNativeAiServices, this );
	unguardSlow;
}

void APawn::execPollWaitForLanding( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollWaitForLanding);
	hp2_rs_engine_pawn_native_ai_poll_wait_for_landing( &GHp2PawnNativeAiServices, this, Result );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollWaitForLanding, execPollWaitForLanding);

void APawn::execPickWallAdjust( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPickWallAdjust);

	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	*(DWORD*)Result = PickWallAdjust();
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}

void APawn::execFindStairRotation( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execFindStairRotation);

	P_GET_FLOAT(deltaTime);
	P_FINISH;

	FCheckResult Hit(1.f);
	*(DWORD*)Result = hp2_rs_engine_pawn_native_ai_find_stair_rotation( &GHp2PawnNativeAiServices, this, deltaTime, &Hit );
	unguardSlow;
}

void APawn::execEAdjustJump( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execEAdjustJump);

	FVector Landing;
	FVector vel = Velocity;
	SuggestJumpVelocity(Destination, vel);

	P_FINISH;

	*(FVector*)Result = vel;
	unguardSlow;
}

void APawn::execactorReachable( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execActorReachable);

	P_GET_ACTOR(actor);
	P_FINISH;
	
	if ( !actor )
	{
		//debugf(NAME_DevPath,"Warning: No goal for ActorReachable by %s in %s",GetName(), GetStateFrame()->Describe() );
		*(DWORD*)Result = 0; 
		return;
	}
	clock(GetLevel()->FindPathCycles);
	*(DWORD*)Result = hp2_rs_engine_pawn_native_move_actor_reachable( &GHp2PawnNativeMoveServices, this, actor );
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}

void APawn::execpointReachable( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPointReachable);

	P_GET_VECTOR(point);
	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	*(DWORD*)Result = pointReachable(point);  
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}




void APawn::execFindPath(  FFrame& Stack, RESULT_DECL )
{

//	guardSlow(APawn::execFindPath);
	P_GET_ACTOR(startPoint);
//	P_GET_OBJECT(UClass,BaseClass);
//	P_GET_OBJECT(pathClass,baseClass);
	P_GET_NAME(ClassName);
	P_FINISH;

	ANavigationPoint* next=NULL;
	

	findPath(next,startPoint,ClassName);
	*(ANavigationPoint**)Result = next; 
//	unguardSlow;




}
/* FindPathTo()
returns the best pathnode toward a point - even if point is directly reachable
If there is no path, returns None
By default clears paths.  If script wants to preset some path weighting, etc., then
it can explicitly clear paths using execClearPaths before presetting the values and 
calling FindPathTo with clearpath = 0

  FIXME add optional bBlockDoors (no paths through doors), bBlockTeleporters, bBlockSwitches,
  maxNodes (max number of nodes), etc.
*/

void APawn::execFindPathTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execFindPathTo);

	P_GET_VECTOR(point);
	P_GET_INT_OPTX(bSinglePath, 0); 
	P_GET_UBOOL_OPTX(bClearPaths, 1);
	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	*(AActor**)Result = (AActor*)hp2_rs_engine_pawn_native_move_find_path_to( &GHp2PawnNativeMoveServices, this, &point, bSinglePath, bClearPaths );
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}

void APawn::execFindPathToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execFindPathToward);

	P_GET_ACTOR(goal);
	P_GET_INT_OPTX(bSinglePath, 0);
	P_GET_UBOOL_OPTX(bClearPaths, 1);
	P_FINISH;

	if ( !goal )
	{
		//debugf(NAME_DevPath,"Warning: No goal for FindPathToward by %s in %s",GetName(), GetStateFrame()->Describe() );
		*(AActor**)Result = NULL; 
		return;
	}
	clock(GetLevel()->FindPathCycles);
	*(AActor**)Result = (AActor*)hp2_rs_engine_pawn_native_move_find_path_toward( &GHp2PawnNativeMoveServices, this, goal, bSinglePath, bClearPaths );
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}

/* FindRandomDest()
returns a random pathnode which is reachable from the creature's location
*/
void APawn::execFindRandomDest( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execFindPathTo);

	P_GET_UBOOL_OPTX(bClearPaths, 1);
	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	*(ANavigationPoint**)Result = (ANavigationPoint*)hp2_rs_engine_pawn_native_move_find_random_dest( &GHp2PawnNativeMoveServices, this, bClearPaths );
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}

void APawn::execClearPaths( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execClearPaths);

	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	clearPaths(); 
	unclock(GetLevel()->FindPathCycles);
	unguardSlow;
}

/*MakeNoise
- check to see if other creatures can hear this noise
*/
void AActor::execMakeNoise( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execMakeNoise);

	P_GET_FLOAT(Loudness);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_make_noise( &GHp2PawnNativeMoveServices, this, Loudness );
	unguardSlow;
}

void APawn::execLineOfSightTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execLineOfSightTo);

	P_GET_ACTOR(Other);
	P_FINISH;

	*(DWORD*)Result = LineOfSightTo(Other);
	unguardSlow;
}

/* execMoveTo()
start moving to a point -does not use routing
Destination is set to a point
//FIXME - don't use ground speed for flyers (or set theirs = flyspeed)
*/
void APawn::execMoveTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execMoveTo);

	P_GET_VECTOR(dest);
	P_GET_FLOAT_OPTX(speed, 1.f);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_move_to( &GHp2PawnNativeMoveServices, this, &dest, speed, AI_PollMoveTo );
	unguardSlow;
}

void APawn::execPollMoveTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollMoveTo);
	hp2_rs_engine_pawn_native_move_poll_move_to( &GHp2PawnNativeMoveServices, this );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollMoveTo, execPollMoveTo);

/* execMoveToward()
start moving toward a goal actor -does not use routing
MoveTarget is set to goal
*/
void APawn::execMoveToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execMoveToward);

	P_GET_ACTOR(goal);
	P_GET_FLOAT_OPTX(speed, 1.f);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_move_toward( &GHp2PawnNativeMoveServices, this, goal, speed, AI_PollMoveToward );
	unguardSlow;
}

void APawn::execPollMoveToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollMoveToward);

	hp2_rs_engine_pawn_native_move_poll_move_toward( &GHp2PawnNativeMoveServices, this );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollMoveToward, execPollMoveToward);

/* execStrafeTo()
Strafe to Destination, pointing at Focus
*/
void APawn::execStrafeTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execStrafeTo);

	P_GET_VECTOR(Dest);
	P_GET_VECTOR(FocalPoint);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_strafe_to( &GHp2PawnNativeMoveServices, this, &Dest, &FocalPoint, AI_PollStrafeTo );
	unguardSlow;
}

void APawn::execPollStrafeTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollStrafeTo);

	hp2_rs_engine_pawn_native_move_poll_strafe_to( &GHp2PawnNativeMoveServices, this );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollStrafeTo, execPollStrafeTo);

/* execStrafeFacing()
strafe to Destination, pointing at FaceTarget
*/
void APawn::execStrafeFacing( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execStrafeFacing);

	P_GET_VECTOR(Dest)
	P_GET_ACTOR(goal);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_strafe_facing( &GHp2PawnNativeMoveServices, this, &Dest, goal, AI_PollStrafeFacing );
	unguardSlow;
}

void APawn::execPollStrafeFacing( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollStrafeFacing);

	hp2_rs_engine_pawn_native_move_poll_strafe_facing( &GHp2PawnNativeMoveServices, this );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollStrafeFacing, execPollStrafeFacing);

/* execTurnToward()
turn toward FaceTarget
*/
void APawn::execTurnToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execTurnToward);

	P_GET_ACTOR(goal);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_turn_toward( &GHp2PawnNativeMoveServices, this, goal, AI_PollTurnToward );
	unguardSlow;
}

void APawn::execPollTurnToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollTurnToward);

	hp2_rs_engine_pawn_native_move_poll_turn_toward( &GHp2PawnNativeMoveServices, this );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollTurnToward, execPollTurnToward);

/* execTurnTo()
Turn to focus
*/
void APawn::execTurnTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execTurnTo);

	P_GET_VECTOR(FocalPoint);
	P_FINISH;

	hp2_rs_engine_pawn_native_move_turn_to( &GHp2PawnNativeMoveServices, this, &FocalPoint, AI_PollTurnTo );
	unguardSlow;
}

void APawn::execPollTurnTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollTurnTo);

	hp2_rs_engine_pawn_native_move_poll_turn_to( &GHp2PawnNativeMoveServices, this );
	unguardSlow;
}
IMPLEMENT_FUNCTION( APawn, AI_PollTurnTo, execPollTurnTo);

//=================================================================================
void APawn::setMoveTimer(FLOAT MoveSize)
{
	hp2_rs_engine_pawn_sense_set_move_timer( this, MoveSize );
}

/* moveToward()
move Actor toward a point.  Returns 1 if Actor reached point
(Set Acceleration, let physics do actual move)
*/
int APawn::moveToward(const FVector &Dest)
{
	return hp2_rs_engine_pawn_sense_move_toward( &GHp2PawnSenseServices, this, &Dest );
}

/* rotateToward()
rotate Actor toward a point.  Returns 1 if target rotation achieved.
(Set DesiredRotation, let physics do actual move)
*/
int APawn::rotateToward(const FVector &FocalPoint)
{
	return hp2_rs_engine_pawn_sense_rotate_toward( &GHp2PawnSenseServices, this, &FocalPoint );
}

DWORD APawn::LineOfSightTo(AActor *Other, int bShowSelf)
{
	return hp2_rs_engine_pawn_sense_line_of_sight_to( &GHp2PawnSenseServices, this, Other, bShowSelf );
}

int APawn::CanHear(FVector NoiseLoc, FLOAT Loudness, APawn *Other)
{
	return hp2_rs_engine_pawn_sense_can_hear( &GHp2PawnSenseServices, this, &NoiseLoc, Loudness, Other );
}

/* Send a HearNoise() message to all Pawns which could possibly hear this noise
NOTE that there is a hard-coded limit to the radius of hearing, which depends on the loudness of the noise
*/
void AActor::CheckNoiseHearing(FLOAT Loudness)
{
	guard(AActor::CheckNoiseHearing);

	if ( !this->Instigator )
		return;

	clock(GetLevel()->SeePlayer);
	hp2_rs_engine_pawn_sense_check_noise_hearing( &GHp2PawnSenseServices, this, Loudness );
	unclock(GetLevel()->SeePlayer);
	unguard;
}

void APawn::CheckEnemyVisible()
{
	guard(APawn::CheckEnemyVisible);

	clock(GetLevel()->SeePlayer);
	if ( Enemy )
	{
		check(Enemy->IsValid());
	}
	hp2_rs_engine_pawn_sense_check_enemy_visible( &GHp2PawnSenseServices, this );
	unclock(GetLevel()->SeePlayer);

	unguard;
}

#if 0
UBOOL APawn::Tick( FLOAT DeltaTime, enum ELevelTick TickType )
{
	return Super::Tick(DeltaTime, TickType);
}
#endif

/* Player shows self to pawns that are ready
*/
void APawn::ShowSelf()
{
	guard(APawn::ShowSelf);

	clock(GetLevel()->SeePlayer);
	hp2_rs_engine_pawn_native_move_show_self( &GHp2PawnNativeMoveServices, this );
	unclock(GetLevel()->SeePlayer);
	unguard;
}

int APawn::actorReachable(AActor *Other, int bKnowVisible)
{
	return hp2_rs_engine_pawn_reach_actor_reachable( &GHp2PawnReachServices, this, Other, bKnowVisible );
}

int APawn::pointReachable(FVector aPoint, int bKnowVisible)
{
	return hp2_rs_engine_pawn_reach_point_reachable( &GHp2PawnReachServices, this, &aPoint, bKnowVisible );
}

int APawn::Reachable(FVector aPoint, FLOAT Threshold, AActor* GoalActor)
{
	return hp2_rs_engine_pawn_reach_reachable( &GHp2PawnReachServices, this, &aPoint, Threshold, GoalActor );
}

int APawn::flyReachable(FVector Dest, FLOAT Threshold, int reachFlags, AActor* GoalActor)
{
	return hp2_rs_engine_pawn_reach_fly_reachable( &GHp2PawnReachServices, this, &Dest, Threshold, reachFlags, GoalActor );
}

int APawn::swimReachable(FVector Dest, FLOAT Threshold, int reachFlags, AActor* GoalActor)
{
	return hp2_rs_engine_pawn_reach_swim_reachable( &GHp2PawnReachServices, this, &Dest, Threshold, reachFlags, GoalActor );
}
/*walkReachable() -
//walkReachable returns 0 if Actor cannot reach dest, and 1 if it can reach dest by moving in
// straight line
FIXME - take into account zones (lava, water, etc.). - note that pathbuilder should
initialize Scout to be able to do all these things
// actor must remain on ground at all times
// Note that Actor is not moved (when all is said and done)
// FIXME - allow jumping up and down if bCanJump (false for Scout!)

*/
int APawn::walkReachable(FVector Dest, FLOAT Threshold, int reachFlags, AActor* GoalActor)
{
	return hp2_rs_engine_pawn_reach_walk_reachable( &GHp2PawnReachServices, this, &Dest, Threshold, reachFlags, GoalActor );
}

int APawn::jumpReachable(FVector Dest, FLOAT Threshold, int reachFlags, AActor* GoalActor)
{
	return hp2_rs_engine_pawn_reach_jump_reachable( &GHp2PawnReachServices, this, &Dest, Threshold, reachFlags, GoalActor );
}

/* jumpLanding()
determine landing position of current fall, given testVel as initial velocity.
Assumes near-zero acceleration by pawn during jump (make sure creatures do this FIXME)
*/
void APawn::jumpLanding(FVector testVel, FVector &Landing, int movePawn)
{
	hp2_rs_engine_pawn_reach_jump_landing( &GHp2PawnReachServices, this, &testVel, &Landing, movePawn );
}

int APawn::FindJumpUp(FVector Dest, FVector vel, FVector &Landing, int moveActor)
{
	return hp2_rs_engine_pawn_reach_find_jump_up( &GHp2PawnReachServices, this, &vel );
}

void APawn::SuggestJumpVelocity(FVector Dest, FVector &vel)
{
	hp2_rs_engine_pawn_reach_suggest_jump_velocity( &GHp2PawnReachServices, this, &Dest, &vel );
}

/* Find best jump from current position toward destination.  Assumes that there is no immediate 
barrier.  Sets vel to the suggested initial velocity, Landing to the expected Landing, 
and moves actor if moveActor is set */
int APawn::FindBestJump(FVector Dest, FVector vel, FVector &Landing, int movePawn)
{
	return hp2_rs_engine_pawn_reach_find_best_jump( &GHp2PawnReachServices, this, &Dest, &vel, &Landing, movePawn );
}


/* walkMove() 
- returns 1 if move happened, zero if it didn't because of barrier, and -1
if it didn't because of ledge
Move direction must not be adjusted.
*/
int APawn::walkMove(FVector Delta, FCheckResult& Hit, AActor* GoalActor, FLOAT threshold, int bAdjust)
{
	return hp2_rs_engine_pawn_walk_move( &GHp2PawnServices, this, &Delta, &Hit, GoalActor, threshold, bAdjust );
}

int APawn::flyMove(FVector Delta, AActor* GoalActor, FLOAT threshold, int bAdjust)
{
	return hp2_rs_engine_pawn_fly_move( &GHp2PawnServices, this, &Delta, GoalActor, threshold, bAdjust );
}

int APawn::swimMove(FVector Delta, AActor* GoalActor, FLOAT threshold, int bAdjust)
{
	return hp2_rs_engine_pawn_swim_move( &GHp2PawnServices, this, &Delta, GoalActor, threshold, bAdjust );
}

/* PickWallAdjust()
Check if could jump up over obstruction (only if there is a knee height obstruction)
If so, start jump, and return current destination
Else, try to step around - return a destination 90 degrees right or left depending on traces
out and floor checks

*/
int APawn::PickWallAdjust()
{
	return hp2_rs_engine_pawn_sense_pick_wall_adjust( &GHp2PawnSenseServices, this );
}

void APawn::HandleSpecial(AActor *&bestPath)
{
	hp2_rs_engine_pawn_sense_handle_special( &GHp2PawnSenseServices, this, (void**)&bestPath );
}

/*-----------------------------------------------------------------------------
	Networking functions.
-----------------------------------------------------------------------------*/

FLOAT APawn::GetNetPriority( AActor* Recent, FLOAT Time, FLOAT Lag )
{
	return hp2_rs_engine_pawn_sense_get_net_priority( this, Recent, Time, Lag );
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
