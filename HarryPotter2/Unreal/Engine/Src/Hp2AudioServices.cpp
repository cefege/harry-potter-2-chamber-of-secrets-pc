/*=============================================================================
	Hp2AudioServices.cpp: Engine callbacks used by the Rust audio port
	(crates/hp2-engine/src/audio.rs). Each entry is the original expression from
	UnAudio.cpp. Rust owns the control flow. Order must match the
	Hp2AudioServices struct in audio.rs.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2AudioServices.h"

// Derived accessor for FLazyLoader::SavedPos, which is protected. TLazyArray<BYTE>
// adds no members, so the layout matches the object passed in.
struct Hp2LazyArrayAccess : public TLazyArray<BYTE>
{
	static INT SavedPosOf( const void* Lazy )
	{
		return ((const Hp2LazyArrayAccess*)Lazy)->FLazyLoader::SavedPos;
	}
};

static void AudioLazyLoad( void* Lazy )
{
	((TLazyArray<BYTE>*)Lazy)->TLazyArray<BYTE>::Load();
}

static void AudioLazyUnload( void* Lazy )
{
	((TLazyArray<BYTE>*)Lazy)->TLazyArray<BYTE>::Unload();
}

static INT AudioLazySavedPos( const void* Lazy )
{
	return Hp2LazyArrayAccess::SavedPosOf( Lazy );
}

static void AudioU8ArraySet( void* Arr, INT Count )
{
	((TArray<BYTE>*)Arr)->Set( Count );
}

static void AudioU8ArrayRemove( void* Arr, INT Index, INT Count )
{
	((TArray<BYTE>*)Arr)->Remove( Index, Count );
}

static void AudioI32ArrayAdd( void* Arr )
{
	((TArray<INT>*)Arr)->Add();
}

static void AudioI32ArrayRemove( void* Arr, INT Index )
{
	((TArray<INT>*)Arr)->Remove( Index );
}

static void AudioI32ArrayEmpty( void* Arr )
{
	((TArray<INT>*)Arr)->Empty();
}

static void AudioMapSet( void* Sound, INT Key, INT Value )
{
	((USound*)Sound)->PlayingStreams.Set( Key, Value );
}

static INT AudioMapFindRef( void* Sound, INT Key )
{
	return ((USound*)Sound)->PlayingStreams.FindRef( Key );
}

static void AudioMapRemove( void* Sound, INT Key )
{
	((USound*)Sound)->PlayingStreams.Remove( Key );
}

static void AudioMapEmpty( void* Sound )
{
	((USound*)Sound)->PlayingStreams.Empty();
}

static INT AudioFileTypeIsPs2( const void* Sound )
{
	return ((const USound*)Sound)->FileType == FName(TEXT("PS2")) ? 1 : 0;
}

// Compress= must parse to "Compress"; Lipsync= must parse to "lipsync".
static INT AudioParseIs( const void* Parms, DWORD Which )
{
	FString Value;
	if( Which == 0 )
	{
		Parse( (const TCHAR*)Parms, TEXT("Compress="), Value );
		return Value == TEXT("Compress") ? 1 : 0;
	}
	Parse( (const TCHAR*)Parms, TEXT("Lipsync="), Value );
	return Value == TEXT("lipsync") ? 1 : 0;
}

static void AudioLogSampleRateUnsupported( const void* Filename, INT Rate )
{
	GWarn->Logf( TEXT("Sample rate not supported:  file = %s, rate = %d"), (const TCHAR*)Filename, Rate );
}

static void AudioLogDeletingBoundStream()
{
	debugf( NAME_DevSound, TEXT("Deleteing a bound stream!") );
}

static INT AudioArchiveIsLoading( void* Ar )
{
	return ((FArchive*)Ar)->IsLoading() ? 1 : 0;
}

static INT AudioArchiveIsSaving( void* Ar )
{
	return ((FArchive*)Ar)->IsSaving() ? 1 : 0;
}

static INT AudioArchiveVer( void* Ar )
{
	return ((FArchive*)Ar)->Ver();
}

static void AudioArchiveSerializeI32( void* Ar, void* Value )
{
	*(FArchive*)Ar << *(INT*)Value;
}

static void AudioArchiveSerializeF32( void* Ar, void* Value )
{
	*(FArchive*)Ar << *(FLOAT*)Value;
}

static void AudioArchiveSerializeName( void* Ar, void* Value )
{
	*(FArchive*)Ar << *(FName*)Value;
}

static void AudioArchiveSerializeSoundData( void* Ar, void* Value )
{
	*(FArchive*)Ar << *(FSoundData*)Value;
}

static void AudioArchiveSerializeLipSyncData( void* Ar, void* Value )
{
	*(FArchive*)Ar << *(FLipSyncData*)Value;
}

static void AudioArchiveCountBytes( void* Ar, INT Num, INT Max )
{
	((FArchive*)Ar)->CountBytes( Num, Max );
}

static void AudioUSoundSuperSerialize( void* Sound, void* Ar )
{
	((USound*)Sound)->UObject::Serialize( *(FArchive*)Ar );
}

const Hp2AudioServices GHp2AudioServices =
{
	AudioLazyLoad,
	AudioLazyUnload,
	AudioLazySavedPos,
	AudioU8ArraySet,
	AudioU8ArrayRemove,
	AudioI32ArrayAdd,
	AudioI32ArrayRemove,
	AudioI32ArrayEmpty,
	AudioMapSet,
	AudioMapFindRef,
	AudioMapRemove,
	AudioMapEmpty,
	AudioFileTypeIsPs2,
	AudioParseIs,
	AudioLogSampleRateUnsupported,
	AudioLogDeletingBoundStream,
	AudioArchiveIsLoading,
	AudioArchiveIsSaving,
	AudioArchiveVer,
	AudioArchiveSerializeI32,
	AudioArchiveSerializeF32,
	AudioArchiveSerializeName,
	AudioArchiveSerializeSoundData,
	AudioArchiveSerializeLipSyncData,
	AudioArchiveCountBytes,
	AudioUSoundSuperSerialize,
};
