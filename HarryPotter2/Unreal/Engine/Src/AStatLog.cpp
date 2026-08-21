/*=============================================================================
	AStatLog.cpp: Unreal Tournament stat logging.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include <cstdint>

struct FStatLogRuntimeState
{
	AStatLogFile* Owner;
	FMD5Context* Context;
	FArchive* LogAr;
};

static TArray<FStatLogRuntimeState> GStatLogRuntimeStates;

static FStatLogRuntimeState* FindStatLogRuntimeState( AStatLogFile* Owner )
{
	for( INT Index = 0; Index < GStatLogRuntimeStates.Num(); ++Index )
		if( GStatLogRuntimeStates(Index).Owner == Owner )
			return &GStatLogRuntimeStates(Index);
	return NULL;
}

static FStatLogRuntimeState& GetStatLogRuntimeState( AStatLogFile* Owner )
{
	if( FStatLogRuntimeState* State = FindStatLogRuntimeState( Owner ) )
		return *State;

	const INT Index = GStatLogRuntimeStates.AddZeroed();
	GStatLogRuntimeStates(Index).Owner = Owner;
	return GStatLogRuntimeStates(Index);
}

static void RemoveStatLogRuntimeState( AStatLogFile* Owner )
{
	for( INT Index = 0; Index < GStatLogRuntimeStates.Num(); ++Index )
	{
		if( GStatLogRuntimeStates(Index).Owner == Owner )
		{
			GStatLogRuntimeStates.Remove( Index );
			return;
		}
	}
}

static void CloseStatLogRuntimeState( AStatLogFile* Owner )
{
	if( FStatLogRuntimeState* State = FindStatLogRuntimeState( Owner ) )
	{
		delete State->Context;
		delete State->LogAr;
		RemoveStatLogRuntimeState( Owner );
	}
}

static UBOOL AppendStatLogUtf16Unit( TArray<BYTE>& Bytes, std::uint16_t Unit )
{
	if( Bytes.Num() > MAXINT - 2 )
		return 0;
	Bytes.AddItem( static_cast<BYTE>(Unit) );
	Bytes.AddItem( static_cast<BYTE>(Unit >> 8) );
	return 1;
}

static UBOOL EncodeStatLogUtf16LE( const FString& Text, TArray<BYTE>& Bytes, UBOOL AppendLineEnding )
{
	Bytes.Empty();
	for( INT Index = 0; Index < Text.Len(); ++Index )
	{
		const TCHAR HostChar = (*Text)[Index];
		std::uint32_t CodePoint = HostChar >= 0
			? static_cast<std::uint32_t>(HostChar)
			: 0xfffdu;

		if( sizeof(TCHAR) == sizeof(UNICHAR) && CodePoint >= 0xd800u && CodePoint <= 0xdbffu )
		{
			if( Index + 1 < Text.Len() )
			{
				const std::uint32_t Low = static_cast<std::uint32_t>((*Text)[Index + 1]);
				if( Low >= 0xdc00u && Low <= 0xdfffu )
				{
					CodePoint = 0x10000u + ((CodePoint - 0xd800u) << 10) + (Low - 0xdc00u);
					++Index;
				}
				else
					CodePoint = 0xfffdu;
			}
			else
				CodePoint = 0xfffdu;
		}
		else if( (CodePoint >= 0xd800u && CodePoint <= 0xdfffu) || CodePoint > 0x10ffffu )
		{
			CodePoint = 0xfffdu;
		}

		if( CodePoint > 0xffffu )
		{
			CodePoint -= 0x10000u;
			if( !AppendStatLogUtf16Unit(Bytes, static_cast<std::uint16_t>(0xd800u + (CodePoint >> 10)))
				|| !AppendStatLogUtf16Unit(Bytes, static_cast<std::uint16_t>(0xdc00u + (CodePoint & 0x3ffu))) )
				return 0;
		}
		else if( !AppendStatLogUtf16Unit(Bytes, static_cast<std::uint16_t>(CodePoint)) )
		{
			return 0;
		}
	}

	if( AppendLineEnding )
		return AppendStatLogUtf16Unit(Bytes, '\r') && AppendStatLogUtf16Unit(Bytes, '\n');
	return 1;
}

void appMD5UpdateStringUtf16LE( FMD5Context* Context, const FString& Text, UBOOL AppendLineEnding )
{
	TArray<BYTE> Bytes;
	if( !EncodeStatLogUtf16LE(Text, Bytes, AppendLineEnding) )
		appErrorf( TEXT("UTF-16LE text exceeds the 32-bit MD5 contract") );
	if( Bytes.Num() )
		appMD5Update( Context, &Bytes(0), Bytes.Num() );
}

/*-----------------------------------------------------------------------------
	Stat Log Implementation.
-----------------------------------------------------------------------------*/

