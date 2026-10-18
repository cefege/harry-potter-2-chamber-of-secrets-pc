/*=============================================================================
	ALAudioSubsystem.cpp: Unreal OpenAL Audio interface object.
	Copyright 1999-2001 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel.

	C++ side only. The subsystem logic lives in the Rust crate hp2-audio and is
	driven through the hp2_rs_audio_* exports (crates/hp2-ffi/src/audio.rs). This
	file holds the UObject shell (config properties, Init/Destroy, engine overrides)
	and the trampolines that let Rust reach engine globals (USound, AActor, ULevel,
	GFileStream, the Ogg decoder) and the OpenAL/ALC entry points. See
	ALAudioSubsystem.h for the ownership split.
=============================================================================*/

/*------------------------------------------------------------------------------------
	Audio includes.
------------------------------------------------------------------------------------*/
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <exception>
#include <string>
#include <vector>
#include <AL/al.h>
#include <AL/alc.h>
#include "ALAudioPrivate.h"
#include "hp2_rs.h"

/*------------------------------------------------------------------------------------
	Layout checks. Each mirror struct must match its Rust #[repr(C)] twin in
	crates/hp2-ffi/src/audio.rs, which has matching size and offset tests.
------------------------------------------------------------------------------------*/
static_assert( sizeof(FTime) == 8, "FTime must be 8 bytes; Rust receives it as i64" );
static_assert( sizeof(FfiVec3) == 12 && offsetof(FfiVec3,z) == 8, "FfiVec3 layout" );
static_assert( sizeof(FfiAudioConfig) == 28, "FfiAudioConfig layout" );
static_assert( sizeof(FfiSoundInfo) == 28, "FfiSoundInfo layout" );
static_assert( sizeof(FfiWaveInfo) == 32, "FfiWaveInfo layout" );
static_assert( sizeof(FfiVorbisInfo) == 24, "FfiVorbisInfo layout" );
static_assert( sizeof(FfiActorInfo) == 72, "FfiActorInfo layout" );
static_assert( sizeof(FfiViewportInfo) == 56, "FfiViewportInfo layout" );
static_assert( sizeof(FfiPopCompleted) == 24 && offsetof(FfiPopCompleted,dest) == 8, "FfiPopCompleted layout" );
static_assert( sizeof(FfiPlayRequest) == 64, "FfiPlayRequest layout" );
static_assert( sizeof(FfiUpdateView) == 48, "FfiUpdateView layout" );
static_assert( sizeof(FfiFidelitySource) == 16 && offsetof(FfiFidelitySource,paused) == 8, "FfiFidelitySource layout" );
static_assert( sizeof(FfiExecCommand) == 4, "FfiExecCommand layout" );
static_assert( sizeof(FfiHost) == 600, "FfiHost layout" );
static_assert( offsetof(FfiHost,alc_close_device) == 74 * sizeof(void*), "FfiHost function table" );
static_assert( offsetof(FfiAudioConfig,max_channels) == 16 && offsetof(FfiAudioConfig,max_radius_multiplier) == 20 && offsetof(FfiAudioConfig,use_precache) == 24 && offsetof(FfiAudioConfig,reverse_stereo) == 27, "FfiAudioConfig offsets" );
static_assert( offsetof(FfiSoundInfo,raw_sample_rate) == 20 && offsetof(FfiSoundInfo,is_ogg) == 24, "FfiSoundInfo offsets" );
static_assert( offsetof(FfiWaveInfo,sample_data) == 16 && offsetof(FfiWaveInfo,sample_data_size) == 24, "FfiWaveInfo offsets" );
static_assert( offsetof(FfiVorbisInfo,rate) == 8 && offsetof(FfiVorbisInfo,time_total) == 16, "FfiVorbisInfo offsets" );
static_assert( offsetof(FfiActorInfo,velocity) == 12 && offsetof(FfiActorInfo,ambient_sound) == 32 && offsetof(FfiActorInfo,actor_level) == 56 && offsetof(FfiActorInfo,sound_pitch) == 64 && offsetof(FfiActorInfo,light_brightness) == 68, "FfiActorInfo offsets" );
static_assert( offsetof(FfiViewportInfo,view_target) == 8 && offsetof(FfiViewportInfo,viewport_actor_level) == 32 && offsetof(FfiViewportInfo,view_target_location) == 40, "FfiViewportInfo offsets" );
static_assert( offsetof(FfiPopCompleted,terminal) == 16, "FfiPopCompleted terminal offset" );
static_assert( offsetof(FfiPlayRequest,sound) == 16 && offsetof(FfiPlayRequest,location) == 24 && offsetof(FfiPlayRequest,flags) == 48 && offsetof(FfiPlayRequest,in_priority) == 56, "FfiPlayRequest offsets" );
static_assert( offsetof(FfiUpdateView,z_axis) == 36, "FfiUpdateView z_axis offset" );

/*------------------------------------------------------------------------------------
	Trampoline plumbing.

	Engine calls made from Rust can throw. Each engine-touching trampoline runs its
	body inside Guarded(), which catches the exception, records the first one in the
	per-call state, and returns 0 so Rust unwinds without any C++ exception crossing
	its frames. The shell rethrows the recorded exception with Finish() once the Rust
	call has returned. OpenAL/ALC and app_seconds cannot throw and are not guarded.
------------------------------------------------------------------------------------*/
namespace
{

// Per-call state. Output is the device that Rust's `output` writes to. Utf8 holds
// the most recent string returned to Rust; Rust copies it before the next call.
struct FHostCall
{
	FOutputDevice*		Output;
	std::exception_ptr	Error;
	std::string			Utf8;

