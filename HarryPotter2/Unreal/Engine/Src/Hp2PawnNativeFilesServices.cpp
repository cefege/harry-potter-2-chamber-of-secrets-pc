/*=============================================================================
	Hp2PawnNativeFilesServices.cpp: Cache, file, URL, console, and viewport
	services called by the Rust engine port
	(crates/hp2-engine/src/pawn_native_files.rs). Each adapter decodes raw
	pointers into the C++ types and forwards to the engine call the original
	UnPawn.cpp body made.
=============================================================================*/

#include "EnginePrivate.h"
#include "FConfigCacheIni.h"
#include "Hp2PawnNativeFilesServices.h"

/* Strings handed to Rust live here. Rust copies every result before it makes
   another services call, so one buffer is enough. */
static TArray<UNICHAR> NativeFilesUnits;

/* Matches of the most recent find_files call, read back by found_file_at. */
static TArray<FString> NativeFilesFound;

/* Widens a TCHAR string into the shared UNICHAR buffer. A null input stays
   null, which Rust reads as absent. */
static const UNICHAR* NativeFilesPublish( const TCHAR* Text )
{
	if( !Text )
		return NULL;
	NativeFilesUnits.Empty();
	for( INT i=0; ; i++ )
	{
		NativeFilesUnits.AddItem( ToUnicode(Text[i]) );
		if( !Text[i] )
			break;
	}
	return NativeFilesUnits.GetData();
}

/* Narrows a NUL-terminated UNICHAR string from Rust to an FString. */
static FString NativeFilesFromUnits( const UNICHAR* Units )
{
	TArray<TCHAR> Text;
	for( INT i=0; Units && Units[i]; i++ )
		Text.AddItem( FromUnicode(Units[i]) );
	Text.AddItem( 0 );
	return FString( Text.GetData() );
}

/* Public forms of the text helpers declared in Hp2PawnNativeFilesServices.h. */
FString Hp2PawnNativeFilesString( const UNICHAR* Units )
{
	return NativeFilesFromUnits( Units );
}

TArray<UNICHAR> Hp2PawnNativeFilesUnits( const TCHAR* Text )
{
	TArray<UNICHAR> Units;
	for( INT i=0; Text && Text[i]; i++ )
		Units.AddItem( ToUnicode(Text[i]) );
	Units.AddItem( 0 );
	return Units;
}

/* The cache.ini path that UnPawn.cpp built with appSprintf. */
static FString NativeFilesCacheIniName()
{
	TCHAR IniName[256];
	appSprintf( IniName, TEXT("%s") PATH_SEPARATOR TEXT("cache.ini"), *GSys->CachePath );
	return FString( IniName );
}

static const UNICHAR* NativeFilesCacheGetString( const UNICHAR* Guid )
{
	FConfigCacheIni CacheIni;
	FString GUID = NativeFilesFromUnits( Guid );
	FString IniName = NativeFilesCacheIniName();
	TCHAR OrigFilename[256];
	if( !CacheIni.GetString( TEXT("Cache"), *GUID, OrigFilename, ARRAY_COUNT(OrigFilename), *IniName ) )
		return NULL;
	return NativeFilesPublish( OrigFilename );
}

static void NativeFilesCacheClear( const UNICHAR* Guid )
{
	FConfigCacheIni CacheIni;
	FString GUID = NativeFilesFromUnits( Guid );
	FString IniName = NativeFilesCacheIniName();
	CacheIni.SetString( TEXT("Cache"), *GUID, TEXT(""), *IniName );
}

static const UNICHAR* NativeFilesCachePath()
{
	return NativeFilesPublish( *GSys->CachePath );
}

static const UNICHAR* NativeFilesCacheExt()
{
	return NativeFilesPublish( *GSys->CacheExt );
}

static const UNICHAR* NativeFilesPathSeparator()
{
	return NativeFilesPublish( PATH_SEPARATOR );
}

static INT NativeFilesPathsCount()
{
	return GSys->Paths.Num();
}

static const UNICHAR* NativeFilesPathAt( INT Index )
{
	return NativeFilesPublish( *GSys->Paths(Index) );
}

static INT NativeFilesFileMove( const UNICHAR* DestUnits, const UNICHAR* SrcUnits )
{
	FString Dest = NativeFilesFromUnits( DestUnits );
	FString Src = NativeFilesFromUnits( SrcUnits );
	return GFileManager->Move( *Dest, *Src ) ? 1 : 0;
}

static void NativeFilesLogNoExtension( const UNICHAR* NewFilenameUnits )
{
	FString NewFilename = NativeFilesFromUnits( NewFilenameUnits );
	debugf( TEXT("MoveCacheEntry: No extension: %s"), *NewFilename );
}

