/*=============================================================================
	ALAudioSubsystem.h: Unreal OpenAL Audio interface object.
	Copyright 1999-2001 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel.
=============================================================================*/

#ifndef _INC_ALAUDIOSUBSYSTEM
#define _INC_ALAUDIOSUBSYSTEM

/*------------------------------------------------------------------------------------
	Dependencies.
------------------------------------------------------------------------------------*/

#include <AL/al.h>
#include <AL/alc.h>

typedef ALenum (*EAXSet)(const void*, ALuint, ALuint, ALvoid*, ALuint);
typedef ALenum (*EAXGet)(const void*, ALuint, ALuint, ALvoid*, ALuint);

/*------------------------------------------------------------------------------------
	Helpers
------------------------------------------------------------------------------------*/

// Constants.

// Maximum number of audio channels.
#define MAX_AUDIOCHANNELS 32

// Maximum number of audio streams.
#define MAX_AUDIOSTREAMS 32

// Maximum listener velocity in meters/ second.
#define MAX_LISTENER_VELOCITY 50

// Maximum source velocity in meters/ seconds.
#define MAX_SOURCE_VELOCITY 300

// Amount of OpenAL buffers per stream.
//#define MAX_BUFFERS_PER_STREAM 4
#define MAX_BUFFERS_PER_STREAM 3

// Size in bytes per buffer in stream.
#define OGGVORBIS_STREAM_CHUNKSIZE	65536
#define XA_STREAM_CHUNKSIZE			32768

// Occlusion factor.
#define OCCLUSION_FACTOR 0.35f

// Priority modifier for playing sounds.
#define PLAYING_PRIORITY_MULTIPLIER 1.0f

// Conversion factor for Unreal units -> meter
#define DISTANCE_FACTOR ( 0.0254f / 2.f )

//#define DISTANCE_FACTOR (( 2.f / 254.f ) / ( 0.0254f / 2.f ))

// Utility Macros.

/*------------------------------------------------------------------------------------
	Lip-sync animation manager class.
------------------------------------------------------------------------------------*/
class FLipSyncAnimMgr
{
public:
	FLipSyncAnimMgr( AActor *ParentActor, FLipSyncData *LipSyncData );
	~FLipSyncAnimMgr( );

	UBOOL	Start( FLOAT fTime );
	void	Update( FLOAT fTime );
	void	EatPauseTime( FLOAT fDeltaTime );

protected:
	AActor*			m_ParentActor;
	FLipSyncData	*m_pLipSyncData;
	FLOAT			m_StartTime;
	FLOAT			m_LastElapsedTime;
	BYTE			m_LastAmp;
	AActor*			m_LipSyncAnimChan;

	FLipSyncAnimMgr() {};	// don't allow default construction
};

/*------------------------------------------------------------------------------------
	UGenericAudioSubsystem.
------------------------------------------------------------------------------------*/

struct ALSource
{
	USound			*Sound;
	ALuint			Source;
	AActor*			Actor;
	FVector			Location;
	FLOAT			Priority;
	FLOAT			Radius;
	FLOAT			ZoneRadius;
	FLOAT			UsedRadius;
	FLOAT			WantedRadius;
	FTime			LastChange;
	FLOAT			Volume;
	FLOAT			FadeDuration;
	FLOAT			FadeTime;
	EFadeMode		FadeMode;
	INT				Flags;
	INT				Id;
	UBOOL			Started;
	UBOOL			Paused;
	FLipSyncAnimMgr	*LipSyncAnimMgr;
};

struct ALBuffer
{
	ALuint		Buffer;
	INT			Flags;
	FString		Name;
};

struct ALStream
{
	ALuint			Buffer[MAX_BUFFERS_PER_STREAM];
	INT				Id;
	INT				Flags;
	INT				Counter;
	INT				Processed;
	INT				Rate;
	UBOOL			Alive;
	UBOOL			Reset;
	ALuint			Format;
	void*			Data;
	INT				StreamChunkSize;
	INT				NumBuffers;
	FString			Name;
};

struct ALAmbient
{
	AActor*		Actor;
	FLOAT		Priority;
	DWORD		Flags;
	INT			Id;
};