	FHostCall() : Output(GLog), Error(), Utf8() {}
};

template< class Fn >
uint8_t Guarded( void* ctx, Fn Body )
{
	FHostCall* Call = static_cast<FHostCall*>(ctx);
	try
	{
		Body();
		return 1;
	}
	catch( ... )
	{
		if( !Call->Error )
			Call->Error = std::current_exception();
		return 0;
	}
}

void Rethrow( FHostCall& Call )
{
	if( Call.Error )
	{
		std::exception_ptr E = Call.Error;
		Call.Error = std::exception_ptr();
		std::rethrow_exception( E );
	}
}

// Rethrows any engine exception recorded during the call. A zero status with no
// recorded exception means the Rust side failed on its own.
void Finish( FHostCall& Call, int32_t Status )
{
	Rethrow( Call );
	if( Status == 0 )
		appErrorf( TEXT("ALAudio: Rust subsystem call failed.") );
}

std::string ToUtf8( const TCHAR* S )
{
	std::vector<ANSICHAR> Buf( 4 * appStrlen(S) + 1 );
	if( !appToUtf8InPlace( &Buf[0], S, (INT)Buf.size() ) )
		appErrorf( TEXT("ALAudio: UTF-8 conversion failed.") );
	return std::string( &Buf[0] );
}

FString FromUtf8( const uint8_t* P, size_t Len )
{
	std::string Tmp( (const char*)P, Len );
	std::vector<TCHAR> Buf( Len + 2 );
	if( !appFromUtf8InPlace( &Buf[0], Tmp.c_str(), (INT)Buf.size() ) )
		appErrorf( TEXT("ALAudio: UTF-8 decode failed.") );
	return FString( &Buf[0] );
}

// Hands an engine string to Rust. The buffer lives in the call state, so Rust
// must copy it before the next call.
void ReturnString( FHostCall& Call, const TCHAR* S, const uint8_t** OutPtr, size_t* OutLen )
{
	Call.Utf8 = ToUtf8( S );
	*OutPtr = (const uint8_t*)Call.Utf8.data();
	*OutLen = Call.Utf8.size();
}

FVector ToFVector( const FfiVec3& V )
{
	return FVector( V.x, V.y, V.z );
}

FfiVec3 ToFfi( const FVector& V )
{
	FfiVec3 R;
	R.x = V.X;
	R.y = V.Y;
	R.z = V.Z;
	return R;
}

// Builds the host table. Every field must be set; a null entry is a crash on the
// Rust side.
FfiHost MakeHost( FHostCall& Call );

}	// namespace

/*------------------------------------------------------------------------------------
	Host trampolines. Rust calls these through FfiHost. Each one is a single engine
	operation. Their signatures must match the field types in FfiHost.
------------------------------------------------------------------------------------*/
extern "C"
{

// Logging and time.

static uint8_t AlHostLog( void* ctx, uint32_t kind, const uint8_t* text, size_t len )
{
	return Guarded( ctx, [&]()
	{
		FString S = FromUtf8( text, len );
		switch( kind )
		{
		case 0:	debugf( NAME_Init,     TEXT("%s"), *S ); break;
		case 1:	debugf( NAME_Exit,     TEXT("%s"), *S ); break;
		case 2:	debugf( NAME_DevSound, TEXT("%s"), *S ); break;
		case 3:	debugf( NAME_DevMusic, TEXT("%s"), *S ); break;
		case 4:	debugf( NAME_Warning,  TEXT("%s"), *S ); break;
		case 5:	debugf( TEXT("%s"), *S ); break;
		default:
			appErrorf( TEXT("ALAudio: unknown log kind %i."), (INT)kind );
		}
	} );
}

static uint8_t AlHostOutput( void* ctx, const uint8_t* text, size_t len )
{
	return Guarded( ctx, [&]()
	{
		FHostCall* Call = (FHostCall*)ctx;
		FString S = FromUtf8( text, len );
		Call->Output->Logf( TEXT("%s"), *S );
	} );
}

static uint8_t AlHostAppSeconds( void* ctx, int64_t* out )
{
	(void)ctx;
	FTime Now = appSeconds();
	memcpy( out, &Now, sizeof(Now) );
	return 1;
}

static uint8_t AlHostAppError( void* ctx, const uint8_t* text, size_t len )
{
	return Guarded( ctx, [&]()
	{
		FString S = FromUtf8( text, len );
		appErrorf( TEXT("%s"), *S );
	} );
}

// USound.

static uint8_t AlHostSoundInfo( void* ctx, size_t sound, FfiSoundInfo* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		out->core_flags = S->CoreFlags;
		out->working_flags = S->WorkingFlags;
		out->raw_num_samples = S->raw_NumSamples;
		out->raw_bits_per_sample = S->raw_BitsPerSample;
		out->raw_num_channels = S->raw_NumChannels;
		out->raw_sample_rate = S->raw_SampleRate;
		out->is_ogg = appStricmp( *S->FileType, TEXT("ogg") ) == 0 ? 1 : 0;
	} );
}

static uint8_t AlHostSoundDataLen( void* ctx, size_t sound, int32_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		*out = S->Data.Num();
	} );
}

static uint8_t AlHostSoundName( void* ctx, size_t sound, const uint8_t** out_ptr, size_t* out_len )
{
	return Guarded( ctx, [&]()
	{
		FHostCall* Call = (FHostCall*)ctx;
		USound* S = (USound*)sound;
		ReturnString( *Call, S->GetName(), out_ptr, out_len );
	} );
}

static uint8_t AlHostSoundPathName( void* ctx, size_t sound, const uint8_t** out_ptr, size_t* out_len )
{
	return Guarded( ctx, [&]()
	{
		FHostCall* Call = (FHostCall*)ctx;
		USound* S = (USound*)sound;
		ReturnString( *Call, S->GetPathName(), out_ptr, out_len );
	} );
}

static uint8_t AlHostSoundFilename( void* ctx, size_t sound, const uint8_t** out_ptr, size_t* out_len )
{
	return Guarded( ctx, [&]()
	{
		FHostCall* Call = (FHostCall*)ctx;
		USound* S = (USound*)sound;
		ReturnString( *Call, *S->Filename, out_ptr, out_len );
	} );
}

static uint8_t AlHostSoundSetWorkingFlags( void* ctx, size_t sound, int32_t flags )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->WorkingFlags = flags;
	} );
}

static uint8_t AlHostSoundSetHandle( void* ctx, size_t sound, int32_t handle )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		// Direct write, as the original RegisterSound and the non-streaming paths do.
		// SetHandle() is a no-op for SF_Streaming sounds, which would drop the write.
		S->Handle = handle;
	} );
}

static uint8_t AlHostSoundGetHandle( void* ctx, size_t sound, int32_t id, int32_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		*out = S->GetHandle( id );
	} );
}

static uint8_t AlHostSoundSetDuration( void* ctx, size_t sound, float duration )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->Duration = duration;
	} );
}