#if ENGINE_VERSION>=230
IMPLEMENT_CLASS(AMutator);
#endif

void AStatLog::execExecuteLocalLogBatcher( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execExecuteLocalLogBatcher);
	P_FINISH;

	appCreateProc( *LocalBatcherURL, *Level->Game->LocalLogFileName, (Level->NetMode != NM_DedicatedServer) );

	unguardexec;
}

void AStatLog::execExecuteSilentLogBatcher( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execExecuteSilentLogBatcher);
	P_FINISH;

	FString ProcArgs = FString::Printf( TEXT("-b false %s"), *Level->Game->LocalLogFileName );
	appCreateProc( *LocalBatcherURL, *ProcArgs, (Level->NetMode != NM_DedicatedServer) );

	unguardexec;
}

void AStatLog::execBatchLocal( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execBatchLocal);
	P_FINISH;

	appCreateProc( *(((AStatLog*)GetClass()->GetDefaultObject())->LocalBatcherURL), *(((AStatLog*)GetClass()->GetDefaultObject())->LocalLogDir), 1 );
	unguardexec;
}

void AStatLog::execBrowseRelativeLocalURL( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execBrowseRelativeLocalURL);
	P_GET_STR(URL);
	P_FINISH;

	appLaunchURL( *(GFileManager->GetDefaultDirectory() * URL) );

	unguardexec;
}

void AStatLog::execExecuteWorldLogBatcher( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execExecuteWorldLogBatcher);
	P_FINISH;
	check(Level->Game);
	if ( !Level->Game->bExternalBatcher )
	{	
		debugf( TEXT("ngWorldStats: ExecuteWorldLogBatcher") );
		static void* LogBatcherHandle = NULL;
		if( LogBatcherHandle )
		{
			INT ReturnCode = 0;
			if( !appGetProcReturnCode( LogBatcherHandle, &ReturnCode ) )
			{
				debugf( TEXT("ngWorldStats: Old batcher was still running or handle was invalid.") );
				// If the batcher was still running, just launch a new copy which will clobber the old one.
			}
			else
			{
				if( ReturnCode >= 0 )
					bWorldBatcherError = 0;
				else
				{
					debugf( TEXT("ngWorldStats: Previous batcher failed with return code %d."), ReturnCode );
					bWorldBatcherError = 1;
				}
			}
		}
		LogBatcherHandle = appCreateProc( *WorldBatcherURL, *WorldBatcherParams, (Level->NetMode != NM_DedicatedServer) );
		if( !LogBatcherHandle )
		{
			debugf( TEXT("ngWorldStats: Failed to launch batcher.") );
			bWorldBatcherError = 1;
		}
		debugf(TEXT("bWorldBatchError is %d"), bWorldBatcherError);
		Cast<AStatLog>(AStatLog::StaticClass()->GetDefaultObject())->bWorldBatcherError = bWorldBatcherError;
		SaveConfig();
	}
	unguardexec;
}