static void NativeFilesLogMove( const UNICHAR* SrcUnits, const UNICHAR* DestUnits )
{
	FString Src = NativeFilesFromUnits( SrcUnits );
	FString Dest = NativeFilesFromUnits( DestUnits );
	debugf( TEXT("MoveCacheEntry: %s -> %s"), *Src, *Dest );
}

static const UNICHAR* NativeFilesUrlDefaultOption( const UNICHAR* OptionUnits )
{
	FString Option = NativeFilesFromUnits( OptionUnits );
	FURL URL;
	URL.LoadURLConfig( TEXT("DefaultPlayer"), TEXT("User") );
	FString Value = FString( URL.GetOption( *Option, TEXT("") ) );
	return NativeFilesPublish( *Value );
}

static void NativeFilesUrlAddOption( void* This, const UNICHAR* TextUnits )
{
	FString Text = NativeFilesFromUnits( TextUnits );
	UGameEngine* GameEngine = CastChecked<UGameEngine>( ((APlayerPawn*)This)->GetLevel()->Engine );
	GameEngine->LastURL.AddOption( *Text );
}

static void NativeFilesUrlSaveDefault( void* This, const UNICHAR* OptionUnits )
{
	FString Option = NativeFilesFromUnits( OptionUnits );
	UGameEngine* GameEngine = CastChecked<UGameEngine>( ((APlayerPawn*)This)->GetLevel()->Engine );
	GameEngine->LastURL.SaveURLConfig( TEXT("DefaultPlayer"), *Option, TEXT("User") );
}

static const UNICHAR* NativeFilesServerPackages()
{
	const TCHAR* ClassName = TEXT("engine.gameengine");
	const TCHAR* PropertyName = TEXT("serverpackages");
	UClass* Class = FindObject<UClass>( ANY_PACKAGE, ClassName );
	if( !Class )
		return NULL;
	UProperty* Property = FindField<UProperty>( Class, PropertyName );
	if( !Property )
		return NULL;
	TCHAR Temp[2048] = TEXT("");
	if( Class->Defaults.Num() )
		Property->ExportText( 0, Temp, &Class->Defaults(0), &Class->Defaults(0), PPF_Localized );
	return NativeFilesPublish( Temp );
}

static INT NativeFilesFindFiles( const UNICHAR* PatternUnits )
{
	FString Pattern = NativeFilesFromUnits( PatternUnits );
	NativeFilesFound = GFileManager->FindFiles( *Pattern, 1, 0 );
	return NativeFilesFound.Num();
}

static const UNICHAR* NativeFilesFoundFileAt( INT Index )
{
	return NativeFilesPublish( *NativeFilesFound(Index) );
}

static const UNICHAR* NativeFilesMapExt()
{
	return NativeFilesPublish( *FURL::DefaultMapExt );
}

static const UNICHAR* NativeFilesCdPath()
{
	return NativeFilesPublish( GCdPath );
}

static FLOAT NativeFilesFindPathTowardBest( void* This, FLOAT Weight, INT Predict, void** NewPath )
{
	AActor* BestPath = NULL;
	FLOAT BestWeight = ((APawn*)This)->findPathTowardBestInventory( BestPath, 1, Weight, Predict );
	*NewPath = BestPath;
	return BestWeight;
}

static void NativeFilesResetSpecialState( void* This )
{
	((APawn*)This)->SpecialPause = 0.f;
	((APawn*)This)->bShootSpecial = 0;
}

static INT NativeFilesIsProbingSpecial( void* Path )
{
	return ((AActor*)Path)->IsProbing( NAME_SpecialHandling ) ? 1 : 0;
}

static void NativeFilesHandleSpecial( void* This, void* Path )
{
	AActor* BestPath = (AActor*)Path;
	((APawn*)This)->HandleSpecial( BestPath );
}

static void* NativeFilesSpecialGoal( void* This )
{
	return ((APawn*)This)->SpecialGoal;
}

static void NativeFilesClearSpecialGoal( void* This )
{
	((APawn*)This)->SpecialGoal = NULL;
}

static void* NativeFilesKeyboardConfigClass( void* This )
{
	UViewport* Viewport = Cast<UViewport>( ((APlayerPawn*)This)->Player );
	if( Viewport && Viewport->Input )
		return Viewport->Input->GetClass();
	return NULL;
}

static void NativeFilesResetConfig( void* Klass )
{
	UObject::ResetConfig( (UClass*)Klass );
}

static INT NativeFilesHasPlayer( void* This )
{
	return ((APlayerPawn*)This)->Player != NULL ? 1 : 0;
}

static const UNICHAR* NativeFilesPlayerExec( void* This, const UNICHAR* CommandUnits )
{
	FString Command = NativeFilesFromUnits( CommandUnits );
	FStringOutputDevice StrOut;
	((APlayerPawn*)This)->Player->Exec( *Command, StrOut );
	return NativeFilesPublish( *StrOut );
}