static uint8_t AlHostSoundAddRef( void* ctx, size_t sound, int32_t delta, int32_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->RefCount += delta;
		*out = S->RefCount;
	} );
}

static uint8_t AlHostSoundIsRegistered( void* ctx, size_t sound, uint8_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		*out = S->IsRegistered() ? 1 : 0;
	} );
}

static uint8_t AlHostSoundAddStreamHandle( void* ctx, size_t sound, int32_t id )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->AddStreamHandle( id );
	} );
}

static uint8_t AlHostSoundDeleteStreamHandle( void* ctx, size_t sound, int32_t handle )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->DeleteStreamHandle( handle );
	} );
}

static uint8_t AlHostSoundBindStream( void* ctx, size_t sound, int32_t id )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->BindStream( id );
	} );
}

static uint8_t AlHostSoundUnbindStream( void* ctx, size_t sound, int32_t id )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->UnbindStream( id );
	} );
}

static uint8_t AlHostSoundUnbindAllStreams( void* ctx, size_t sound )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->UnbindAllStreams();
	} );
}

static uint8_t AlHostSoundDeleteAllStreamHandles( void* ctx, size_t sound )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->DeleteAllStreamHandles();
	} );
}

static uint8_t AlHostSoundPlayingStreamCount( void* ctx, size_t sound, size_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		size_t N = 0;
		for( TMap<INT,INT>::TIterator It(S->PlayingStreams); It; ++It )
			++N;
		*out = N;
	} );
}

static uint8_t AlHostSoundPlayingStreamFill( void* ctx, size_t sound, int32_t* out, size_t cap )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		size_t I = 0;
		for( TMap<INT,INT>::TIterator It(S->PlayingStreams); It; ++It )
		{
			if( I >= cap )
				appErrorf( TEXT("ALAudio: playing stream count grew since count.") );
			out[I++] = S->OpenStreams( It.Value() ) / 2;
		}
		if( I != cap )
			appErrorf( TEXT("ALAudio: playing stream count changed since count.") );
	} );
}

static uint8_t AlHostSoundLoadData( void* ctx, size_t sound )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->Data.Load();
	} );
}

static uint8_t AlHostSoundUnloadData( void* ctx, size_t sound )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		S->Data.Unload();
	} );
}

static uint8_t AlHostSoundWaveInfo( void* ctx, size_t sound, FfiWaveInfo* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		FWaveModInfo W;
		W.ReadWaveInfo( S->Data );
		out->sample_loops_num = (int32_t)W.SampleLoopsNum;
		out->bits_per_sample = (int32_t)*W.pBitsPerSample;
		out->channels = (int32_t)*W.pChannels;
		out->samples_per_sec = (int32_t)*W.pSamplesPerSec;
		out->sample_data = (size_t)W.SampleDataStart;
		out->sample_data_size = (int32_t)W.SampleDataSize;
	} );
}

static uint8_t AlHostSoundCount( void* ctx, size_t* out )
{
	return Guarded( ctx, [&]()
	{
		size_t N = 0;
		for( TObjectIterator<USound> It; It; ++It )
			++N;
		*out = N;
	} );
}

static uint8_t AlHostSoundFill( void* ctx, size_t* out, size_t cap )
{
	return Guarded( ctx, [&]()
	{
		size_t I = 0;
		for( TObjectIterator<USound> It; It; ++It )
		{
			if( I >= cap )
				appErrorf( TEXT("ALAudio: sound count grew since count.") );
			out[I++] = (size_t)*It;
		}
		if( I != cap )
			appErrorf( TEXT("ALAudio: sound count changed since count.") );
	} );
}

static uint8_t AlHostMusicSoundNew( void* ctx, const uint8_t* filename, size_t len, size_t* out )
{
	return Guarded( ctx, [&]()
	{
		FString F = FromUtf8( filename, len );
		USound* M = new USound( *F, SF_Streaming | SF_Music );
		M->FileType = FName( TEXT("ogg") );
		*out = (size_t)M;
	} );
}

// Ogg decoder (hp2_rs_vorbis_*). The subsystem owns the decoder it creates until
// GFileStream takes it.

static uint8_t AlHostVorbisNew( void* ctx, size_t* out )
{
	return Guarded( ctx, [&]()
	{
		*out = (size_t)hp2_rs_vorbis_new();
	} );
}

static uint8_t AlHostVorbisFree( void* ctx, size_t decoder )
{
	return Guarded( ctx, [&]()
	{
		hp2_rs_vorbis_free( (Hp2RsVorbisDecoder*)decoder );
	} );
}

static uint8_t AlHostVorbisInfo( void* ctx, size_t decoder, FfiVorbisInfo* out )
{
	return Guarded( ctx, [&]()
	{
		Hp2RsVorbisDecoder* D = (Hp2RsVorbisDecoder*)decoder;
		out->pcm_total = hp2_rs_vorbis_pcm_total( D );
		out->rate = (int32_t)hp2_rs_vorbis_rate( D );
		out->channels = (int32_t)hp2_rs_vorbis_channels( D );
		out->time_total = (double)hp2_rs_vorbis_time_total( D );
	} );
}

// GFileStream.

static uint8_t AlHostGfileCreateOggStream( void* ctx, size_t sound, int32_t chunk, int32_t num, uint8_t* prime, uint8_t looping, size_t decoder, int32_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		*out = GFileStream->CreateStream( *S->Filename, chunk, num, (void*)prime, looping ? ST_OggLooping : ST_Ogg, (void*)decoder );
	} );
}

static uint8_t AlHostGfileCreateXaStream( void* ctx, size_t sound, int32_t num_samples, int32_t chunk, int32_t num, uint8_t* prime, uint8_t looping, int32_t* out )
{
	return Guarded( ctx, [&]()
	{
		USound* S = (USound*)sound;
		*out = GFileStream->CreateStream( &S->Data, num_samples, chunk, num, (void*)prime, looping ? ST_XALooping : ST_XA, NULL );
	} );
}

static uint8_t AlHostGfileDestroyStream( void* ctx, int32_t id )
{
	return Guarded( ctx, [&]()
	{
		GFileStream->DestroyStream( id, false );
	} );
}

static uint8_t AlHostGfileRequestChunk( void* ctx, int32_t id, size_t dest, uint8_t* out )
{
	return Guarded( ctx, [&]()
	{
		*out = GFileStream->RequestChunk( id, (void*)dest ) ? 1 : 0;
	} );
}

