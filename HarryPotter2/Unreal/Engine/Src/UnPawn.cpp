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

namespace
{
struct FModernHarryProperties
{
	UClass* HarryClass;
	UBOOL bResolved;
	UBOOL bValid;
	UBoolProperty* ScreenRelativeMovement;
	UBoolProperty* LockedOnTarget;
	UBoolProperty* FixedFaceDirection;
	UBoolProperty* InDuelingMode;
	UBoolProperty* ReverseInput;
	UBoolProperty* KeepStationary;
	UBoolProperty* LockOutForward;
	UBoolProperty* LockOutBackward;
	UBoolProperty* LockOutStrafeLeft;
	UBoolProperty* LockOutStrafeRight;
	UBoolProperty* IsAiming;
	UBoolProperty* E3DemoLockout;
	UBoolProperty* AutoCenterCamera;
	UObjectProperty* BossTarget;
};

static FModernHarryProperties GModernHarryProperties;
static UBOOL GReportedModernHarryContractFailure;

struct FModernAutoCenterOverride
{
	APlayerPawn* PlayerPawn;
	UBoolProperty* Property;
	UBOOL SavedValue;
};

static FModernAutoCenterOverride GModernAutoCenterOverride;

static UBOOL ReadModernHarryBool( const APlayerPawn* PlayerPawn, const UBoolProperty* Property )
{
	return (*(const BITFIELD*)((const BYTE*)PlayerPawn + Property->Offset) & Property->BitMask) != 0;
}

static void WriteModernHarryBool( APlayerPawn* PlayerPawn, const UBoolProperty* Property, UBOOL Value )
{
	BITFIELD& Storage = *(BITFIELD*)((BYTE*)PlayerPawn + Property->Offset);
	if( Value )
		Storage |= Property->BitMask;
	else
		Storage &= ~Property->BitMask;
}

static void RestoreModernAutoCenter( APlayerPawn* PlayerPawn )
{
	if( GModernAutoCenterOverride.PlayerPawn!=PlayerPawn )
		return;
	WriteModernHarryBool
	(
		PlayerPawn,
		GModernAutoCenterOverride.Property,
		GModernAutoCenterOverride.SavedValue
	);
	appMemzero( &GModernAutoCenterOverride, sizeof(GModernAutoCenterOverride) );
}

static void SuppressModernAutoCenter
(
	APlayerPawn* PlayerPawn,
	const FModernHarryProperties& Properties
)
{
	if( !ReadModernHarryBool(PlayerPawn,Properties.AutoCenterCamera) )
		return;
	GModernAutoCenterOverride.PlayerPawn = PlayerPawn;
	GModernAutoCenterOverride.Property = Properties.AutoCenterCamera;
	GModernAutoCenterOverride.SavedValue = 1;
	WriteModernHarryBool( PlayerPawn, Properties.AutoCenterCamera, 0 );
}

static FLOAT ExpandModernAnalogMovement
(
	FLOAT Value,
	FLOAT DeltaSeconds,
	FLOAT MaxAnalogFrameContribution,
	FLOAT Scale
)
{
	if( DeltaSeconds<=0.f || Abs(Value * DeltaSeconds)>MaxAnalogFrameContribution + 0.0001f )
		return Value;
	// A normalized SDL axis contributes at most 0.2*Speed before UInput divides by DeltaSeconds.
	// Classify in that pre-divide domain so full-stick response stays monotonic at every frame rate.
	return Value * DeltaSeconds * Scale;
}

static UObject* ReadModernHarryObject( const APlayerPawn* PlayerPawn, const UObjectProperty* Property )
{
	return *(UObject* const*)((const BYTE*)PlayerPawn + Property->Offset);
}

static UBOOL IsInScriptState( UObject* Object, const TCHAR* StateName )
{
	if( !Object || !Object->GetStateFrame() )
		return 0;

	const FName RequiredState( StateName, FNAME_Find );
	for( UState* State=Object->GetStateFrame()->StateNode; State; State=State->GetSuperState() )
		if( State->GetFName()==RequiredState )
			return 1;
	return 0;
}

static FModernHarryProperties* GetModernHarryProperties( APlayerPawn* PlayerPawn )
{
	FModernHarryProperties& Properties = GModernHarryProperties;
	UClass* HarryClass = Properties.HarryClass;
	if( !HarryClass || !PlayerPawn->IsA(HarryClass) )
	{
		HarryClass = NULL;
		for( UClass* Candidate=PlayerPawn->GetClass(); Candidate; Candidate=Candidate->GetSuperClass() )
			if( appStricmp(Candidate->GetPathName(),TEXT("HGame.Harry"))==0 )
			{
				HarryClass = Candidate;
				break;
			}
		if( !HarryClass )
			return NULL;
	}
	if( Properties.HarryClass!=HarryClass )
	{
		appMemzero( &Properties, sizeof(Properties) );
		Properties.HarryClass = HarryClass;
	}

	if( !Properties.bResolved )
	{
		Properties.bResolved = 1;
		const TCHAR* MissingProperty = NULL;

		Properties.ScreenRelativeMovement = FindField<UBoolProperty>( HarryClass, TEXT("bScreenRelativeMovement") );
		if( !Properties.ScreenRelativeMovement && !MissingProperty ) MissingProperty = TEXT("bScreenRelativeMovement");
		Properties.LockedOnTarget = FindField<UBoolProperty>( HarryClass, TEXT("bLockedOnTarget") );
		if( !Properties.LockedOnTarget && !MissingProperty ) MissingProperty = TEXT("bLockedOnTarget");
		Properties.FixedFaceDirection = FindField<UBoolProperty>( HarryClass, TEXT("bFixedFaceDirection") );
		if( !Properties.FixedFaceDirection && !MissingProperty ) MissingProperty = TEXT("bFixedFaceDirection");
		Properties.InDuelingMode = FindField<UBoolProperty>( HarryClass, TEXT("bInDuelingMode") );
		if( !Properties.InDuelingMode && !MissingProperty ) MissingProperty = TEXT("bInDuelingMode");
		Properties.ReverseInput = FindField<UBoolProperty>( HarryClass, TEXT("bReverseInput") );
		if( !Properties.ReverseInput && !MissingProperty ) MissingProperty = TEXT("bReverseInput");
		Properties.KeepStationary = FindField<UBoolProperty>( HarryClass, TEXT("bKeepStationary") );
		if( !Properties.KeepStationary && !MissingProperty ) MissingProperty = TEXT("bKeepStationary");
		Properties.LockOutForward = FindField<UBoolProperty>( HarryClass, TEXT("bLockOutForward") );
		if( !Properties.LockOutForward && !MissingProperty ) MissingProperty = TEXT("bLockOutForward");
		Properties.LockOutBackward = FindField<UBoolProperty>( HarryClass, TEXT("bLockOutBackward") );
		if( !Properties.LockOutBackward && !MissingProperty ) MissingProperty = TEXT("bLockOutBackward");
		Properties.LockOutStrafeLeft = FindField<UBoolProperty>( HarryClass, TEXT("bLockOutStrafeLeft") );
		if( !Properties.LockOutStrafeLeft && !MissingProperty ) MissingProperty = TEXT("bLockOutStrafeLeft");
		Properties.LockOutStrafeRight = FindField<UBoolProperty>( HarryClass, TEXT("bLockOutStrafeRight") );
		if( !Properties.LockOutStrafeRight && !MissingProperty ) MissingProperty = TEXT("bLockOutStrafeRight");
		Properties.IsAiming = FindField<UBoolProperty>( HarryClass, TEXT("bIsAiming") );
		if( !Properties.IsAiming && !MissingProperty ) MissingProperty = TEXT("bIsAiming");
		Properties.E3DemoLockout = FindField<UBoolProperty>( HarryClass, TEXT("bE3DemoLockout") );
		if( !Properties.E3DemoLockout && !MissingProperty ) MissingProperty = TEXT("bE3DemoLockout");
		Properties.AutoCenterCamera = FindField<UBoolProperty>( HarryClass, TEXT("bAutoCenterCamera") );
		if( !Properties.AutoCenterCamera && !MissingProperty ) MissingProperty = TEXT("bAutoCenterCamera");
		Properties.BossTarget = FindField<UObjectProperty>( HarryClass, TEXT("BossTarget") );
		if( !Properties.BossTarget && !MissingProperty ) MissingProperty = TEXT("BossTarget");

		Properties.bValid = MissingProperty==NULL;
		if( MissingProperty && !GReportedModernHarryContractFailure )
		{
			debugf
			(
				NAME_Warning,
				TEXT("Modern third-person controls disabled: HGame.Harry is missing required property %s"),
				MissingProperty
			);
			GReportedModernHarryContractFailure = 1;
		}
	}

	return Properties.bValid ? &Properties : NULL;
}

static UBOOL IsModernThirdPersonContext( APlayerPawn* PlayerPawn, const FModernHarryProperties& Properties )
{
	return
		IsInScriptState(PlayerPawn,TEXT("PlayerWalking"))
	&&	IsInScriptState(PlayerPawn->ViewTarget,TEXT("StateStandardCam"))
	&&	!PlayerPawn->bShowMenu
	&&	!PlayerPawn->bIsCaptured
	&&	!ReadModernHarryBool(PlayerPawn,Properties.KeepStationary)
	&&	!ReadModernHarryBool(PlayerPawn,Properties.E3DemoLockout)
	&&	!ReadModernHarryObject(PlayerPawn,Properties.BossTarget)
	&&	!ReadModernHarryBool(PlayerPawn,Properties.InDuelingMode)
	&&	!ReadModernHarryBool(PlayerPawn,Properties.LockedOnTarget)
	&&	!ReadModernHarryBool(PlayerPawn,Properties.FixedFaceDirection)
	&&	!ReadModernHarryBool(PlayerPawn,Properties.ReverseInput);
}
}

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

	if( !bModernThirdPersonControls )
		return 0;

	RawBaseX = aBaseX;
	RawStrafe = aStrafe;
	RawCameraYaw = aTurn;
	RawCameraPitch = aLookUp;

	FModernHarryProperties* Properties = GetModernHarryProperties( this );
	if( !Properties )
		return 0;

	WriteModernHarryBool( this, Properties->ScreenRelativeMovement, 0 );
	if( !IsModernThirdPersonContext(this,*Properties) )
		return 0;

	aBaseX = 0.f;
	aTurn = 0.f;
	aLookUp = 0.f;
	return 1;

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

	if( !Active || !bModernThirdPersonControls )
		return;

	FModernHarryProperties* Properties = GetModernHarryProperties( this );
	if( !Properties )
		return;

	WriteModernHarryBool( this, Properties->ScreenRelativeMovement, 0 );
	if
	(	ReadModernHarryBool(this,Properties->KeepStationary)
	||	!IsModernThirdPersonContext(this,*Properties) )
	{
		aForward = 0.f;
		aStrafe = 0.f;
		aSideMove = 0.f;
		return;
	}

	const FLOAT CameraScale = DesiredFOV * 0.01111f;
	SmoothMouseX += RawCameraYaw * CameraScale;
	SmoothMouseY += RawCameraPitch * CameraScale;

	RawBaseX = ExpandModernAnalogMovement( RawBaseX, DeltaSeconds, 0.2f, 20000.f ); // JoyX Speed=1 -> RotLeft/Right Speed=200.
	aForward = ExpandModernAnalogMovement( aForward, DeltaSeconds, 0.4f, 15000.f ); // JoyY Speed=2 -> MoveForward Speed=300.
	FLOAT EffectiveSide = 1.2f * RawBaseX + 0.8f * RawStrafe;
	if
	(	(EffectiveSide<0.f && ReadModernHarryBool(this,Properties->LockOutStrafeLeft))
	||	(EffectiveSide>0.f && ReadModernHarryBool(this,Properties->LockOutStrafeRight)) )
		EffectiveSide = 0.f;
	if
	(	(aForward>0.f && ReadModernHarryBool(this,Properties->LockOutForward))
	||	(aForward<0.f && ReadModernHarryBool(this,Properties->LockOutBackward)) )
		aForward = 0.f;

	aSideMove = EffectiveSide;
	aStrafe = 1.25f * EffectiveSide;
	SuppressModernAutoCenter( this, *Properties );
	WriteModernHarryBool( this, Properties->ScreenRelativeMovement, 1 );

	unguard;
}