//
// The Generic implementation of UAudioSubsystem.
//
class ALAUDIO_API UALAudioSubsystem : public UAudioSubsystem
{
	DECLARE_CLASS(UALAudioSubsystem,UAudioSubsystem,CLASS_Config,ALAudio)

	// Variables.
	UViewport*	Viewport;
	UViewport*	DummyViewport;
//	DOUBLE		LastTime;
	FTime		LastTime;
	FVector		LastPosition;
	UBOOL		Initialized;
	UBOOL		LastRealtime;
	UBOOL		LastPaused;
	UBOOL		RestartSounds;
	UI3DL2Listener*		OldListener;
	FPointRegion		RegionListener;

	// Channels.
	TArray<ALSource>	Sources;
	TArray<ALBuffer>	Buffers;
	TArray<ALStream>	Streams;

	// AL specific
	ALCdevice*	SoundDevice;
	ALCcontext* SoundContext;
	EAXGet		alEAXGet;
	EAXSet		alEAXSet;

	// Configuration.
	FLOAT		DopplerFactor,
				MusicVolume,
				SoundVolume,
				RollOff;
	INT			MaxChannels;
	UBOOL		ReverseStereo,
				UsePrecache,
				UseEAX,
				UseMMSYSTEM;


	// Stats.
	class FALAudioStats
	{
	public:

		INT		STATS_FirstEntry,
				STATS_PlaySoundCycles,
				STATS_UpdateCycles,
				STATS_OcclusionCycles,
				STATS_PlaySoundCalls,
				STATS_OccludedSounds,
				STATS_ActiveStreamingSounds,
				STATS_ActiveRegularSounds,
				STATS_StoppedSounds,
				STATS_LastEntry;
		FALAudioStats();
		void Init();
	} ALAudioStats;

	// Constructor.
	UALAudioSubsystem();
	void StaticConstructor();

	// UObject interface.
	void Destroy();
	void PostEditChange();
	void ShutdownAfterError();

	// UAudioSubsystem interface.
	UBOOL Init();
	void SetViewport( UViewport* Viewport );
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar=*GLog );
	void Update( FSceneNode* SceneNode );
	UBOOL RegisterSound( USound* Sound );
	void UnregisterSound( USound* Sound, INT Id = -1 );
	INT PlaySound( AActor* Actor, INT Id, USound* Sound, FVector Location, FLOAT Volume, FLOAT Radius, FLOAT Pitch, INT Flags, FLOAT FadeDuration, FLOAT Priority = 0.f );
	UBOOL StopSound( AActor* Actor, USound* Sound );
	
	INT PlayMusic( FString Song, FLOAT FadeInTime );
	UBOOL StopMusic( INT SongHandle, FLOAT FadeOutTime );
	INT StopAllMusic( FLOAT FadeOutTime );
	void PauseSounds( void );
	void UnpauseSounds( void );
	void NoteDestroy( AActor* Actor );
	UViewport* GetViewport();

	// Internal functions.
	void SetVolumes();
	void StopSoundSource( INT Index );
	void OldStopSound( INT Index );
	FLOAT SoundPriority( UViewport* Viewport, FVector Location, FLOAT Volume, FLOAT Radius, INT Flags );
	INT GetNewStreamStruct();
	void SetI3DL2Listener( UI3DL2Listener* Listener );
	UBOOL alError( TCHAR* Text, UBOOL Log = true );
	INT FindLeastImportantSound( USound *pIncomingSound );

	//	Added by TG as part of initial test port
	//
	// TG ALPHA
	void PostRender( FSceneNode* Frame ){};
	UBOOL ModifySound( AActor* Actor, INT Id, USound * Sound, BYTE parameter, FLOAT Value );
	UBOOL StopSound  ( AActor* Actor, INT Id, USound * Sound, FLOAT fFadeOutTime = 0.f );
	void RenderAudioGeometry( FSceneNode* Frame );
};

#define AUTO_INITIALIZE_REGISTRANTS_ALAUDIO	\
	UALAudioSubsystem::StaticClass();

#endif