static uint8_t AlHostGfilePopCompletedChunk( void* ctx, int32_t id, FfiPopCompleted* out )
{
	return Guarded( ctx, [&]()
	{
		void* D = NULL;
		UBOOL Term = 0;
		UBOOL Has = GFileStream->PopCompletedChunk( id, D, Term );
		out->has = Has ? 1 : 0;
		out->dest = (size_t)D;
		out->terminal = Term ? 1 : 0;
	} );
}

// Actors, levels, viewports.

static uint8_t AlHostActorInfo( void* ctx, size_t actor, FfiActorInfo* out )
{
	return Guarded( ctx, [&]()
	{
		AActor* A = (AActor*)actor;
		out->location = ToFfi( A->Location );
		out->velocity = ToFfi( A->Velocity );
		out->sound_radius = (FLOAT)A->SoundRadius;
		out->ambient_sound = (size_t)A->AmbientSound;
		out->index = A->GetIndex();
		out->level = (size_t)A->GetLevel();
		out->actor_level = (size_t)A->Level;
		out->sound_pitch = (uint8_t)A->SoundPitch;
		out->sound_volume = (uint8_t)A->SoundVolume;
		out->sound_occlusion = (uint8_t)A->SoundOcclusion;
		out->light_is_none = A->LightType == LT_None ? 1 : 0;
		out->light_brightness = (uint8_t)A->LightBrightness;
	} );
}

static uint8_t AlHostActorIsOwnedBy( void* ctx, size_t actor, size_t owner, uint8_t* out )
{
	return Guarded( ctx, [&]()
	{
		*out = ((AActor*)actor)->IsOwnedBy( (AActor*)owner ) ? 1 : 0;
	} );
}

static uint8_t AlHostLevelIsAudibleAt( void* ctx, size_t level, FfiVec3 sound_location, FfiVec3 listener_location, size_t actor, uint8_t occlusion, uint8_t* out )
{
	return Guarded( ctx, [&]()
	{
		*out = ((ULevel*)level)->IsAudibleAt( ToFVector(sound_location), ToFVector(listener_location), (AActor*)actor, (ESoundOcclusion)occlusion ) ? 1 : 0;
	} );
}

static uint8_t AlHostLevelPointRegionZone( void* ctx, size_t level, size_t actor_level, FfiVec3 location, int32_t* out )
{
	return Guarded( ctx, [&]()
	{
		*out = (int32_t)((ULevel*)level)->Model->PointRegion( (ALevelInfo*)actor_level, ToFVector(location) ).ZoneNumber;
	} );
}

static uint8_t AlHostLevelRadiusMultiplier( void* ctx, size_t level, int32_t zone_a, int32_t zone_b, float* out )
{
	return Guarded( ctx, [&]()
	{
		*out = ((ULevel*)level)->CalculateRadiusMultiplier( zone_a, zone_b );
	} );
}

static uint8_t AlHostViewportInfo( void* ctx, size_t viewport, FfiViewportInfo* out )
{
	return Guarded( ctx, [&]()
	{
		UViewport* V = (UViewport*)viewport;
		AActor* VT = V->Actor->ViewTarget ? V->Actor->ViewTarget : V->Actor;
		out->realtime = V->IsRealtime() ? 1 : 0;
		out->paused = V->Actor->Level->Pauser != TEXT("") ? 1 : 0;
		out->view_target = (size_t)VT;
		out->view_target_level = (size_t)VT->GetLevel();
		out->view_target_actor_level = (size_t)VT->Level;
		out->viewport_actor_level = (size_t)V->Actor->Level;
		out->view_target_location = ToFfi( VT->Location );
	} );
}

static uint8_t AlHostLevelActorCount( void* ctx, size_t level, size_t* out )
{
	return Guarded( ctx, [&]()
	{
		*out = ((ULevel*)level)->Actors.Num();
	} );
}

static uint8_t AlHostLevelActorFill( void* ctx, size_t level, size_t* out, size_t cap )
{
	return Guarded( ctx, [&]()
	{
		ULevel* L = (ULevel*)level;
		if( L->Actors.Num() != (INT)cap )
			appErrorf( TEXT("ALAudio: level actor count changed since count.") );
		for( INT i = 0; i < L->Actors.Num(); i++ )
			out[i] = (size_t)L->Actors( i );
	} );
}

// Lip sync (UAnimChannel). Channel 0 means none.

static uint8_t AlHostLipsyncStart( void* ctx, size_t actor, size_t* out )
{
	return Guarded( ctx, [&]()
	{
		LoadObject<UClass>( AActor::StaticClass(), TEXT("AnimChannel"), NULL, 0, NULL );
		AActor* Ch = ((AActor*)actor)->CreateAnimChannel( AAnimChannel::StaticClass(), AT_Replace, TEXT("mouth_root"), false, true );
		if( Ch )
			Ch->PlayAnim( TEXT("mouth_anim"), false, 0.0 );
		*out = (size_t)Ch;
	} );
}

static uint8_t AlHostLipsyncSetFrame( void* ctx, size_t channel, size_t sound, float elapsed )
{
	return Guarded( ctx, [&]()
	{
		((AActor*)channel)->AnimFrame = ((USound*)sound)->LipSyncData.GetMouthPos( elapsed );
	} );
}

static uint8_t AlHostLipsyncRelease( void* ctx, size_t channel )
{
	return Guarded( ctx, [&]()
	{
		if( channel )
			((AActor*)channel)->PlayAnim( TEXT("mouth_rest"), false, 1.0, 1.0, 1.0 );
	} );
}

// OpenAL and ALC. These cannot throw, so they are not guarded.

static int32_t AlHostAlGetError( void* ctx )
{
	(void)ctx;
	return (int32_t)alGetError();
}

static uint32_t AlHostAlGenSource( void* ctx )
{
	(void)ctx;
	ALuint S = 0;
	alGenSources( 1, &S );
	return (uint32_t)S;
}

static void AlHostAlDeleteSource( void* ctx, uint32_t source )
{
	(void)ctx;
	ALuint S = source;
	alDeleteSources( 1, &S );
}

static void AlHostAlGenBuffers( void* ctx, uint32_t* out, size_t n )
{
	(void)ctx;
	alGenBuffers( (ALsizei)n, (ALuint*)out );
}

