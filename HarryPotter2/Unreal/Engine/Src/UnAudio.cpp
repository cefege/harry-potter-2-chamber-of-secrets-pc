/*=============================================================================
	UnAudio.cpp: Unreal base audio.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney
	* Wave modification code by Erik de Neve
=============================================================================*/

#include "EnginePrivate.h" 
#include <math.h>

//	Lip-sync constants
//
const float	FLipSyncData::fAmpSampleInterval = 0.020;
const float	FLipSyncData::fAmpSampleWindowSize = 0.005;

/*-----------------------------------------------------------------------------
	USound implementation.
-----------------------------------------------------------------------------*/

#pragma DISABLE_OPTIMIZATION
void FSoundData::Load()
{
	guard(FSoundData::Load);
	UBOOL Loaded = SavedPos>0;

	guard(0);
	TLazyArray<BYTE>::Load();
	unguard;

	if( Loaded && (Owner->FileType != FName(TEXT("PS2"))) )
	{
		// Calculate our duration.
		guard(1);
		guard(callingGetPerioid);
// TG ALPHA		Owner->Duration = GetPeriod();
		unguard;
		unguard;

		// Derive these from the exposed 'low quality' preference setting.
		INT Force8Bit = 0;
		INT ForceHalve = 0;
		guard(3);
//VOGEL:SOUNDHACK		if( Owner->Audio && Owner->Audio->GetLowQualitySetting() && !GIsEditor )
		if (0)
		{
			Force8Bit = 1;
			ForceHalve = 1;
		}
		unguard;

		// Frequencies below this sample rate will NOT be downsampled.
		DWORD FreqThreshold = 22050;

		// Reduce sound frequency and/or bit depth if required.
		if( Force8Bit || ForceHalve )
		{		
			// If ReadWaveInfo returns true, all relevant Wave chunks were found and 
			// all pointers in the WaveInfo structure have been successfully initialized.			
			guard(4);
			FWaveModInfo WaveInfo;
			if( WaveInfo.ReadWaveInfo(*this) && WaveInfo.SampleDataSize>4  ) 
			{				
				// Three main conversions:
				// * Halving the frequency -> simple 0.25, 0.50, 0.25 kernel. 
				// * Reducing bit-depth 8->16  
				// * Both in one sweep.  
				//
				// Important: Wave data ALWAYS padded to use 16-bit alignment even
				// though the number of bytes in pWaveDataSize may be odd.	
				UBOOL ReduceBits = ((Force8Bit) && (*WaveInfo.pBitsPerSample == 16));
				UBOOL ReduceFreq = ((ForceHalve) && (*WaveInfo.pSamplesPerSec >= FreqThreshold));
				if( ReduceBits && ReduceFreq )
				{
					// Convert 16-bit sample to 8 bit and halve the frequency too.
					guard(5);
					WaveInfo.HalveReduce16to8();
					unguard;
				}
				else if (ReduceBits && (!ReduceFreq))
				{	
					// Convert 16-bit sample down to 8-bit.
					guard(6);
					WaveInfo.Reduce16to8();
					unguard;
				}
				else if( ReduceFreq )
				{
					// Just halve the frequency. Separate cases for 16 and 8 bits.
					guard(7);
					WaveInfo.HalveData();
					unguard;
				}
				guard(8);
				WaveInfo.UpdateWaveData( *this );
				unguard;
			}
			unguard;
		}

		// Register it.
		guard(2);
		Owner->OriginalSize = Num();
//		if( Owner->Audio && !GIsEditor )
//			Owner->Audio->RegisterSound( Owner );
		unguard;
	}
	else if ( Loaded && (Owner->FileType == FName(TEXT("PS2"))) ) 
	{
		// Register it.
		guard(RegisterSound);
// TG ALPHA		Owner->Duration = GetPeriod();
		Owner->OriginalSize = Num();
//		if( Owner->Audio && !GIsEditor )
//			Owner->Audio->RegisterSound( Owner );
		unguard;
	}
	unguard;
}

FLOAT FSoundData::GetPeriod()
{
	FLOAT Period = 0.f;
	if( Owner->FileType != FName(TEXT("PS2")) )
	{
		// Ensure the data is present.
		TLazyArray<BYTE>::Load();

		// Calculate the sound's duration.
		FWaveModInfo WaveInfo;
		if( WaveInfo.ReadWaveInfo(*this) )
		{
			#define DEFAULT_FREQUENCY (22050)
			INT DurDiv =  *WaveInfo.pChannels * *WaveInfo.pBitsPerSample  * *WaveInfo.pSamplesPerSec;  
			if ( DurDiv ) Period = *WaveInfo.pWaveDataSize * 8.f / (FLOAT)DurDiv;
		}	
		return Period;
	} else {
		// Ensure the data is present.
		TLazyArray<BYTE>::Load();

		// Get the period from the header.
		appMemcpy( &Period, ((BYTE*) Data) + sizeof(INT), sizeof(FLOAT) );
	}
	return Period;
}