void APlayerPawn::ApplyModernThirdPersonMovement()
{
	guard(APlayerPawn::ApplyModernThirdPersonMovement);

	RestoreModernAutoCenter( this );

	if( !bModernThirdPersonControls )
		return;

	FModernHarryProperties* Properties = GetModernHarryProperties( this );
	if( !Properties )
		return;

	const UBOOL WasActive = ReadModernHarryBool( this, Properties->ScreenRelativeMovement );
	WriteModernHarryBool( this, Properties->ScreenRelativeMovement, 0 );
	if( !WasActive || !IsModernThirdPersonContext(this,*Properties) )
		return;
	WriteModernHarryBool( this, Properties->ScreenRelativeMovement, 1 );

	const FCoords CameraAxes = GMath.UnitCoords / FRotator(0,ViewTarget->Rotation.Yaw,0);
	Acceleration = aForward * CameraAxes.XAxis + aSideMove * CameraAxes.YAxis;
	Acceleration.Z = 0.f;

	if( !ReadModernHarryBool(this,Properties->IsAiming) )
	{
		if( Acceleration.X!=0.f || Acceleration.Y!=0.f )
			DesiredRotation.Yaw = Acceleration.Rotation().Yaw;
		else if( Velocity.X!=0.f || Velocity.Y!=0.f )
		{
			FVector PlanarVelocity = Velocity;
			PlanarVelocity.Z = 0.f;
			DesiredRotation.Yaw = PlanarVelocity.Rotation().Yaw;
		}
		else
			DesiredRotation.Yaw = Rotation.Yaw;
	}

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

	*(FString*)Result = (CurrentInt<List.Num()) ? List(CurrentInt).Object : TEXT("");

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

	*EntryName = (CurrentInt<List.Num()) ? List(CurrentInt).Object : TEXT("");
	*Description = (CurrentInt<List.Num()) ? List(CurrentInt).Description : TEXT("");

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
		INT i = 0;
		for( TMultiMap<FString,FString>::TIterator It(*Sec); It; ++It )
		{
			if( *(*It.Value()) )
			{
				if( i == Num )
				{
					*GUID = *It.Key();
					*Filename = *It.Value();
					*(UBOOL*)Result = 1;
					return;
				}
				i++;
			}
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

	*(UBOOL*)Result = 0;
	TCHAR IniName[256], OrigFilename[256];
	FConfigCacheIni CacheIni;
	appSprintf( IniName, TEXT("%s") PATH_SEPARATOR TEXT("cache.ini"), *GSys->CachePath );
	if( !CacheIni.GetString( TEXT("Cache"), *GUID, OrigFilename, ARRAY_COUNT(OrigFilename), IniName ) )
		return;
	if( !*(*NewFilename) )
		NewFilename = OrigFilename;
	INT i;
	while( (i=NewFilename.InStr(TEXT("/"))) != -1 )
		NewFilename = NewFilename.Mid(i+1);
	while( (i=NewFilename.InStr(TEXT("\\"))) != -1 )
		NewFilename = NewFilename.Mid(i+1);
	while( (i=NewFilename.InStr(TEXT(":"))) != -1 )
		NewFilename = NewFilename.Mid(i+1);

	// get file extension
	FString FileExt = NewFilename;
	i = FileExt.InStr( TEXT(".") );
	if( i == -1 )
	{
		debugf(TEXT("MoveCacheEntry: No extension: %s"), *NewFilename);
		return;
	}
	while( i != -1 )
	{
		FileExt = FileExt.Mid( i + 1 );
		i = FileExt.InStr( TEXT(".") );
	}
	FileExt = FString(TEXT(".")) + FileExt;
	
	// get the destination path from a Paths array entry such as ../Maps/*.unr
	for( i=0; i<GSys->Paths.Num(); i++ )
	{
		if( GSys->Paths(i).Right( FileExt.Len() ) == FileExt )
		{
			INT SeparatorIndex = GSys->Paths(i).Len() - 1;
			for( ; SeparatorIndex>=0; --SeparatorIndex )
			{
				const TCHAR Ch = (*GSys->Paths(i))[SeparatorIndex];
				if( Ch=='/' || Ch=='\\' || Ch==':' )
					break;
			}

			FString DestPath = GSys->Paths(i).Left( SeparatorIndex + 1 );
			debugf( TEXT("MoveCacheEntry: %s -> %s"), *(GSys->CachePath + FString(PATH_SEPARATOR) + GUID + GSys->CacheExt), *(DestPath + NewFilename) );
			if( GFileManager->Move( *(DestPath + NewFilename), *(GSys->CachePath + FString(PATH_SEPARATOR) + GUID + GSys->CacheExt) ) )
			{
				CacheIni.SetString( TEXT("Cache"), *GUID, TEXT(""), IniName );
				*(UBOOL*)Result = 1;
			}
			return;
		}
	}
	unguard;
}

// hack check for valid skins for Unreal Tournament
void APawn::execCheckValidSkinPackage( FFrame& Stack, RESULT_DECL )
{
	guard(APawn::execCheckValidSkinPackage);

	P_GET_STR(Pkg);
	P_GET_STR(MeshName);
	P_FINISH;

	INT bProscribed = 0;
	INT bFound = 0;
	Pkg = Pkg.Caps();

	// make sure package is in serverpackages
	TCHAR *ClassName = TEXT("engine.gameengine");
	TCHAR *PropertyName = TEXT("serverpackages");
	UClass* Class;
	UProperty* Property;
	if
	( (Class=FindObject<UClass>( ANY_PACKAGE, ClassName))!=NULL )
	{
		if
		( (Property=FindField<UProperty>( Class, PropertyName))!=NULL )
		{
			TCHAR Temp[2048] = TEXT("");
			if( Class->Defaults.Num() )
				Property->ExportText( 0, Temp, &Class->Defaults(0), &Class->Defaults(0), PPF_Localized );
			FString List = Temp;
			List.Caps();
			if ( Pkg.InStr(List) )
			{
				bFound = 1;
			}
		}
	}
	// make sure player isn't getting textures from wrong package
	if( !bFound )
		bProscribed = true;
	else if ( (appStricmp(TEXT("UNREALSHARE"),*Pkg) == 0) || (appStricmp(TEXT("UNREALI"),*Pkg) == 0) || (appStricmp(TEXT("BOTPACK"),*Pkg) == 0) )
		bProscribed = true;
	else
	{
		FString LegalName = MeshName + TEXT("SKINS");
		LegalName = LegalName.Caps();
		if ( FString(Pkg).InStr(*LegalName) == -1 )
		{
			FString SkinPacks[] = {
				TEXT("BOSSSKINS"), 
				TEXT("SOLDIERSKINS"),
				TEXT("SGIRLSKINS"),
				TEXT("COMMANDOSKINS"),
				TEXT("TNALIMESHSKINS"),
				TEXT("TSKMSKINS"),
				TEXT("TCOWMESHSKINS") };
			for ( INT i=0; i<8; i++ )
			{
				if ( FString(Pkg).InStr(*SkinPacks[i]) != -1 )
				{
					bProscribed = true;
					break;
				}
			}
		}
	}
	*(UBOOL*)Result = !bProscribed;
	unguard;
}

void APlayerPawn::execUpdateURL( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execUpdateURL);

	P_GET_STR(NewOption);
	P_GET_STR(NewValue);
	P_GET_UBOOL(bSaveDefault);
	P_FINISH;

	UGameEngine* GameEngine = CastChecked<UGameEngine>( GetLevel()->Engine );
	GameEngine->LastURL.AddOption( *(NewOption + TEXT("=") + NewValue) );
	if( bSaveDefault )
		GameEngine->LastURL.SaveURLConfig( TEXT("DefaultPlayer"), *NewOption, TEXT("User") );
	unguard;
}