static void AlHostAlDeleteBuffers( void* ctx, const uint32_t* buffers, size_t n )
{
	(void)ctx;
	alDeleteBuffers( (ALsizei)n, (const ALuint*)buffers );
}

static void AlHostAlBufferData( void* ctx, uint32_t buffer, int32_t format, const uint8_t* data, size_t len, int32_t frequency )
{
	(void)ctx;
	alBufferData( (ALuint)buffer, (ALenum)format, (const ALvoid*)data, (ALsizei)len, (ALsizei)frequency );
}

static void AlHostAlBufferDataAt( void* ctx, uint32_t buffer, int32_t format, size_t data, int32_t size, int32_t frequency )
{
	(void)ctx;
	alBufferData( (ALuint)buffer, (ALenum)format, (const ALvoid*)data, (ALsizei)size, (ALsizei)frequency );
}

static int32_t AlHostAlBufferSize( void* ctx, uint32_t buffer )
{
	(void)ctx;
	ALint V = 0;
	alGetBufferi( (ALuint)buffer, AL_SIZE, &V );
	return (int32_t)V;
}

static void AlHostAlSourceQueueBuffers( void* ctx, uint32_t source, const uint32_t* buffers, size_t n )
{
	(void)ctx;
	alSourceQueueBuffers( (ALuint)source, (ALsizei)n, (const ALuint*)buffers );
}

static uint32_t AlHostAlSourceUnqueueBuffer( void* ctx, uint32_t source )
{
	(void)ctx;
	ALuint B = 0;
	alSourceUnqueueBuffers( (ALuint)source, 1, &B );
	return (uint32_t)B;
}

static int32_t AlHostAlGetSourceI( void* ctx, uint32_t source, int32_t param )
{
	(void)ctx;
	ALint V = 0;
	alGetSourcei( (ALuint)source, (ALenum)param, &V );
	return (int32_t)V;
}

static void AlHostAlSourceI( void* ctx, uint32_t source, int32_t param, int32_t value )
{
	(void)ctx;
	alSourcei( (ALuint)source, (ALenum)param, (ALint)value );
}

static void AlHostAlSourceF( void* ctx, uint32_t source, int32_t param, float value )
{
	(void)ctx;
	alSourcef( (ALuint)source, (ALenum)param, (ALfloat)value );
}

static void AlHostAlSourceFv( void* ctx, uint32_t source, int32_t param, FfiVec3 value )
{
	(void)ctx;
	ALfloat V[3] = { value.x, value.y, value.z };
	alSourcefv( (ALuint)source, (ALenum)param, V );
}

static void AlHostAlSourcePlay( void* ctx, uint32_t source )
{
	(void)ctx;
	alSourcePlay( (ALuint)source );
}

static void AlHostAlSourcePause( void* ctx, uint32_t source )
{
	(void)ctx;
	alSourcePause( (ALuint)source );
}

static void AlHostAlSourceStop( void* ctx, uint32_t source )
{
	(void)ctx;
	alSourceStop( (ALuint)source );
}

static void AlHostAlSourceRewind( void* ctx, uint32_t source )
{
	(void)ctx;
	alSourceRewind( (ALuint)source );
}

static void AlHostAlListenerFv( void* ctx, int32_t param, const float* value )
{
	(void)ctx;
	alListenerfv( (ALenum)param, (const ALfloat*)value );
}

static void AlHostAlDistanceModel( void* ctx, int32_t model )
{
	(void)ctx;
	alDistanceModel( (ALenum)model );
}

static void AlHostAlDopplerFactor( void* ctx, float factor )
{
	(void)ctx;
	alDopplerFactor( (ALfloat)factor );
}

static size_t AlHostAlcOpenDevice( void* ctx )
{
	(void)ctx;
	return (size_t)alcOpenDevice( NULL );
}

static size_t AlHostAlcCreateContext( void* ctx, size_t device, const int32_t* attributes )
{
	(void)ctx;
	return (size_t)alcCreateContext( (ALCdevice*)device, (const ALCint*)attributes );
}

static void AlHostAlcMakeContextCurrent( void* ctx, size_t context )
{
	(void)ctx;
	alcMakeContextCurrent( (ALCcontext*)context );
}

static void AlHostAlcDestroyContext( void* ctx, size_t context )
{
	(void)ctx;
	alcDestroyContext( (ALCcontext*)context );
}

static void AlHostAlcCloseDevice( void* ctx, size_t device )
{
	(void)ctx;
	alcCloseDevice( (ALCdevice*)device );
}

}	// extern "C"

