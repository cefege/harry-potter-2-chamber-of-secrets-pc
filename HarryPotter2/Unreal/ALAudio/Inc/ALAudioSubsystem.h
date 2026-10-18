/*=============================================================================
	ALAudioSubsystem.h: Unreal OpenAL Audio interface object.
	Copyright 1999-2001 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel.
=============================================================================*/

#ifndef _INC_ALAUDIOSUBSYSTEM
#define _INC_ALAUDIOSUBSYSTEM

/*------------------------------------------------------------------------------------
	Ownership split.

	All audio logic (channel and source bookkeeping, stream feeding, listener and
	attenuation math, playback decisions, music, lip-sync timing, stats) lives in
	the Rust crate hp2-audio and is reached through the hp2_rs_audio_* exports in
	crates/hp2-ffi/src/audio.rs. The Rust subsystem is held in Handle.

	What remains in C++ and why:
	- This UObject shell: UObject lifetime, config properties (their ini keys are
	  part of the engine's config surface), and the engine virtual overrides that
	  the engine calls by name.
	- Thin trampolines in ALAudioSubsystem.cpp that give Rust access to engine-global
	  services (USound, AActor, ULevel, GFileStream, the OpenAL and ALC entry points).
	  Engine globals cannot move to Rust, so Rust reaches them only through the
	  FfiHost table that the shell fills.
	- USound::Audio, the engine hook that routes USound calls to this subsystem.
------------------------------------------------------------------------------------*/

/*------------------------------------------------------------------------------------
	Helpers
------------------------------------------------------------------------------------*/

// Opaque Rust subsystem (crates/hp2-ffi/src/audio.rs).
struct FfiAudioSubsystem;
struct FfiAudioConfig;

// Immutable audio-source state exposed for fidelity telemetry. The subsystem
// retains ownership of all source and sound objects.
struct FALAudioFidelitySource
{
	FString	Id;
	UBOOL	Paused;
};

//
// The OpenAL implementation of UAudioSubsystem. Logic is in Rust; see the header
// comment above.
//
class ALAUDIO_API UALAudioSubsystem : public UAudioSubsystem
{
	DECLARE_CLASS(UALAudioSubsystem,UAudioSubsystem,CLASS_Config,ALAudio)

	// Viewport mirror. Rust owns the viewport reference and reports it back.
	UViewport*	Viewport;

	// Rust subsystem handle. Created in the constructor, freed in Destroy.
	FfiAudioSubsystem*	Handle;

	// Configuration (ini keys unchanged).
	FLOAT		DopplerFactor,
				MusicVolume,
				SoundVolume,
				RollOff;
	INT			MaxChannels;
	UBOOL		ReverseStereo,
				UsePrecache,
				UseEAX,
				UseMMSYSTEM;

	// Constructor.
	UALAudioSubsystem();
	void StaticConstructor();
	void MakeConfig( FfiAudioConfig& Cfg ) const;

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
	void GetFidelityActiveSources( TArray<FALAudioFidelitySource>& Out ) const;

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