void APlayerPawn::execGetDefaultURL( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execGetDefaultURL);

	P_GET_STR(Option);
	P_FINISH;

	FURL URL;
	URL.LoadURLConfig( TEXT("DefaultPlayer"), TEXT("User") );

	*(FString*)Result = FString( URL.GetOption(*(Option + FString(TEXT("="))), TEXT("")) );
	unguard;
}

void AActor::execGetURLMap( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetURLMap);

	P_FINISH;

	*(FString*)Result = ((UGameEngine*)GetLevel()->Engine)->LastURL.Map;
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

	INT UseSkin = -1;
	INT FirstSkin = -1;
	INT PrevSkin = -1;
	INT UseNext = 0;

	INT n = appStrlen( *Prefix );

	for( INT i=0; i<List.Num(); i++ )
	{
		// if it matches the prefix
		if( appStrnicmp(*List(i).Object, *Prefix, n) == 0 )
		{
			if( UseNext )
			{
				UseSkin = i;
				UseNext = 0;
				break;
			}

			if( FirstSkin == -1 )
				FirstSkin = i;

			if( appStricmp(*List(i).Object, *CurrentSkin) == 0 ) 
			{
				if ( Dir == -1 )
				{
					UseSkin = PrevSkin;
					break;
				}
				else if ( Dir == 0 )
				{
					UseSkin = i;
					break;
				}
				else
				{
					UseNext = 1;
				}
			}
			PrevSkin = i;
		}	
	}
	
	if( UseNext )
		UseSkin = FirstSkin;

	// if we wanted to use the previous skin and
	// it didn't exist, choose the last skin.
	if( UseSkin == -1 )
		UseSkin = PrevSkin;

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

	check(XLevel);
	check(XLevel->Engine);
	check((UGameEngine*)(XLevel->Engine));
	check(((UGameEngine*)(XLevel->Engine))->GEntry);

	*(ALevelInfo**)Result = ((UGameEngine*)(XLevel->Engine))->GEntry->GetLevelInfo();

	unguard;
}