namespace
{

FfiHost MakeHost( FHostCall& Call )
{
	FfiHost H;
	memset( &H, 0, sizeof(H) );
	H.ctx = &Call;
	H.log = AlHostLog;
	H.output = AlHostOutput;
	H.app_seconds = AlHostAppSeconds;
	H.app_error = AlHostAppError;
	H.sound_info = AlHostSoundInfo;
	H.sound_data_len = AlHostSoundDataLen;
	H.sound_name = AlHostSoundName;
	H.sound_path_name = AlHostSoundPathName;
	H.sound_filename = AlHostSoundFilename;
	H.sound_set_working_flags = AlHostSoundSetWorkingFlags;
	H.sound_set_handle = AlHostSoundSetHandle;
	H.sound_get_handle = AlHostSoundGetHandle;
	H.sound_set_duration = AlHostSoundSetDuration;
	H.sound_add_ref = AlHostSoundAddRef;
	H.sound_is_registered = AlHostSoundIsRegistered;
	H.sound_add_stream_handle = AlHostSoundAddStreamHandle;
	H.sound_delete_stream_handle = AlHostSoundDeleteStreamHandle;
	H.sound_bind_stream = AlHostSoundBindStream;
	H.sound_unbind_stream = AlHostSoundUnbindStream;
	H.sound_unbind_all_streams = AlHostSoundUnbindAllStreams;
	H.sound_delete_all_stream_handles = AlHostSoundDeleteAllStreamHandles;
	H.sound_playing_stream_count = AlHostSoundPlayingStreamCount;
	H.sound_playing_stream_fill = AlHostSoundPlayingStreamFill;
	H.sound_load_data = AlHostSoundLoadData;
	H.sound_unload_data = AlHostSoundUnloadData;
	H.sound_wave_info = AlHostSoundWaveInfo;
	H.sound_count = AlHostSoundCount;
	H.sound_fill = AlHostSoundFill;
	H.music_sound_new = AlHostMusicSoundNew;
	H.vorbis_new = AlHostVorbisNew;
	H.vorbis_free = AlHostVorbisFree;
	H.vorbis_info = AlHostVorbisInfo;
	H.gfile_create_ogg_stream = AlHostGfileCreateOggStream;
	H.gfile_create_xa_stream = AlHostGfileCreateXaStream;
	H.gfile_destroy_stream = AlHostGfileDestroyStream;
	H.gfile_request_chunk = AlHostGfileRequestChunk;
	H.gfile_pop_completed_chunk = AlHostGfilePopCompletedChunk;
	H.actor_info = AlHostActorInfo;
	H.actor_is_owned_by = AlHostActorIsOwnedBy;
	H.level_is_audible_at = AlHostLevelIsAudibleAt;
	H.level_point_region_zone = AlHostLevelPointRegionZone;
	H.level_radius_multiplier = AlHostLevelRadiusMultiplier;
	H.viewport_info = AlHostViewportInfo;
	H.level_actor_count = AlHostLevelActorCount;
	H.level_actor_fill = AlHostLevelActorFill;
	H.lipsync_start = AlHostLipsyncStart;
	H.lipsync_set_frame = AlHostLipsyncSetFrame;
	H.lipsync_release = AlHostLipsyncRelease;
	H.al_get_error = AlHostAlGetError;
	H.al_gen_source = AlHostAlGenSource;
	H.al_delete_source = AlHostAlDeleteSource;
	H.al_gen_buffers = AlHostAlGenBuffers;
	H.al_delete_buffers = AlHostAlDeleteBuffers;
	H.al_buffer_data = AlHostAlBufferData;
	H.al_buffer_data_at = AlHostAlBufferDataAt;
	H.al_buffer_size = AlHostAlBufferSize;
	H.al_source_queue_buffers = AlHostAlSourceQueueBuffers;
	H.al_source_unqueue_buffer = AlHostAlSourceUnqueueBuffer;
	H.al_get_source_i = AlHostAlGetSourceI;
	H.al_source_i = AlHostAlSourceI;
	H.al_source_f = AlHostAlSourceF;
	H.al_source_fv = AlHostAlSourceFv;
	H.al_source_play = AlHostAlSourcePlay;
	H.al_source_pause = AlHostAlSourcePause;
	H.al_source_stop = AlHostAlSourceStop;
	H.al_source_rewind = AlHostAlSourceRewind;
	H.al_listener_fv = AlHostAlListenerFv;
	H.al_distance_model = AlHostAlDistanceModel;
	H.al_doppler_factor = AlHostAlDopplerFactor;
	H.alc_open_device = AlHostAlcOpenDevice;
	H.alc_create_context = AlHostAlcCreateContext;
	H.alc_make_context_current = AlHostAlcMakeContextCurrent;
	H.alc_destroy_context = AlHostAlcDestroyContext;
	H.alc_close_device = AlHostAlcCloseDevice;
	return H;
}

// Runs one Rust exec command. The Rust side writes its report to Ar.
void RunExec( FfiAudioSubsystem* Handle, FfiExecCommand Command, FOutputDevice& Ar )
{
	FHostCall Call;
	Call.Output = &Ar;
	FfiHost Host = MakeHost( Call );
	Finish( Call, hp2_rs_audio_exec( Handle, &Host, (uint32_t)Command ) );
}

}	// namespace

/*------------------------------------------------------------------------------------
	UALAudioSubsystem.
------------------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UALAudioSubsystem);

//
// UALAudioSubsystem::UALAudioSubsystem
//
UALAudioSubsystem::UALAudioSubsystem()
{
	guard(UALAudioSubsystem::UALAudioSubsystem);
	// The Rust subsystem starts its timers from the engine clock at construction.
	FTime Now = appSeconds();
	int64_t Raw = 0;
	memcpy( &Raw, &Now, sizeof(Raw) );
	Handle = hp2_rs_audio_subsystem_new( Raw );
	Viewport = NULL;
	unguard;
}

//
// Fills the Rust configuration mirror from the config properties.
//
void UALAudioSubsystem::MakeConfig( FfiAudioConfig& Cfg ) const
{
	Cfg.doppler_factor = DopplerFactor;
	Cfg.music_volume = MusicVolume;
	Cfg.sound_volume = SoundVolume;
	Cfg.roll_off = RollOff;
	Cfg.max_channels = MaxChannels;
	Cfg.max_radius_multiplier = GAudioMaxRadiusMultiplier;
	Cfg.use_precache = UsePrecache ? 1 : 0;
	Cfg.use_eax = UseEAX ? 1 : 0;
	Cfg.use_mmsystem = UseMMSYSTEM ? 1 : 0;
	Cfg.reverse_stereo = ReverseStereo ? 1 : 0;
}

//
// UALAudioSubsystem::StaticConstructor
//
void UALAudioSubsystem::StaticConstructor()
{
	guard(UALAudioSubsystem::StaticConstructor);
	new(GetClass(),TEXT("UseEAX"),				RF_Public)UBoolProperty  (CPP_PROPERTY(UseEAX          ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("CompatibilityMode"),	RF_Public)UBoolProperty  (CPP_PROPERTY(UseMMSYSTEM     ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("UsePrecache"),			RF_Public)UBoolProperty  (CPP_PROPERTY(UsePrecache     ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("ReverseStereo"),		RF_Public)UBoolProperty  (CPP_PROPERTY(ReverseStereo   ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("Channels"),			RF_Public)UIntProperty   (CPP_PROPERTY(MaxChannels     ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("MusicVolume"),			RF_Public)UFloatProperty (CPP_PROPERTY(MusicVolume     ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("SoundVolume"),			RF_Public)UFloatProperty (CPP_PROPERTY(SoundVolume     ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("DopplerFactor"),		RF_Public)UFloatProperty (CPP_PROPERTY(DopplerFactor   ), TEXT("ALAudio"), CPF_Config );
	new(GetClass(),TEXT("RollOff"),				RF_Public)UFloatProperty (CPP_PROPERTY(RollOff		   ), TEXT("ALAudio"), CPF_Config );
	unguard;
}


/*------------------------------------------------------------------------------------
	UObject Interface.
------------------------------------------------------------------------------------*/

