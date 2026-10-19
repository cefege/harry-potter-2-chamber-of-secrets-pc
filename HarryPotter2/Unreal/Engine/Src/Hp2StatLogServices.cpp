// C callbacks behind the Rust AStatLog port (crates/hp2-engine/src/stat_log.rs).
// Each thunk performs one member read, one global access, one virtual call or one
// engine free function from the original AStatLog code, so the Rust side never
// re-implements engine object layout or dispatch.
// Object pointers arrive as void*. FString arguments are addresses of FString objects.

#include "EnginePrivate.h"
#include "Hp2StatLogServices.h"

static void* Hp2StatLogActorLevel( void* Actor )
{
	return static_cast<AActor*>(Actor)->Level;
}

static void* Hp2StatLogLevelGame( void* LevelInfo )
{
	return static_cast<ALevelInfo*>(LevelInfo)->Game;
}

static int Hp2StatLogLevelNotDedicated( void* LevelInfo )
{
	return static_cast<ALevelInfo*>(LevelInfo)->NetMode != NM_DedicatedServer;
}

static int Hp2StatLogGameExternalBatcher( void* Game )
{
	return static_cast<AGameInfo*>(Game)->bExternalBatcher;
}

static void* Hp2StatLogGameLocalLogFileName( void* Game )
{
	return &static_cast<AGameInfo*>(Game)->LocalLogFileName;
}

static const void* Hp2StatLogFStrChars( void* FStr )
{
	return static_cast<FString*>(FStr)->operator*();
}

static int Hp2StatLogFStrLen( void* FStr )
{
	return static_cast<FString*>(FStr)->Len();
}

static void Hp2StatLogFStrAssign( void* FStr, const void* Text )
{
	*static_cast<FString*>(FStr) = static_cast<const TCHAR*>(Text);
}

static void Hp2StatLogFStrAppend( void* FStr, const void* Text )
{
	*static_cast<FString*>(FStr) += static_cast<const TCHAR*>(Text);
}

static void* Hp2StatLogLocalBatcherURL( void* StatLog )
{
	return &static_cast<AStatLog*>(StatLog)->LocalBatcherURL;
}

static void* Hp2StatLogLocalLogDir( void* StatLog )
{
	return &static_cast<AStatLog*>(StatLog)->LocalLogDir;
}

static void* Hp2StatLogWorldBatcherURL( void* StatLog )
{
	return &static_cast<AStatLog*>(StatLog)->WorldBatcherURL;
}

static void* Hp2StatLogWorldBatcherParams( void* StatLog )
{
	return &static_cast<AStatLog*>(StatLog)->WorldBatcherParams;
}

static void* Hp2StatLogClassDefaultObject( void* StatLog )
{
	return static_cast<AStatLog*>(StatLog)->GetClass()->GetDefaultObject();
}

static void* Hp2StatLogStaticDefaultObject( void )
{
	return AStatLog::StaticClass()->GetDefaultObject();
}

static int Hp2StatLogGetWorldBatcherError( void* StatLog )
{
	return static_cast<AStatLog*>(StatLog)->bWorldBatcherError;
}

static void Hp2StatLogSetWorldBatcherError( void* StatLog, int Value )
{
	static_cast<AStatLog*>(StatLog)->bWorldBatcherError = Value;
}

static void Hp2StatLogSaveConfig( void* StatLog )
{
	static_cast<AStatLog*>(StatLog)->SaveConfig();
}

static int Hp2StatLogGetWorld( void* StatLog )
{
	return static_cast<AStatLog*>(StatLog)->bWorld;
}

static void* Hp2StatLogFileName( void* StatLog )
{
	return &static_cast<AStatLogFile*>(StatLog)->StatLogFile;
}

static void* Hp2StatLogFileFinal( void* StatLog )
{
	return &static_cast<AStatLogFile*>(StatLog)->StatLogFinal;
}

static void* Hp2StatLogPlayerName( void* Pawn )
{
	return &static_cast<APlayerPawn*>(Pawn)->PlayerReplicationInfo->PlayerName;
}

static void* Hp2StatLogPlayerSecret( void* Pawn )
{
	return &static_cast<APlayerPawn*>(Pawn)->ngWorldSecret;
}

static void Hp2StatLogEventSpecialClass( void* StatLog, const void* Id, void* Target )
{
	static_cast<AStatLog*>(StatLog)->eventLogGameSpecial(
		static_cast<const TCHAR*>(Id), static_cast<UObject*>(Target)->GetClass()->GetFullName() );
}

static void Hp2StatLogEventSpecial2( void* StatLog, const void* Id, const void* Param1, const void* Param2 )
{
	static_cast<AStatLog*>(StatLog)->eventLogGameSpecial2(
		static_cast<const TCHAR*>(Id), static_cast<const TCHAR*>(Param1), static_cast<const TCHAR*>(Param2) );
}

static void Hp2StatLogClassIteratePackages( void (*Callback)( void* User, void* Package ), void* User )
{
	for( TObjectIterator<UClass> It; It; ++It )
	{
		Callback( User, CastChecked<UPackage>((*It)->GetOuter()) );
	}
}

static const void* Hp2StatLogPackageFullName( void* Package )
{
	return static_cast<UPackage*>(Package)->GetFullName();
}