void USound::Serialize( FArchive& Ar )
{
	guard(USound::Serialize);
	Super::Serialize( Ar );

	Ar << FileType;
	Ar << CoreFlags;
	Ar << Duration;

	//	Adding number of samples to the sound object
	//
	if (Ar. Ver() > 76)
	{
		Ar << raw_NumSamples;
	}

	if (Ar. Ver() > 77)
	{
		Ar << raw_BitsPerSample;
		Ar << raw_NumChannels;
	}

	if (Ar. Ver() > 78)
	{
		Ar << raw_SampleRate;
	}

// TG ALPHA - check CountBytes wrt lip sync data
	if( Ar.IsLoading() || Ar.IsSaving() )
	{
		Ar << Data;

		if (CoreFlags & SF_HasLipSync)
		{
			Ar << LipSyncData;
		}
	}
	else 
		Ar.CountBytes( OriginalSize, OriginalSize );

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


//-----------------------------------------------------------------------------
//	Bind( INT Id )
//
//	Associates a USound object with a unique Id (as sent to PlaySound).
//-----------------------------------------------------------------------------
void USound::BindStream( INT Id )
{
//	debugf(NAME_DevSound, TEXT("BindStream()"));

	if (CoreFlags & SF_Streaming)
	{
		UBOOL	bDone = false;

		for (int i = 0; !bDone && i < OpenStreams.Num(); i++)
		{
			if ( !(OpenStreams(i) & 1) )
			{
//				debugf(NAME_DevSound, TEXT("Binding stream Id = %d, OpenStream(i) = %d"), Id, OpenStreams(i));

				PlayingStreams.Set(Id, OpenStreams(i));
				OpenStreams(i) |= 1;

				bDone = true;
			}
		}
	}
}


//-----------------------------------------------------------------------------
//	Unbind( INT Id )
//
//	Disassociates a USound object from a unique Id (as sent to PlaySound).
//-----------------------------------------------------------------------------
void USound::UnbindStream( INT Id )
{
//	debugf(NAME_DevSound, TEXT("UnbindStream()"));

	//	Clear the busy flag
	//
	int		iOpenStream = PlayingStreams.FindRef(Id);
	UBOOL	bDone = false;

	for (int i = 0; !bDone && i < OpenStreams.Num(); i++)
	{
		if ((OpenStreams(i) & ~1) == iOpenStream)
		{
			OpenStreams(i) &= 0xFFFFFFFEL;

//			debugf(NAME_DevSound, TEXT("Unbinding stream Id = %d, OpenStream9i0 = %d"), Id, OpenStreams(i));

			bDone = true;
		}
	}

	//	Remove the mapping of an open stream to a playing stream
	//
	PlayingStreams.Remove(Id);
}


//-----------------------------------------------------------------------------
//	Unbind( INT Id )
//
//	Disassociates a USound object from a unique Id (as sent to PlaySound).
//-----------------------------------------------------------------------------
void USound::UnbindAllStreams( void )
{
//	debugf(NAME_DevSound, TEXT("UnbindAllStreams()"));

	PlayingStreams.Empty();
}

//-----------------------------------------------------------------------------
//	IsRegistered( void )
//
//	Returns true if the sound is registered. 
//
//	Streaming sounds are considered registered if they have an unused
//	stream available in the OpenStreams array.  Non-streaming sounds
//	are registered if their handle is non-zero.
//-----------------------------------------------------------------------------
UBOOL USound::IsRegistered( void )
{
	UBOOL	Ret = false;

	//
	if (CoreFlags & SF_Streaming)
	{
		for (int i = 0; !Ret && i < OpenStreams.Num(); i++)
			Ret |= !(OpenStreams(i) & 1);
	}
	else
	{
		Ret = Handle != 0;
	}

//	debugf(NAME_DevSound, TEXT("IsRegistered() returns %d"), Ret);

	return Ret;
}


//-----------------------------------------------------------------------------
//	SetHandle( INT iHandle )
//-----------------------------------------------------------------------------
void USound::SetHandle( INT iHandle )
{
	if (CoreFlags & SF_Streaming)
	{
	}
	else
	{
		Handle = iHandle;
	}
}


//-----------------------------------------------------------------------------
//	AddStreamHandle( INT iHandle )
//-----------------------------------------------------------------------------
void USound::AddStreamHandle( INT iStreamID )
{
	OpenStreams.Add();
	OpenStreams(OpenStreams.Num() - 1) = iStreamID * 2;

//	debugf(NAME_DevSound, TEXT("AddStreamHandle() - i = %d, iStreamID = %d"), OpenStreams.Num() - 1, iStreamID);
}


//-----------------------------------------------------------------------------
//	DeleteStreamHandle( INT iHandle )
//-----------------------------------------------------------------------------
void USound::DeleteStreamHandle( INT iStreamID )
{
	INT		iComp = iStreamID * 2;
	UBOOL	Done = false;

//	debugf(NAME_DevSound, TEXT("DeleteStreamHandle()"));

	for (int i = 0; !Done && i < OpenStreams.Num(); i++)
	{
		if (iComp == (OpenStreams(i) & ~1))
		{
			if (iComp != OpenStreams(i))
				debugf(NAME_DevSound, TEXT("Deleteing a bound stream!"));

//			debugf(NAME_DevSound, TEXT("DeleteStreamHandle() iStreamID =  %d, i = %d, OpenStreams(i) = %d"), iStreamID, i, OpenStreams(i));

			OpenStreams.Remove(i);
			Done = true;
		}
	}
}


//-----------------------------------------------------------------------------
//	DeleteStreamHandle( INT iHandle )
//-----------------------------------------------------------------------------
void USound::DeleteAllStreamHandles( void )
{
//	debugf(NAME_DevSound, TEXT("DeleteAllStreamHandles()"));
	OpenStreams.Empty();
}


//-----------------------------------------------------------------------------
//	GetHandle( INT Id )
//
//	Sets the handle of a sound associated with an Id.  Returns 0 if no stream
//	exists for Id.
//-----------------------------------------------------------------------------
INT	USound::GetHandle( INT Id )
{
	INT	Ret;

	if (CoreFlags & SF_Streaming)
	{
		//	FindRef will return 0 if the mapping is not found, therefore so will we.
		//
		int	iOpenStream = PlayingStreams.FindRef(Id);

		Ret = iOpenStream / 2;
	}
	else
	{
		Ret = Handle;
	}

	return Ret;
}


//-----------------------------------------------------------------------------
//	ImportPostProcess( Filename, Parms )
//
//	Allows for post processing after import
//-----------------------------------------------------------------------------
void USound::ImportPostProcess( const TCHAR* Filename, const TCHAR* Parms )
{
	//	We're going to save this now instead of calculating at load time.
	//
	Duration = Data.GetPeriod();

	FString	Compress;
	FString	LipSync;

	Parse( Parms, TEXT("Compress="), Compress );
	Parse( Parms, TEXT("Lipsync="), LipSync );

	//	Create lip sync data.
	//
	//	NOTE:  Do this before we compress the data.
	//
	if (LipSync == TEXT("lipsync"))
	{
//		debugf(NAME_Log, TEXT(" LipSync filename = %s"), Filename);
		CalcLipSyncData();
	}

	//	*** IMPORTANT!!! ***
 	//
	//	This will replace the WAV data with XA-compressed data, so make sure
	//	all processing that requires the native WAV data is done before calling
	//	CompressXA()!
	//
	if (Compress == TEXT("Compress"))
	{
		CompressXA(Filename);
	}

	return;
}


//-----------------------------------------------------------------------------
//	CalcLipSyncData( )
//
//	Post processing after import
//-----------------------------------------------------------------------------
UBOOL USound::CalcLipSyncData( void )
{
	UBOOL	bRet = false;
	
	//	Make sure the wav data is loaded
	//
	Data.Load();

	//	Get the WAV data info
	//
	FWaveModInfo	WavData;
	WavData.ReadWaveInfo(Data);

	//	Get the sample data from the WAV
	//
	int		iNumAmplitudeSamples = (int)(Duration / FLipSyncData::fAmpSampleInterval);

	//	Reserve memory for the array.  Add a little pad.
	//
	LipSyncData.Set(iNumAmplitudeSamples + 1);

	//	debugf(NAME_Log, TEXT("      CalcLipSyncData"));
	//	Get the amplitude info
	//
	int		iIndex;
	float	fTime;
	for (iIndex = 0, fTime = 0.0; iIndex < iNumAmplitudeSamples; iIndex++, fTime += FLipSyncData::fAmpSampleInterval)
	{
		LipSyncData(iIndex) = WavData.GetSampleWindowAmplitude(fTime, FLipSyncData::fAmpSampleWindowSize);
	//	debugf(NAME_Log, TEXT(" LipSyncData(iIndex) = %i, time = %i"), LipSyncData(iIndex), (int)(fTime * 1000));
	}

	LipSyncData(iIndex) = WavData.GetSampleWindowAmplitude(Duration, FLipSyncData::fAmpSampleWindowSize, Sample_window_leading);
//	debugf(NAME_Log, TEXT(" LipSyncData(iIndex) = %i, time = %i"), LipSyncData(iIndex), (int)(fTime * 1000));

	CoreFlags |= SF_HasLipSync;

	bRet = true;

	return bRet;
}


//-----------------------------------------------------------------------------
//	CompressXA
//
//	Post processing after import
//-----------------------------------------------------------------------------
UBOOL USound::CompressXA( const TCHAR* SourceFilename )
{
	UBOOL	bRet = false;

	Data.Load();

	FWaveModInfo	WaveModInfo;
	WaveModInfo.ReadWaveInfo(Data);

#if 1
	//	We don't want to compress sounds under a particular size.  Bail here
	//	if need be.
	//
	//	Calculation considers the smallest compressable file + the amount of
	//	space consumed by the compressed data (figure 8:1)
	const int	iSmallestCompressableFileK = 32;
	if (Data.Num() < iSmallestCompressableFileK * 1024 + (iSmallestCompressableFileK / 8))
	{
		Data.Unload();
		return false;
	}
#endif

	//	These fields used when using raw data
	//
	raw_NumSamples = WaveModInfo.SampleDataSize / WaveModInfo.SampleSize();
	raw_BitsPerSample = *WaveModInfo.pBitsPerSample;
	raw_NumChannels = *WaveModInfo.pChannels;
	raw_SampleRate = *WaveModInfo.pSamplesPerSec;

	Data.Unload();

	FString		strSrcSampleRate;

	switch (*WaveModInfo.pSamplesPerSec)
	{
	case 16000:
		strSrcSampleRate = TEXT("16");
		break;
		
	case 22050:
		strSrcSampleRate = TEXT("22.05");
		break;
		
	case 32000:
		strSrcSampleRate = TEXT("32");
		break;
		
	case 44100:
		strSrcSampleRate = TEXT("44.1");
		break;

	default:
		strSrcSampleRate = TEXT("");
		break;
	}

	if (strSrcSampleRate == TEXT(""))
	{
		GWarn->Logf( TEXT("Sample rate not supported:  file = %s, rate = %d"), SourceFilename, *WaveModInfo.pSamplesPerSec );
	}
	else
	{
		TCHAR	acOutfile[1024];
		TCHAR	acExecCmdLine[1024];

		//	Create the name of the output XA file
		//
		//	NOTE:  Relies on the fact that this function returns a pointer within the
		//	source string.  Dirty dirty dirty.  Sorry.  -tg
		//
		appStrcpy(acOutfile, SourceFilename);

		TCHAR	*pc = (TCHAR *)appFExt(acOutfile);
		if (pc)
			*(pc - 1) = '\0';
		appStrcat(acOutfile, TEXT(".XA"));

		void* hProcess = NULL;
#ifdef _WINDOWS
		//	Build the TooLame command line.  Compressing to XA.
		//		
	//	appSprintf(acExecCmdLine, TEXT("TooLame -s %s -b 64 %s %s"), *strSrcSampleRate, SourceFilename, acOutfile);
		appSprintf(acExecCmdLine, TEXT("sx -eaxa_blk -raw  %s -=%s"), SourceFilename, acOutfile);

		//	Spawn the TooLame process and weit until it's done.
		//
		hProcess = appCreateProc(acExecCmdLine, TEXT(""), false);
#else
		return false;
#endif

		INT	iRet;
		while (!appGetProcReturnCode(hProcess, &iRet))
			appSleep(0.1);

		//	Load the compressed data back into the Data array
		//
		Data.Unload();
		appLoadFileToArray(Data, acOutfile);

#if 1
		//	Delete the compressed file now that we're done with it.
		//
		//	NOTE: Wintel-specific command
		//
		FString	strFile = FString(TEXT("\"")) + FString(acOutfile) + FString(TEXT("\""));

#ifdef _WINDOWS
		hProcess = appCreateProc(TEXT("cmd /c del /q"), *strFile, false);
#else
		check(0);
#endif	

		while (!appGetProcReturnCode(hProcess, &iRet))
			appSleep(0.1);
#endif

		//	Mark this file as compressed
		//
		CoreFlags |= SF_Compressed | SF_Streaming;
		FileType = FName(TEXT("XA"));

		bRet = true;
	}

	return bRet;
}


UAudioSubsystem* USound::Audio;
void USound::PS2Convert()
{
	guard(USound::PS2Convert);

	#if HAVE_VAG

	/*
	 * Convert a standard wave sound to PS2 format.
	 *
	 * 3 Steps:
	 * - Promote 8 bit sounds to 16 bit for ADPCM compression.
	 * - Perform VAG conversion.
	 */

	Data.Load();

	FWaveModInfo WavData;
	WavData.ReadWaveInfo( Data );

	// Check loop.
	INT SoundLoops = WavData.SampleLoopsNum;

	// GWarn->Logf( TEXT("Sound: %s Size: %i Depth: %i Rate: %i"), GetPathName(), WavData.SampleDataSize, *WavData.pBitsPerSample, *WavData.pSamplesPerSec );
	// Copy the sample data into a working buffer.
	// Convert 8 bit samples to 16 bit data as we go.
	TArray<BYTE> WorkData;

	INT NewDataSize = WavData.SampleDataSize;
	INT SoundRate = *WavData.pSamplesPerSec;

	FLOAT Period = 0.f;
	INT DurDiv =  *WavData.pChannels * *WavData.pBitsPerSample * *WavData.pSamplesPerSec;  
	if ( DurDiv )
		Period = *WavData.pWaveDataSize * 8.f / (FLOAT) DurDiv;

	guard(Promote8to16);
	if (*WavData.pBitsPerSample == 8)
	{
		WorkData.Add( NewDataSize*2 );
		for (INT i=0; i<NewDataSize; i++)
		{
			WorkData(i*2)	= 0;
			WorkData(i*2+1)	= WavData.SampleDataStart[i] + 128;
		}
	} else {
		WorkData.Add( NewDataSize );
		for (INT i=0; i<NewDataSize; i++)
			WorkData(i) = WavData.SampleDataStart[i];
	}
	unguard;

	EncVagInit( ENC_VAG_MODE_NORMAL );

	INT WorkSamples = WorkData.Num() / 2;
	INT NeededBlocks = WorkSamples / 28;
	if (WorkSamples % 28 != 0)
		NeededBlocks++;

	// Prepare output data.
	Data.Empty();

	// Add custom PS2 header.
	guard(CustomHeader);
	Data.Add( 16 );
	appMemcpy( &Data(0), &SoundRate, sizeof(INT) );
	appMemcpy( &Data(4), &Period, sizeof(FLOAT) );
	appMemcpy( &Data(8), &SoundLoops, sizeof(INT) );
	INT Pad=0;
	appMemcpy( &Data(12), &Pad, sizeof(INT) );
	unguard;

	// Encode one VAG block per every 28 samples.
	guard(EncodeVAGBlocks);
	INT ByteCount = 0;
	for (INT i=0; i<NeededBlocks; i++)
	{
		TArray<BYTE> WorkBlock;
		WorkBlock.Add( 28*2 ); // Add 28 samples worth of room.
		for (INT j=0; j<28*2; j++)
		{
			if (ByteCount < WorkData.Num())
				WorkBlock(j) = WorkData(ByteCount++);
			else
				WorkBlock(j) = 0; // Pad with zero.
		}
		INT BlockAttribute;
		if (SoundLoops)
		{
			if (i == 0)
				BlockAttribute = ENC_VAG_LOOP_START;
			else if (i+1 == NeededBlocks)
				BlockAttribute = ENC_VAG_LOOP_END;
			else
				BlockAttribute = ENC_VAG_LOOP_BODY;
		} else {
			if (i+1 == NeededBlocks)
				BlockAttribute = ENC_VAG_1_SHOT_END;
			else
				BlockAttribute = ENC_VAG_1_SHOT;
		}

		TArray<BYTE> EncodedBlock;
		EncodedBlock.Add( 16 );
		EncVag( (short*) &WorkBlock(0), (short*) &EncodedBlock(0), BlockAttribute );

		INT StartPosition = Data.Num();
		Data.Add( 16 );
		for( INT j=0; j<16; j++)
			Data(StartPosition+j) = EncodedBlock(j);
	}
	unguard;

	// Finish off the one shot conversion.
	if (SoundLoops == 0)
	{
		guard(FinishOneShot);
		INT StartPosition = Data.Num();
		Data.Add( 16 );
		EncVagFin( (short*) &Data(StartPosition) );
		unguard;
	}

	// Set the file type.
	FileType = FName( TEXT("PS2") );

	/*
	for (INT i=0; i<Data.Num()/16; i++)
	{
		GWarn->Logf( TEXT("%02x%02x %02x%02x %02x%02x %02x%02x %02x%02x %02x%02x %02x%02x %02x%02x"),
			Data(0 + i*16),  Data(1 + i*16),  Data(2 + i*16),  Data(3 + i*16),
			Data(4 + i*16),  Data(5 + i*16),  Data(6 + i*16),  Data(7 + i*16),
			Data(8 + i*16),  Data(9 + i*16),  Data(10 + i*16), Data(11 + i*16),
			Data(12 + i*16), Data(13 + i*16), Data(14 + i*16), Data(15 + i*16) );
	}
	*/

	#endif

	unguard;
};
IMPLEMENT_CLASS(USound);

/*-----------------------------------------------------------------------------
	WaveModInfo implementation - downsampling of wave files.
-----------------------------------------------------------------------------*/

//
//	Figure out the WAVE file layout.
//
UBOOL FWaveModInfo::ReadWaveInfo( TArray<BYTE>& WavData )
{
	guard(FWaveModInfo::ReadWaveInfo);

	FFormatChunk* FmtChunk;
	FRiffWaveHeader* RiffHdr = (FRiffWaveHeader*)&WavData(0);
	WaveDataEnd = &WavData(0) + WavData.Num();	
	
	// Verify we've got a real 'WAVE' header.
	if( RiffHdr->wID != ( mmioFOURCC('W','A','V','E') )  )
		return 0;

	pMasterSize = &RiffHdr->ChunkLen;

	// The appMemcpy indirect accessing is necessary to avoid PSX2 read alignment problems but
	// somehow causes a crash on PCs in "Look for a 'smpl' chunk."

#if __PSX2_EE__

	FRiffChunkOld RiffChunk;
	FRiffChunkOld* RiffChunkPtr = (FRiffChunkOld*)&WavData(3*4);
	appMemcpy( &RiffChunk, RiffChunkPtr, sizeof(FRiffChunkOld) );

	// Look for the 'fmt ' chunk.
	while( ( ((BYTE*)RiffChunkPtr + 8) < WaveDataEnd)  && ( RiffChunk.ChunkID != mmioFOURCC('f','m','t',' ') ) )
	{
		// Go to next chunk.
		RiffChunkPtr = (FRiffChunkOld*) ( (BYTE*)RiffChunkPtr + Pad16Bit(RiffChunk.ChunkLen) + 8);
		appMemcpy( &RiffChunk, RiffChunkPtr, sizeof(FRiffChunkOld) );
	}
	// Chunk found ?
	if( RiffChunk.ChunkID != mmioFOURCC('f','m','t',' ') )
		return 0;

	FmtChunk = (FFormatChunk*)((BYTE*)RiffChunkPtr + 8);
	pBitsPerSample  = &FmtChunk->wBitsPerSample;
	pSamplesPerSec  = &FmtChunk->nSamplesPerSec;
	pAvgBytesPerSec = &FmtChunk->nAvgBytesPerSec;
	pBlockAlign		= &FmtChunk->nBlockAlign;
	pChannels       = &FmtChunk->nChannels;

	// re-initalize the RiffChunk pointer
	RiffChunkPtr = (FRiffChunkOld*)&WavData(3*4);
	appMemcpy( &RiffChunk, RiffChunkPtr, sizeof(FRiffChunkOld) );

	// Look for the 'data' chunk.
	while( ( ((BYTE*)RiffChunkPtr + 8) < WaveDataEnd) && ( RiffChunk.ChunkID != mmioFOURCC('d','a','t','a') ) )
	{
		// Go to next chunk.
		RiffChunkPtr = (FRiffChunkOld*) ( (BYTE*)RiffChunkPtr + Pad16Bit(RiffChunk.ChunkLen) + 8); 
		appMemcpy( &RiffChunk, RiffChunkPtr, sizeof(FRiffChunkOld) );
	} 
	// Chunk found ?
	if( RiffChunk.ChunkID != mmioFOURCC('d','a','t','a') )
		return 0;

	SampleDataStart = (BYTE*)RiffChunkPtr + 8;
	pWaveDataSize   = &RiffChunkPtr->ChunkLen;
	SampleDataSize  =  RiffChunk.ChunkLen;
	OldBitsPerSample = FmtChunk->wBitsPerSample;
	SampleDataEnd   =  SampleDataStart+SampleDataSize;

	NewDataSize	= SampleDataSize;

	// Re-initalize the RiffChunk pointer
	RiffChunkPtr = (FRiffChunkOld*)&WavData(3*4);
	appMemcpy( &RiffChunk, RiffChunkPtr, sizeof(FRiffChunkOld) );

	// Look for a 'smpl' chunk.
	while( ( (((BYTE*)RiffChunkPtr) + 8) < WaveDataEnd) && ( RiffChunk.ChunkID != mmioFOURCC('s','m','p','l') ) )
	{
		// Go to next chunk.
		RiffChunkPtr = (FRiffChunkOld*) ( (BYTE*)RiffChunkPtr + Pad16Bit(RiffChunk.ChunkLen) + 8); 
		appMemcpy( &RiffChunk, RiffChunkPtr, sizeof(FRiffChunkOld) );
	} 

	// Chunk found ? smpl chunk is optional.
	// Find the first sample-loop structure, and the total number of them.
	if( (BYTE*)RiffChunkPtr + 4 < WaveDataEnd && RiffChunk.ChunkID == mmioFOURCC('s','m','p','l') )
	{
		FSampleChunk pSampleChunk;
		appMemcpy(&pSampleChunk, (BYTE*)RiffChunkPtr + 8, sizeof(FSampleChunk));
		SampleLoopsNum = pSampleChunk.cSampleLoops;
		pSampleLoop = (FSampleLoop*) ((BYTE*)RiffChunkPtr + 8 + sizeof(FSampleChunk)); 
		/*
		FSampleChunk* pSampleChunk =  (FSampleChunk*)( (BYTE*)RiffChunk + 8);
		SampleLoopsNum  = pSampleChunk->cSampleLoops; // Number of tSampleLoop structures.
		// First tSampleLoop structure starts right after the tSampleChunk.
		pSampleLoop = (FSampleLoop*) ((BYTE*)pSampleChunk + sizeof(FSampleChunk)); 
		*/		
	}

#else
	
	FRiffChunkOld* RiffChunk = (FRiffChunkOld*)&WavData(3*4);
	// Look for the 'fmt ' chunk.
	while( ( ((BYTE*)RiffChunk + 8) < WaveDataEnd)  && ( RiffChunk->ChunkID != mmioFOURCC('f','m','t',' ') ) )
	{
		// Go to next chunk.
		RiffChunk = (FRiffChunkOld*) ( (BYTE*)RiffChunk + Pad16Bit(RiffChunk->ChunkLen) + 8); 
	}
	// Chunk found ?
	if( RiffChunk->ChunkID != mmioFOURCC('f','m','t',' ') )
		return 0;

	FmtChunk = (FFormatChunk*)((BYTE*)RiffChunk + 8);
	pBitsPerSample  = &FmtChunk->wBitsPerSample;
	pSamplesPerSec  = &FmtChunk->nSamplesPerSec;
	pAvgBytesPerSec = &FmtChunk->nAvgBytesPerSec;
	pBlockAlign		= &FmtChunk->nBlockAlign;
	pChannels       = &FmtChunk->nChannels;

	//GWarn->Logf( TEXT("look for data chunk") );
	// re-initalize the RiffChunk pointer
	RiffChunk = (FRiffChunkOld*)&WavData(3*4);
	// Look for the 'data' chunk.
	while( ( ((BYTE*)RiffChunk + 8) < WaveDataEnd) && ( RiffChunk->ChunkID != mmioFOURCC('d','a','t','a') ) )
	{
		// Go to next chunk.
		RiffChunk = (FRiffChunkOld*) ( (BYTE*)RiffChunk + Pad16Bit(RiffChunk->ChunkLen) + 8); 
	} 
	// Chunk found ?
	if( RiffChunk->ChunkID != mmioFOURCC('d','a','t','a') )
		return 0;

	SampleDataStart = (BYTE*)RiffChunk + 8;
	pWaveDataSize   = &RiffChunk->ChunkLen;
	SampleDataSize  =  RiffChunk->ChunkLen;
	OldBitsPerSample = FmtChunk->wBitsPerSample;
	SampleDataEnd   =  SampleDataStart+SampleDataSize;

	NewDataSize	= SampleDataSize;

	//GWarn->Logf( TEXT("look for smpl chunk 0x%x"), RiffChunk );
	// Re-initalize the RiffChunk pointer
	RiffChunk = (FRiffChunkOld*)&WavData(3*4);
	// Look for a 'smpl' chunk.
	while( ( (((BYTE*)RiffChunk) + 8) < WaveDataEnd) && ( RiffChunk->ChunkID != mmioFOURCC('s','m','p','l') ) )
	{
		// Go to next chunk.
		//GWarn->Logf( TEXT("go to next\n") );
		RiffChunk = (FRiffChunkOld*) ( (BYTE*)RiffChunk + Pad16Bit(RiffChunk->ChunkLen) + 8); 
		//GWarn->Logf( TEXT("riff: 0x%x\n"), RiffChunk );
		//GWarn->Logf( TEXT("%i"), RiffChunk->ChunkID );
	} 

	// Chunk found ? smpl chunk is optional.
	// Find the first sample-loop structure, and the total number of them.
	// GWarn->Logf( TEXT("find sample-loop 0x%x"), RiffChunk );
	if( (BYTE*)RiffChunk + 4 < WaveDataEnd && RiffChunk->ChunkID == mmioFOURCC('s','m','p','l') )
	{
		//GWarn->Logf( TEXT("loop") );
		FSampleChunk pSampleChunk;
		appMemcpy(&pSampleChunk, (BYTE*)RiffChunk + 8, sizeof(FSampleChunk));
		SampleLoopsNum = pSampleChunk.cSampleLoops;
		pSampleLoop = (FSampleLoop*) ((BYTE*)RiffChunk + 8 + sizeof(FSampleChunk)); 
		/*
		FSampleChunk* pSampleChunk =  (FSampleChunk*)( (BYTE*)RiffChunk + 8);
		SampleLoopsNum  = pSampleChunk->cSampleLoops; // Number of tSampleLoop structures.
		// First tSampleLoop structure starts right after the tSampleChunk.
		pSampleLoop = (FSampleLoop*) ((BYTE*)pSampleChunk + sizeof(FSampleChunk)); 
		*/
	}
#endif
	
	return 1;
	unguard;
}

//
// Update internal variables and shrink the data fields.
//
UBOOL FWaveModInfo::UpdateWaveData( TArray<BYTE>& WavData )
{
	guard(FWaveModInfo::UpdateWaveData);
	if( NewDataSize < SampleDataSize )
	{		
		// Shrinkage of data chunk in bytes -> chunk data must always remain 16-bit-padded.
		INT ChunkShrinkage = Pad16Bit(SampleDataSize)  - Pad16Bit(NewDataSize);

		// Update sizes.
		*pWaveDataSize  = NewDataSize;
		*pMasterSize   -= ChunkShrinkage;

		// Refresh all wave parameters depending on bit depth and/or sample rate.
		*pBlockAlign    =  *pChannels * (*pBitsPerSample >> 3); // channels * Bytes per sample
		*pAvgBytesPerSec = *pBlockAlign * *pSamplesPerSec; //sample rate * Block align

		// Update 'smpl' chunk data also, if present.
		if (SampleLoopsNum)
		{
			FSampleLoop* pTempSampleLoop = pSampleLoop;
			INT SampleDivisor = ( (SampleDataSize *  *pBitsPerSample) / (NewDataSize ) );
			for (INT SL = 0; SL<SampleLoopsNum; SL++)
			{
				pTempSampleLoop->dwStart = pTempSampleLoop->dwStart  * OldBitsPerSample / SampleDivisor;
				pTempSampleLoop->dwEnd   = pTempSampleLoop->dwEnd  * OldBitsPerSample / SampleDivisor;
				pTempSampleLoop++; // Next TempSampleLoop structure.
			}	
		}		
			
		// Now shuffle back all data after wave data by SampleDataSize/2 (+ padding) bytes 
		// INT SampleShrinkage = ( SampleDataSize/4) * 2;
		BYTE* NewWaveDataEnd = SampleDataEnd - ChunkShrinkage;

		for ( INT pt = 0; pt< ( WaveDataEnd -  SampleDataEnd); pt++ )
		{ 
			NewWaveDataEnd[pt] =  SampleDataEnd[pt];
		}			

		// Resize the dynamic array.
		WavData.Remove( WavData.Num() - ChunkShrinkage, ChunkShrinkage );
		
		/*
		static INT SavedBytes = 0;
		SavedBytes += ChunkShrinkage;
		debugf(NAME_Log," Audio shrunk by: %i bytes, total savings %i bytes.",ChunkShrinkage,SavedBytes);
		debugf(NAME_Log," New BitsPerSample: %i ", *pBitsPerSample);
		debugf(NAME_Log," New SamplesPerSec: %i ", *pSamplesPerSec);
		debugf(NAME_Log," Olddata/NEW*wav* sizes: %i %i ", SampleDataSize, *pMasterSize);
		*/
	}

	// Noise gate filtering assumes 8-bit sound.
	// Warning: While very useful on SOME sounds, it erased too many low-volume sound fx
	// in its current form - even when 'noise' level scaled to average sound amplitude.
	// if (NoiseGate) NoiseGateFilter();

	return 1;
	unguard;
}

//
// Reduce bit depth and halve the number of samples simultaneously.
//
void FWaveModInfo::HalveReduce16to8()
{
	guard(FWaveModInfo::HalveReduce16to8);

	DWORD SampleWords =  SampleDataSize >> 1;
	INT OrigValue,NewValue;
	INT ErrorDiff = 0;

	DWORD SampleBytes = SampleWords >> 1;

	INT NextSample0 = (INT)((SWORD*) SampleDataStart)[0];
	INT NextSample1,NextSample2;

	BYTE* SampleData =  SampleDataStart;
	for (DWORD T=0; T<SampleBytes; T++)
	{
		NextSample1 = (INT)((SWORD*)SampleData)[T*2];
		NextSample2 = (INT)((SWORD*)SampleData)[T*2+1];
		INT Filtered16BitSample = 32768*4 + NextSample0 + NextSample1 + NextSample1 + NextSample2;
		NextSample0 = NextSample2;

		// Error diffusion works in '18 bit' resolution.
		OrigValue = ErrorDiff + Filtered16BitSample;
		// Rounding: "+0.5"
		NewValue  = (OrigValue + 512) & 0xfffffc00;
		if (NewValue > 0x3fc00) NewValue = 0x3fc00;

		INT NewSample = NewValue >> (8+2);
		SampleData[T] = (BYTE)NewSample;	// Output byte.

		ErrorDiff = OrigValue - NewValue;   // Error cycles back into input.
	}

	NewDataSize = SampleBytes;

	*pBitsPerSample  = 8;
	*pSamplesPerSec  = *pSamplesPerSec >> 1;

	NoiseGate = true;
	unguard;
}

//
// Reduce bit depth.
//
void FWaveModInfo::Reduce16to8()
{
	guard(FWaveModInfo::Reduce16to8);

	DWORD SampleBytes =  SampleDataSize >> 1;
	INT OrigValue,NewValue;
	INT ErrorDiff = 0;
	BYTE* SampleData = SampleDataStart;

	for (DWORD T=0; T<SampleBytes; T++)
	{
		// Error diffusion works in 16 bit resolution.
		OrigValue = ErrorDiff + 32768 + (INT)((SWORD*)SampleData)[T];
		// Rounding: '+0.5', then mask off low 2 bits.
		NewValue  = (OrigValue + 127 ) & 0xffffff00;  // + 128
		if (NewValue > 0xff00) NewValue = 0xff00;

		INT NewSample = NewValue >> 8;
		SampleData[T] = NewSample;

		ErrorDiff = OrigValue - NewValue;  // Error cycles back into input.
	}				

	NewDataSize = SampleBytes;
	*pBitsPerSample  = 8;
	NoiseGate = true;

	unguard;
}

//
// Halve the number of samples.
//
void FWaveModInfo::HalveData()
{
	guard(FWaveModInfo::HalveData);
	if( *pBitsPerSample == 16)
	{						
		DWORD SampleWords =  SampleDataSize >> 1;
		INT OrigValue,NewValue;
		INT ErrorDiff = 0;

		DWORD ScaledSampleWords = SampleWords >> 1; 

		INT NextSample0 = (INT)((SWORD*) SampleDataStart)[0];
		INT NextSample1, NextSample2;

		BYTE* SampleData =  SampleDataStart;
		for (DWORD T=0; T<ScaledSampleWords; T++)
		{
			NextSample1 = (INT)((SWORD*)SampleData)[T*2];
			NextSample2 = (INT)((SWORD*)SampleData)[T*2+1];
			INT Filtered18BitSample = 32768*4 + NextSample0 + NextSample1 + NextSample1 + NextSample2;
			NextSample0 = NextSample2;

			// Error diffusion works with '18 bit' resolution.
			OrigValue = ErrorDiff + Filtered18BitSample;
			// Rounding: '+0.5', then mask off low 2 bits.
			NewValue  = (OrigValue + 2) & 0x3fffc;
			if (NewValue > 0x3fffc) NewValue = 0x3fffc;
			((SWORD*)SampleData)[T] = (NewValue >> 2) - 32768;  // Output SWORD.
			ErrorDiff = OrigValue - NewValue; // Error cycles back into input.
		}				
		NewDataSize = (ScaledSampleWords * 2);
		*pSamplesPerSec  = *pSamplesPerSec >> 1;
	}
	else if( *pBitsPerSample == 8 )
	{									
		INT OrigValue,NewValue;
		INT ErrorDiff = 0;
	
		DWORD SampleBytes = SampleDataSize >> 1;  
		BYTE* SampleData = SampleDataStart;

		INT NextSample0 = SampleData[0];
		INT NextSample1, NextSample2;

		for (DWORD T=0; T<SampleBytes; T++)
		{
			NextSample1 =  SampleData[T*2];
			NextSample2 =  SampleData[T*2+1];
			INT Filtered10BitSample =  NextSample0 + NextSample1 + NextSample1 + NextSample2;
			NextSample0 =  NextSample2;

			// Error diffusion works with '10 bit' resolution.
			OrigValue = ErrorDiff + Filtered10BitSample;
			// Rounding: '+0.5', then mask off low 2 bits.
			NewValue  = (OrigValue + 2) & 0x3fc;
			if (NewValue > 0x3fc) NewValue = 0x3fc;
			SampleData[T] = (BYTE)(NewValue >> 2);	// Output BYTE.
			ErrorDiff = OrigValue - NewValue;		// Error cycles back into input.
		}				
		NewDataSize = SampleBytes; 
		*pSamplesPerSec  = *pSamplesPerSec >> 1;
	}
	unguard;
}

//
//	Noise gate filter. Hard to make general-purpose without hacking up some (soft) sounds.
//
void FWaveModInfo::NoiseGateFilter()
{
	guard(FWaveModInfo::NoiseGateFilter);

	BYTE* SampleData  =  SampleDataStart;
	INT   SampleBytes = *pWaveDataSize; 
	INT	  MinBlankSize = 860 * ((*pSamplesPerSec)/11025); // 600-800...

	// Threshold sound amplitude. About 18 seems OK.
	INT Amplitude	 = 18;

	// Ignore any over-threshold signals if under this size. ( 32 ?)
	INT GlitchSize	 = 32 * ((*pSamplesPerSec)/11025);
	INT StartSilence =  0;
	INT EndSilence	 =  0;
	INT LastErased   = -1;

	for( INT T = 0; T< SampleBytes; T++ )
	{
		UBOOL Loud;
		if	( Abs(SampleData[T]-128) >= Amplitude )
		{
			Loud = true;							
			if (StartSilence > 0)
			{
				if ( (T-StartSilence) < GlitchSize ) Loud = false;
			}							
		}
		else Loud = false;

		if( StartSilence == 0 )
		{
			if( !Loud )
				StartSilence = T;
		}
		else
		{													
			if( ((EndSilence == 0) && (Loud)) || (T ==(SampleBytes-1) ) )
			{
				EndSilence = T;
				//
				// Erase an area of low-amplitude sound ( noise... ) if size >= MinBlankSize.
				//
				// Todo: try erasing smoothly - decay, create some attack, 
				// proportional to the size of the area. ?
				//
				if	(( EndSilence - StartSilence) >= MinBlankSize )
				{					
					for ( INT Er = StartSilence; Er< EndSilence; Er++ )
					{
						SampleData[Er] = 128;
					}
				}
				LastErased = EndSilence-1;
				StartSilence = 0;
				EndSilence   = 0;
			}
		}										
	}
	unguard;
}


//-----------------------------------------------------------------------------
//	GetSampleWindowAmplitude( float fTime, float fWindowSize, eSampleWindowType SampleWindowType = Sample_window_trailing  )
//
//	Returns average sample value over a given time interval
//-----------------------------------------------------------------------------
DWORD FWaveModInfo::GetSampleWindowAmplitude( float fTime, float fWindowSize, eSampleWindowType SampleWindowType )
{
	FLOAT	fBufferStartTime;
	FLOAT	fBufferEndTime;

	switch (SampleWindowType)
	{
	default:
	case Sample_window_trailing:
		fBufferStartTime = fTime;
		fBufferEndTime = fTime + fWindowSize;
		break;

	case Sample_window_leading:
		fBufferStartTime = fTime - fWindowSize;
		fBufferEndTime = fTime;
		break;

	case Sample_window_centered:
		fBufferStartTime = fTime - (fWindowSize / 2);
		fBufferEndTime = fTime + (fWindowSize / 2);
		break;
	}

	BYTE	*pcBuffer = OffsetFromTime(fBufferStartTime);
	BYTE	*pcBufferEnd = OffsetFromTime(fBufferEndTime);

#if 1	// PEAK
	DWORD	dwMax = 0;

	while (pcBuffer < pcBufferEnd)
	{
		BYTE	bSampleAmp = SampleAmplitude(pcBuffer);
		
		if (bSampleAmp > dwMax)
			dwMax = bSampleAmp;

		pcBuffer += SampleSize();
	}

	return dwMax;
#endif

#if 0	// RMS
	int		iNumSamples = (pcBufferEnd - pcBuffer) / SampleSize();
	DWORD	dwSqAvg = 0;

	while (pcBuffer < pcBufferEnd)
	{
		DWORD	dwSampleAmp = SampleAmplitude(pcBuffer);
		
		dwSqAvg += dwSampleAmp * dwSampleAmp;
		pcBuffer += SampleSize();
	}

	dwSqAvg /= iNumSamples;
	dwSqAvg = sqrt(dwSqAvg);

	return dwSqAvg;
#endif

#if 0	// AVERAGE
	int	iNumSamples = (pcBufferEnd - pcBuffer) / SampleSize();

	DWORD	dwAvg = 0;

	while (pcBuffer < pcBufferEnd)
	{
		dwAvg += SampleAmplitude(pcBuffer);
		pcBuffer += SampleSize();
	}

	dwAvg /= iNumSamples;

	return dwAvg;
#endif

}


//-----------------------------------------------------------------------------
//	OffsetFromTime( float fTime )
//
//	Returns a pointer into a sample buffer at a given time
//-----------------------------------------------------------------------------
BYTE *FWaveModInfo::OffsetFromTime( float fTime )
{
	int		iBytesPerSample = (*pBitsPerSample + 7) / 8;

	BYTE	*pbRet = SampleDataStart + (int)((*pSamplesPerSec * *pChannels * iBytesPerSample) * fTime);

	//	Make sure the pointer we return is aligned on the proper boundary
	//
	if (iBytesPerSample > 1)
		pbRet = reinterpret_cast<BYTE*>( reinterpret_cast<UPTRINT>(pbRet) & ~UPTRINT(1) );

	return pbRet;
}


//-----------------------------------------------------------------------------
//	SampleValue( BYTE *pcBuffer )
//
//	Returns value of wav sample pointer to by pcBuffer.  Averages samples
//	if stereo.
//-----------------------------------------------------------------------------
BYTE FWaveModInfo::SampleAmplitude( BYTE *pcBuffer )
{
	BYTE	bAmp;

	if (*pBitsPerSample <= 8)
	{
		if (*pChannels == 1)
		{
			bAmp = *pcBuffer;
		}
		else
		{
			bAmp = (*pcBuffer + *(pcBuffer + 1)) / 2;
		}

		//	Silence in an 8-bit sample is 128 (0x80)
		//
		if (bAmp < 128)
			bAmp = 128 - bAmp;
		else
			bAmp -= 128;
	}
	else	// 16-bit
	{
		//	Scale the amplitude to fit in one byte.  To do this, divide by 128.  Note that
		//	for a stereo sample, we divide by 256, the factor of 128 for the scaling and
		//	the factor of 2 for the averaging of the stereo sample.
		//
		if (*pChannels == 1)
			bAmp = abs(*(SWORD *)pcBuffer) / 128;
		else
			bAmp = ( abs(*(SWORD *)pcBuffer) + abs(*( ((SWORD *)pcBuffer) + 1)) ) / 256;
	}

	return bAmp;
}


//-----------------------------------------------------------------------------
//	SampleSize( )
//
//	Returns size of a sample for this wav
//-----------------------------------------------------------------------------
int FWaveModInfo::SampleSize(  )
{
	int	iBytesPerSample = (*pBitsPerSample + 7) / 8;

	return *pChannels * iBytesPerSample;
}



//-----------------------------------------------------------------------------
//
//	class FLipSyncData
//
//-----------------------------------------------------------------------------
FLipSyncData::FLipSyncData( USound* InOwner ) : 
	Owner(InOwner)
{
	m_iMaxAmplitude = -1;
}


void FLipSyncData::Load()
{
	guard(FLipSyncData::Load);
//	UBOOL Loaded = SavedPos > 0;

	guard(0);
	TLazyArray<BYTE>::Load();
	unguard;

	unguard;
}


//-----------------------------------------------------------------------------
//
//-----------------------------------------------------------------------------
FLOAT FLipSyncData::GetMouthPos( FLOAT fTime )
{
	const FLOAT kfAnimEndRange = 0.9;
	const FLOAT kfMuteFactor = 0.7;

	BYTE	bAmp = GetInterpAmplitude(fTime);
	BYTE	bMaxAmp = GetMaxAmplitude();

	return ((FLOAT)bAmp / (FLOAT)bMaxAmp) * kfAnimEndRange * kfMuteFactor;
}


//-----------------------------------------------------------------------------
//
//-----------------------------------------------------------------------------
BYTE FLipSyncData::GetInterpAmplitude( FLOAT fTime )
{
	BYTE	bAmplitude;

	//	Make sure we're not overshooting the duration of the sound
	//
	fTime = Clamp<FLOAT>(fTime, 0.0, Owner->Duration);

	//	If this represents the last amplitude sample, just return it.
	//	Otherwise calculate and return a weighted average.
	//
	if (fTime + FLipSyncData::fAmpSampleInterval > Owner->Duration)
	{
		INT	iDataIndex = appFloor(fTime / fAmpSampleInterval);
		iDataIndex = Clamp<INT>(iDataIndex, 0, Num() - 1);
		bAmplitude = Data[iDataIndex];
	}
	else
	{
		INT		iIndexStart	= appFloor(fTime / fAmpSampleInterval);
		INT		iIndexEnd	= appFloor((fTime + fAmpSampleInterval) / fAmpSampleInterval);
		FLOAT	fWeight		= appFmod(fTime, FLipSyncData::fAmpSampleInterval);

		//	Weighted average is as follows:
		//
		//	[RangeStart] * (sample interval - weight) + [RangeEnd] * weight.
		//
		bAmplitude = (BYTE) ((( FLOAT(Data[iIndexStart]) * (fAmpSampleInterval - fWeight) + 
							  FLOAT(Data[iIndexEnd])   *  fWeight )) / fAmpSampleInterval);
	}

	return bAmplitude;
}


//-----------------------------------------------------------------------------
//
//-----------------------------------------------------------------------------
BYTE FLipSyncData::GetMaxAmplitude( void )
{
	if (m_iMaxAmplitude < 0)
		CalcMaxAmplitude();

	return (BYTE)m_iMaxAmplitude;
}


//-----------------------------------------------------------------------------
//
//-----------------------------------------------------------------------------
void FLipSyncData::CalcMaxAmplitude( void )
{
	m_iMaxAmplitude = 0;

	for (int i = Num(); i-- > 0; )
		if (Data[i] > m_iMaxAmplitude)
			m_iMaxAmplitude = Data[i];

	return;
}


/*-----------------------------------------------------------------------------
	UAudioSubsystem implementation.
-----------------------------------------------------------------------------*/

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