void AStatLog::execInitialCheck( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execInitialCheck);
	P_GET_OBJECT(AGameInfo, Game);
	P_FINISH;

	// Log the class in C++ to avoid trickery.
	eventLogGameSpecial(TEXT("GameClass"), Game->GetClass()->GetFullName());

	// Log all the loaded code packages and their checksums.
	TArray<UPackage*> Packages;
	for( TObjectIterator<UClass> It; It; ++It )
		Packages.AddUniqueItem(CastChecked<UPackage>((*It)->GetOuter()));
	for (INT i=0; i<Packages.Num(); i++)
	{
		// Get checksum values.
		FString FileName = FString::Printf( TEXT("%s.u"), Packages(i)->GetFullName() );
		INT Space = FileName.InStr(TEXT(" "));
		FileName = FileName.Right( FileName.Len() - (Space+1) );
		INT FileSize = GFileManager->FileSize( *FileName );

		// Promote lowercase character values (a cool way of saying CAPITALIZE)
		FString CapsName;
		for (INT j=0; j<FileName.Len(); j++)
		{
			TCHAR c = (*FileName)[j];
			if ((c >= 'a') && (c <= 'z'))
				c = c + ('A' - 'a');
			CapsName += FString::Printf( TEXT("%c"), c );
		}

		// Checksum the .u files.
		FString CheckString = CapsName + FString::Printf( TEXT("%i"), FileSize );
		if (FileSize != -1)
		{
			FMD5Context PContext;
			appMD5Init( &PContext );
			appMD5UpdateStringUtf16LE( &PContext, CheckString, 0 );
			BYTE Digest[16];
			appMD5Final( Digest, &PContext );
			FString Checksum;
			for (INT j=0; j<16; j++)
				Checksum += FString::Printf(TEXT("%02x"), Digest[j]);
			eventLogGameSpecial2(TEXT("CodePackageChecksum"), *FileName, *Checksum);
		}

		// Get checksum values.
		FileName = FString::Printf( TEXT("%s%s"), Packages(i)->GetFullName(), DLLEXT );
		Space = FileName.InStr(TEXT(" "));
		FileName = FileName.Right( FileName.Len() - (Space+1) );
		FileSize = GFileManager->FileSize( *FileName );

		// Capitalize.
		for (INT j=0; j<FileName.Len(); j++)
		{
			TCHAR c = (*FileName)[j];
			if ((c >= 'a') && (c <= 'z'))
				c = c + ('A' - 'a');
			CapsName += FString::Printf( TEXT("%c"), c );
		}

		// Checksum the .dll files.
		CheckString = CapsName + FString::Printf( TEXT("%i"), FileSize );
		if (FileSize != -1)
		{
			FMD5Context PContext;
			appMD5Init( &PContext );
			appMD5UpdateStringUtf16LE( &PContext, CheckString, 0 );
			BYTE Digest[16];
			appMD5Final( Digest, &PContext );
			FString Checksum;
			for (INT j=0; j<16; j++)
				Checksum += FString::Printf(TEXT("%02x"), Digest[j]);
			eventLogGameSpecial2(TEXT("CodePackageChecksum"), *FileName, *Checksum);
		}
	}

	unguardexec;
}

void AStatLog::execLogMutator( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execInitialCheck);
	P_GET_OBJECT(AMutator, M);
	P_FINISH;

	eventLogGameSpecial(TEXT("GameMutator"), M->GetClass()->GetFullName());

	unguardexec;
}

void AStatLog::execGetGMTRef( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execGetGMTRef);
	P_FINISH;

	*(FString*)Result = appGetGMTRef();

	unguardexec;
}

void AStatLog::execGetMapFileName( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execGetMapFileName);
	P_FINISH;

	*(FString*)Result = XLevel->URL.Map;

	unguardexec;
}

void AStatLog::execGetPlayerChecksum( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLog::execGetPlayerChecksum);
	P_GET_OBJECT(APlayerPawn, P);
	P_GET_STR_REF(Checksum);
	P_FINISH;

	if( P->ngWorldSecret.Len() == 0 )
		*Checksum = FString::Printf( TEXT("NoChecksum") );
	else
	{
		FMD5Context PContext;
		appMD5Init( &PContext );
		appMD5UpdateStringUtf16LE( &PContext, P->PlayerReplicationInfo->PlayerName, 0 );
		appMD5UpdateStringUtf16LE( &PContext, P->ngWorldSecret, 0 );
		BYTE Digest[16];
		appMD5Final( Digest, &PContext );
		*Checksum = FString::Printf( TEXT("") );
		for (INT i=0; i<16; i++)
			*Checksum += FString::Printf(TEXT("%02x"), Digest[i]);
	}
	unguardexec;
}

void AStatLogFile::execOpenLog( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execOpenLog);
	P_FINISH;

	CloseStatLogRuntimeState( this );
	FStatLogRuntimeState& Runtime = GetStatLogRuntimeState( this );
	GFileManager->MakeDirectory( TEXT("..") PATH_SEPARATOR TEXT("Logs") );
	Runtime.LogAr = GFileManager->CreateFileWriter( *StatLogFile, FILEWRITE_EvenIfReadOnly );
	if( bWorld )
	{
		Runtime.Context = new FMD5Context;
		appMD5Init( Runtime.Context );
	}
	unguardexec;
}

void AStatLogFile::execCloseLog( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execCloseLog);
	P_FINISH;

	CloseStatLogRuntimeState( this );

	GFileManager->Move( *StatLogFinal, *StatLogFile, 1, 1, 1 );

	unguardexec;
}