static const void* Hp2StatLogDllExt( void )
{
	static FString Ext;
	Ext = FString::Printf( TEXT("%s"), DLLEXT );
	return *Ext;
}

static int Hp2StatLogFileSize( const void* Name )
{
	return GFileManager->FileSize( static_cast<const TCHAR*>(Name) );
}

static void* Hp2StatLogFileCreateWriter( const void* Name )
{
	return GFileManager->CreateFileWriter( static_cast<const TCHAR*>(Name), FILEWRITE_EvenIfReadOnly );
}

static void Hp2StatLogFileMakeDirectory( const void* Name )
{
	GFileManager->MakeDirectory( static_cast<const TCHAR*>(Name) );
}

static void Hp2StatLogFileMove( const void* Dst, const void* Src )
{
	GFileManager->Move( static_cast<const TCHAR*>(Dst), static_cast<const TCHAR*>(Src), 1, 1, 1 );
}

static void Hp2StatLogArchiveSerialize( void* Ar, const void* Data, int Len )
{
	static_cast<FArchive*>(Ar)->Serialize( const_cast<void*>(Data), Len );
}

static void Hp2StatLogArchiveFlush( void* Ar )
{
	static_cast<FArchive*>(Ar)->Flush();
}

static void Hp2StatLogArchiveDelete( void* Ar )
{
	delete static_cast<FArchive*>(Ar);
}

static void* Hp2StatLogMD5ContextNew( void )
{
	return new FMD5Context;
}

static void Hp2StatLogMD5ContextInit( void* Context )
{
	appMD5Init( static_cast<FMD5Context*>(Context) );
}

static void Hp2StatLogMD5ContextUpdate( void* Context, const void* Data, int Len )
{
	appMD5Update( static_cast<FMD5Context*>(Context), const_cast<BYTE*>(static_cast<const BYTE*>(Data)), Len );
}

static void Hp2StatLogMD5ContextFinal( void* Digest, void* Context )
{
	appMD5Final( static_cast<BYTE*>(Digest), static_cast<FMD5Context*>(Context) );
}

static void Hp2StatLogMD5ContextDelete( void* Context )
{
	delete static_cast<FMD5Context*>(Context);
}

static void Hp2StatLogDebugfText( const void* Text )
{
	debugf( TEXT("%s"), static_cast<const TCHAR*>(Text) );
}

static void Hp2StatLogWarnText( const void* Text )
{
	GWarn->Logf( TEXT("%s"), static_cast<const TCHAR*>(Text) );
}

static void Hp2StatLogSetRequestingExit( void )
{
	GIsRequestingExit = 1;
}

static void* Hp2StatLogProcCreate( const void* Url, const void* Params, int Show )
{
	return appCreateProc( static_cast<const TCHAR*>(Url), static_cast<const TCHAR*>(Params), Show );
}

static int Hp2StatLogProcReturnCode( void* Handle, int* Rc )
{
	return appGetProcReturnCode( Handle, Rc );
}

static void Hp2StatLogLaunchURL( const void* Url )
{
	appLaunchURL( *(GFileManager->GetDefaultDirectory() * static_cast<const TCHAR*>(Url)) );
}

const Hp2StatLogServices GHp2StatLogServices =
{
	Hp2StatLogActorLevel,
	Hp2StatLogLevelGame,
	Hp2StatLogLevelNotDedicated,
	Hp2StatLogGameExternalBatcher,
	Hp2StatLogGameLocalLogFileName,
	Hp2StatLogFStrChars,
	Hp2StatLogFStrLen,
	Hp2StatLogFStrAssign,
	Hp2StatLogFStrAppend,
	Hp2StatLogLocalBatcherURL,
	Hp2StatLogLocalLogDir,
	Hp2StatLogWorldBatcherURL,
	Hp2StatLogWorldBatcherParams,
	Hp2StatLogClassDefaultObject,
	Hp2StatLogStaticDefaultObject,
	Hp2StatLogGetWorldBatcherError,
	Hp2StatLogSetWorldBatcherError,
	Hp2StatLogSaveConfig,
	Hp2StatLogGetWorld,
	Hp2StatLogFileName,
	Hp2StatLogFileFinal,
	Hp2StatLogPlayerName,
	Hp2StatLogPlayerSecret,
	Hp2StatLogEventSpecialClass,
	Hp2StatLogEventSpecial2,
	Hp2StatLogClassIteratePackages,
	Hp2StatLogPackageFullName,
	Hp2StatLogDllExt,
	Hp2StatLogFileSize,
	Hp2StatLogFileCreateWriter,
	Hp2StatLogFileMakeDirectory,
	Hp2StatLogFileMove,
	Hp2StatLogArchiveSerialize,
	Hp2StatLogArchiveFlush,
	Hp2StatLogArchiveDelete,
	Hp2StatLogMD5ContextNew,
	Hp2StatLogMD5ContextInit,
	Hp2StatLogMD5ContextUpdate,
	Hp2StatLogMD5ContextFinal,
	Hp2StatLogMD5ContextDelete,
	Hp2StatLogDebugfText,
	Hp2StatLogWarnText,
	Hp2StatLogSetRequestingExit,
	Hp2StatLogProcCreate,
	Hp2StatLogProcReturnCode,
	Hp2StatLogLaunchURL,
};
