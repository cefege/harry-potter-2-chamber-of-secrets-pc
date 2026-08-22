/*=======================================s======================================
	ALAudioSubsystem.cpp: Unreal OpenAL Audio interface object.
	Copyright 1999-2001 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Daniel Vogel.
=============================================================================*/

/*------------------------------------------------------------------------------------
	Audio includes.
------------------------------------------------------------------------------------*/
#include <math.h>
#include <stdlib.h>
#include "ALAudioPrivate.h"
#if SUPPORTS_PRAGMA_PACK
#pragma pack (push,8)
#endif
#include "vorbis/codec.h"
#include "vorbis/vorbisfile.h"
#if SUPPORTS_PRAGMA_PACK
#pragma pack (pop)
#endif

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
	LastTime	 = appSeconds();
	LastPosition = FVector(0,0,0);
	Initialized = 0;
	RestartSounds = false;
	unguard;
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

	SetVolumes();

	unguard;
}

//
// UALAudioSubsystem::Destroy
//
void UALAudioSubsystem::Destroy()
{
	guard(UALAudioSubsystem::Destroy);
	if(Initialized)
	{
		INT i;

		// Unhook.
		USound::Audio = NULL;

		for (i=0; i<Sources.Num(); i++)
		{
			StopSoundSource(i);
			alDeleteSources( 1, & Sources(i).Source );
		}
		Sources.Empty();

		if (Viewport)
		{
			for(TObjectIterator<USound> SoundIt;SoundIt;++SoundIt)
				UnregisterSound(*SoundIt);
		}

		for (i=0; i<Buffers.Num(); i++ )
			if (Buffers(i).Buffer)
				alDeleteBuffers( 1, &Buffers(i).Buffer );

		Buffers.Empty();

		for (i=0; i<Streams.Num(); i++ )
		{
			if (Streams(i).Id)
				GFileStream->DestroyStream(Streams(i).Id - 1, false);
			if (Streams(i).NumBuffers)
				alDeleteBuffers( Streams(i).NumBuffers, &Streams(i).Buffer[0] );
			for (INT BufferIndex = 0; BufferIndex < MAX_BUFFERS_PER_STREAM; ++BufferIndex)
				delete [] Streams(i).RefillData[BufferIndex];
		}
		Streams.Empty();

		alcMakeContextCurrent( NULL );
		alcDestroyContext( SoundContext );
		alcCloseDevice( SoundDevice );

		SetViewport(NULL);
		
		// Unhook.
		USound::Audio = NULL; 
		debugf(NAME_Exit,TEXT("OpenAL Audio subsystem shut down."));
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

	if(Initialized)
	{
		// Unhook.
		USound::Audio = NULL;
		debugf(NAME_Exit,TEXT("UALAudioSubsystem::ShutdownAfterError"));
	}

	Super::ShutdownAfterError();

	unguard;
}

//
// UALAudioSubsystem::Init
//
UBOOL UALAudioSubsystem::Init()
{
	guard(UALAudioSubsystem::Init);

	if (Initialized)
		return 1;

	SoundDevice = alcOpenDevice( nullptr );
	if (!SoundDevice)
	{
		debugf(NAME_Init,TEXT("ALAudio: no OpenAL devices found."));
		return 0;
	}

	INT Caps[] = { ALC_FREQUENCY, 44100, 0 };
	SoundContext = alcCreateContext( SoundDevice, Caps );
	if (!SoundContext)
	{
		debugf(NAME_Init, TEXT("ALAudio: context creation failed."));
		return 0;
	}

	alcMakeContextCurrent( SoundContext );
	//alcProcessContext( SoundContext );
	
		if ( alError(TEXT("Init")) )
	{
		debugf(NAME_Init,TEXT("ALAudio: makecurrent failed."));
		return 0;
	}

	if ( !MaxChannels )
	{
		debugf( NAME_Init, TEXT("MaxChannels == 0") );
		return 0;
	}

	// Initialize channels.
	Sources.Empty();
	for (INT i=0; i<Min(MaxChannels, MAX_AUDIOCHANNELS); i++)
	{
		ALuint sid;
		alGenSources( 1, &sid );
		if ( !alError(TEXT("Init (creating sources)"),false) )
		{
			Sources.AddZeroed(1);
			Sources(i).Source = sid;
		}
		else
			break;
	}
	if (!Sources.Num())
	{
		debugf(NAME_Init,TEXT("ALAudio: couldn't allocate sources"));
		return 0;
	}

	// Initialize streams.
	Streams.Empty( MAX_AUDIOSTREAMS );
	Streams.AddZeroed( MAX_AUDIOSTREAMS );

	// Adjust global rolloff factor.
	if( RollOff <= 0.0f )
		RollOff = 1.0f;
	
	// Use DS3D distance model.
	alDistanceModel( AL_INVERSE_DISTANCE_CLAMPED );
	alDopplerFactor( DopplerFactor );

	// Check for EAX support.
	OldListener = NULL;

	// disable this code until the next code drop from Epic.  Current drop is 927.  -tg
	//
#if 1
	alEAXGet = NULL;
	alEAXSet = NULL;
#else
	// TG ALPHA
	if( 0 && UseEAX && alIsExtensionPresent((ALubyte*)"EAX2.0") == AL_TRUE )
	{
		alEAXSet	= (EAXSet) alGetProcAddress((ALubyte*)"EAXSet");
		alEAXGet	= (EAXGet) alGetProcAddress((ALubyte*)"EAXGet");

		if( alEAXSet && alEAXGet )
		{
			// Set 'plain' EAX preset.
			ALuint uEnvironment = EAX_ENVIRONMENT_PLAIN;
			alEAXSet(&DSPROPSETID_EAX20_ListenerProperties,DSPROPERTY_EAXLISTENER_ENVIRONMENT,NULL, &uEnvironment, sizeof(ALuint));
			debugf(NAME_Init,TEXT("ALAudio: using EAX"));
		}
	}
	else
	{
		alEAXGet = NULL;
		alEAXSet = NULL;
	}
#endif

	Initialized			= 1;
	LastRealtime		= false;	
	LastPaused			= false;	
	
	// Initialize stats.
	ALAudioStats.Init();

	// Initialized.
	USound::Audio = this;

	debugf(NAME_Init,TEXT("ALAudio: subsystem initialized."));
	return 1;

	unguard;
}


//
// UALAudioSubsystem::SetViewport
//
void UALAudioSubsystem::SetViewport(UViewport* InViewport)
{
	guard(UALAudioSubsystem::SetViewport);

	// Stop playing sounds.
	for(INT i=0; i<Sources.Num(); i++)
		if( Sources(i).Started && !(Sources(i).Flags & SF_Music) )
			StopSoundSource(i);

	if(Viewport != InViewport)
	{
		// Switch viewports.
		Viewport = InViewport;
		
		// Set listener region to 0.
		appMemzero( &RegionListener, sizeof(RegionListener) );

		if( Viewport && UsePrecache )
		{
			// Register everything.
			for(TObjectIterator<USound> SoundIt;SoundIt;++SoundIt)
				RegisterSound(*SoundIt);
		}
	}

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
UBOOL UALAudioSubsystem::RegisterSound(USound* Sound)
{ 
	guard(UALAudioSubsystem::RegisterSound);

	checkSlow(Sound);

//	if (Sound->CoreFlags & SF_Streaming)
//		debugf( NAME_DevSound, TEXT("RegisterSound(): %s  NumSamples = %d  time = %d"), Sound->GetName(), Sound->raw_NumSamples,  (int)(appSeconds().GetFloat() * 10000));

	if( !Sound->IsRegistered())
	{
		// Avoid recursion as USound->Load calls RegisterSound.
		//
		Sound->Handle = 0xDEADBEEF;
		INT	Flags = Sound->CoreFlags | Sound->WorkingFlags;

		if ( Flags & SF_Streaming )
		{
			INT		iStreamChunkSize;
			UBOOL	bIsOggVorbis = appStricmp(*Sound->FileType, TEXT("ogg")) == 0;
			
			//	We currently support Ogg Vorbis and XA streaming audio files.  Set up the stream
			//	for the given type here.
			//
			if (bIsOggVorbis)
			{
				iStreamChunkSize = OGGVORBIS_STREAM_CHUNKSIZE;

				BYTE* PrimeData = new BYTE[iStreamChunkSize * MAX_BUFFERS_PER_STREAM];
				if (!PrimeData)
					return false;

				BYTE* RefillData[MAX_BUFFERS_PER_STREAM] = { NULL };
				for (INT i = 0; i < MAX_BUFFERS_PER_STREAM; ++i)
				{
					RefillData[i] = new BYTE[iStreamChunkSize];
					if (!RefillData[i])
					{
						for (INT j = 0; j < i; ++j)
							delete [] RefillData[j];
						delete [] PrimeData;
						return false;
					}
				}

				OggVorbis_File* OggFile = new OggVorbis_File;
				if (!OggFile)
				{
					for (INT i = 0; i < MAX_BUFFERS_PER_STREAM; ++i)
						delete [] RefillData[i];
					delete [] PrimeData;
					return false;
				}

				EFileStreamType Type = (Flags & SF_Music) ? ST_OggLooping : ST_Ogg;
				INT Id = GFileStream->CreateStream( *Sound->Filename, iStreamChunkSize, MAX_BUFFERS_PER_STREAM, PrimeData, Type, OggFile );
				if ( Id < 0 )
				{
					debugf( NAME_DevSound, TEXT("Failed to register sound: %s"), *Sound->Filename );

					delete OggFile;
					for (INT i = 0; i < MAX_BUFFERS_PER_STREAM; ++i)
						delete [] RefillData[i];
					delete [] PrimeData;

					return false;
				}

				if ( ov_pcm_total(OggFile, -1) < iStreamChunkSize * MAX_BUFFERS_PER_STREAM )
					debugf( NAME_DevSound, TEXT("Sound is too short for streaming: %s"), *Sound->Filename );

				vorbis_info *VorbisInfo = ov_info(OggFile, -1);

				Sound->Duration			= ov_time_total(OggFile, -1);
				//Sound->Handle			= GetNewStreamStruct() + 1;

				int	iStreamIndex			= GetNewStreamStruct() + 1;
				if (iStreamIndex == 0)
				{
					debugf( NAME_DevSound, TEXT("Failed to register sound: %s"), *Sound->Filename );

					GFileStream->DestroyStream( Id, false );
					// A successful CreateStream transfers OggFile ownership to FFileStream.
					for (INT i = 0; i < MAX_BUFFERS_PER_STREAM; ++i)
						delete [] RefillData[i];
					delete [] PrimeData;

					return false;
				}
				Sound->AddStreamHandle(iStreamIndex);

				ALStream& Stream		= Streams(iStreamIndex - 1);
				Stream.Flags			= Flags;
				Stream.Id				= Id + 1;
				Stream.Processed		= 0;
				Stream.Alive			= 1;
				Stream.Reset			= 0;
				check( VorbisInfo->rate>=0 && VorbisInfo->rate<=MAXINT );
				Stream.Rate				= static_cast<INT>(VorbisInfo->rate);
				Stream.Name				= Sound->GetPathName();
				Stream.StreamChunkSize	= iStreamChunkSize;
				Stream.NumBuffers		= MAX_BUFFERS_PER_STREAM;
				for (INT i = 0; i < Stream.NumBuffers; ++i)
				{
					Stream.RefillData[i] = RefillData[i];
					Stream.BufferState[i] = QueuedNoPrefetch;
				}

				alGenBuffers( Stream.NumBuffers, &Stream.Buffer[0] );
				alError(TEXT("Creating streaming buffer."));

				switch ( VorbisInfo->channels )
				{
				case 1:
					Stream.Format = AL_FORMAT_MONO16;
					break;
				case 2:
					Stream.Format = AL_FORMAT_STEREO16;
					break;
				default:
					Stream.Format = 0;
					appErrorf(TEXT("Invalid sound data"));
					break;
				}

				for (INT i = 0; i < Stream.NumBuffers; i++)
					alBufferData( Stream.Buffer[i], Stream.Format, PrimeData + i * iStreamChunkSize, iStreamChunkSize, Stream.Rate );
				for (INT i = 0; i < Stream.NumBuffers; ++i)
				{
					if (GFileStream->RequestChunk(Stream.Id - 1, Stream.RefillData[i]))
						Stream.BufferState[i] = QueuedPending;
				}


				// error checking!!
				delete [] PrimeData;
			}
			else
			{
				int		iDecodedLength = Sound->raw_NumSamples * (Sound->raw_BitsPerSample / 8) * Sound->raw_NumChannels;
				int		iDecodedSizeInChunks = iDecodedLength == 0 ? 0 : ((iDecodedLength - 1) / XA_STREAM_CHUNKSIZE) + 1;
				int		iNumBuffers = (Flags & SF_Looping) ? MAX_BUFFERS_PER_STREAM : Min(iDecodedSizeInChunks, MAX_BUFFERS_PER_STREAM);
				UBOOL	bNeedsOngoingChunks = (Flags & SF_Looping) || (iDecodedSizeInChunks > MAX_BUFFERS_PER_STREAM);

				iStreamChunkSize = XA_STREAM_CHUNKSIZE;

				//	Allocate priming buffer
				//
				BYTE* PrimeData = new BYTE[iStreamChunkSize * iNumBuffers];
				if (!PrimeData)
					return false;

				BYTE* RefillData[MAX_BUFFERS_PER_STREAM] = { NULL };
				if (bNeedsOngoingChunks)
				{
					for (INT i = 0; i < iNumBuffers; ++i)
					{
						RefillData[i] = new BYTE[iStreamChunkSize];
						if (!RefillData[i])
						{
							for (INT j = 0; j < i; ++j)
								delete [] RefillData[j];
							delete [] PrimeData;
							return false;
						}
					}
				}

				EFileStreamType Type = (Flags & SF_Looping) ? ST_XALooping :  ST_XA;
				INT Id = GFileStream->CreateStream( &Sound->Data, Sound->raw_NumSamples, iStreamChunkSize, iNumBuffers, PrimeData, Type, NULL );
				if ( Id < 0 )
				{
					debugf( NAME_DevSound, TEXT("Failed to register sound: %s"), Sound->GetName() );

					for (INT i = 0; i < iNumBuffers; ++i)
						delete [] RefillData[i];
					delete [] PrimeData;

					return false;
				}

				//Sound->Duration		= // already have duration
				int	iStreamIndex = GetNewStreamStruct() + 1;
				if (iStreamIndex == 0)
				{
					debugf( NAME_DevSound, TEXT("Failed to register sound: %s"), Sound->GetName() );

					GFileStream->DestroyStream( Id, false );
					for (INT i = 0; i < iNumBuffers; ++i)
						delete [] RefillData[i];
					delete [] PrimeData;

					return false;
				}
				Sound->AddStreamHandle(iStreamIndex);

				ALStream& Stream		= Streams(iStreamIndex - 1);
				Stream.Flags			= Flags;
				Stream.Id				= Id + 1;
				Stream.Processed		= 0;
				Stream.Alive			= bNeedsOngoingChunks;
				Stream.Reset			= 0;
				Stream.Rate				= Sound->raw_SampleRate;
				Stream.Name				= Sound->GetPathName();
				Stream.StreamChunkSize	= iStreamChunkSize;
				Stream.NumBuffers		= iNumBuffers;
				for (INT i = 0; i < Stream.NumBuffers; ++i)
				{
					Stream.RefillData[i] = RefillData[i];
					Stream.BufferState[i] = QueuedNoPrefetch;
				}

				//	debugf( NAME_DevSound, TEXT("RegisterSound(): %s  CreateStream() returned %d,  GetNewStreamStruct() + 1 = %d "), Sound->GetName(), Id, iStreamIndex);

				alGenBuffers( Stream.NumBuffers, &Stream.Buffer[0] );
				alError(TEXT("Creating streaming buffer."));

				if ( Sound->raw_BitsPerSample == 8 )
				{
					debugf( NAME_DevSound, TEXT("WARNING: 8 bit sound detected [%s]"), Sound->GetPathName() );
					if (Sound->raw_NumChannels == 1)
						Stream.Format = AL_FORMAT_MONO8;
					else
						Stream.Format = AL_FORMAT_STEREO8;
				}
				else
				{
					if (Sound->raw_NumChannels == 1)
						Stream.Format = AL_FORMAT_MONO16;
					else
						Stream.Format = AL_FORMAT_STEREO16;
				}

				for (INT i = 0; i < Stream.NumBuffers; i++)
					alBufferData( Stream.Buffer[i], Stream.Format, PrimeData + i * iStreamChunkSize, iStreamChunkSize, Stream.Rate );
				for (INT i = 0; i < Stream.NumBuffers; ++i)
				{
					if (Stream.RefillData[i] && GFileStream->RequestChunk(Stream.Id - 1, Stream.RefillData[i]))
						Stream.BufferState[i] = QueuedPending;
				}


				//	error checking!!
				//
				delete [] PrimeData;
			}
		}
		else
		{
			// Load the data.
			Sound->Data.Load();
			check(Sound->Data.Num()>0);
	
			FWaveModInfo WaveInfo;
			WaveInfo.ReadWaveInfo(Sound->Data);

			INT Flags = WaveInfo.SampleLoopsNum ? SF_Looping : 0;

			ALint Format;
			if ( *WaveInfo.pBitsPerSample == 8 )
			{
				debugf( NAME_DevSound, TEXT("WARNING: 8 bit sound detected [%s]"), Sound->GetPathName() );
				if (*WaveInfo.pChannels == 1)
					Format = AL_FORMAT_MONO8;
				else
					Format = AL_FORMAT_STEREO8;
			}
			else
			{
				if (*WaveInfo.pChannels == 1)
					Format = AL_FORMAT_MONO16;
				else
					Format = AL_FORMAT_STEREO16;				
			}

			ALuint bid;
			alGenBuffers( 1, &bid );
			alError(TEXT("RegisterSound (generating buffer)"));
		
			alBufferData( bid, Format, WaveInfo.SampleDataStart, WaveInfo.SampleDataSize, *WaveInfo.pSamplesPerSec );
	
			// Unload the data.
			Sound->Data.Unload();

			if (alError(TEXT("RegisterSound (creating buffer)")))
				Sound->Handle = 0;
			else
			{
				Sound->SetHandle(Buffers.AddZeroed( 1 ) + 1);
				ALBuffer& Buffer = Buffers(Sound->GetHandle() - 1);
				Buffer.Buffer = bid;
				Buffer.Flags  = Flags;
				Buffer.Name	= Sound->GetPathName();
			}
		}
	}

	//	Need to keep track of how many times a sound object is SUCCESSFULLY registered.
	//
	Sound->RefCount++;

	return true;

	unguard;
}

//
// UALAudioSubsystem::UnregisterSound
//
void UALAudioSubsystem::UnregisterSound(USound* Sound, INT Id)
{
	guard(UALAudioSubsystem::UnregisterSound);

//	if (Sound->CoreFlags & SF_Streaming)
//		debugf( NAME_DevSound, TEXT("UnregisterSound(): %s"), Sound->GetName() );

	//	Stop the sound source
	//
	for( INT i = 0; i < Sources.Num(); i++ )
	{
		if (Id == -1)
		{
			if( Sources(i).Sound && Sources(i).Sound == Sound )
				OldStopSound(i);
		}
		else
		{
			if( Sources(i).Sound && Sources(i).Id == Id )
				OldStopSound(i);
		}
	}

	//	Sources(i) has been wiped clean by call to OldStopSound().  DON'T USE
	//	ANY OF THE Sources() FIELDS AFTER THIS POINT.
	//
	alError(TEXT(""),false);

	//	Need to clean up if this is the last playing instance of a sound object
	//
	Sound->RefCount--;

	if (Sound->GetHandle(Id))
	{
		if ( Sound->WorkingFlags & SF_Streaming )
		{
			if (Id == -1)
			{
				for( TMap<INT, INT>::TIterator It(Sound->PlayingStreams); It; ++It )
				{					
					INT	SoundHandle = Sound->OpenStreams(It.Value()) / 2;
					INT i = SoundHandle - 1;

					GFileStream->DestroyStream( Streams(i).Id - 1, false );

					alDeleteBuffers( Streams(i).NumBuffers, &Streams(i).Buffer[0] );
					appMemzero( Streams(i).Buffer, sizeof(Streams(i).Buffer) );
					for (INT BufferIndex = 0; BufferIndex < MAX_BUFFERS_PER_STREAM; ++BufferIndex)
						delete [] Streams(i).RefillData[BufferIndex];
					appMemzero( &Streams(i), sizeof(ALStream) );

				}

				Sound->UnbindAllStreams();
				Sound->DeleteAllStreamHandles();
			}
			else
			{
				INT	SoundHandle = Sound->GetHandle(Id);
				INT i = SoundHandle - 1;
	
				GFileStream->DestroyStream( Streams(i).Id - 1, false );

				alDeleteBuffers( Streams(i).NumBuffers, &Streams(i).Buffer[0] );
				appMemzero( Streams(i).Buffer, sizeof(Streams(i).Buffer) );
				for (INT BufferIndex = 0; BufferIndex < MAX_BUFFERS_PER_STREAM; ++BufferIndex)
					delete [] Streams(i).RefillData[BufferIndex];
				appMemzero( &Streams(i), sizeof(ALStream) );
	
				Sound->UnbindStream(Id);
				Sound->DeleteStreamHandle(SoundHandle);
			}
		}
		else if ( Sound->RefCount == 0 )
		{
			INT i = Sound->GetHandle() - 1;

			alDeleteBuffers( 1, &Buffers(i).Buffer );
			Buffers(i).Buffer = 0;
			Buffers(i).Flags  = 0;
	
			Sound->SetHandle(0);
		}
	}

	unguard;
}

//
// UALAudioSubsystem::PlaySound
//
INT UALAudioSubsystem::PlaySound( AActor* Actor, INT Id, USound* Sound, FVector Location, FLOAT Volume, FLOAT Radius, FLOAT Pitch, INT Flags, FLOAT FadeDuration, FLOAT InPriority )
{
	guard(UALAudioSubsystem::PlaySound);

	static INT FreeSlot;
	check(Radius);

	if (Sound->CoreFlags & SF_Streaming)
		debugf(NAME_DevSound, TEXT("Playing Streaming sound %s"), Sound->GetName());
	else
		debugf(NAME_DevSound, TEXT("Playing sound %s"), Sound->GetName());

	//	Add any resident flags from the sound -tg
	//
	Sound->WorkingFlags = Sound->CoreFlags | Flags;

	if( !Viewport || !Sound)
		return -1;

	//	If we failrd to register the sound, kill the least important playing sound (if any are "least important")
	//	and try again.
	//
	if (!RegisterSound(Sound))
	{
		INT iIndex = FindLeastImportantSound(Sound);

		if (iIndex == -1)
			return -1;
		else
		{
			StopSoundSource(iIndex);

			if (!RegisterSound(Sound))
				return -1;
		}
	}

	//clock(GStats.DWORDStats(ALAudioStats.STATS_PlaySoundCycles));
	//GStats.DWORDStats(ALAudioStats.STATS_PlaySoundCalls)++;

	// Global volume.
	Volume = Clamp( Volume * SoundVolume, 0.f, 1.f );

	// Compute priority.
	FLOAT Priority;
	
	//	Boost priority of sounds played in SLOT_Talk to make sure they get played.
	//
	if ((Id & 14) == (SLOT_Talk * 2))
	{
		Priority = 100.f;
	}
	else if( InPriority && ((Id & 14) == SLOT_Ambient * 2) )
	{
		// Ambient sounds have their priority calculated in Update.
		Priority = InPriority;
	}
	else
	{
		// Calculate occlusion before priority.
		FLOAT OcclusionRadius = Radius;
		if( !(Flags & SF_No3D) && Actor && Actor->GetLevel() )
		{
			guard(SoundOcclusion);
			//clock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
			if( !Actor->GetLevel()->IsAudibleAt( Location, LastPosition, Actor, (ESoundOcclusion) Actor->SoundOcclusion ) )
			{
				FPointRegion RegionSource	= Actor->GetLevel()->Model->PointRegion( Actor->Level, Location );
				OcclusionRadius = Radius * Actor->GetLevel()->CalculateRadiusMultiplier( RegionSource.ZoneNumber, RegionListener.ZoneNumber );
				OcclusionRadius *= OCCLUSION_FACTOR;
			}
			//unclock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
			unguard;
		}
		Priority = SoundPriority( Viewport, Location, Volume, OcclusionRadius, Flags );
	}
	
	FLOAT BestPriority	= Priority;

	// Allocate a new slot if requested. (Id & 00001110)
	if( (Id & 14) == 2 * SLOT_None )
		Id = 16 * --FreeSlot;

	INT Index = -1;

	// Find a voice to play the sound in.
	for (INT i = 0; i < Sources.Num(); i++ )
	{
		if (( Sources(i).Id & ~1 ) == ( Id & ~1))
		{
			// Skip if not interruptable.
			if ( Id & 1 )
				return -1;

			// Stop the sound.
			Index = i;
			break;
		}
		else if( (Sources(i).Priority * PLAYING_PRIORITY_MULTIPLIER ) < BestPriority )
		{
			Index = i;
			BestPriority = Sources(i).Priority;
		}
	}

	//	If no room to play the sound, see if we can make some room by stopping
	//	a sound that is currently playing multiple instances.
	if( Index == -1 )
		Index = FindLeastImportantSound(Sound);

	if( Index == -1 )
		return -1;

	StopSoundSource(Index);

	//	Here's where we bind the stream to the sound (if a streaming sound)
	//
	if (Sound->CoreFlags & SF_Streaming)
		Sound->BindStream(Id);

	Pitch  = Clamp<FLOAT>(Pitch, 0.1f, 2.0f);
	Volume = Clamp<FLOAT>(Volume, 0.01f, 1.0f);

	// Set default values.
	Sources(Index).ZoneRadius	= Radius;
	Sources(Index).UsedRadius	= Radius;
	Sources(Index).WantedRadius = Radius;

	// Calculate initial radius.
	if( !(Flags & SF_No3D) && Actor && Actor->GetLevel() )
	{
		guard(SoundOcclusion);
//		clock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
		if( !Actor->GetLevel()->IsAudibleAt( Location, LastPosition, Actor, (ESoundOcclusion) Actor->SoundOcclusion ) )
		{
			FPointRegion RegionSource	= Actor->GetLevel()->Model->PointRegion( Actor->Level, Location );
			FLOAT TempRadius = Radius * Actor->GetLevel()->CalculateRadiusMultiplier( RegionSource.ZoneNumber, RegionListener.ZoneNumber );
			Sources(Index).ZoneRadius	= TempRadius;
			TempRadius *= OCCLUSION_FACTOR;
			Sources(Index).UsedRadius	= TempRadius;
			Sources(Index).WantedRadius	= TempRadius;
		}
//		unclock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
		unguard;
	}

	// Setup the voice.
	Sources(Index).Sound		= Sound;
	Sources(Index).Id			= Id;
	Sources(Index).Actor		= Actor;
	Sources(Index).Priority		= Priority;
	Sources(Index).Location		= (Flags & SF_No3D) ? FVector(0,0,0) : Location;
	Sources(Index).Radius		= Radius ? Radius : 10;
	Sources(Index).LastChange	= appSeconds();
	Sources(Index).FadeDuration	= FadeDuration;
	Sources(Index).FadeTime		= 0.f;
	Sources(Index).FadeMode		= FadeDuration > 0.f ? FADE_In : FADE_None;
	Sources(Index).Volume		= (Flags & SF_Music) ? MusicVolume : Volume;
	Sources(Index).Started		= 1;
	Sources(Index).Paused		= 0;

	if (Sound->WorkingFlags & SF_Streaming)
		Sources(Index).Flags	= Flags | Streams(Sound->GetHandle(Id) - 1).Flags;
	else
		Sources(Index).Flags	= Flags | Buffers(Sound->GetHandle() - 1).Flags;

	//	If this fails, no big deal, as subsequent code will assume NULL means it has no
	//	lip-sync data.
	//
	if (Sound->WorkingFlags & SF_HasLipSync && Actor)
		Sources(Index).LipSyncAnimMgr = new FLipSyncAnimMgr(Actor, &Sound->LipSyncData);

	ALuint sid = Sources(Index).Source;

	alSourcef( sid, AL_REFERENCE_DISTANCE, Sources(Index).UsedRadius * DISTANCE_FACTOR );
	alSourcef( sid, AL_GAIN, Sources(Index).FadeMode == FADE_In ? 0.f : Sources(Index).Volume );
	alSourcef( sid, AL_PITCH, Pitch );

	if (Sound->WorkingFlags & SF_Streaming)
		alSourcei( sid, AL_LOOPING, AL_FALSE );
	else
		alSourcei( sid, AL_LOOPING, (Flags & SF_Looping) ? AL_TRUE : AL_FALSE );

	FVector Location =  Sources(Index).Location * DISTANCE_FACTOR;
	alSourcefv( sid, AL_POSITION, (ALfloat*) &Location); 

	FVector Velocity = FVector(0,0,0);
	alSourcefv( sid, AL_VELOCITY, (ALfloat *) &Velocity );

	//!!WARNING: DS3D doesn't support per source rolloff factor!
	alSourcef( sid, AL_ROLLOFF_FACTOR, RollOff );

	if( Flags & SF_No3D )
		alSourcei( sid, AL_SOURCE_RELATIVE, AL_TRUE );
	else
		alSourcei( sid, AL_SOURCE_RELATIVE, AL_FALSE );

	if( Sound->WorkingFlags & SF_Streaming )
		alSourceQueueBuffers( sid, Streams(Sound->GetHandle(Id) - 1).NumBuffers, &Streams( Sound->GetHandle(Id) - 1 ).Buffer[0] );
	else
		alSourcei( sid, AL_BUFFER, Buffers( Sound->GetHandle() - 1 ).Buffer );

	alSourceStop( sid );
	alSourceRewind( sid );

	alSourcePlay( sid );

	//	Lip sync also needs to know how long a sound has been 
	//	playing so it can determine the appropriate lip frame at any given time.
	//
	if (Sources(Index).LipSyncAnimMgr)
		Sources(Index).LipSyncAnimMgr->Start( appSeconds().GetFloat() );

//	unclock(GStats.DWORDStats(ALAudioStats.STATS_PlaySoundCycles));
	return Index;

	unguard;
}


//-----------------------------------------------------------------------------
//	StopSound( AActor* Actor, USound* Sound )
//
//	New Warfare StopSound()
//-----------------------------------------------------------------------------
UBOOL UALAudioSubsystem::StopSound( AActor* Actor, USound* Sound )
{
	UBOOL Stopped = false;

	// Stop all sounds.
	if ( !Actor && !Sound )
	{
		for (INT i=0; i<Sources.Num(); i++)	
			StopSoundSource(i);
		return true;
	}

	// Stop selected sound.
	for (INT i=0; i<Sources.Num(); i++)	
	{
		if ( Sound )
		{
			if ( Actor )
			{
				if ( (Actor == Sources(i).Actor) && (Sound == Sources(i).Sound) )
				{
					StopSoundSource(i);
					Stopped = true;
				}
			}
			else
			{
				if ( Sound == Sources(i).Sound )
				{
					StopSoundSource(i);
					Stopped = true;
				}
			}
		}
		else if ( Actor )
		{
			if ( Actor == Sources(i).Actor )
			{
				StopSoundSource(i);
				Stopped = true;
			}
		}
	}
	return Stopped;
}


//-----------------------------------------------------------------------------
//	StopSound( AActor* Actor, INT Id, USound * Sound )
//
//	Old-style StopSound() from UT.  
//
//	This is the StopSound that is called from script!
//-----------------------------------------------------------------------------
UBOOL UALAudioSubsystem::StopSound( AActor* Actor, INT Id, USound * Sound, FLOAT fFadeOutTime )
{
	UBOOL Stopped = false;
	guard(UALAudioSubsystem::StopSound);

	// Stop selected sound(s).
	//
	for (INT i = 0; i < Sources.Num(); i++)	
	{
		UBOOL	bSoundFound;

		//	Breaking down the test criteria for readability.
		//
		bSoundFound = Sources(i).Actor == Actor;						//	actors match
		bSoundFound &= (((Sources(i).Id & ~1) == (Id & ~1)) ||			//	same slot or SLOT_None and sound was played in SLOT_None
					   (((Id & 14) == SLOT_None) && (Sources(i).Id < 0)));	
		bSoundFound &= Sources(i).Sound == Sound || Sound == NULL;		//	Sounds match or NUMM sent in

		if (bSoundFound)
		{
			if (fFadeOutTime > 0.f)
			{
				Sources(i).FadeTime		= 0.f;
				Sources(i).FadeDuration	= fFadeOutTime;
				Sources(i).FadeMode		= FADE_Out;
			}
			else
			{
				StopSoundSource(i);
			}

			Stopped = true;
		}
	}

	return Stopped;

	unguard;
}

	
//-----------------------------------------------------------------------------
//	ModifySound( AActor* Actor, INT Id, USound * Sound, BYTE parameter, FLOAT Value )
//
//	Old-style ModifySound() from UT
//-----------------------------------------------------------------------------
UBOOL UALAudioSubsystem::ModifySound( AActor* Actor, INT Id, USound * Sound, BYTE parameter, FLOAT Value)
{
	guard(UALAudioSubsystem::ModifySound);

	UBOOL	bRet = false;

	for (INT i = 0; i < Sources.Num(); i++)	
	{
		UBOOL	bSoundFound;

		//	Breaking down the test criteria for readability.
		//
		bSoundFound = Sources(i).Actor == Actor;						//	actors match
		bSoundFound &= (((Sources(i).Id & ~1) == (Id & ~1)) ||			//	same slot or SLOT_None and sound was played in SLOT_None
					   (((Id & 14) == SLOT_None) && (Sources(i).Id < 0)));	
		bSoundFound &= Sources(i).Sound == Sound || Sound == NULL;		//	Sounds match or NUMM sent in

		if (bSoundFound)
		{
			// found the sound
			//
			ALuint sid = Sources(i).Source;

			switch (parameter)
			{
			case SOUND_Volume:
				Sources(i).Volume = Clamp<FLOAT>(Value, 0.01f, 1.0f);
				alSourcef( sid, AL_GAIN, Sources(i).Volume );
				break;

			case SOUND_Radius:
				// Set default values.
				//
				Sources(i).ZoneRadius	= Value;
				Sources(i).UsedRadius	= Value;
				Sources(i).WantedRadius = Value;

				// Calculate initial radius.
				//
				if( !(Sources(i).Flags & SF_No3D) && Sources(i).Actor && Sources(i).Actor->GetLevel() )
				{
					guard(SoundOcclusion);
					if( !Actor->GetLevel()->IsAudibleAt( Sources(i).Actor->Location, LastPosition, Actor, (ESoundOcclusion) Actor->SoundOcclusion ) )
					{
						FPointRegion RegionSource	= Actor->GetLevel()->Model->PointRegion( Actor->Level, Sources(i).Actor->Location );
						FLOAT TempRadius = Value * Actor->GetLevel()->CalculateRadiusMultiplier( RegionSource.ZoneNumber, RegionListener.ZoneNumber );
						Sources(i).ZoneRadius	= TempRadius;
						TempRadius *= OCCLUSION_FACTOR;
						Sources(i).UsedRadius	= TempRadius;
						Sources(i).WantedRadius	= TempRadius;
					}
					unguard;
				}

				alSourcef( sid, AL_REFERENCE_DISTANCE, Sources(i).UsedRadius * DISTANCE_FACTOR );

				break;

			case SOUND_Pitch:
				Value  = Clamp<FLOAT>(Value, 0.1f, 2.0f);
				alSourcef( sid, AL_PITCH, Value );
				break;
			}

			bRet = true;
		}
	}

	return bRet;
	unguard;
}


void UALAudioSubsystem::RenderAudioGeometry( FSceneNode* Frame )
{
	guard(UALAudioSubsystem::RenderAudioGeometry);
	unguard;
}


void UALAudioSubsystem::NoteDestroy(AActor* Actor)
{
	guard(UALAudioSubsystem::NoteDestroy);
	check(Actor);
	check(Actor->IsValid());

	// Stop the actor's sound, and dereference owned sounds.
	for (INT i=0; i<Sources.Num(); i++)
	{
		if (Sources(i).Actor == Actor)
		{
			if (( Sources(i).Id & 14 ) == SLOT_Ambient * 2)
				StopSoundSource(i);
			else 
				Sources(i).Actor = NULL; // Not interruptable sound.
		}
	}

	unguard;
}


INT UALAudioSubsystem::PlayMusic( FString Song, FLOAT FadeInTime )
{
	guard(UALAudioSubsystem::PlayMusic);
	// Start music.

	if ( Song != TEXT("") && appStricmp(*Song, TEXT("none")) != 0 )
	{
		UBOOL	bAppendExt = appStricmp(appFExt(*Song), TEXT("ogg")) != 0;

		FString Filename = TEXT("..\\music\\");
		Filename += Song;

		if (bAppendExt)
			Filename += TEXT(".ogg");

		USound* Music = new USound( *Filename, SF_Streaming | SF_Music );
		if (!Music)
		{
			debugf(NAME_DevSound, TEXT("Failed to allocate sound object for music file %s"), *Song );
			return 0;
		}

		//debugf(NAME_DevSound, TEXT("PlayMusic(%s) - name = %s"), *Song, Music->GetName());

		Music->FileType = FName(TEXT("ogg"));

		RegisterSound( Music );
		PlaySound( NULL, 2*SLOT_None | 1, Music, FVector(0,0,0), MusicVolume, 1000, 1.f, SF_Streaming | SF_No3D | SF_Music, FadeInTime );
	
		for( INT i=0; i<Sources.Num(); i++ )
			if( Sources(i).Sound == Music )
				return i+1;

		//!!TODO: warning message.
	}
	return 0;
	unguard;
}


UBOOL UALAudioSubsystem::StopMusic( INT SongHandle, FLOAT FadeOutTime )
{
	guard(UALAudioSubsystem::StopMusic);

	// Stop music.
	SongHandle--;

	//debugf(NAME_DevSound, TEXT("StopMusic(%s)"), Sources(SongHandle).Sound->GetName());

   	if( (SongHandle < Sources.Num()) && (SongHandle >= 0) )
	{
		if( FadeOutTime > 0.f )
		{
			Sources(SongHandle).FadeTime		= 0.f;
			Sources(SongHandle).FadeDuration	= FadeOutTime;
			Sources(SongHandle).FadeMode		= FADE_Out;
		}
		else
		{
			StopSoundSource(SongHandle);
		}

		return 1;
	}
	else
		return 0;
	unguard;
}


INT UALAudioSubsystem::StopAllMusic( FLOAT FadeOutTime )
{
	guard(UALAudioSubsystem::StopAllMusic);
	// Stop all music.
	for (INT i=0; i<Sources.Num(); i++)
		if ( Sources(i).Flags & SF_Music )
				StopMusic(i+1, FadeOutTime);
	return 0;
	unguard;
}

void UALAudioSubsystem::PauseSounds( void )
{
	for(INT i=0; i<Sources.Num(); i++)
	{
		if(Sources(i).Id)
		{
			Sources(i).Paused = 1;
			alSourcePause( Sources(i).Source );
		}
	}
}

void UALAudioSubsystem::UnpauseSounds( void )
{
	for(INT i=0; i<Sources.Num(); i++)
	{
		if(Sources(i).Id)
		{
			Sources(i).Paused = 0;
			if (Sources(i).Started)
				alSourcePlay( Sources(i).Source );
		}
	}
}

static INT Compare(ALAmbient& A,ALAmbient& B)
{
	return (B.Priority - A.Priority >= 0) ? 1 : -1;
}

void UALAudioSubsystem::Update( FSceneNode* SceneNode )
{
	guard(UALAudioSubsystem::Update);

	INT		i;

	if(!Viewport)
		return;

//	clock(GStats.DWORDStats(ALAudioStats.STATS_UpdateCycles));

	// Projection/ Orientation.
	FVector ViewLocation = SceneNode->Coords.Origin;
	FVector ProjUp;
	FVector ProjRight;
	FVector ProjFront;

	//	Coordinates are different if using DirectSound3D (UseEAX && !UseMMSYSTEM)
	//
	if (UseEAX && !UseMMSYSTEM)
	{
		ProjUp = SceneNode->Coords.ZAxis;

		ProjRight = ReverseStereo ? -SceneNode->Coords.XAxis : SceneNode->Coords.XAxis;
		ProjRight.Z = -ProjRight.Z;

		ProjFront = ProjRight ^ ProjUp;
	}
	else
	{
		ProjUp = -SceneNode->Coords.YAxis;
		ProjUp.Z = -ProjUp.Z;

		ProjRight = ReverseStereo ? -SceneNode->Coords.XAxis : SceneNode->Coords.XAxis;
		ProjRight.Y = -ProjRight.Y;

		ProjFront = ProjRight ^ ProjUp;
		ProjFront.Z = -ProjFront.Z;
	}

	ProjUp.Normalize();
	ProjRight.Normalize();
	ProjFront.Normalize();

	// Find out which zone the listener is in.
	AActor* ViewTarget = Viewport->Actor->ViewTarget ? Viewport->Actor->ViewTarget : Viewport->Actor;

	ULevel* Level = ViewTarget->GetLevel();
	RegionListener = Level->Model->PointRegion( ViewTarget->Level, ViewLocation );

	// Time passes...
	FTime CurrentTime	= appSeconds();
	FLOAT  DeltaTime	= CurrentTime.GetFloat() - LastTime.GetFloat();
	LastTime  = CurrentTime;
	DeltaTime = Clamp(DeltaTime,0.0001f,1.0f);
	
	UBOOL Realtime = Viewport->IsRealtime();
	UBOOL Paused = Viewport->Actor->Level->Pauser != TEXT("");

	//	Stop all sounds if transitioning out of realtime, pause or unpause sounds if pause
	//	transition.
	if( (!Realtime && LastRealtime) || (Paused && !LastPaused) )
		PauseSounds();
	else if ( (Realtime && !LastRealtime) || (!Paused && LastPaused) )
		UnpauseSounds();
	else if (RestartSounds)
	{
		RestartSounds = false;
		UnpauseSounds();
	}

	LastRealtime = Realtime;
	LastPaused = Paused;

	// Check for finished sounds
	for (i=0; i<Sources.Num(); i++)
	{
		if(	Sources(i).Id && Sources(i).Sound && !(Sources(i).Flags & (SF_Looping | SF_Music)) )
		{
			ALint State;
			alGetSourcei( Sources(i).Source, AL_SOURCE_STATE, &State );
			if ( State == AL_STOPPED )
			{
				if (Sources(i).Flags & SF_Streaming)
				{
					if (Streams(Sources(i).Sound->GetHandle(Sources(i).Id) - 1).Alive)
					{
						// Stream ran out of buffers!!!
						debugf(NAME_DevSound, TEXT("WARNING: audio buffer underrun - %s   time = %d"), Sources(i).Sound->GetName(), (int)(appSeconds().GetFloat() * 10000));
						Streams(Sources(i).Sound->GetHandle(Sources(i).Id) - 1).Reset = 1;
					}
					else
					{
						debugf(NAME_DevSound, TEXT("WARNING: was check(0) from Vogel, now UnregisterSound(%d, %d) - %s"), Sources(i).Sound, Sources(i).Id, Sources(i).Sound->GetName());
						UnregisterSound(Sources(i).Sound, Sources(i).Id);
					//	check(0);
					}
				}
				else
				{
					// Stream is finished.
					StopSoundSource(i);
				}
			}
		}
	}

	// See if any new ambient sounds need to be started.
	guard(HandleAmbience);
	if( Realtime && !Paused )
	{		
		TArray<ALAmbient> AmbientSounds;
		
		for(INT i=0; i<Level->Actors.Num(); i++)
		{
			AActor* Actor = Level->Actors(i);
			if ( Actor && Actor->AmbientSound )
			{
				INT Id = Actor->GetIndex()*16 + SLOT_Ambient*2;
				INT j;
				for( j=0; j<Sources.Num(); j++ )
					if( Sources(j).Id==Id )
						break;
				if( j==Sources.Num() )
				{
					if( Actor->IsOwnedBy( ViewTarget ) )
					{
						INT iAmbient = AmbientSounds.Add( 1 );
						AmbientSounds(iAmbient).Actor		= Actor;
						AmbientSounds(iAmbient).Priority	= SoundPriority( Viewport, Actor->Location, Actor->SoundVolume/255.f, Actor->SoundRadius, SF_Looping | SF_No3D | SF_UpdatePitch );
						AmbientSounds(iAmbient).Flags		= SF_Looping | SF_No3D | SF_UpdatePitch;
						AmbientSounds(iAmbient).Id			= Id;
					}
					else
					{
						// Sound occclusion.
//						clock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
						FLOAT RM = 1.f;
						if( !Level->IsAudibleAt( Actor->Location, ViewLocation, Actor, (ESoundOcclusion) Actor->SoundOcclusion ) )
						{
							FPointRegion RegionSource	= Level->Model->PointRegion( Actor->Level, Actor->Location );
							RM *= Level->CalculateRadiusMultiplier( RegionSource.ZoneNumber, RegionListener.ZoneNumber );
						}
//						unclock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
					
						if (FDistSquared(ViewLocation,Actor->Location)<=Square(RM*Actor->SoundRadius*GAudioMaxRadiusMultiplier) )
						{
							INT iAmbient = AmbientSounds.Add( 1 );
							AmbientSounds(iAmbient).Actor		= Actor;
							AmbientSounds(iAmbient).Priority	= SoundPriority( Viewport, Actor->Location, Actor->SoundVolume/255.f, RM*Actor->SoundRadius, SF_Looping | SF_UpdatePitch );
							AmbientSounds(iAmbient).Flags		= SF_Looping | SF_UpdatePitch;
							AmbientSounds(iAmbient).Id			= Id;
						}
					}
				}
			}
		}

		// Sort ambient sounds by priority to avoid thrashing of channels.
		//
		if( AmbientSounds.Num() )
		{
			Sort( &AmbientSounds(0), AmbientSounds.Num() );
			for( INT iAmbient = 0; iAmbient < AmbientSounds.Num(); iAmbient++ )
			{
				AActor* &Actor = AmbientSounds(iAmbient).Actor;

				// Early out if sound couldn't be played because of priority.
				//
				if (PlaySound( Actor, AmbientSounds(iAmbient).Id, Actor->AmbientSound, Actor->Location, Actor->SoundVolume/255.f, 
					 Actor->SoundRadius, Actor->SoundPitch / 64.f, AmbientSounds(iAmbient).Flags, 0.f, AmbientSounds(iAmbient).Priority ) == -1)
				{
					break;
				}
			}
		}
	}
	unguard;
	// Update all playing ambient sounds.
	guard(UpdateAmbience);
	for(INT i=0; i<Sources.Num(); i++)
	{
		if((Sources(i).Id&14) == SLOT_Ambient*2)
		{
			if( Sources(i).Actor )
			{
				FLOAT NewDist = FDistSquared(ViewLocation,Sources(i).Actor->Location);				
				if( 
					(NewDist > Square(Sources(i).ZoneRadius*GAudioMaxRadiusMultiplier)) || 
					(Sources(i).Actor->AmbientSound != Sources(i).Sound)
				)
				{
					StopSoundSource(i);
				}
			}
		}
	}
	unguard;

	// Update all active sounds.
	guard(UpdateSounds);
	for(INT i=0; i<Sources.Num(); i++)
	{
		if(Sources(i).Actor)
			check(Sources(i).Actor->IsValid());

		if(Sources(i).Id != 0)
		{
			//	If sounds are paused, we need to let the lip sync mgr adjust
			//
			if (Sources(i).Paused)
			{
				if (Sources(i).LipSyncAnimMgr)
				{
					Sources(i).LipSyncAnimMgr->EatPauseTime(DeltaTime);
				}
			}
			else
			{
				// Manage streaming sounds.
				if (Sources(i).Flags & SF_Streaming)
				{
					guard(UpdateStreamingSounds);
					INT Processed = 0;

					ALStream& Stream = Streams(Sources(i).Sound->GetHandle(Sources(i).Id) - 1);
					alGetSourcei( Sources(i).Source, AL_BUFFERS_PROCESSED, &Processed );

					UBOOL TerminalObserved = !Stream.Alive;

					alError(TEXT("Before queueing."),false);
					for (INT ProcessedIndex = 0; ProcessedIndex < Processed; ++ProcessedIndex)
					{
						ALuint ProcessedBuffer = 0;
						alSourceUnqueueBuffers(Sources(i).Source, 1, &ProcessedBuffer);
						alError(TEXT("unqueueing"));

						INT BufferIndex = INDEX_NONE;
						for (INT Candidate = 0; Candidate < Stream.NumBuffers; ++Candidate)
						{
							if (Stream.Buffer[Candidate] == ProcessedBuffer)
							{
								BufferIndex = Candidate;
								break;
							}
						}
						check(BufferIndex != INDEX_NONE);

						if (Stream.BufferState[BufferIndex] == QueuedPending)
							Stream.BufferState[BufferIndex] = FreePending;
						else if (Stream.BufferState[BufferIndex] == QueuedReady)
							Stream.BufferState[BufferIndex] = FreeReady;
						else if (Stream.BufferState[BufferIndex] != QueuedNoPrefetch)
							check(0);
					}

					void* CompletedDestination = NULL;
					UBOOL CompletionTerminal = 0;
					while (GFileStream->PopCompletedChunk(Stream.Id - 1, CompletedDestination, CompletionTerminal))
					{
						INT BufferIndex = INDEX_NONE;
						INT Matches = 0;
						for (INT Candidate = 0; Candidate < Stream.NumBuffers; ++Candidate)
						{
							if (Stream.RefillData[Candidate] == CompletedDestination)
							{
								BufferIndex = Candidate;
								++Matches;
							}
						}
						check(Matches == 1);

						if (Stream.BufferState[BufferIndex] == QueuedPending)
							Stream.BufferState[BufferIndex] = QueuedReady;
						else if (Stream.BufferState[BufferIndex] == FreePending)
							Stream.BufferState[BufferIndex] = FreeReady;
						else
							check(0);

						if (CompletionTerminal)
						{
							TerminalObserved = 1;
							Stream.Alive = 0;
						}
					}

					for (INT BufferIndex = 0; BufferIndex < Stream.NumBuffers; ++BufferIndex)
					{
						if (Stream.BufferState[BufferIndex] != FreeReady)
							continue;

						check(Stream.RefillData[BufferIndex] != NULL);
						alBufferData(
							Stream.Buffer[BufferIndex],
							Stream.Format,
							Stream.RefillData[BufferIndex],
							Stream.StreamChunkSize,
							Stream.Rate);
						alError(TEXT("bufferdata"));
						alSourceQueueBuffers(Sources(i).Source, 1, &Stream.Buffer[BufferIndex]);
						alError(TEXT("queueing"));

						if (!TerminalObserved
							&& GFileStream->RequestChunk(Stream.Id - 1, Stream.RefillData[BufferIndex]))
						{
							Stream.BufferState[BufferIndex] = QueuedPending;
						}
						else
						{
							Stream.BufferState[BufferIndex] = QueuedNoPrefetch;
						}
					}
					
					if ( Stream.Reset )
					{
						// Start playing again as source was stopped because it ran out of buffers.
						alSourcePlay( Sources(i).Source );
						Stream.Reset = 0;
					}

	//				GStats.DWORDStats(ALAudioStats.STATS_ActiveStreamingSounds)++;
					unguard;
				}
				else
				{
	//				GStats.DWORDStats(ALAudioStats.STATS_ActiveRegularSounds)++;
				}

				//	Update lip sync data if present
				//
				guard(UpdateLipSync);
				if (Sources(i).LipSyncAnimMgr)
					Sources(i).LipSyncAnimMgr->Update(CurrentTime.GetFloat());
				unguard;

				guard(UpdateActorLocPitchVolAndFadeInFadeOut);
				// Update position, velocity, pitch and volume from actor (if wanted)
				FVector Velocity	= FVector(0,0,0);
				FLOAT Volume		= Sources(i).Volume;;
				if( Sources(i).Actor && !(Sources(i).Flags & SF_NoUpdates) )
				{
					// Set location.
					Sources(i).Location = Sources(i).Actor->Location;
					if (!(Sources(i).Flags & SF_No3D))
						Velocity = Sources(i).Actor->Velocity * DISTANCE_FACTOR;

					// Set pitch.
					if ( Sources(i).Flags & SF_UpdatePitch )
						alSourcef( Sources(i).Source, AL_PITCH, Sources(i).Actor->SoundPitch / 64.f );

					// Set Volume.
					if( (Sources(i).Id&14) == SLOT_Ambient*2 )
					{
						Volume = Sources(i).Actor->SoundVolume / 255.f;
						Sources(i).Volume = Volume;
					}
					if( Sources(i).Actor->LightType!=LT_None )
						Volume *= Sources(i).Actor->LightBrightness / 255.f;
				}

				if( Sources(i).FadeMode == FADE_In )
				{
					// Disregard initial loading time.
					if( DeltaTime < 1.f )
						Sources(i).FadeTime += DeltaTime;
					if( Sources(i).FadeTime >= Sources(i).FadeDuration )
						Sources(i).FadeMode = FADE_None;
					else
						Volume *= Sources(i).FadeTime / Sources(i).FadeDuration;
				}

				if( Sources(i).FadeMode == FADE_Out )
				{
					// Disregard initial loading time.
					if( DeltaTime < 1.f )
						Sources(i).FadeTime += DeltaTime;
					if( Sources(i).FadeTime >= Sources(i).FadeDuration )
					{
						// Stop sound.
						StopSoundSource(i);

						continue;
					}
					else
						Volume *= (1.f - Sources(i).FadeTime / Sources(i).FadeDuration);
				}

				alSourcef ( Sources(i).Source, AL_GAIN, Volume );

				if( !(Sources(i).Flags & SF_No3D) )
				{
					// Sound occclusion.
					FLOAT Radius = Sources(i).Radius;
	//				clock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
					if( !Level->IsAudibleAt(	
							Sources(i).Location, 
							ViewLocation, 
							Sources(i).Actor, 
							Sources(i).Actor ? (ESoundOcclusion) Sources(i).Actor->SoundOcclusion : OCCLUSION_Default ) 
					)
					{					
						FPointRegion RegionSource = Level->Model->PointRegion( Viewport->Actor->Level, Sources(i).Location );
						Radius *= Level->CalculateRadiusMultiplier( RegionSource.ZoneNumber, RegionListener.ZoneNumber );
						Sources(i).ZoneRadius = Radius;
						Radius *= OCCLUSION_FACTOR;
	//					GStats.DWORDStats(ALAudioStats.STATS_OccludedSounds)++;
					}
					else
						Sources(i).ZoneRadius = Radius;
	//				unclock(GStats.DWORDStats(ALAudioStats.STATS_OcclusionCycles));
					
					// Smooth transition between radii.
					if( Sources(i).WantedRadius != Radius )
					{
						Sources(i).LastChange	= CurrentTime;
						Sources(i).WantedRadius = Radius;
					}

					if( (CurrentTime - Sources(i).LastChange) < 1.f )
						Radius = Lerp( Sources(i).UsedRadius, Radius, CurrentTime - Sources(i).LastChange );
					else
						Sources(i).UsedRadius = Radius;
					
					// Set Radius.
					alSourcef( Sources(i).Source, AL_REFERENCE_DISTANCE, Radius * DISTANCE_FACTOR );
				}

				// Update the priority.
				Sources(i).Priority = SoundPriority(
					Viewport,
					Sources(i).Location,
					Sources(i).Volume,
					Sources(i).ZoneRadius,
					Sources(i).Flags
				);

				if( !(Sources(i).Flags & SF_No3D) )
				{
					FVector Location = Sources(i).Location * DISTANCE_FACTOR;
					Velocity *= DISTANCE_FACTOR;
					alSourcefv( Sources(i).Source, AL_POSITION, (ALfloat *) &Location );
					if( Velocity.Size() < MAX_SOURCE_VELOCITY )
						alSourcefv( Sources(i).Source, AL_VELOCITY, (ALfloat *) &Velocity );
				}
				unguard;

				//sceSdRemote(1,rSdSetParam,CoreVoiceMask(i) | SD_VP_PITCH,(INT) ((((float) Voices[i].Sound->SoundRate) / 48000.0f) * Voices[i].Pitch * ViewActors[Voices[i].ViewportNum]->Level->TimeDilation * Doppler * (44100.0f / 48000.0f) * 4096.0f));
			}
		}
	}

	guard(UpdateListenerVelocityAndPos);
	// Set Player position and orientation.
	FVector Orientation[2];
	Orientation[0]		= ProjUp;
	Orientation[1]		= ProjFront;
	//Orientation[1].Z	= -Orientation[1].Z;

	FVector Velocity	= (ViewLocation - LastPosition) / DeltaTime * DISTANCE_FACTOR;
	LastPosition		= ViewLocation;
	ViewLocation		*= DISTANCE_FACTOR;

	alListenerfv( AL_POSITION, (ALfloat *) &ViewLocation );
	alListenerfv( AL_ORIENTATION, (ALfloat *) Orientation );
	if( Velocity.Size() < MAX_LISTENER_VELOCITY )
		alListenerfv( AL_VELOCITY, (ALfloat *) &Velocity );
	unguard;

	// Set I3DL2 listener zone effect.
	SetI3DL2Listener( RegionListener.Zone->ZoneEffect );

//	unclock(GStats.DWORDStats(ALAudioStats.STATS_UpdateCycles));

	unguard;
	unguard;
}


UBOOL UALAudioSubsystem::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UALAudioSubsystem::Exec);
	const TCHAR*	Str = Cmd;
	if(ParseCommand(&Str,TEXT("ASTAT")))
	{
		if(ParseCommand(&Str,TEXT("AUDIO")))
		{
//			AudioStats ^= 1;
			return 1;
		}
	}
	else if(ParseCommand(&Str,TEXT("PAUSESOUNDS")))
	{
		PauseSounds();
		return 1;
	}
	else if(ParseCommand(&Str,TEXT("UNPAUSESOUNDS")))
	{
		UnpauseSounds();
		return 1;
	}
	else if(ParseCommand(&Str,TEXT("RESTARTSOUNDS")))
	{
		RestartSounds = true;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("WEAPONRADIUS")) )
	{
		GAudioDefaultRadius=appAtof(Cmd);
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("ROLLOFF")) )
	{
		RollOff=appAtof(Cmd);
		StopSound( NULL, NULL );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("PLAYINGSOUNDSTATS")) )
	{
		int	iTotalMem = 0;
		int	iOverhead = 0;

		for (int i = 0; i < MAX_AUDIOSTREAMS; i++)
		{
			USound	*Sound = Sources(i).Sound;

			if (Sound)
			{
				int	iSoundDataSize = Sound->Data.Num();
				int	iALBufferSize;

				if (Sound->WorkingFlags & SF_Streaming)
				{
					//	Calc straming buffer size
					//
					ALStream& Stream = Streams(Sound->GetHandle(Sources(i).Id) - 1);
					int	iStreamingBufferSize;

					if (appStricmp(*Sound->FileType, TEXT("ogg")) == 0)
					{
						iStreamingBufferSize = OGGVORBIS_STREAM_CHUNKSIZE;
					}
					else
					{
						iStreamingBufferSize = XA_STREAM_CHUNKSIZE;
					}
					iOverhead += iStreamingBufferSize;

					//	Calc ALAudio buffer sizes
					//
					iALBufferSize = 0;
					for (int j = 0; j < Stream.NumBuffers; j++)
					{
						int	iTemp;

						alGetBufferi(Stream.Buffer[j], AL_SIZE, &iTemp);
						iALBufferSize += iTemp;
					}

					Ar.Logf( TEXT("Streaming Sound:  Name = %s  Sound data = %iK  ALAudio buffer = %iK  Streaming buffer = %iK"), Sound->GetName(), iSoundDataSize / 1024, iALBufferSize / 1024, iStreamingBufferSize / 1024 );
				}
				else
				{
					alGetBufferi(Buffers( Sound->GetHandle() - 1 ).Buffer, AL_SIZE, &iALBufferSize);
					Ar.Logf( TEXT("Sound:  Name = %s  Sound data = %iK  ALAudio buffer = %iK"), Sound->GetName(), iSoundDataSize / 1024, iALBufferSize / 1024);
				}

				iOverhead += iALBufferSize;
				iTotalMem += iOverhead + iSoundDataSize;
			}
		}

		Ar.Logf( TEXT("Total memory = %iK  Overhead = %iK"), iTotalMem / 1024, iOverhead / 1024);

		return 1;
	}
	return 0;	
	unguard;
}