void APlayerPawn::execResetKeyboard( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execResetKeyboard);

	P_FINISH;

	UViewport* Viewport = Cast<UViewport>(Player);
	if( Viewport && Viewport->Input )
		ResetConfig(Viewport->Input->GetClass());
	unguard;
}

void APawn::execFindBestInventoryPath( FFrame& Stack, RESULT_DECL )
{
	guard(APawn::execFindBestInventoryPath);

	P_GET_FLOAT_REF(Weight);
	P_GET_UBOOL(bPredictRespawns);
	P_FINISH;

	clock(GetLevel()->FindPathCycles);
	AActor * bestPath = NULL;
	AActor * newPath;
	FLOAT BestWeight = findPathTowardBestInventory(newPath, 1, *Weight, bPredictRespawns);
	//debugf(NAME_DevPath,"BestWeight is %f compared to weight %f", BestWeight, *Weight);
	if ( BestWeight > *Weight )
	{
		bestPath = newPath;
		*Weight = BestWeight;
		//debugf(NAME_DevPath,"Recommend move to %s", bestPath->GetName());

		SpecialPause = 0.f;
		bShootSpecial = 0;

		if ( bestPath && bestPath->IsProbing(NAME_SpecialHandling) )
		{
			//debugf(NAME_DevPath,"Handle Special");
			HandleSpecial(bestPath);
			//debugf(NAME_DevPath,"Done Handle Special");
		}

		if ( bestPath == SpecialGoal )
			SpecialGoal = NULL;
	}
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

	*(FString*)Result = TEXT("");
	TCHAR Wildcard[256];
	TArray<FString> MapNames;
	appSprintf( Wildcard, TEXT("*.%s"), *FURL::DefaultMapExt );
	for( INT DoCD=0; DoCD<1+(GCdPath[0]!=0); DoCD++ )
	{
		for( INT i=0; i<GSys->Paths.Num(); i++ )
		{
			if( appStrstr( *GSys->Paths(i), Wildcard ) )
			{
				TCHAR Tmp[256]=TEXT("");
				if( DoCD )
				{
					appStrcat( Tmp, GCdPath );
					appStrcat( Tmp, TEXT("System") PATH_SEPARATOR );
				}
				appStrcat( Tmp, *GSys->Paths(i) );
				*appStrstr( Tmp, Wildcard )=0;
				appStrcat( Tmp, *Prefix );
				appStrcat( Tmp, Wildcard );
				TArray<FString>	TheseNames = GFileManager->FindFiles(Tmp,1,0);
				for( INT i=0; i<TheseNames.Num(); i++ )
				{
					INT j;
					for( j=0; j<MapNames.Num(); j++ )
						if( appStricmp(*MapNames(j),*TheseNames(i))==0 )
							break;
					if( j==MapNames.Num() )
						new(MapNames)FString(TheseNames(i));
				}
			}
		}
	}
	FString Seperator = FString::Printf(TEXT("\\"));
	for( INT i=0; i<MapNames.Num(); i++ )
	{
		// Strip out any leading path crap (for umod meta packages).
		while( MapNames(i).InStr( Seperator ) >= 0 )
		{
			INT Pos = MapNames(i).InStr( Seperator ) + 1;
			MapNames(i) = MapNames(i).Right( MapNames(i).Len() - Pos );
		}
	}
	for( INT i=0; i<MapNames.Num(); i++ )
	{
		if( appStrcmp(*MapNames(i),*MapName)==0 )
		{
			INT Offset = i+Dir;
			if( Offset < 0 )
				Offset = MapNames.Num() - 1;
			else if( Offset >= MapNames.Num() )
				Offset = 0;
			*(FString*)Result = MapNames(Offset);
			return;
		}
	}
	if( MapNames.Num() > 0 )
		*(FString*)Result = MapNames(0);
	else
		*(FString*)Result = FString(TEXT(""));

	unguard;
}

