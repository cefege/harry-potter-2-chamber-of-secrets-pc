/*=============================================================================
	Hp2PawnNativeInputServices.cpp: Modern third-person input services called
	by the Rust engine port (crates/hp2-engine/src/pawn_native_input.rs). Each
	adapter decodes raw pointers into the C++ types and forwards to the engine
	reflection or math call the original UnPawn.cpp body made.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2PawnNativeInputServices.h"

/* Widens an ASCII property or state name to TCHAR. The buffer is reused, so
   the result is valid only until the next call. */
static const TCHAR* NativeInputName( const char* Name )
{
	static TCHAR Buffer[64];
	INT Index = 0;
	for( ; Name[Index] && Index < 63; Index++ )
		Buffer[Index] = (TCHAR)(unsigned char)Name[Index];
	Buffer[Index] = 0;
	return Buffer;
}

static void* NativeInputFindBoolProperty( void* Class, const char* Name )
{
	return FindField<UBoolProperty>( (UClass*)Class, NativeInputName(Name) );
}

static void* NativeInputFindObjectProperty( void* Class, const char* Name )
{
	return FindField<UObjectProperty>( (UClass*)Class, NativeInputName(Name) );
}

static INT NativeInputPropertyOffset( void* Property )
{
	return ((UProperty*)Property)->Offset;
}

static DWORD NativeInputBoolPropertyMask( void* Property )
{
	return ((UBoolProperty*)Property)->BitMask;
}

static void* NativeInputObjectClass( void* Object )
{
	return ((UObject*)Object)->GetClass();
}

static void* NativeInputClassSuper( void* Class )
{
	return ((UClass*)Class)->GetSuperClass();
}

static INT NativeInputIsA( void* Object, void* Class )
{
	return ((UObject*)Object)->IsA( (UClass*)Class );
}

static INT NativeInputClassPathEquals( void* Class, const char* Path )
{
	return appStricmp( ((UClass*)Class)->GetPathName(), NativeInputName(Path) )==0;
}

static void* NativeInputStateNode( void* Object )
{
	UObject* Obj = (UObject*)Object;
	if( !Obj || !Obj->GetStateFrame() )
		return NULL;
	return Obj->GetStateFrame()->StateNode;
}

static void* NativeInputStateSuper( void* State )
{
	return ((UState*)State)->GetSuperState();
}

static INT NativeInputStateNameEquals( void* State, const char* Name )
{
	const FName RequiredState( NativeInputName(Name), FNAME_Find );
	return ((UState*)State)->GetFName()==RequiredState;
}

static void NativeInputLogMissingProperty( const char* Name )
{
	debugf
	(
		NAME_Warning,
		TEXT("Modern third-person controls disabled: HGame.Harry is missing required property %s"),
		NativeInputName(Name)
	);
}

static FLOAT NativeInputSinTab( INT Angle )
{
	return GMath.SinTab( Angle );
}

static FLOAT NativeInputCosTab( INT Angle )
{
	return GMath.CosTab( Angle );
}

const Hp2PawnNativeInputServices GHp2PawnNativeInputServices =
{
	NativeInputFindBoolProperty,
	NativeInputFindObjectProperty,
	NativeInputPropertyOffset,
	NativeInputBoolPropertyMask,
	NativeInputObjectClass,
	NativeInputClassSuper,
	NativeInputIsA,
	NativeInputClassPathEquals,
	NativeInputStateNode,
	NativeInputStateSuper,
	NativeInputStateNameEquals,
	NativeInputLogMissingProperty,
	NativeInputSinTab,
	NativeInputCosTab,
};