/*------------------------------------------------------------------------------------
	Global Pause/Resume music functions, needed for sync with StaticLoadObject.
------------------------------------------------------------------------------------*/

void appPauseMusic()
{
	guard(UALAudioSubsystem::PauseMusic);
	unguard;
}

void appResumeMusic()
{
	guard(UALAudioSubsystem::ResumeMusic);
	unguard;
}

/*------------------------------------------------------------------------------------
	Internals.
------------------------------------------------------------------------------------*/


void UALAudioSubsystem::SetVolumes()
{
	guard(UALAudioSubsystem::SetVolumes);
	// Update the music volume.
	unguard;
}

//-----------------------------------------------------------------------------
//	The UnregisterSound/StopSound thing has gone through a few too many 
//	modifications and is now a hack at best.  I am adding a wrapper to
//	StopSound that will call UnregisterSound for streaming sounds and
//	the old StopSound for other sounds.  Hack hack hack hack hack.  -tg
//-----------------------------------------------------------------------------
void UALAudioSubsystem::StopSoundSource( INT i )
{
#if 1
	if (Sources(i).Sound)
	{
		UnregisterSound(Sources(i).Sound, Sources(i).Id);
	}
#else
	if (Sources(i).Sound)
	{
		//	We create the music object with the C++ new operator.  The destructor for a
		//	USound object calls UnregisterObject for us.  Hacky but true.
		//
		if (Sources(i).Sound->WorkingFlags & SF_Music)
			delete Sources(i).Sound;
		else
			UnregisterSound(Sources(i).Sound, Sources(i).Id);
	}
#endif
}