void AStatLogFile::execWatermark( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execWatermark);
	P_GET_STR(EventString);
	P_FINISH;

	// Update the context...
	FStatLogRuntimeState* Runtime = FindStatLogRuntimeState( this );
	check(Runtime && Runtime->Context);
	appMD5UpdateStringUtf16LE( Runtime->Context, EventString, 1 );

	unguardexec;
}

void AStatLogFile::execGetChecksum( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execGetChecksum);
	P_GET_STR_REF(Checksum);
	P_FINISH;

	FStatLogRuntimeState* Runtime = FindStatLogRuntimeState( this );
	check(Runtime && Runtime->Context);
	BYTE Secret[16];	// Must be bytes.  Used by MD5.
	Secret[0] = 'M';
	Secret[5] = 'p';
	Secret[2] = 'y';
	Secret[3] = 'f';
	Secret[1] = '4';
	Secret[11] = 'd';
	Secret[7] = '9';
	Secret[4] = 'G';
	Secret[12] = 'D';
	Secret[6] = '6';
	Secret[9] = 'e';
	Secret[10] = 'J';
	Secret[14] = '1';
	Secret[15] = 'q';
	Secret[8] = 'k';
	Secret[13] = 'V';

	BYTE Digest[16];

	appMD5Update( Runtime->Context, Secret, 16 );
	appMD5Final( Digest, Runtime->Context ); // Outputs a 16 byte digest.

	// Copy each byte into a string of arbitrary character size. (UNICODE safe.)
	INT i;
	for (i=0; i<16; i++) {
		*Checksum += FString::Printf(TEXT("%02x"), Digest[i]);
	}

	unguardexec;
}

void AStatLogFile::execFileFlush( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execFileFlush);
	P_FINISH;

	if( FStatLogRuntimeState* Runtime = FindStatLogRuntimeState( this ) )
		if( Runtime->LogAr )
			Runtime->LogAr->Flush();

	unguardexec;
}

void AStatLogFile::execFileLog( FFrame& Stack, RESULT_DECL )
{
	guard(AStatLogFile::execFileLog);
	P_GET_STR(EventString);
	P_FINISH;
	FStatLogRuntimeState* Runtime = FindStatLogRuntimeState( this );
	FArchive* RuntimeLogAr = Runtime ? Runtime->LogAr : NULL;

	TArray<BYTE> LogBytes;
	if( !EncodeStatLogUtf16LE(EventString, LogBytes, 1) )
		appErrorf( TEXT("Stat log entry exceeds the 32-bit archive contract") );

	if( bWorld )
		for( INT Index = 0; Index < LogBytes.Num(); ++Index )
			LogBytes(Index) ^= 0xa7;

	if( RuntimeLogAr )
		RuntimeLogAr->Serialize( &LogBytes(0), LogBytes.Num() );

	unguardexec;
}


/*-----------------------------------------------------------------------------
	UCheckSumCommandlet.
-----------------------------------------------------------------------------*/

class UCheckSumCommandlet : public UCommandlet
{
	DECLARE_CLASS(UCheckSumCommandlet,UCommandlet,CLASS_Transient,Engine);

	INT Main( const TCHAR* FileName )
	{
		guard(UCheckSumCommandlet::Main);
		INT FileSize = GFileManager->FileSize( FileName );

		if( FileSize < 0 )
			appErrorf( TEXT("Could not open file: %s"), FileName );

		FString CapsName = FString(FileName).Caps();
		INT i = CapsName.InStr( TEXT("\\"), 1 );
		if( i != -1 )
			CapsName = CapsName.Mid( i );
		i = CapsName.InStr( TEXT("/"), 1 );
		if( i != -1 )
			CapsName = CapsName.Mid( i );
				
		FString CheckString = CapsName + FString::Printf( TEXT("%i"), FileSize );
		FMD5Context PContext;
		appMD5Init( &PContext );
		appMD5UpdateStringUtf16LE( &PContext, CheckString, 0 );
		BYTE Digest[16];
		appMD5Final( Digest, &PContext );
		FString Checksum;
		for (INT j=0; j<16; j++)
			Checksum += FString::Printf(TEXT("%02x"), Digest[j]);
		GWarn->Logf( TEXT("Package %s has checksum %s"), FileName, *Checksum);
		
		GIsRequestingExit=1;
		return 0;
		unguard;
	}
};
IMPLEMENT_CLASS(UCheckSumCommandlet)
void RegisterCheckSumCommandletClass()
{
	UCheckSumCommandlet::StaticClass();
}



/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/
