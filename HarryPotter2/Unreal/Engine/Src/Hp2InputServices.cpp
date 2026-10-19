/*=============================================================================
	Hp2InputServices.cpp: UInput engine services called by the Rust engine port
	(crates/hp2-engine/src/input.rs). Each adapter wraps one engine call with
	the exact C signature mirrored in Rust.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2InputServices.h"

static_assert( NAME_None==0, "Hp2InputServices: FName index 0 must be NAME_None" );

static void* Hp2InputViewportActor( void* Viewport )
{
	return ((UViewport*)Viewport)->Actor;
}

static void* Hp2InputViewportInput( void* Viewport )
{
	return ((UViewport*)Viewport)->Input;
}

static void Hp2InputViewportUpdateInput( void* Viewport, int32_t Reset )
{
	((UViewport*)Viewport)->UpdateInput( Reset );
}

static void Hp2InputViewportExec( void* Viewport, const uint32_t* Cmd, void* Ar )
{
	((UViewport*)Viewport)->Exec( (const TCHAR*)Cmd, *(FOutputDevice*)Ar );
}

static void Hp2InputLogInitialized( void* Viewport )
{
	debugf( NAME_Init, TEXT("Input system initialized for %s"), ((UViewport*)Viewport)->GetName() );
}

static void* Hp2InputActorViewport( void* Actor )
{
	APlayerPawn* PlayerPawn = Cast<APlayerPawn>( (UObject*)Actor );
	UViewport* Viewport = PlayerPawn ? Cast<UViewport>(PlayerPawn->Player) : NULL;
	return Viewport;
}

static void* Hp2InputObjectClass( void* Object )
{
	return ((UObject*)Object)->GetClass();
}

static int32_t Hp2InputIsRunning()
{
	return GIsRunning;
}

static void* Hp2InputGLog()
{
	return GLog;
}

static void Hp2InputCheckFailed( const char* Expr, int32_t Line )
{
	appFailAssert( Expr, "HarryPotter2/Unreal/Engine/Src/UnIn.cpp", Line );
}

static void Hp2InputOutputLog( void* Ar, const uint32_t* Text )
{
	((FOutputDevice*)Ar)->Log( (const TCHAR*)Text );
}

static void Hp2InputOutputLogf( void* Ar, const uint32_t* Text )
{
	((FOutputDevice*)Ar)->Logf( (const TCHAR*)Text );
}

static int32_t Hp2InputParseLine( const uint32_t** Stream, uint32_t* Out, int32_t Max )
{
	return ParseLine( (const TCHAR**)Stream, (TCHAR*)Out, Max );
}

static int32_t Hp2InputParseCommand( const uint32_t** Stream, const uint32_t* Match )
{
	return ParseCommand( (const TCHAR**)Stream, (const TCHAR*)Match );
}

static int32_t Hp2InputParseToken( const uint32_t** Stream, uint32_t* Out, int32_t Max, int32_t UseEscape )
{
	/* ParseToken advances its const TCHAR*& argument; write the cursor back. */
	const TCHAR* Str = (const TCHAR*)*Stream;
	UBOOL Result = ParseToken( Str, (TCHAR*)Out, Max, UseEscape );
	*Stream = (const uint32_t*)Str;
	return Result;
}

static int32_t Hp2InputParseFloat( const uint32_t* Stream, const uint32_t* Match, float* Out )
{
	return Parse( (const TCHAR*)Stream, (const TCHAR*)Match, *Out );
}

static int32_t Hp2InputAppAtoi( const uint32_t* Str )
{
	return appAtoi( (const TCHAR*)Str );
}

static int32_t Hp2InputAppStrlen( const uint32_t* Str )
{
	return appStrlen( (const TCHAR*)Str );
}

static void Hp2InputAppSprintfIK( uint32_t* Dest, const uint32_t* Name )
{
	appSprintf( (TCHAR*)Dest, TEXT("IK_%s"), (const TCHAR*)Name );
}

static int32_t Hp2InputFNameFind( const uint32_t* Text )
{
	return FName( (const TCHAR*)Text, FNAME_Find ).GetIndex();
}

static int32_t Hp2InputAliasNameIndex( void* This, int32_t Index )
{
	return ((UInput*)This)->Aliases[Index].Alias.GetIndex();
}

static const uint32_t* Hp2InputAliasCommand( void* This, int32_t Index )
{
	return (const uint32_t*)*((UInput*)This)->Aliases[Index].Command;
}

static int32_t Hp2InputBindingLen( void* This, int32_t Key )
{
	return ((UInput*)This)->Bindings[Key].Len();
}

static const uint32_t* Hp2InputBindingChars( void* This, int32_t Key )
{
	return (const uint32_t*)*((UInput*)This)->Bindings[Key];
}