void UALAudioSubsystem::OldStopSound( INT i )
{
	guard(UALAudioSubsystem::StopSound);

	if (Sources(i).Id != 0)
	{
//		GStats.DWORDStats(ALAudioStats.STATS_StoppedSounds)++;
//		if (Sources(i).Flags & SF_Streaming && Sources(i).Sound->GetHandle(Sources(i).Id) > 0)
//			GFileStream->DestroyStream( Streams( Sources(i).Sound->GetHandle(Sources(i).Id) - 1 ).Id - 1, false );
			
		if ( Sources(i).Source )
		{
			alSourceStop( Sources(i).Source );
			alSourcei( Sources(i).Source, AL_BUFFER, NULL );
		}

		if (Sources(i).LipSyncAnimMgr)
		{
			delete Sources(i).LipSyncAnimMgr;
			Sources(i).LipSyncAnimMgr = NULL;
		}

		Sources(i).Sound	= NULL;
		Sources(i).Actor	= NULL;
		Sources(i).Flags	= 0;
		Sources(i).Priority	= 0;
		Sources(i).Id		= 0;
		Sources(i).Started	= 0;
		Sources(i).Paused	= 0;
	}

	unguard;
}


FLOAT UALAudioSubsystem::SoundPriority( UViewport* Viewport, FVector Location, FLOAT Volume, FLOAT Radius, INT Flags )
{
	guard(UALAudioSubsystem::SoundPriority);
	FLOAT RadiusFactor;
	if ( Radius )
	{
		AActor *ViewTarget = Viewport->Actor->ViewTarget ? Viewport->Actor->ViewTarget : Viewport->Actor;
		RadiusFactor = 1 - FDistSquared(Location, ViewTarget->Location) / Square(GAudioMaxRadiusMultiplier*Radius);
	}
	else
		RadiusFactor = 1;
	RadiusFactor = Clamp(RadiusFactor, 0.01f, 1.f);

	return Volume * RadiusFactor + ((Flags & SF_HasLipSync)? 4 : 0) + ((Flags & SF_Music)? 2 : 0) + ((Flags & SF_No3D)? 1 : 0);
	unguard;
}