//
// UALAudioSubsystem::PostEditChange
//
void UALAudioSubsystem::PostEditChange()
{
	guard(UALAudioSubsystem::PostEditChange);

	// Validate configurable variables.
	SoundVolume = Clamp(SoundVolume,0.f,1.f);
	MusicVolume = Clamp(MusicVolume,0.f,1.f);

	unguard;
}

//
// UALAudioSubsystem::Destroy
//
void UALAudioSubsystem::Destroy()
{
	guard(UALAudioSubsystem::Destroy);

	if( Handle )
	{
		FfiAudioConfig Cfg;
		MakeConfig( Cfg );
		FHostCall Call;
		FfiHost Host = MakeHost( Call );

		if( hp2_rs_audio_is_initialized( Handle ) )
			USound::Audio = NULL;
		int32_t Status = hp2_rs_audio_destroy( Handle, &Host, &Cfg );
		Viewport = (UViewport*)hp2_rs_audio_viewport( Handle );

		hp2_rs_audio_subsystem_free( Handle );
		Handle = NULL;
		Finish( Call, Status );
	}

	Super::Destroy();
	unguard;
}

//
// UALAudioSubsystem::ShutdownAfterError
//
void UALAudioSubsystem::ShutdownAfterError()
{
	guard(UALAudioSubsystem::ShutdownAfterError);

	if( Handle )
	{
		FHostCall Call;
		FfiHost Host = MakeHost( Call );
		if( hp2_rs_audio_is_initialized( Handle ) )
			USound::Audio = NULL;
		Finish( Call, hp2_rs_audio_shutdown_after_error( Handle, &Host ) );
	}

	Super::ShutdownAfterError();
	unguard;
}


/*------------------------------------------------------------------------------------
	UAudioSubsystem Interface.
------------------------------------------------------------------------------------*/

//
// UALAudioSubsystem::Init
//
UBOOL UALAudioSubsystem::Init()
{
	guard(UALAudioSubsystem::Init);

	FfiAudioConfig Cfg;
	MakeConfig( Cfg );
	FHostCall Call;
	FfiHost Host = MakeHost( Call );

	UBOOL WasInit = hp2_rs_audio_is_initialized( Handle ) != 0;
	uint8_t Ok = 0;
	int32_t Status = hp2_rs_audio_init( Handle, &Host, &Cfg, &Ok );
	RollOff = Cfg.roll_off;
	Finish( Call, Status );

	if( !WasInit && Ok )
		USound::Audio = this;
	return Ok ? 1 : 0;
	unguard;
}

//
// UALAudioSubsystem::SetViewport
//
void UALAudioSubsystem::SetViewport( UViewport* InViewport )
{
	guard(UALAudioSubsystem::SetViewport);

	FfiAudioConfig Cfg;
	MakeConfig( Cfg );
	FHostCall Call;
	FfiHost Host = MakeHost( Call );

	int32_t Status = hp2_rs_audio_set_viewport( Handle, &Host, &Cfg, (size_t)InViewport );
	Viewport = (UViewport*)hp2_rs_audio_viewport( Handle );
	Finish( Call, Status );
	unguard;
}

//
// UALAudioSubsystem::GetViewport
//
UViewport* UALAudioSubsystem::GetViewport()
{
	guard(UALAudioSubsystem::GetViewport);
	return Viewport;
	unguard;
}

//
// UALAudioSubsystem::RegisterSound
//
UBOOL UALAudioSubsystem::RegisterSound( USound* Sound )
{
	guard(UALAudioSubsystem::RegisterSound);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	uint8_t Out = 0;
	int32_t Status = hp2_rs_audio_register_sound( Handle, &Host, (size_t)Sound, &Out );
	Finish( Call, Status );
	return Out ? 1 : 0;
	unguard;
}

//
// UALAudioSubsystem::UnregisterSound
//
void UALAudioSubsystem::UnregisterSound( USound* Sound, INT Id )
{
	guard(UALAudioSubsystem::UnregisterSound);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	Finish( Call, hp2_rs_audio_unregister_sound( Handle, &Host, (size_t)Sound, Id ) );
	unguard;
}

//
// UALAudioSubsystem::PlaySound
//
INT UALAudioSubsystem::PlaySound( AActor* Actor, INT Id, USound* Sound, FVector Location, FLOAT Volume, FLOAT Radius, FLOAT Pitch, INT Flags, FLOAT FadeDuration, FLOAT Priority )
{
	guard(UALAudioSubsystem::PlaySound);

	FfiAudioConfig Cfg;
	MakeConfig( Cfg );
	FHostCall Call;
	FfiHost Host = MakeHost( Call );

	FfiPlayRequest Req;
	Req.actor = (size_t)Actor;
	Req.id = Id;
	Req.sound = (size_t)Sound;
	Req.location = ToFfi( Location );
	Req.volume = Volume;
	Req.radius = Radius;
	Req.pitch = Pitch;
	Req.flags = Flags;
	Req.fade_duration = FadeDuration;
	Req.in_priority = Priority;

	int32_t Out = 0;
	Finish( Call, hp2_rs_audio_play_sound( Handle, &Host, &Cfg, &Req, &Out ) );
	return Out;
	unguard;
}

//
// UALAudioSubsystem::StopSound (actor and sound)
//
UBOOL UALAudioSubsystem::StopSound( AActor* Actor, USound* Sound )
{
	guard(UALAudioSubsystem::StopSound);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	uint8_t Out = 0;
	Finish( Call, hp2_rs_audio_stop_sound_by_actor( Handle, &Host, (size_t)Actor, (size_t)Sound, &Out ) );
	return Out ? 1 : 0;
	unguard;
}

//
// UALAudioSubsystem::StopSound (actor, id and sound)
//
UBOOL UALAudioSubsystem::StopSound( AActor* Actor, INT Id, USound* Sound, FLOAT fFadeOutTime )
{
	guard(UALAudioSubsystem::StopSound);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	uint8_t Out = 0;
	Finish( Call, hp2_rs_audio_stop_sound_by_id( Handle, &Host, (size_t)Actor, Id, (size_t)Sound, fFadeOutTime, &Out ) );
	return Out ? 1 : 0;
	unguard;
}