static void* Hp2InputDirectAxisCommand( void* This, int32_t Key, float Length )
{
	FString* Command = new FString( FString::Printf( TEXT("%s Speed=%f"), *((UInput*)This)->Bindings[Key], Length ) );
	return Command;
}

static const uint32_t* Hp2InputFStringChars( void* Handle )
{
	FString* Command = (FString*)Handle;
	return (const uint32_t*)**Command;
}

static void Hp2InputFStringFree( void* Handle )
{
	delete (FString*)Handle;
}

static const uint32_t* Hp2InputKeyNameChars( void* InputKeys, int32_t Key )
{
	return (const uint32_t*)*((UEnum*)InputKeys)->Names(Key);
}

static int32_t Hp2InputKeyNameFindItem( void* InputKeys, const uint32_t* Text, int32_t* Out )
{
	FName N( (const TCHAR*)Text, FNAME_Find );
	if( N != NAME_None )
		return ((UEnum*)InputKeys)->Names.FindItem( N, *Out );
	return 0;
}

template <class T> static void* Hp2InputNthField( UClass* Klass, INT N )
{
	INT Index = 0;
	for( TFieldIterator<T> It(Klass); It; ++It, Index++ )
		if( Index==N )
			return *It;
	return NULL;
}

static void* Hp2InputClassNthProperty( void* Klass, int32_t Kind, int32_t N )
{
	if( Kind==0 )
		return Hp2InputNthField<UProperty>( (UClass*)Klass, N );
	if( Kind==1 )
		return Hp2InputNthField<UByteProperty>( (UClass*)Klass, N );
	if( Kind==2 )
		return Hp2InputNthField<UFloatProperty>( (UClass*)Klass, N );
	return NULL;
}

static uint32_t Hp2InputPropertyFlags( void* Prop )
{
	return (DWORD)((UProperty*)Prop)->PropertyFlags;
}

static int32_t Hp2InputPropertyFNameIndex( void* Prop )
{
	return ((UProperty*)Prop)->GetFName().GetIndex();
}

static int32_t Hp2InputPropertyOffset( void* Prop )
{
	return ((UProperty*)Prop)->Offset;
}

static int32_t Hp2InputPropertyIsByte( void* Prop )
{
	return Cast<UByteProperty>( (UProperty*)Prop ) != NULL;
}

static int32_t Hp2InputPropertyIsFloat( void* Prop )
{
	return Cast<UFloatProperty>( (UProperty*)Prop ) != NULL;
}

static void* Hp2InputCacheGet( void* Klass, void** Item )
{
	FMemCache::FCacheItem* CacheItem = (FMemCache::FCacheItem*)*Item;
	void* Result = GCache.Get( MakeCacheID( CID_InputMap, (UClass*)Klass ), CacheItem );
	*Item = CacheItem;
	return Result;
}

static void* Hp2InputCacheCreate( void* Klass, void** Item, uintptr_t Size )
{
	FMemCache::FCacheItem* CacheItem = (FMemCache::FCacheItem*)*Item;
	void* Result = GCache.Create( MakeCacheID( CID_InputMap, (UClass*)Klass ), CacheItem, appCheckedIntSize( (SIZE_T)Size ) );
	*Item = CacheItem;
	return Result;
}

static void Hp2InputCacheUnlock( void* Item )
{
	((FMemCache::FCacheItem*)Item)->Unlock();
}

/* Field order must match struct Hp2InputServices in crates/hp2-engine/src/input.rs. */
const Hp2InputServices GHp2InputServices =
{
	Hp2InputViewportActor,
	Hp2InputViewportInput,
	Hp2InputViewportUpdateInput,
	Hp2InputViewportExec,
	Hp2InputLogInitialized,
	Hp2InputActorViewport,
	Hp2InputObjectClass,
	Hp2InputIsRunning,
	Hp2InputGLog,
	Hp2InputCheckFailed,
	Hp2InputOutputLog,
	Hp2InputOutputLogf,
	Hp2InputParseLine,
	Hp2InputParseCommand,
	Hp2InputParseToken,
	Hp2InputParseFloat,
	Hp2InputAppAtoi,
	Hp2InputAppStrlen,
	Hp2InputAppSprintfIK,
	Hp2InputFNameFind,
	Hp2InputAliasNameIndex,
	Hp2InputAliasCommand,
	Hp2InputBindingLen,
	Hp2InputBindingChars,
	Hp2InputDirectAxisCommand,
	Hp2InputFStringChars,
	Hp2InputFStringFree,
	Hp2InputKeyNameChars,
	Hp2InputKeyNameFindItem,
	Hp2InputClassNthProperty,
	Hp2InputPropertyFlags,
	Hp2InputPropertyFNameIndex,
	Hp2InputPropertyOffset,
	Hp2InputPropertyIsByte,
	Hp2InputPropertyIsFloat,
	Hp2InputCacheGet,
	Hp2InputCacheCreate,
	Hp2InputCacheUnlock,
};