INT UALAudioSubsystem::GetNewStreamStruct()
{
	guard(UALAudioSubsystem::GetNewStreamStruct);
	for( INT i=0; i< Streams.Num(); i++ )
	{
		if( Streams(i).Id == 0 )
		{
			appMemzero( &Streams(i), sizeof(ALStream) );
			return i;
		}
	}

	debugf(NAME_DevSound, TEXT("More than %i streams in use"), MAX_AUDIOSTREAMS );
//	appErrorf(TEXT("More than %i streams in use"), MAX_AUDIOSTREAMS );
	return -1;
	unguard;
}


void UALAudioSubsystem::SetI3DL2Listener( UI3DL2Listener* Listener )
{
	guard(SetI3DL2Listener);

	// Do nothing if EAX isn't supported.
	if( !UseEAX || !alEAXSet || !alEAXGet )
		return;

	// disable this code until the next code drop from Epic.  Current drop is 927.  -tg
	//
#if 0
	// Check whether update is necessary.
	if( (OldListener == Listener) && (!Listener || !Listener->Updated) )
		return;

	OldListener	= Listener;
	
	if( !Listener )
	{
		// Set 'plain' EAX preset.
		DWORD Environment = EAX_ENVIRONMENT_PLAIN;
		alEAXSet(&DSPROPSETID_EAX20_ListenerProperties,DSPROPERTY_EAXLISTENER_ENVIRONMENT			,NULL, &Environment						, sizeof(INT	));
	}
	else
	{
		DWORD Flags = 0;
		if( Listener->bDecayTimeScale )
			Flags |= EAXLISTENERFLAGS_DECAYTIMESCALE;
		if( Listener->bReflectionsScale )
			Flags |= EAXLISTENERFLAGS_REFLECTIONSSCALE;
		if( Listener->bReflectionsDelayScale )
			Flags |= EAXLISTENERFLAGS_REFLECTIONSDELAYSCALE;
		if( Listener->bReverbScale )
			Flags |= EAXLISTENERFLAGS_REVERBSCALE;
		if( Listener->bReverbDelayScale )
			Flags |= EAXLISTENERFLAGS_REVERBDELAYSCALE;
		if( Listener->bDecayHFLimit )
			Flags |= EAXLISTENERFLAGS_DECAYHFLIMIT;
		
		_EAXLISTENERPROPERTIES EAXProperties;

		EAXProperties.dwEnvironment				= EAX_ENVIRONMENT_GENERIC;
		EAXProperties.lRoom						= Listener->Room;
		EAXProperties.lRoomHF					= Listener->RoomHF;
		EAXProperties.flRoomRolloffFactor		= Listener->RoomRolloffFactor;
		EAXProperties.flDecayTime				= Listener->DecayTime;
		EAXProperties.flDecayHFRatio			= Listener->DecayHFRatio;
		EAXProperties.lReflections				= Listener->Reflections;
		EAXProperties.flReflectionsDelay		= Listener->ReflectionsDelay;
		EAXProperties.lReverb					= Listener->Reverb;
		EAXProperties.flReverbDelay				= Listener->ReverbDelay;
		EAXProperties.flEnvironmentSize			= Listener->EnvironmentSize;
		EAXProperties.flEnvironmentDiffusion	= Listener->EnvironmentDiffusion;
		EAXProperties.flAirAbsorptionHF			= Listener->AirAbsorptionHF;
		EAXProperties.dwFlags					= Flags;
		
		alEAXSet(&DSPROPSETID_EAX20_ListenerProperties,DSPROPERTY_EAXLISTENER_ALLPARAMETERS, NULL, &EAXProperties, sizeof(EAXProperties));

		// Change has been commited.
		Listener->Updated = false;

	}
#endif

	unguard;
}