void APlayerPawn::execConsoleCommand( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execConsoleCommand);

	P_GET_STR(Command);
	P_FINISH;

	*(FString*)Result = TEXT("");
	FStringOutputDevice StrOut;
	if( Player )
	{
		Player->Exec( *Command, StrOut );
		*(FString*)Result = *StrOut;
	}
	else
	{
		GetLevel()->Engine->Exec( *Command, StrOut );
		*(FString*)Result = *StrOut;
	}
	unguard;
}

void APlayerPawn::execScreenToWorld( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execScreenToWorld);

	P_GET_VECTOR(S);
	P_FINISH;

	// Unproject point using viewport dimensions.
	UViewport* Viewport = GetLevel()->Engine->Client->Viewports(0);
	const FLOAT EffectiveFovAngle = UClient::GetEffectiveFovAngle( FovAngle, Viewport->SizeX, Viewport->SizeY, GetLevel()->Engine->Client->MaintainVerticalFOV );
	FLOAT ProjZ = Viewport->SizeX / appTan(EffectiveFovAngle * PI/360.f);
	FLOAT Z = S.Z / ProjZ;
	FVector C( (2.f * S.X - Viewport->SizeX) * Z, (2.f * S.Y - Viewport->SizeY) * Z, S.Z );

	// Convert from camera to world space, using player's current view.
	AActor*		ViewActor    = this;
	FVector		ViewLocation = Location;
	FRotator	ViewRotation = Rotation;
	eventPlayerCalcView( ViewActor, ViewLocation, ViewRotation );

	*(FVector*)Result = C.TransformVectorBy( GMath.UnitCoords * ViewRotation ) + ViewLocation;
	unguard;
}

void APawn::execStopWaiting( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execStopWaiting);

	P_FINISH;

	if( GetStateFrame()->LatentAction == EPOLL_Sleep )
		LatentFloat = -1.f;

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

	int seen = 0;
	int NetMode = GetLevel()->GetLevelInfo()->NetMode;
	if ( (NetMode == NM_Standalone)
		|| ((NetMode == NM_Client) && (bNetTemporary || (Role == ROLE_Authority))) )
	{
		// just check local player visibility
		for( INT i=0; i<GetLevel()->Engine->Client->Viewports.Num(); i++ )
			if ( TestCanSeeMe( GetLevel()->Engine->Client->Viewports(i)->Actor ) )
			{
				seen = 1;
				break;
			}
	}
	else
	{
		for ( APawn *next=GetLevel()->GetLevelInfo()->PawnList; next!=NULL; next=next->nextPawn )
			if ( TestCanSeeMe((APlayerPawn *)next) )
			{
				seen = 1;
				break;
			}
	}
	*(DWORD*)Result = seen;
	unguard;
}