static const UNICHAR* NativeFilesEngineExec( void* This, const UNICHAR* CommandUnits )
{
	FString Command = NativeFilesFromUnits( CommandUnits );
	FStringOutputDevice StrOut;
	((APlayerPawn*)This)->GetLevel()->Engine->Exec( *Command, StrOut );
	return NativeFilesPublish( *StrOut );
}

static void NativeFilesViewportSize( void* This, INT* SizeX, INT* SizeY )
{
	UViewport* Viewport = ((APlayerPawn*)This)->GetLevel()->Engine->Client->Viewports(0);
	*SizeX = Viewport->SizeX;
	*SizeY = Viewport->SizeY;
}

static FLOAT NativeFilesPawnFov( void* This )
{
	return ((APlayerPawn*)This)->FovAngle;
}

static INT NativeFilesMaintainVerticalFov( void* This )
{
	return ((APlayerPawn*)This)->GetLevel()->Engine->Client->MaintainVerticalFOV;
}

static FLOAT NativeFilesEffectiveFov( FLOAT Fov, INT SizeX, INT SizeY, INT Maintain )
{
	return UClient::GetEffectiveFovAngle( Fov, SizeX, SizeY, Maintain );
}

static void NativeFilesPawnLocation( void* This, void* Out )
{
	*(FVector*)Out = ((APlayerPawn*)This)->Location;
}

static void NativeFilesPawnRotation( void* This, void* Out )
{
	*(FRotator*)Out = ((APlayerPawn*)This)->Rotation;
}

static void NativeFilesCalcView( void* This, void* Loc, void* Rot )
{
	APlayerPawn* Pawn = (APlayerPawn*)This;
	AActor* ViewActor = Pawn;
	FVector ViewLocation = *(FVector*)Loc;
	FRotator ViewRotation = *(FRotator*)Rot;
	Pawn->eventPlayerCalcView( ViewActor, ViewLocation, ViewRotation );
	*(FVector*)Loc = ViewLocation;
	*(FRotator*)Rot = ViewRotation;
}

static FLOAT NativeFilesSinTab( INT Angle )
{
	return GMath.SinTab( Angle );
}

static FLOAT NativeFilesCosTab( INT Angle )
{
	return GMath.CosTab( Angle );
}

static const UNICHAR* NativeFilesLastURLMap( void* This )
{
	AActor* Actor = (AActor*)This;
	FString Map = ((UGameEngine*)Actor->GetLevel()->Engine)->LastURL.Map;
	return NativeFilesPublish( *Map );
}

static void* NativeFilesEntryLevel( void* This )
{
	APlayerPawn* Pawn = (APlayerPawn*)This;
	check(Pawn->XLevel);
	check(Pawn->XLevel->Engine);
	check((UGameEngine*)(Pawn->XLevel->Engine));
	check(((UGameEngine*)(Pawn->XLevel->Engine))->GEntry);
	return ((UGameEngine*)(Pawn->XLevel->Engine))->GEntry->GetLevelInfo();
}

const Hp2PawnNativeFilesServices GHp2PawnNativeFilesServices = {
	NativeFilesCacheGetString,
	NativeFilesCacheClear,
	NativeFilesCachePath,
	NativeFilesCacheExt,
	NativeFilesPathSeparator,
	NativeFilesPathsCount,
	NativeFilesPathAt,
	NativeFilesFileMove,
	NativeFilesLogNoExtension,
	NativeFilesLogMove,
	NativeFilesUrlDefaultOption,
	NativeFilesUrlAddOption,
	NativeFilesUrlSaveDefault,
	NativeFilesServerPackages,
	NativeFilesFindFiles,
	NativeFilesFoundFileAt,
	NativeFilesMapExt,
	NativeFilesCdPath,
	NativeFilesFindPathTowardBest,
	NativeFilesResetSpecialState,
	NativeFilesIsProbingSpecial,
	NativeFilesHandleSpecial,
	NativeFilesSpecialGoal,
	NativeFilesClearSpecialGoal,
	NativeFilesKeyboardConfigClass,
	NativeFilesResetConfig,
	NativeFilesHasPlayer,
	NativeFilesPlayerExec,
	NativeFilesEngineExec,
	NativeFilesViewportSize,
	NativeFilesPawnFov,
	NativeFilesMaintainVerticalFov,
	NativeFilesEffectiveFov,
	NativeFilesPawnLocation,
	NativeFilesPawnRotation,
	NativeFilesCalcView,
	NativeFilesSinTab,
	NativeFilesCosTab,
	NativeFilesLastURLMap,
	NativeFilesEntryLevel,
};