UBOOL UALAudioSubsystem::alError( TCHAR* Text, UBOOL Log )
{
	ALint Error = alGetError();
	if ( Error == AL_NO_ERROR )
		return false;
	else
	{
		if ( Log )
		{
			switch ( Error )
			{
			case AL_INVALID_NAME:
				debugf(TEXT("ALAudio: AL_INVALID_NAME in %s"), Text);
				break;
			case AL_INVALID_ENUM:
				debugf(TEXT("ALAudio: AL_INVALID_ENUM in %s"), Text);
				break;
			case AL_INVALID_VALUE:
				debugf(TEXT("ALAudio: AL_INVALID_VALUE in %s"), Text);
				break;
			case AL_INVALID_OPERATION:
				debugf(TEXT("ALAudio: AL_INVALID_OPERATION in %s"), Text);
				break;
			case AL_OUT_OF_MEMORY:
				debugf(TEXT("ALAudio: AL_OUT_OF_MEMORY in %s"), Text);
				break;
			default:
				debugf(TEXT("ALAudio: Unknown error in %s"), Text);
			}
		}
		return true;
	}
}

#define INITIAL_MIN_COUNT	3	//	we need to find at least this many playing instances of a sound before we'll kill it

INT UALAudioSubsystem::FindLeastImportantSound( USound *pIncomingSound )
{
	INT					iRet = -1;

	TMap<USound *, INT>	SoundInstances;
	INT					iMaxCount = INITIAL_MIN_COUNT;			
	USound				*pMaxCountSound = NULL;

	//	Count the number of instances of each playing sound
	//
	for (INT i = 0; i < Sources.Num(); i++ )
	{
		if (Sources(i).Id != 0)
		{
			INT iNumInstances = SoundInstances.FindRef(Sources(i).Sound);
			iNumInstances++;
			SoundInstances.Set(Sources(i).Sound, iNumInstances);

			if (iNumInstances > iMaxCount)
			{
				iMaxCount = iNumInstances;
				pMaxCountSound = Sources(i).Sound;
			}
		}
	}

	//	If we found a sound with enough playing instances AND it's not the same
	//	sound we're trying to play, determine the most distant instance and return it's Sources() index.
	//
	if (pMaxCountSound && pMaxCountSound != pIncomingSound)
	{
		FLOAT	fLowestPriority = 1000.f;

		for (INT i = 0; i < Sources.Num(); i++ )
		{
			//	Don't even check sounds that are playing in SLOT_Talk - we never want to kill them.	
			//
			if (Sources(i).Id != 0 && ((Sources(i).Id & 14) != (SLOT_Talk * 2)) && pMaxCountSound == Sources(i).Sound)
			{
				FLOAT	fPriority = SoundPriority(Viewport, Sources(i).Location, Sources(i).Volume, Sources(i).Radius, Sources(i).Sound->WorkingFlags);

				if (fPriority < fLowestPriority)
				{
					fLowestPriority = fPriority;
					iRet = i;
				}
			}
		}
	}

	return iRet;
}