int AActor::TestCanSeeMe( APlayerPawn *Viewer )
{
	guard(AActor::TestCanSeeMe);

	if ( !Viewer )
		return 0;
	if ( Viewer->ViewTarget == this )
		return 1;

	float distSq = (Location - Viewer->Location).SizeSquared();
	return ( (distSq < 100000.f * (CollisionRadius + 3.6)) 
		&& (Viewer->bBehindView 
			|| (Square(Viewer->ViewRotation.Vector() | (Location - Viewer->Location)) >= 0.25f * distSq))
		&& Viewer->LineOfSightTo(this) );

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
	APawn *pick = NULL;
	for ( APawn *next=GetLevel()->GetLevelInfo()->PawnList; next!=NULL; next=next->nextPawn )
	{
		if ( (next != this) && (next->Health > 0) && next->bProjTarget
			&& (!PlayerReplicationInfo || !next->PlayerReplicationInfo
				|| !GetLevel()->GetLevelInfo()->Game->bTeamGame
				|| (PlayerReplicationInfo->Team != next->PlayerReplicationInfo->Team)) )
		{
			FLOAT newAim = FireDir | (next->Location - projStart);
			if ( newAim > 0 )
			{
				FLOAT FireDist = (next->Location - projStart).SizeSquared();
				if ( FireDist < 4000000.f )
				{
					FireDist = appSqrt(FireDist);
					newAim = newAim/FireDist;
					if ( (newAim > *bestAim) && LineOfSightTo(next) )
					{
						pick = next;
						*bestAim = newAim;
						*bestDist = FireDist;
					}
				}
			}
		}
	}

	*(APawn**)Result = pick; 
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
	AActor *pick = NULL;

	for( INT iActor=0; iActor<GetLevel()->Actors.Num(); iActor++ )
		if( GetLevel()->Actors(iActor) )
		{
			AActor* next = GetLevel()->Actors(iActor);
			if ( next->bProjTarget && !next->IsA(APawn::StaticClass()) )
			{
				FLOAT newAim = FireDir | (next->Location - projStart);
				if ( newAim > 0 )
				{
					FLOAT FireDist = (next->Location - projStart).SizeSquared();
					if ( FireDist < 4000000.f )
					{
						FireDist = appSqrt(FireDist);
						newAim = newAim/FireDist;
						if ( (newAim > *bestAim) && LineOfSightTo(next) )
						{
							pick = next;
							*bestAim = newAim;
							*bestDist = FireDist;
						}
					}
				}
			}
		}

	*(AActor**)Result = pick; 
	unguardSlow;
}

void APawn::execAddPawn( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execAddPawn);

	P_FINISH;

	nextPawn = GetLevel()->GetLevelInfo()->PawnList;
	GetLevel()->GetLevelInfo()->PawnList = this;
	unguardSlow;
}

void APawn::execRemovePawn( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execRemovePawn);

	P_FINISH;

	APawn *next = GetLevel()->GetLevelInfo()->PawnList;
	if ( next == this )
		GetLevel()->GetLevelInfo()->PawnList = next->nextPawn;
	else
	{
		while ( next )
		{
			if ( next->nextPawn == this )
			{
				next->nextPawn = nextPawn;
				break;
			}
			next = next->nextPawn;
		}
	}

	unguardSlow;
}

/* execWaitForLanding()
wait until physics is not PHYS_Falling
*/
void APawn::execWaitForLanding( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execWaitForLanding);

	P_FINISH;

	LatentFloat = 2.5;
	if (Physics == PHYS_Falling)
		GetStateFrame()->LatentAction = AI_PollWaitForLanding;
	unguardSlow;
}