//
// UALAudioSubsystem::ModifySound
//
UBOOL UALAudioSubsystem::ModifySound( AActor* Actor, INT Id, USound* Sound, BYTE parameter, FLOAT Value )
{
	guard(UALAudioSubsystem::ModifySound);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	uint8_t Out = 0;
	Finish( Call, hp2_rs_audio_modify_sound( Handle, &Host, (size_t)Actor, Id, (size_t)Sound, parameter, Value, &Out ) );
	return Out ? 1 : 0;
	unguard;
}

//
// UALAudioSubsystem::RenderAudioGeometry
//
void UALAudioSubsystem::RenderAudioGeometry( FSceneNode* Frame )
{
	guard(UALAudioSubsystem::RenderAudioGeometry);
	unguard;
}

//
// UALAudioSubsystem::NoteDestroy
//
void UALAudioSubsystem::NoteDestroy( AActor* Actor )
{
	guard(UALAudioSubsystem::NoteDestroy);
	check(Actor);
	check(Actor->IsValid());

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	Finish( Call, hp2_rs_audio_note_destroy( Handle, &Host, (size_t)Actor ) );
	unguard;
}

//
// UALAudioSubsystem::PlayMusic
//
INT UALAudioSubsystem::PlayMusic( FString Song, FLOAT FadeInTime )
{
	guard(UALAudioSubsystem::PlayMusic);

	std::string U = ToUtf8( *Song );
	FfiAudioConfig Cfg;
	MakeConfig( Cfg );
	FHostCall Call;
	FfiHost Host = MakeHost( Call );

	int32_t Out = 0;
	Finish( Call, hp2_rs_audio_play_music( Handle, &Host, &Cfg, (const uint8_t*)U.data(), U.size(), FadeInTime, &Out ) );
	return Out;
	unguard;
}

//
// UALAudioSubsystem::StopMusic
//
UBOOL UALAudioSubsystem::StopMusic( INT SongHandle, FLOAT FadeOutTime )
{
	guard(UALAudioSubsystem::StopMusic);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	uint8_t Out = 0;
	Finish( Call, hp2_rs_audio_stop_music( Handle, &Host, SongHandle, FadeOutTime, &Out ) );
	return Out ? 1 : 0;
	unguard;
}

//
// UALAudioSubsystem::StopAllMusic
//
INT UALAudioSubsystem::StopAllMusic( FLOAT FadeOutTime )
{
	guard(UALAudioSubsystem::StopAllMusic);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	Finish( Call, hp2_rs_audio_stop_all_music( Handle, &Host, FadeOutTime ) );
	return 0;
	unguard;
}

//
// UALAudioSubsystem::PauseSounds
//
void UALAudioSubsystem::PauseSounds( void )
{
	guard(UALAudioSubsystem::PauseSounds);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	hp2_rs_audio_pause_sounds( Handle, &Host );
	Rethrow( Call );
	unguard;
}

//
// UALAudioSubsystem::UnpauseSounds
//
void UALAudioSubsystem::UnpauseSounds( void )
{
	guard(UALAudioSubsystem::UnpauseSounds);

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	hp2_rs_audio_unpause_sounds( Handle, &Host );
	Rethrow( Call );
	unguard;
}

//
// UALAudioSubsystem::Update
//
void UALAudioSubsystem::Update( FSceneNode* SceneNode )
{
	guard(UALAudioSubsystem::Update);
	if( !Viewport )
		return;

	FfiAudioConfig Cfg;
	MakeConfig( Cfg );

	FfiUpdateView View;
	View.view_location = ToFfi( SceneNode->Coords.Origin );
	View.x_axis = ToFfi( SceneNode->Coords.XAxis );
	View.y_axis = ToFfi( SceneNode->Coords.YAxis );
	View.z_axis = ToFfi( SceneNode->Coords.ZAxis );

	FHostCall Call;
	FfiHost Host = MakeHost( Call );
	Finish( Call, hp2_rs_audio_update( Handle, &Host, &Cfg, &View ) );
	unguard;
}

//
// UALAudioSubsystem::Exec
//
UBOOL UALAudioSubsystem::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UALAudioSubsystem::Exec);
	const TCHAR*	Str = Cmd;
	if( ParseCommand(&Str,TEXT("ASTAT")) )
	{
		if( ParseCommand(&Str,TEXT("AUDIO")) )
		{
			return 1;
		}
	}
	else if( ParseCommand(&Str,TEXT("PAUSESOUNDS")) )
	{
		PauseSounds();
		return 1;
	}
	else if( ParseCommand(&Str,TEXT("UNPAUSESOUNDS")) )
	{
		UnpauseSounds();
		return 1;
	}
	else if( ParseCommand(&Str,TEXT("RESTARTSOUNDS")) )
	{
		RunExec( Handle, FfiExecCommand::RestartSounds, Ar );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("WEAPONRADIUS")) )
	{
		GAudioDefaultRadius = appAtof(Cmd);
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("ROLLOFF")) )
	{
		RollOff = appAtof(Cmd);
		StopSound( NULL, NULL );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("PLAYINGSOUNDSTATS")) )
	{
		RunExec( Handle, FfiExecCommand::PlayingSoundStats, Ar );
		return 1;
	}
	return 0;
	unguard;
}

//
// UALAudioSubsystem::GetFidelityActiveSources
//
void UALAudioSubsystem::GetFidelityActiveSources( TArray<FALAudioFidelitySource>& Out ) const
{
	guard(UALAudioSubsystem::GetFidelityActiveSources);

	Out.Empty();
	size_t Count = hp2_rs_audio_fidelity_sources( Handle, NULL, 0 );
	std::vector<FfiFidelitySource> Buf( Count );
	size_t Filled = hp2_rs_audio_fidelity_sources( Handle, Buf.data(), Buf.size() );
	if( Filled != Count )
		appErrorf( TEXT("ALAudio: fidelity source count changed since count.") );

	for( size_t I = 0; I < Count; I++ )
	{
		FALAudioFidelitySource Snapshot;
		Snapshot.Id = ((USound*)Buf[I].sound)->GetPathName();
		Snapshot.Paused = Buf[I].paused ? 1 : 0;
		Out.AddItem( Snapshot );
	}
	unguard;
}

//
// UALAudioSubsystem::GetFidelityActiveSources end
//