UALAudioSubsystem::FALAudioStats::FALAudioStats()
:	STATS_FirstEntry			(-1)
,	STATS_PlaySoundCycles		(-1)
,	STATS_UpdateCycles			(-1)
,	STATS_OcclusionCycles		(-1)
,	STATS_PlaySoundCalls			(-1)
,	STATS_OccludedSounds			(-1)
,	STATS_ActiveStreamingSounds	(-1)
,	STATS_ActiveRegularSounds	(-1)
,	STATS_StoppedSounds			(-1)
,	STATS_LastEntry				(-1)
{
	guard(FALAudioStats::FALAudioStats)
	unguard;
}

void UALAudioSubsystem::FALAudioStats::Init()
{
	guard(FALAudioStats::Init);
#if 0
	// If already initialized retrieve indices from GStats.
	if( GStats.Registered[STATSTYPE_Audio] )
	{
		INT* Dummy = &STATS_PlaySoundCycles;
		for( INT i=0; i<GStats.Stats[STATSTYPE_Audio].Num(); i++ )
			*(Dummy++) = GStats.Stats[STATSTYPE_Audio](i).Index;
		return;
	}

	// Register stats with GStat.
	STATS_PlaySoundCalls				= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("PlaySound"		), TEXT("Audio"		), STATSUNIT_Combined_Default_MSec	);
	STATS_PlaySoundCycles				= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("PlaySound"		), TEXT("Audio"		), STATSUNIT_MSec					);
	STATS_OccludedSounds				= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("Occlusion"		), TEXT("Audio"		), STATSUNIT_Combined_Default_MSec	);
	STATS_OcclusionCycles				= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("Occlusion"		), TEXT("Audio"		), STATSUNIT_MSec					);
	STATS_UpdateCycles					= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("Update"			), TEXT("Audio"		), STATSUNIT_MSec					);
	STATS_ActiveStreamingSounds			= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("Streaming"		), TEXT("Audio"		), STATSUNIT_Default				);
	STATS_ActiveRegularSounds			= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("Regular"		), TEXT("Audio"		), STATSUNIT_Default				);
	STATS_StoppedSounds					= GStats.RegisterStats( STATSTYPE_Audio, STATSDATATYPE_DWORD, TEXT("StoppedSounds"	), TEXT("Audio"		), STATSUNIT_Default				);
	
	// Initialized.
	GStats.Registered[STATSTYPE_Audio] = 1;