void APawn::execPollWaitForLanding( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollWaitForLanding);
	if( Physics != PHYS_Falling )
	{
		GetStateFrame()->LatentAction = 0;
	}
	else
	{
		FLOAT DeltaSeconds = *(FLOAT*)Result;
		LatentFloat -= DeltaSeconds;
		if ( LatentFloat < 0 )
			eventLongFall();
	}
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

	if(deltaTime > 0.33f)
	{
		*(DWORD*)Result = ViewRotation.Pitch;
		return;
	}
	if(ViewRotation.Pitch > 32768)
		ViewRotation.Pitch = (ViewRotation.Pitch & 65535) - 65536;
	
	FCheckResult Hit(1.f);
	FRotator LookRot = ViewRotation;
	LookRot.Pitch = 0;
	FVector Dir = LookRot.Vector();
	FVector EyeSpot = Location + FVector(0,0,BaseEyeHeight);
	FLOAT height = CollisionHeight + BaseEyeHeight; 
	FVector CollisionSlice(CollisionRadius,CollisionRadius,1);

	GetLevel()->SingleLineCheck(Hit, this, EyeSpot + 2 * height * Dir, EyeSpot, TRACE_VisBlocking, CollisionSlice);
	FLOAT Dist = 2 * height * Hit.Time;
	INT stairRot = 0;
	if(Dist > 0.8 * height)
	{
		FVector Spot = EyeSpot + 0.5f * Dist * Dir;
		FLOAT Down = 3 * height;
		GetLevel()->SingleLineCheck(Hit, this, Spot - FVector(0,0,Down), Spot, TRACE_VisBlocking, CollisionSlice);
		if(Hit.Time < 1.f)
		{
			FLOAT firstDown = Down * Hit.Time;
			if (firstDown < 0.7f * height - 6.f) // then up or level
			{
				Spot = EyeSpot + Dist * Dir;
				GetLevel()->SingleLineCheck(Hit, this, Spot - FVector(0,0,Down), Spot, TRACE_VisBlocking, CollisionSlice);
				stairRot = ::Max(0, ViewRotation.Pitch);
				if( Down * Hit.Time < firstDown - 10 ) 
					stairRot = 5400;
			}
			else if(firstDown > 0.7f * height + 6.f) // then down or level
			{
				GetLevel()->SingleLineCheck(Hit, this, Location + 0.9f*Dist*Dir, Location, TRACE_VisBlocking);
				if(Hit.Time == 1.f)
				{
					Spot = EyeSpot + Dist * Dir;
					GetLevel()->SingleLineCheck(Hit, this, Spot - FVector(0,0,Down), Spot, TRACE_VisBlocking, CollisionSlice);
					stairRot = Min(0, ViewRotation.Pitch);
					if (Down * Hit.Time > firstDown + 10)
						stairRot = -5000;
				}
			}
		}
	}
	INT Diff = Abs(ViewRotation.Pitch - stairRot);
	if( Diff > 0 )
	{
		FLOAT RotRate = 8;
		if( Diff < 1000 )
			RotRate = 8000/Diff; 

		RotRate = ::Min(1.f, RotRate * deltaTime);
		stairRot = appRound(FLOAT(ViewRotation.Pitch) * ( 1 - RotRate) + FLOAT(stairRot) * RotRate);
	}
	*(DWORD*)Result = stairRot; 
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
	AActor *RealActor = NULL;

	if ( actor->IsA(AInventory::StaticClass()) && ((AInventory *)actor)->MyMarker )
	{
		RealActor = actor;
		actor = ((AInventory *)actor)->MyMarker;
	}
	if ( actor->IsA(ANavigationPoint::StaticClass()) && GetLevel()->ReachSpecs.Num() && (CollisionRadius <= MAXCOMMONRADIUS) )
	{
		FLOAT MaxDistSq = ::Max(48.f, CollisionRadius);
		FVector Dir;
		INT bOnPath = 0;
		MaxDistSq = MaxDistSq * MaxDistSq;
		if ( MoveTarget && MoveTarget->IsA(ANavigationPoint::StaticClass())
			&& (Abs(MoveTarget->Location.Z - Location.Z) < CollisionHeight) ) 
		{
			Dir = MoveTarget->Location - Location;
			Dir.Z = 0;
			if ( (Dir | Dir) < MaxDistSq )
			{
				bOnPath = 1;
				if ( (MoveTarget == actor) || CanMoveTo(MoveTarget, actor) )
				{
					*(DWORD*)Result = 1;
					unclock(GetLevel()->FindPathCycles);
					return;
				}
			}
		}
		ANavigationPoint *Nav = GetLevel()->GetLevelInfo()->NavigationPointList;
		while (Nav)
		{
			if ( Abs(Nav->Location.Z - Location.Z) < CollisionHeight )
			{
				Dir = Nav->Location - Location;
				Dir.Z = 0;
				if ( (Dir | Dir) < MaxDistSq ) 
				{
					bOnPath = 1;
					if ( (Nav == actor) || CanMoveTo(Nav, actor) )
					{
						*(DWORD*)Result = 1;
						unclock(GetLevel()->FindPathCycles);
						return;
					}
				}
			}
			Nav = Nav->nextNavigationPoint;
		}
		if ( bOnPath && (Physics != PHYS_Flying) )
		{
			*(DWORD*)Result = 0;
			unclock(GetLevel()->FindPathCycles);
			return;
		}
	}	
	if ( RealActor )
		actor = RealActor;

	*(DWORD*)Result = actorReachable(actor);  
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
	AActor * bestPath = NULL;
	AActor * newPath;
	if (findPathTo(point, bSinglePath, newPath, bClearPaths))
		bestPath = newPath;
	SpecialPause = 0.f;
	bShootSpecial = 0;

	if ( bestPath && bestPath->IsProbing(NAME_SpecialHandling) )
	{
		//debugf(NAME_DevPath,"Handle Special");
		HandleSpecial(bestPath);
		//debugf(NAME_DevPath,"Done Handle Special");
	}

	if ( bestPath == SpecialGoal )
		SpecialGoal = NULL;
	unclock(GetLevel()->FindPathCycles);
	//debugf("Find path to time was %f", GetLevel()->FindPathCycles * MSecPerCycle);

	*(AActor**)Result = bestPath; 
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
	AActor * bestPath = NULL;
	AActor * newPath;
	if (findPathToward(goal, bSinglePath, newPath, bClearPaths))
		bestPath = newPath;
	SpecialPause = 0.f;
	bShootSpecial = 0;

	if ( bestPath && bestPath->IsProbing(NAME_SpecialHandling) )
	{
		//debugf(NAME_DevPath,"Handle Special");
		HandleSpecial(bestPath);
		//debugf(NAME_DevPath,"Done Handle Special");
	}

	if ( bestPath == SpecialGoal )
		SpecialGoal = NULL;
	unclock(GetLevel()->FindPathCycles);
	//debugf("Find path toward time was %f", GetLevel()->FindPathCycles * MSecPerCycle);

	*(AActor**)Result = bestPath; 
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
	if (bClearPaths)
		clearPaths();
	ANavigationPoint * bestPath = NULL;
	AActor * newPath;
	if ( findRandomDest(newPath) )
		bestPath = (ANavigationPoint *)newPath;

	unclock(GetLevel()->FindPathCycles);

	*(ANavigationPoint**)Result = bestPath; 
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
	
	//debugf(" %s Make Noise with instigator", GetFullName(),Instigator->GetClass()->GetName());
	if ( GetLevel()->GetLevelInfo()->NetMode != NM_Client )
		CheckNoiseHearing(Loudness);
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

	FVector Move = dest - Location;
	MoveTarget = NULL;
	bReducedSpeed = 0;
	DesiredSpeed = ::Max(0.f, Min(MaxDesiredSpeed, speed));
	FLOAT MoveSize = Move.Size();
	setMoveTimer(MoveSize); 
	GetStateFrame()->LatentAction = AI_PollMoveTo;
	Destination = dest;
	Focus = dest;
	rotateToward(Focus);
	moveToward(Destination);
	unguardSlow;
}

void APawn::execPollMoveTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollMoveTo);
	rotateToward(Focus);
	if( moveToward(Destination) )
		GetStateFrame()->LatentAction = 0;
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

	if (!goal)
	{
		//Stack.Log("MoveToward with no goal");
		return;
	}

	FVector Move = goal->Location - Location;	
	bReducedSpeed = 0;
	DesiredSpeed = ::Max(0.f, Min(MaxDesiredSpeed, speed));
	if (goal->IsA(APawn::StaticClass()))
		MoveTimer = 1.2f; //max before re-assess movetoward
	else
	{
		FLOAT MoveSize = Move.Size();
		setMoveTimer(MoveSize);
	}
	MoveTarget = goal;
	Destination = MoveTarget->Location; 
	Focus = Destination;
	GetStateFrame()->LatentAction = AI_PollMoveToward;
	rotateToward(Focus);
	moveToward(Destination);
	unguardSlow;
}

