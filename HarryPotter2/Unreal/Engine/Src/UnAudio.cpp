/*=============================================================================
	UnAudio.cpp: Unreal base audio.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney
	* Wave modification code by Erik de Neve
=============================================================================*/

#include "EnginePrivate.h" 
#include "Hp2AudioServices.h"

/*-----------------------------------------------------------------------------
	USound implementation.

	The logic lives in crates/hp2-engine/src/audio.rs. This file keeps the
	UObject shell (IMPLEMENT_CLASS, Destroy, PostLoad) and forwards each method
	to its Rust export, which reaches engine state through GHp2AudioServices.
-----------------------------------------------------------------------------*/

#pragma DISABLE_OPTIMIZATION
void FSoundData::Load()
{
	guard(FSoundData::Load);
	hp2_rs_engine_audio_fsound_load( this, &GHp2AudioServices );
	unguard;
}

FLOAT FSoundData::GetPeriod()
{
	return hp2_rs_engine_audio_fsound_get_period( this, &GHp2AudioServices );
}


void USound::Serialize( FArchive& Ar )
{
	guard(USound::Serialize);
	hp2_rs_engine_audio_usound_serialize( this, &Ar, &GHp2AudioServices );
	unguard;
}

void USound::Destroy()
{
	guard(USound::Destroy);
	if( Audio )
		Audio->UnregisterSound( this );
	Super::Destroy();
	unguard;
}

void USound::PostLoad()
{
	guard(USound::PostLoad);
	Super::PostLoad();
	unguard;
};


void USound::BindStream( INT Id )
{
	hp2_rs_engine_audio_usound_bind_stream( this, Id, &GHp2AudioServices );
}

void USound::UnbindStream( INT Id )
{
	hp2_rs_engine_audio_usound_unbind_stream( this, Id, &GHp2AudioServices );
}

void USound::UnbindAllStreams( void )
{
	hp2_rs_engine_audio_usound_unbind_all_streams( this, &GHp2AudioServices );
}

UBOOL USound::IsRegistered( void )
{
	return hp2_rs_engine_audio_usound_is_registered( this ) != 0;
}

void USound::SetHandle( INT iHandle )
{
	hp2_rs_engine_audio_usound_set_handle( this, iHandle );
}

void USound::AddStreamHandle( INT iStreamID )
{
	hp2_rs_engine_audio_usound_add_stream_handle( this, iStreamID, &GHp2AudioServices );
}

void USound::DeleteStreamHandle( INT iStreamID )
{
	hp2_rs_engine_audio_usound_delete_stream_handle( this, iStreamID, &GHp2AudioServices );
}

void USound::DeleteAllStreamHandles( void )
{
	hp2_rs_engine_audio_usound_delete_all_stream_handles( this, &GHp2AudioServices );
}

INT USound::GetHandle( INT Id )
{
	return hp2_rs_engine_audio_usound_get_handle( this, Id, &GHp2AudioServices );
}


//	Allows for post processing after import
//-----------------------------------------------------------------------------
void USound::ImportPostProcess( const TCHAR* Filename, const TCHAR* Parms )
{
	hp2_rs_engine_audio_usound_import_post_process( this, Filename, Parms, &GHp2AudioServices );
}

UBOOL USound::CalcLipSyncData( void )
{
	return hp2_rs_engine_audio_usound_calc_lip_sync( this, &GHp2AudioServices ) != 0;
}

UBOOL USound::CompressXA( const TCHAR* SourceFilename )
{
	return hp2_rs_engine_audio_usound_compress_xa( this, SourceFilename, &GHp2AudioServices ) != 0;
}


UAudioSubsystem* USound::Audio;

// The PS2 VAG conversion body is compiled out (HAVE_VAG 0), so this is empty.
void USound::PS2Convert()
{
	guard(USound::PS2Convert);
	unguard;
};
IMPLEMENT_CLASS(USound);
IMPLEMENT_CLASS(UMusic);

/*-----------------------------------------------------------------------------
	FLipSyncData implementation.
-----------------------------------------------------------------------------*/

FLipSyncData::FLipSyncData( USound* InOwner ) : 
	Owner(InOwner)
{
	m_iMaxAmplitude = -1;
}


void FLipSyncData::Load()
{
	guard(FLipSyncData::Load);
	guard(0);
	TLazyArray<BYTE>::Load();
	unguard;
	unguard;
}


FLOAT FLipSyncData::GetMouthPos( FLOAT fTime )
{
	return hp2_rs_engine_audio_lip_get_mouth_pos( this, fTime );
}


/*-----------------------------------------------------------------------------
	WaveModInfo implementation: ReadWaveInfo forwards to the Rust port.
-----------------------------------------------------------------------------*/

UBOOL FWaveModInfo::ReadWaveInfo( TArray<BYTE>& WavData )
{
	return hp2_rs_engine_audio_wave_read_info( this, &WavData ) != 0;
}


IMPLEMENT_CLASS(UAudioSubsystem);

/*-----------------------------------------------------------------------------
	UI3DL2Listener implementation.
-----------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UI3DL2Listener);
void UI3DL2Listener::PostEditChange()
{
	Super::PostEditChange();
	Updated = true;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