#endif

	unguard;
}


//-----------------------------------------------------------------------------
//
//	class FLipSyncMgr
//
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
//	FLipSyncMgr public functions
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
//	FLipSyncAnimMgr( AActor *ParentActor, FLipSyncData *LipSyncData )
//
//	Constructor
//-----------------------------------------------------------------------------
FLipSyncAnimMgr::FLipSyncAnimMgr( AActor *ParentActor, FLipSyncData *pLipSyncData )
{
	m_ParentActor = ParentActor;
	m_pLipSyncData = pLipSyncData;
}


//-----------------------------------------------------------------------------
//	~FLipSyncAnimMgr( )
//
//	Destructor
//-----------------------------------------------------------------------------
FLipSyncAnimMgr::~FLipSyncAnimMgr( )
{
	if (m_LipSyncAnimChan)
		m_LipSyncAnimChan->PlayAnim( TEXT("mouth_rest"), false, 1.0, 1.0, 1.0 );

	m_ParentActor = NULL;
	m_pLipSyncData = NULL;
}


//-----------------------------------------------------------------------------
//	Start( FLOAT fTime )
//
//	Tells a lip sync animation manager object that the sound is starting.
//-----------------------------------------------------------------------------
UBOOL FLipSyncAnimMgr::Start( FLOAT fTime )
{
	m_StartTime = fTime;
	m_LastElapsedTime = 0.0;
	m_LastAmp = 0;

	LoadObject<UClass>( AActor::StaticClass(), TEXT("AnimChannel"), NULL, 0, NULL );
  	m_LipSyncAnimChan = m_ParentActor->CreateAnimChannel( AAnimChannel::StaticClass(), AT_Replace, TEXT("mouth_root"), false, true );
	
	if (m_LipSyncAnimChan)
		m_LipSyncAnimChan->PlayAnim( TEXT("mouth_anim"), false, 0.0 );

	return m_LipSyncAnimChan != NULL;
}