void APawn::execPollMoveToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollMoveToward);

	if( !MoveTarget )
	{
		//Stack.Log("MoveTarget cleared during movetoward");
		GetStateFrame()->LatentAction = 0;
		return;
	}

	Destination = MoveTarget->Location;
	if( Physics==PHYS_Flying && MoveTarget->IsA(APawn::StaticClass()) )
		Destination.Z += 0.7f * MoveTarget->CollisionHeight;
	else if( Physics == PHYS_Spider )
		Destination = Destination - MoveTarget->CollisionRadius * Floor;

	Focus = Destination;
	rotateToward(Focus);
	FLOAT oldDesiredSpeed = DesiredSpeed;
	if ( bAdvancedTactics && (Physics == PHYS_Walking) )
		eventAlterDestination();
	if( moveToward(Destination) )
		GetStateFrame()->LatentAction = 0;
	if ( bAdvancedTactics && (Physics == PHYS_Walking) )
		Destination = MoveTarget->Location;
	if( MoveTarget->IsA(APawn::StaticClass()) )
	{
		DesiredSpeed = oldDesiredSpeed; //don't slow down when moving toward a pawn
		if (!bCanSwim && MoveTarget->Region.Zone->bWaterZone)
			MoveTimer = -1.f; //give up
	}

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

	FVector Move = Dest - Location;
	MoveTarget = NULL;
	bReducedSpeed = 0;
	if (bIsPlayer)
		DesiredSpeed = MaxDesiredSpeed;
	else
		DesiredSpeed = 0.8f * MaxDesiredSpeed;
	FLOAT MoveSize = Move.Size();
	setMoveTimer(MoveSize); 
	GetStateFrame()->LatentAction = AI_PollStrafeTo;
	Destination = Dest;
	Focus = FocalPoint;
	rotateToward(Focus);
	moveToward(Destination);
	unguardSlow;
}

void APawn::execPollStrafeTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollStrafeTo);

	rotateToward( Focus );
	if( moveToward(Destination) )
		GetStateFrame()->LatentAction = 0;

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

	if (!goal)
	{
		//Stack.Log("StrafeFacing without goal");
		return;
	}
	FVector Move = Dest - Location;	
	bReducedSpeed = 0;
	if (bIsPlayer)
		DesiredSpeed = MaxDesiredSpeed;
	else
		DesiredSpeed = 0.8f * MaxDesiredSpeed;
	FLOAT MoveSize = Move.Size();
	setMoveTimer(MoveSize); 
	Destination = Dest;
	FaceTarget = goal;
	Focus = FaceTarget->Location;
	GetStateFrame()->LatentAction = AI_PollStrafeFacing;
	rotateToward(Focus);
	moveToward(Destination);
	unguardSlow;
}

void APawn::execPollStrafeFacing( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollStrafeFacing);

	if( !FaceTarget )
	{
		//Stack.Log("FaceTarget cleared during strafefacing");
		GetStateFrame()->LatentAction = 0;
		return;
	}

	Focus = FaceTarget->Location;
	FVector RealDest = Destination;
	rotateToward( Focus );
	if ( bAdvancedTactics && (Physics == PHYS_Walking) )
		eventAlterDestination();
	if( moveToward(Destination) )
		GetStateFrame()->LatentAction = 0;
	Destination = RealDest;

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
	
	if (!goal)
		return;

	FaceTarget = goal;
	GetStateFrame()->LatentAction = AI_PollTurnToward;
	if ( !bCanStrafe && ((Physics == PHYS_Flying) || (Physics == PHYS_Swimming)) )
		Acceleration = Rotation.Vector() * AccelRate;

	Focus = FaceTarget->Location;
	rotateToward(Focus);
	unguardSlow;
}

void APawn::execPollTurnToward( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollTurnToward);

	if( !FaceTarget )
	{
		//Stack.Log("FaceTarget cleared during turntoward");
		GetStateFrame()->LatentAction = 0;
		return;
	}

	if( !bCanStrafe && (Physics==PHYS_Flying || Physics==PHYS_Swimming) )
		Acceleration = Rotation.Vector() * AccelRate;

	Focus = FaceTarget->Location;
	if( rotateToward(Focus) )
		GetStateFrame()->LatentAction = 0;  

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

	MoveTarget = NULL;
	GetStateFrame()->LatentAction = AI_PollTurnTo;
	Focus = FocalPoint;

	//FT: if in phys_walking, make the turn to focus be in the same plane as this
	if( Physics == PHYS_Walking )
		Focus.Z = Location.Z;

	if ( !bCanStrafe && ((Physics == PHYS_Flying) || (Physics == PHYS_Swimming)) )
		Acceleration = Rotation.Vector() * AccelRate;

	rotateToward(Focus);
	unguardSlow;
}

void APawn::execPollTurnTo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APawn::execPollTurnTo);

	if( !bCanStrafe && (Physics==PHYS_Flying || Physics==PHYS_Swimming) )
		Acceleration = Rotation.Vector() * AccelRate;

	if( rotateToward(Focus) )
		GetStateFrame()->LatentAction = 0;  

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
	for ( APawn *Pawn=GetLevel()->GetLevelInfo()->PawnList; Pawn!=NULL; Pawn=Pawn->nextPawn )
		if( Pawn!=this && Pawn->SightCounter<0.f )
		{
			//check visibility
			if ( Pawn->IsProbing(NAME_SeePlayer) && Pawn->LineOfSightTo(this, 1) )
				Pawn->eventSeePlayer(this);
		}

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