//-----------------------------------------------------------------------------
//	Update( FLOAT fTime )
//
//	Updates the actor's animation state based on time elapsed from beginning
//	of sound.
//-----------------------------------------------------------------------------
#define LIP_SYNC_UPDATE_INTERVAL	(0.05f)

void FLipSyncAnimMgr::Update( FLOAT fTime )
{
	//	Bail if we failed to get an anim channnel earlier
	//
	if (!m_LipSyncAnimChan)
		return;

	FLOAT	ElapsedTime = fTime - m_StartTime;
	FLOAT	fFrameDeltaTime = ElapsedTime - m_LastElapsedTime;

	if ( fFrameDeltaTime >= LIP_SYNC_UPDATE_INTERVAL )
	{
		m_LastElapsedTime = ElapsedTime;

//		m_LipSyncAnimChan->PlayAnim( TEXT("mouth_anim"), false, 0.0 );
		m_LipSyncAnimChan->AnimFrame = m_pLipSyncData->GetMouthPos(ElapsedTime);
	}
}

//-----------------------------------------------------------------------------
//	EatPauseTime( FLOAT fDeltaTime )
//
//	Called while the parent sound is paused.  
//	of sound.
//-----------------------------------------------------------------------------
void FLipSyncAnimMgr::EatPauseTime( FLOAT fDeltaTime )
{
	m_StartTime += fDeltaTime;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
