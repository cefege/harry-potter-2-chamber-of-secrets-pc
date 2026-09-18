/*=============================================================================
	UnIX: Unix port of UnVcWin32.cpp.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Cloned by Mike Danylchuk
		* Severely amputated and mutilated by Brandon Reinhart
		* Surgically altered by Jack Porter
		* Mangled and rehabilitated by Brandon Reinhart
		* Obfuscated by Daniel Vogel
=============================================================================*/
#if __GNUG__

// Standard includes.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <float.h>
#include <time.h>
#include <errno.h>
#include <stdint.h>
#include <chrono>

// System includes.
#include <unistd.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <dlfcn.h>
#include <netdb.h>

// Core includes.
#include "CorePrivate.h"
#include "HP2RngSeed.h"

// Context runs before appInit and class registration. Retain only libc state
// here: the trace subsystem becomes available later in appPlatformInit.
static int GUnixDiagnosticRngSeeded = 0;
static unsigned GUnixDiagnosticRngRequestedSeed = 0;
static unsigned GUnixDiagnosticRngEffectiveSeed = 0;

static void appUnixBootstrapDiagnosticRngSeed()
{
	const char* Text = getenv( "HP2_RNG_SEED" );
	unsigned RequestedSeed = 0;
	unsigned EffectiveSeed = 0;
	switch( HP2PrepareDiagnosticRngSeed(Text,&RequestedSeed,&EffectiveSeed) )
	{
		case HP2DiagnosticRngSeedAbsent:
			return;
		case HP2DiagnosticRngSeedInvalid:
			fprintf(
				stderr,
				"Invalid HP2_RNG_SEED: expected an unsigned decimal value from 0 to %u\n",
				(unsigned)RAND_MAX
			);
			exit(1);
		case HP2DiagnosticRngSeedAccepted:
			GUnixDiagnosticRngRequestedSeed = RequestedSeed;
			GUnixDiagnosticRngEffectiveSeed = EffectiveSeed;
			GUnixDiagnosticRngSeeded = 1;
			srand( EffectiveSeed );
			return;
	}
}

static void* appUnixSystemMalloc( size_t Size )
{
	void* Result = malloc( Size ? Size : 1 );
	if( !Result )
	{
		fputs( "Unreal: out of memory in Unix platform bootstrap\n", stderr );
		abort();
	}
	return Result;
}

static void* appUnixSystemRealloc( void* Pointer, size_t Size )
{
	void* Result = realloc( Pointer, Size ? Size : 1 );
	if( !Result )
	{
		fputs( "Unreal: out of memory in Unix platform bootstrap\n", stderr );
		abort();
	}
	return Result;
}

static size_t appUnixTCharLength( const TCHAR* Text )
{
	const TCHAR* End = Text ? Text : TEXT("");
	const TCHAR* Start = End;
	while( *End )
		++End;
	return static_cast<size_t>(End-Start);
}

static ANSICHAR* appUnixToUtf8System( const TCHAR* Text )
{
	const size_t CharCount = appUnixTCharLength( Text );
	if( CharCount > (SIZE_MAX-1)/4 )
	{
		fputs( "Unreal: path is too large to encode as UTF-8\n", stderr );
		abort();
	}

	ANSICHAR* Bytes = static_cast<ANSICHAR*>(appUnixSystemMalloc(CharCount*4+1));
	ANSICHAR* Dest = Bytes;
	Text = Text ? Text : TEXT("");
	while( *Text )
	{
		DWORD CodePoint;
		if( sizeof(TCHAR)==sizeof(UNICHAR) )
		{
			const DWORD First = static_cast<UNICHAR>(*Text++);
			if( First>=0xd800u && First<=0xdbffu )
			{
				const DWORD Second = static_cast<UNICHAR>(*Text);
				if( Second>=0xdc00u && Second<=0xdfffu )
				{
					++Text;
					CodePoint = 0x10000u + ((First-0xd800u)<<10) + (Second-0xdc00u);
				}
				else
					CodePoint = 0xfffdu;
			}
			else
				CodePoint = First>=0xdc00u && First<=0xdfffu ? 0xfffdu : First;
		}
		else
		{
			const PTRINT HostChar = static_cast<PTRINT>(*Text++);
			CodePoint = HostChar>=0 && static_cast<UPTRINT>(HostChar)<=0x10ffffu
				? static_cast<DWORD>(HostChar)
				: 0xfffdu;
			if( CodePoint>=0xd800u && CodePoint<=0xdfffu )
				CodePoint = 0xfffdu;
		}

		if( CodePoint<=0x7fu )
			*Dest++ = static_cast<ANSICHAR>(CodePoint);
		else if( CodePoint<=0x7ffu )
		{
			*Dest++ = static_cast<ANSICHAR>(0xc0u | (CodePoint>>6));
			*Dest++ = static_cast<ANSICHAR>(0x80u | (CodePoint&0x3fu));
		}
		else if( CodePoint<=0xffffu )
		{
			*Dest++ = static_cast<ANSICHAR>(0xe0u | (CodePoint>>12));
			*Dest++ = static_cast<ANSICHAR>(0x80u | ((CodePoint>>6)&0x3fu));
			*Dest++ = static_cast<ANSICHAR>(0x80u | (CodePoint&0x3fu));
		}
		else
		{
			*Dest++ = static_cast<ANSICHAR>(0xf0u | (CodePoint>>18));
			*Dest++ = static_cast<ANSICHAR>(0x80u | ((CodePoint>>12)&0x3fu));
			*Dest++ = static_cast<ANSICHAR>(0x80u | ((CodePoint>>6)&0x3fu));
			*Dest++ = static_cast<ANSICHAR>(0x80u | (CodePoint&0x3fu));
		}
	}
	*Dest = 0;
	return Bytes;
}

static TCHAR* appUnixFromUtf8System( const ANSICHAR* Text )
{
	const BYTE* Source = reinterpret_cast<const BYTE*>(Text ? Text : "");
	const size_t ByteCount = strlen( reinterpret_cast<const char*>(Source) );
	if( ByteCount > (SIZE_MAX/sizeof(TCHAR))-1 )
	{
		fputs( "Unreal: UTF-8 string is too large to decode\n", stderr );
		abort();
	}

	TCHAR* Chars = static_cast<TCHAR*>(appUnixSystemMalloc((ByteCount+1)*sizeof(TCHAR)));
	TCHAR* Dest = Chars;
	while( *Source )
	{
		DWORD CodePoint;
		INT Extra;
		const BYTE First = *Source++;
		if( First<0x80u )
		{
			CodePoint = First;
			Extra = 0;
		}
		else if( First>=0xc2u && First<=0xdfu )
		{
			CodePoint = First&0x1fu;
			Extra = 1;
		}
		else if( First>=0xe0u && First<=0xefu )
		{
			CodePoint = First&0x0fu;
			Extra = 2;
		}
		else if( First>=0xf0u && First<=0xf4u )
		{
			CodePoint = First&0x07u;
			Extra = 3;
		}
		else
		{
			CodePoint = 0xfffdu;
			Extra = 0;
		}

		const DWORD Minimum = Extra==1 ? 0x80u : Extra==2 ? 0x800u : Extra==3 ? 0x10000u : 0u;
		UBOOL Valid = 1;
		for( INT Index=0; Index<Extra; ++Index )
		{
			const BYTE Continuation = *Source;
			if( Continuation==0 || (Continuation&0xc0u)!=0x80u )
			{
				Valid = 0;
				break;
			}
			++Source;
			CodePoint = (CodePoint<<6) | (Continuation&0x3fu);
		}
		if( !Valid || CodePoint<Minimum || CodePoint>0x10ffffu || (CodePoint>=0xd800u && CodePoint<=0xdfffu) )
			CodePoint = 0xfffdu;

		if( sizeof(TCHAR)==sizeof(UNICHAR) && CodePoint>0xffffu )
		{
			CodePoint -= 0x10000u;
			*Dest++ = static_cast<TCHAR>(0xd800u + (CodePoint>>10));
			*Dest++ = static_cast<TCHAR>(0xdc00u + (CodePoint&0x3ffu));
		}
		else
			*Dest++ = static_cast<TCHAR>(CodePoint);
	}
	*Dest = 0;
	return Chars;
}

class FUnixUtf8String
{
public:
	explicit FUnixUtf8String( const TCHAR* Text )
	: Data( appUnixToUtf8System(Text) )
	{}

	~FUnixUtf8String()
	{
		free( Data );
	}

	const ANSICHAR* operator*() const
	{
		return Data;
	}

	void NormalizePath()
	{
		for( ANSICHAR* Cursor=Data; *Cursor; ++Cursor )
			if( *Cursor=='\\' )
				*Cursor = '/';
	}

private:
	ANSICHAR* Data;
	FUnixUtf8String( const FUnixUtf8String& );
	FUnixUtf8String& operator=( const FUnixUtf8String& );
};

CORE_API UBOOL appToUtf8NativePath( const TCHAR* Filename, std::string& Out )
{
	Out.clear();
	if( !Filename || !*Filename )
		return 0;
	FUnixUtf8String NativeFilename( Filename );
	NativeFilename.NormalizePath();
	const ANSICHAR* Source = *NativeFilename;
	UBOOL PreviousSlash = 0;
	for( ; *Source; ++Source )
	{
		if( *Source=='/' )
		{
			if( PreviousSlash )
				continue;
			PreviousSlash = 1;
		}
		else
		{
			PreviousSlash = 0;
		}
		Out.push_back(*Source);
	}
	return !Out.empty();
}

CORE_API FILE* appFopen( const TCHAR* Filename, const ANSICHAR* Mode )
{
	if( !Mode || !*Mode )
		return NULL;
	std::string NativeFilename;
	if( !appToUtf8NativePath(Filename,NativeFilename) )
		return NULL;
	return fopen( NativeFilename.c_str(), Mode );
}

static FString appUnixFromUtf8( const ANSICHAR* Text )
{
	TCHAR* Decoded = appUnixFromUtf8System( Text );
	FString Result( Decoded );
	free( Decoded );
	return Result;
}

/*-----------------------------------------------------------------------------
	Globals
-----------------------------------------------------------------------------*/

// Module
ANSICHAR GModule[32];

// Environment
extern char **environ;

// Signal
static int SignalExit = 0;
static int SignalCritical = 0;
static bool AlreadyAborting = false;

/*-----------------------------------------------------------------------------
	USystem.
-----------------------------------------------------------------------------*/

//
// System manager.
//
static void Recurse()
{
	guard(Recurse);
	Recurse();
	unguard;
}
USystem::USystem()
:	SavePath	( E_NoInit )
,	CachePath	( E_NoInit )
,	CacheExt	( E_NoInit )
,	Paths		( E_NoInit )
,	Suppress	( E_NoInit )
{}
void USystem::StaticConstructor()
{
	guard(USystem::StaticConstructor);

	new(GetClass(),TEXT("PurgeCacheDays"),      RF_Public)UIntProperty   (CPP_PROPERTY(PurgeCacheDays    ), TEXT("Options"), CPF_Config );
	new(GetClass(),TEXT("SavePath"),            RF_Public)UStrProperty   (CPP_PROPERTY(SavePath          ), TEXT("Options"), CPF_Config );
	new(GetClass(),TEXT("CachePath"),           RF_Public)UStrProperty   (CPP_PROPERTY(CachePath         ), TEXT("Options"), CPF_Config );
	new(GetClass(),TEXT("CacheExt"),            RF_Public)UStrProperty   (CPP_PROPERTY(CacheExt          ), TEXT("Options"), CPF_Config );

	UArrayProperty* A = new(GetClass(),TEXT("Paths"),RF_Public)UArrayProperty( CPP_PROPERTY(Paths), TEXT("Options"), CPF_Config );
	A->Inner = new(A,TEXT("StrProperty0"),RF_Public)UStrProperty;

	UArrayProperty* B = new(GetClass(),TEXT("Suppress"),RF_Public)UArrayProperty( CPP_PROPERTY(Suppress), TEXT("Options"), CPF_Config );
	B->Inner = new(B,TEXT("NameProperty0"),RF_Public)UNameProperty;

	unguard;
}
UBOOL USystem::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	if( ParseCommand(&Cmd,TEXT("MEMSTAT")) )
	{
		//!UNIX No MEMSTAT command.
		Ar.Logf( TEXT("MEMSTAT command not available.") );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("APP")) )
	{
		//!UNIX No APP command.
		Ar.Logf( TEXT("APP command not available.") );
		return 1;
	}
	else if( ParseCommand( &Cmd, TEXT("RELAUNCH") ) )
	{
		debugf( TEXT("Relaunch: %s"), Cmd );
		GConfig->Flush( 0 );

		// Fork out a new process using the first command token as the server URL.
		const TCHAR* EndArg0 = appStrchr( Cmd, TEXT(' ') );
		const size_t Arg0Length = EndArg0
			? static_cast<size_t>(EndArg0-Cmd)
			: appUnixTCharLength(Cmd);
		if( Arg0Length > (SIZE_MAX/sizeof(TCHAR))-1 )
			appErrorf( TEXT("Relaunch argument is too large.") );
		TCHAR* Arg0 = static_cast<TCHAR*>(appUnixSystemMalloc((Arg0Length+1)*sizeof(TCHAR)));
		memcpy( Arg0, Cmd, Arg0Length*sizeof(TCHAR) );
		Arg0[Arg0Length] = 0;
		FUnixUtf8String Arg0Utf8( Arg0 );
		Arg0Utf8.NormalizePath();
		free( Arg0 );

		const pid_t Pid = fork();
		if( Pid==0 )
		{
			sleep( 3 );
			execl( "./ucc", "ucc", "server", *Arg0Utf8, static_cast<char*>(NULL) );
			_exit( 127 );
		}
		if( Pid<0 )
			appErrorf( TEXT("Failed to launch process.") );
		appRequestExit( 0 );

		return 1;
	}
	else if( ParseCommand( &Cmd, TEXT("DEBUG") ) )
	{
		if( ParseCommand(&Cmd,TEXT("CRASH")) )
		{
			appErrorf( TEXT("%s"), TEXT("Unreal crashed at your request") );
			return 1;
		}
		else if( ParseCommand( &Cmd, TEXT("GPF") ) )
		{
			Ar.Log( TEXT("Unreal crashing with voluntary GPF") );
			*(int *)NULL = 123;
			return 1;
		}
		else if( ParseCommand( &Cmd, TEXT("RECURSE") ) )
		{
			Ar.Logf( TEXT("Recursing") );
			Recurse();
			return 1;
		}
		else if( ParseCommand( &Cmd, TEXT("EATMEM") ) )
		{
			Ar.Log( TEXT("Eating up all available memory") );
			while( 1 )
			{
				void* Eat = appMalloc(65536,TEXT("EatMem"));
				memset( Eat, 0, 65536 );
			}
			return 1;
		}
		else return 0;
	}
	else return 0;
}
IMPLEMENT_CLASS(USystem);

/*-----------------------------------------------------------------------------
	Exit.
-----------------------------------------------------------------------------*/

//
// Immediate exit.
//
CORE_API void appRequestExit( UBOOL Force )
{
	guard(appForceExit);
	debugf( TEXT("appRequestExit(%i)"), Force );
	if( Force )
	{
		// Force immediate exit. Dangerous because config code isn't flushed, etc.
		exit( 1 );
	}
	else
	{
		// Tell the platform specific code we want to exit cleanly from the main loop.
		//!UNIX No quit message in UNIX.
		GIsRequestingExit = 1;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Clipboard.
-----------------------------------------------------------------------------*/

CORE_API void ClipboardCopy( const TCHAR* Str )
{
	guard(ClipboardCopy);
	//!UNIX Not supported in UNIX.
	unguard;
}

CORE_API void ClipboardPaste( FString& Result )
{
	guard(ClipboardPasteString);
	//!UNIX Not supported in UNIX.
	unguard;
}

/*-----------------------------------------------------------------------------
	Shared libraries.
-----------------------------------------------------------------------------*/

//
// Load a library.
//
void* appGetDllHandle( const TCHAR* Filename )
{
	guard(appGetDllHandle);

	#if __STATIC_LINK
		return dlopen( NULL, RTLD_NOW );
	#endif
	check(Filename);
	const TCHAR* PackageName = Filename;
	for( const TCHAR* Cursor=Filename; *Cursor; ++Cursor )
		if( *Cursor==TEXT('/') || *Cursor==TEXT('\\') )
			PackageName = Cursor+1;

	FUnixUtf8String PackageNameUtf8( PackageName );
	const ANSICHAR* PackageEnd = strchr( *PackageNameUtf8, '.' );
	const size_t PackageLength = PackageEnd
		? static_cast<size_t>(PackageEnd-*PackageNameUtf8)
		: strlen(*PackageNameUtf8);
	static const ANSICHAR SymbolPrefix[] = "GLoaded";
	if( PackageLength > SIZE_MAX-sizeof(SymbolPrefix) )
		appErrorf( TEXT("Dynamic library package name is too large.") );
	ANSICHAR* Symbol = static_cast<ANSICHAR*>(appUnixSystemMalloc(sizeof(SymbolPrefix)+PackageLength));
	memcpy( Symbol, SymbolPrefix, sizeof(SymbolPrefix)-1 );
	memcpy( Symbol+sizeof(SymbolPrefix)-1, *PackageNameUtf8, PackageLength );
	Symbol[sizeof(SymbolPrefix)-1+PackageLength] = 0;

	dlerror(); // Clear any error condition.
	void* Result = dlopen( NULL, RTLD_NOW );
	const ANSICHAR* Error = dlerror();
	if( Error != NULL )
		debugf( TEXT("dlerror(): %s"), *appUnixFromUtf8(Error) );
	else
	{
		dlsym( Result, Symbol );
		Error = dlerror();
		if( Error == NULL )
		{
			free( Symbol );
			return Result;
		}
	}
	free( Symbol );

	// Load the requested library, then retry with the platform extension.
	FUnixUtf8String FilenameUtf8( Filename );
	FilenameUtf8.NormalizePath();
	Result = dlopen( *FilenameUtf8, RTLD_NOW );
	if( Result == NULL )
	{
		FUnixUtf8String ExtensionUtf8( DLLEXT );
		const size_t FilenameLength = strlen( *FilenameUtf8 );
		const size_t ExtensionLength = strlen( *ExtensionUtf8 );
		if( FilenameLength > SIZE_MAX-ExtensionLength-1 )
			appErrorf( TEXT("Dynamic library path is too large.") );
		ANSICHAR* ExtendedFilename = static_cast<ANSICHAR*>(
			appUnixSystemMalloc(FilenameLength+ExtensionLength+1)
		);
		memcpy( ExtendedFilename, *FilenameUtf8, FilenameLength );
		memcpy( ExtendedFilename+FilenameLength, *ExtensionUtf8, ExtensionLength+1 );
		Result = dlopen( ExtendedFilename, RTLD_NOW );
		free( ExtendedFilename );
	}

	return Result;
	unguard;
}

//
// Free a library.
//
void appFreeDllHandle( void* DllHandle )
{
	guard(appFreeDllHandle);
	check(DllHandle);

	dlclose( DllHandle );

	unguard;
}

//
// Lookup the address of a shared library function.
//
void* appGetDllExport( void* DllHandle, const TCHAR* ProcName )
{
	guard(appGetDllExport);
	check(DllHandle);
	check(ProcName);

	dlerror(); // Clear any error condition.
	FUnixUtf8String ProcNameUtf8( ProcName );
	void* Result = dlsym( DllHandle, *ProcNameUtf8 );
	const ANSICHAR* Error = dlerror();
	if( Error != NULL )
		debugf( TEXT("dlerror: %s"), *appUnixFromUtf8(Error) );
	return Result;

	unguard;
}

//
// Break the debugger.
//
#ifndef DEFINED_appDebugBreak
void appDebugBreak()
{
	guard(appDebugBreak);
#if ASMLINUX && (defined(__i386__) || defined(__x86_64__))
	asm("int $03");
#else
	raise(SIGTRAP);
#endif
	unguard;
}
#endif

/*-----------------------------------------------------------------------------
	External processes.
-----------------------------------------------------------------------------*/
static TMap<pid_t,int>* ExitCodeMap = NULL;

void* appCreateProc( const TCHAR* URL, const TCHAR* Parms , UBOOL bRealTime )
{
	guard(appCreateProc);

	debugf( TEXT("Create Proc: %s %s"), URL ? URL : TEXT(""), Parms ? Parms : TEXT("") );

	FUnixUtf8String URLUtf8( URL ? URL : TEXT("") );
	URLUtf8.NormalizePath();
	FUnixUtf8String ParmsUtf8( Parms ? Parms : TEXT("") );
	const size_t URLLength = strlen( *URLUtf8 );
	const size_t ParmsLength = strlen( *ParmsUtf8 );
	if( URLLength > SIZE_MAX-ParmsLength-2 )
		appErrorf( TEXT("Process command line is too large.") );
	ANSICHAR* LocalCommand = static_cast<ANSICHAR*>(
		appUnixSystemMalloc(URLLength+ParmsLength+2)
	);
	memcpy( LocalCommand, *URLUtf8, URLLength );
	LocalCommand[URLLength] = ' ';
	memcpy( LocalCommand+URLLength+1, *ParmsUtf8, ParmsLength+1 );

	const pid_t Pid = fork();
	if( Pid==0 )
		_exit( system(LocalCommand) );
	free( LocalCommand );
	if( Pid<0 )
		return NULL;

	return reinterpret_cast<void*>(static_cast<UPTRINT>(Pid));
	unguard;
}

UBOOL appGetProcReturnCode( void* ProcHandle, INT* ReturnCode )
{
	guard(appGetProcReturnCode);
	const UPTRINT RawPid = reinterpret_cast<UPTRINT>(ProcHandle);
	if( RawPid>static_cast<UPTRINT>(MAXINT) )
		return 0;
	const pid_t Pid = static_cast<pid_t>(RawPid);
	int* p = ExitCodeMap->Find( Pid );
	if(p)
	{
		*ReturnCode = *p;
		ExitCodeMap->Remove( Pid );
		return 1;
	}
	return 0;
	unguard;
}

void HandleChild(int Signal)
{
	int Status;
	pid_t pid;
	while( (pid=waitpid(-1, &Status, WNOHANG)) > 0 )
		ExitCodeMap->Set( pid, WEXITSTATUS(Status) );
}

/*-----------------------------------------------------------------------------
	Timing.
-----------------------------------------------------------------------------*/

//
// String timestamp.
//
CORE_API const TCHAR* appTimestamp()
{
	guard(appTimestamp);

	TCHAR* Result = appStaticString1024();
	const time_t CurTime = time( NULL );
	struct tm SysTime;
	if( localtime_r(&CurTime,&SysTime)==NULL
	||  wcsftime(Result,1024,TEXT("%a %b %e %H:%M:%S %Y"),&SysTime)==0 )
	{
		Result[0] = 0;
	}
	return Result;

	unguard;
}

//
// Get file time.
//
CORE_API DWORD appGetTime( const TCHAR* Filename )
{
	guard(appGetTime);

	struct stat FileInfo;
	FUnixUtf8String FilenameUtf8( Filename );
	FilenameUtf8.NormalizePath();
	if( stat(*FilenameUtf8,&FileInfo)!=0 || FileInfo.st_mtime<static_cast<time_t>(0) )
		return 0;
	if( static_cast<uint64_t>(FileInfo.st_mtime)>static_cast<uint64_t>(MAXDWORD) )
	{
		debugf( TEXT("File timestamp exceeds the 32-bit engine range: %s"), Filename );
		return MAXDWORD;
	}
	return static_cast<DWORD>(FileInfo.st_mtime);

	unguard;
}

//
// Get monotonic time.
//
namespace
{
	using FSteadyClock = std::chrono::steady_clock;
	static_assert( FSteadyClock::is_steady, "The platform timer must be monotonic" );

	const FSteadyClock::time_point& GetSteadyStart()
	{
		static const FSteadyClock::time_point Start = FSteadyClock::now();
		return Start;
	}

	FSteadyClock::duration GetSteadyElapsed()
	{
		const FSteadyClock::time_point& Start = GetSteadyStart();
		return FSteadyClock::now() - Start;
	}

	double GetSteadySeconds()
	{
		return std::chrono::duration<double>( GetSteadyElapsed() ).count();
	}
}

CORE_API FTime appSecondsSlow()
{
	return GetSteadySeconds();
}

#if !DEFINED_appSeconds
CORE_API FTime appSeconds()
{
	return GetSteadySeconds();
}
#endif

#if !DEFINED_appCycles
CORE_API DWORD appCycles()
{
	const FSteadyClock::duration::rep Count = GetSteadyElapsed().count();
	if( Count<0 )
		return 0;
	// appCycles historically wraps at 32 bits; make that narrowing explicit.
	return static_cast<DWORD>(static_cast<uint64_t>(Count)&static_cast<uint64_t>(MAXDWORD));
}
#endif

// Return CPU time consumed by this process, excluding time spent asleep.
CORE_API FTime appProcessSeconds()
{
	struct timespec ProcessTime;
	if( clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &ProcessTime)==0 )
		return static_cast<double>(ProcessTime.tv_sec)
			+ static_cast<double>(ProcessTime.tv_nsec) / 1000000000.0;
	return 0.0;
}

//
// Return the system time.
//
CORE_API void appSystemTime( INT& Year, INT& Month, INT& DayOfWeek, INT& Day, INT& Hour, INT& Min, INT& Sec, INT& MSec )
{
	guard(appSystemTime);

	time_t			CurTime;
	struct tm		*St;		// System time.
	struct timeval	Tv;			// Use timeval to get milliseconds.

	gettimeofday( &Tv, NULL );
	CurTime = time( NULL );
	St = localtime( &CurTime );
	
	Year		= St->tm_year + 1900;
	Month		= St->tm_mon + 1;
	DayOfWeek	= St->tm_wday;
	Day			= St->tm_mday;
	Hour		= St->tm_hour;
	Min			= St->tm_min;
	Sec			= St->tm_sec;
	MSec		= (INT) (Tv.tv_usec / 1000);

	unguard;
}

CORE_API void appSleep( FLOAT Seconds )
{
	guard(appSleep);

	INT SleepTime = appRound(Seconds * 1000000);
	usleep( SleepTime );

	unguard;
}

/*-----------------------------------------------------------------------------
	Link functions.
-----------------------------------------------------------------------------*/

//
// Launch a uniform resource locator (i.e. http://www.epicgames.com/unreal).
// This is expected to return immediately as the URL is launched by another
// task.
//
void appLaunchURL( const TCHAR* URL, const TCHAR* Parms, FString* Error )
{
	guard(appLaunchURL);
	//!UNIX Server doesn't need to launch URLs.
	unguard;
}

/*-----------------------------------------------------------------------------
	File finding.
-----------------------------------------------------------------------------*/

//
// Clean out the file cache.
//
static INT GetFileAgeDays( const TCHAR* Filename )
{
	guard(GetFileAgeDays);
	struct stat Buf;
	FUnixUtf8String FilenameUtf8( Filename );
	FilenameUtf8.NormalizePath();
	const INT Result = stat(*FilenameUtf8,&Buf);
	if( Result==0 )
	{
		time_t CurrentTime, FileTime;
		FileTime = Buf.st_mtime;
		time( &CurrentTime );
		FLOAT DiffSeconds = difftime( CurrentTime, FileTime );
		return appRound(DiffSeconds / 60.0f / 60.0f / 24.0f);
	}
	return 0;
	unguard;
}

CORE_API void appCleanFileCache()
{
	guard(appCleanFileCache);

	// Delete all temporary files.
	guard(DeleteTemps);
	FString Temp = FString::Printf( TEXT("%s") PATH_SEPARATOR TEXT("*.tmp"), *GSys->CachePath );
	TArray<FString> Found = GFileManager->FindFiles( *Temp, 1, 0 );
	for( INT i=0; i<Found.Num(); i++ )
	{
		Temp = FString::Printf( TEXT("%s") PATH_SEPARATOR TEXT("%s"), *GSys->CachePath, *Found(i) );
		debugf( TEXT("Deleting temporary file: %s"), *Temp );
		GFileManager->Delete( *Temp );
	}
	unguard;

	// Delete cache files that are no longer wanted.
	guard(DeleteExpired);
	TArray<FString> Found = GFileManager->FindFiles( *(GSys->CachePath * TEXT("*") + GSys->CacheExt), 1, 0 );
	if( GSys->PurgeCacheDays )
	{
		for( INT i=0; i<Found.Num(); i++ )
		{
			FString Temp = FString::Printf( TEXT("%s") PATH_SEPARATOR TEXT("%s"), *GSys->CachePath, *Found(i) );
			INT DiffDays = GetFileAgeDays( *Temp );
			if( DiffDays > GSys->PurgeCacheDays )
			{
				debugf( TEXT("Purging outdated file from cache: %s (%i days old)"), *Temp, DiffDays );
				GFileManager->Delete( *Temp );
			}
		}
	}
	unguard;

	unguard;
}

/*-----------------------------------------------------------------------------
	Guids.
-----------------------------------------------------------------------------*/

void appGetGUID( void* GUID )
{
	if( !GUID )
		return;

	BYTE* Bytes = static_cast<BYTE*>(GUID);
	arc4random_buf( Bytes, 16 );

	// RFC 4122 version 4 and variant bits.
	Bytes[6] = static_cast<BYTE>((Bytes[6] & 0x0f) | 0x40);
	Bytes[8] = static_cast<BYTE>((Bytes[8] & 0x3f) | 0x80);
}

//
// Create a new globally unique identifier.
//
CORE_API FGuid appCreateGuid()
{
	guard(appCreateGuid);

	FGuid Result;
	appGetGUID( (void*)&Result );
	return Result;

	unguard;
}

/*-----------------------------------------------------------------------------
	Clipboard
-----------------------------------------------------------------------------*/
//static FString ClipboardText;
CORE_API void appClipboardCopy( const TCHAR* Str )
{
	guard(appClipboardCopy);
//	ClipboardText = FString( Str );
	unguard;
}

CORE_API FString appClipboardPaste()
{
	guard(appClipboardPaste);
//	return ClipboardText;
	FString Empty;
	return Empty;
	unguard;
}
/*-----------------------------------------------------------------------------
	Command line.
-----------------------------------------------------------------------------*/

static const TCHAR GUnixEmptyDir[] = TEXT("");
static const TCHAR* GUnixBaseDir = GUnixEmptyDir;
static const TCHAR* GUnixUserDir = GUnixEmptyDir;

static TCHAR* appUnixNormalizeRootText( const TCHAR* Dir )
{
	const size_t SourceLength = appUnixTCharLength( Dir );
	if( SourceLength > (SIZE_MAX/sizeof(TCHAR))-2 )
	{
		fputs( "Unreal: root path is too large\n", stderr );
		abort();
	}

	TCHAR* Result = static_cast<TCHAR*>(
		appUnixSystemMalloc((SourceLength+2)*sizeof(TCHAR))
	);
	size_t DestLength = 0;
	Dir = Dir ? Dir : TEXT("");
	for( const TCHAR* Source=Dir; *Source; ++Source )
	{
		const TCHAR Ch = *Source==TEXT('\\') ? TEXT('/') : *Source;
		if( Ch==TEXT('/') && DestLength>0 && Result[DestLength-1]==TEXT('/') )
			continue;
		Result[DestLength++] = Ch;
	}

	while( DestLength>1 && Result[DestLength-1]==TEXT('/') )
		--DestLength;
	if( DestLength>0 && Result[DestLength-1]!=TEXT('/') )
		Result[DestLength++] = TEXT('/');
	Result[DestLength] = 0;
	return Result;
}

static TCHAR* appUnixCanonicalRoot( const TCHAR* Dir )
{
	if( !Dir || !Dir[0] )
		return appUnixNormalizeRootText( Dir );

	FUnixUtf8String Utf8Dir( Dir );
	Utf8Dir.NormalizePath();
	ANSICHAR* ResolvedUtf8 = realpath( *Utf8Dir, NULL );
	if( !ResolvedUtf8 )
		return appUnixNormalizeRootText( Dir );

	TCHAR* ResolvedDir = appUnixFromUtf8System( ResolvedUtf8 );
	free( ResolvedUtf8 );
	TCHAR* Result = appUnixNormalizeRootText( ResolvedDir );
	free( ResolvedDir );
	return Result;
}

CORE_API void appSetBaseDir( const TCHAR* Dir )
{
	// These setters run before appInit/GMalloc. Retain prior values so any
	// pointer already handed to engine bootstrap code remains valid.
	GUnixBaseDir = appUnixCanonicalRoot( Dir );
}

CORE_API void appSetUserDir( const TCHAR* Dir )
{
	GUnixUserDir = appUnixCanonicalRoot( Dir );
}

static ANSICHAR* appUnixGetCurrentDirectory()
{
	size_t Capacity = 256;
	ANSICHAR* Buffer = static_cast<ANSICHAR*>(appUnixSystemMalloc(Capacity));
	for( ;; )
	{
		errno = 0;
		if( getcwd(Buffer,Capacity) )
			return Buffer;
		if( errno!=ERANGE || Capacity>SIZE_MAX/2 )
		{
			free( Buffer );
			return NULL;
		}
		Capacity *= 2;
		Buffer = static_cast<ANSICHAR*>(appUnixSystemRealloc(Buffer,Capacity));
	}
}

static ANSICHAR* appUnixGetHostName()
{
	const long Maximum = sysconf( _SC_HOST_NAME_MAX );
	if( Maximum>0 && static_cast<uint64_t>(Maximum)>=static_cast<uint64_t>(SIZE_MAX) )
		return NULL;
	const size_t Capacity = Maximum>0 ? static_cast<size_t>(Maximum)+1 : 256;
	ANSICHAR* Buffer = static_cast<ANSICHAR*>(appUnixSystemMalloc(Capacity));
	if( gethostname(Buffer,Capacity)!=0 )
	{
		free( Buffer );
		return NULL;
	}
	Buffer[Capacity-1] = 0;
	return Buffer;
}

static ANSICHAR* appUnixGetLoginName()
{
	const long Maximum = sysconf( _SC_LOGIN_NAME_MAX );
	if( Maximum>0 && static_cast<uint64_t>(Maximum)>=static_cast<uint64_t>(SIZE_MAX) )
		return NULL;
	size_t Capacity = Maximum>0 ? static_cast<size_t>(Maximum)+1 : 256;
	ANSICHAR* Buffer = static_cast<ANSICHAR*>(appUnixSystemMalloc(Capacity));
	for( ;; )
	{
		const INT Error = getlogin_r( Buffer, Capacity );
		if( Error==0 )
			return Buffer;
		if( Error!=ERANGE || Capacity>SIZE_MAX/2 )
		{
			free( Buffer );
			return NULL;
		}
		Capacity *= 2;
		Buffer = static_cast<ANSICHAR*>(appUnixSystemRealloc(Buffer,Capacity));
	}
}

// Get startup directory.
CORE_API const TCHAR* appBaseDir()
{
	if( !GUnixBaseDir[0] )
	{
		ANSICHAR* Utf8BaseDir = appUnixGetCurrentDirectory();
		if( Utf8BaseDir )
		{
			TCHAR* DecodedBaseDir = appUnixFromUtf8System( Utf8BaseDir );
			free( Utf8BaseDir );
			appSetBaseDir( DecodedBaseDir );
			free( DecodedBaseDir );
		}
	}
	return GUnixBaseDir;
}

CORE_API const TCHAR* appUserDir()
{
	return GUnixUserDir;
}

// Get computer name.
CORE_API const TCHAR* appComputerName()
{
	static const TCHAR* Result = NULL;
	if( !Result )
	{
		ANSICHAR* Utf8Name = appUnixGetHostName();
		Result = appUnixFromUtf8System( Utf8Name ? Utf8Name : "" );
		free( Utf8Name );
	}
	return Result;
}

// Get user name.
CORE_API const TCHAR* appUserName()
{
	static const TCHAR* Result = NULL;
	if( !Result )
	{
		ANSICHAR* Utf8Name = appUnixGetLoginName();
		Result = appUnixFromUtf8System( Utf8Name ? Utf8Name : "" );
		free( Utf8Name );
	}
	return Result;
}

// Get launch package base name.
CORE_API const TCHAR* appPackage()
{
	static ANSICHAR CachedModule[sizeof(GModule)] = {0};
	static const TCHAR* Result = NULL;
	if( !Result || memcmp(CachedModule,GModule,sizeof(GModule))!=0 )
	{
		memcpy( CachedModule, GModule, sizeof(GModule) );
		size_t ModuleLength = 0;
		while( ModuleLength<sizeof(GModule) && GModule[ModuleLength] )
			++ModuleLength;
		ANSICHAR* TerminatedModule = static_cast<ANSICHAR*>(
			appUnixSystemMalloc(ModuleLength+1)
		);
		memcpy( TerminatedModule, GModule, ModuleLength );
		TerminatedModule[ModuleLength] = 0;
		Result = appUnixFromUtf8System( TerminatedModule );
		free( TerminatedModule );
	}
	return Result;
}

/*-----------------------------------------------------------------------------
	App init/exit.
-----------------------------------------------------------------------------*/

//
// Platform specific initialization.
//
void appPlatformPreInit()
{
	GTimestamp = 1;
	GSecondsPerCycle = static_cast<FLOAT>(
		static_cast<double>(FSteadyClock::period::num) /
		static_cast<double>(FSteadyClock::period::den)
	);
}

void appPlatformInit()
{
	guard(appPlatformInit);

	// System initialization.
	GSys = new USystem;
	GSys->LoadConfig();
	if( GSys->Paths.Num()<=5 )
		appErrorf(TEXT("Core.System Paths did not load from System.ini"));
	GSys->AddToRoot();
	for( INT i=0; i<GSys->Suppress.Num(); i++ )
		GSys->Suppress(i).SetFlags( RF_Suppress );

	// An explicit seed was applied by __Context::StaticInit before appInit and
	// class registration. Publish it only after the trace subsystem can start;
	// ordinary launches keep the original wall-clock initialization.
	if( GUnixDiagnosticRngSeeded )
		appSetRandTraceDiagnosticSeed(
			GUnixDiagnosticRngRequestedSeed,
			GUnixDiagnosticRngEffectiveSeed
		);
	else
		srand( (unsigned)time( NULL ) );

	// Exit code handling
	ExitCodeMap = new TMap<pid_t,int>;

	// The monotonic timer frequency is fixed by steady_clock's duration.
	debugf( NAME_Init, TEXT("Steady timer frequency=%f MHz"), 0.000001 / GSecondsPerCycle );

	struct sigaction sa_child;
	sa_child.sa_handler = HandleChild;
	sigemptyset( &sa_child.sa_mask );
	sa_child.sa_flags = 0;
	sigaction(SIGCHLD, &sa_child, 0);
	
	unguard;
}

void appPlatformPreExit()
{
}

void appPlatformExit()
{
}

void appEnableFastMath( UBOOL Enable )
{
	guard(appEnableFastMath);

	unguard;
}

/*-----------------------------------------------------------------------------
	Pathnames.
-----------------------------------------------------------------------------*/

// Convert pathname to Unix format.
char* appUnixPath( const char* Path )
{
	guard(appUnixPath);
	static char* UnixPath = NULL;
	static size_t Capacity = 0;
	const size_t Length = strlen( Path ? Path : "" );
	if( Length==SIZE_MAX )
		appErrorf( TEXT("Unix path is too large.") );
	if( Capacity<Length+1 )
	{
		Capacity = Length+1;
		UnixPath = static_cast<char*>(appUnixSystemRealloc(UnixPath,Capacity));
	}
	memcpy( UnixPath, Path ? Path : "", Length+1 );
	for( char* Cursor=UnixPath; *Cursor; ++Cursor )
		if( *Cursor=='\\' )
			*Cursor = '/';
	return UnixPath;
	unguard;
}

/*-----------------------------------------------------------------------------
	Networking.
-----------------------------------------------------------------------------*/

DWORD appGetLocalIP( void )
{
	static DWORD LocalIP = 0;
	struct hostent* Hostinfo;

	if( LocalIP==0 )
	{
		ANSICHAR* Hostname = appUnixGetHostName();
		Hostinfo = Hostname ? gethostbyname(Hostname) : NULL;
		free( Hostname );
		if( Hostinfo && Hostinfo->h_addr_list[0] )
			appMemcpy( &LocalIP, Hostinfo->h_addr_list[0], appCheckedIntSize(sizeof(LocalIP)) );
	}

	return LocalIP;
}

/*-----------------------------------------------------------------------------
	String functions.
-----------------------------------------------------------------------------*/

int stricmp( const char* s, const char* t )
{
	int	i;
	for( i = 0; tolower(s[i]) == tolower(t[i]); i++ )
		if( s[i] == '\0' )
			return 0;
	return s[i] - t[i];
}

int strnicmp( const char* s, const char* t, int n )
{
	int	i;
	if( n <= 0 )
		return 0;
	for( i = 0; tolower(s[i]) == tolower(t[i]); i++ )
		if( (s[i] == '\0') || (i == n - 1) )
			return 0;
	return s[i] - t[i];
}

char* strupr( char* s )
{
	int	i;
	for( i = 0; s[i] != '\0'; i++ )
		s[i] = toupper(s[i]);
	return s;
}

/*-----------------------------------------------------------------------------
	Signal Handling
-----------------------------------------------------------------------------*/
jmp_buf __Context::Env;
struct sigaction __Context::Act_SIGHUP;
struct sigaction __Context::Act_SIGQUIT;
struct sigaction __Context::Act_SIGILL;
struct sigaction __Context::Act_SIGTRAP;
struct sigaction __Context::Act_SIGIOT;
struct sigaction __Context::Act_SIGBUS;
struct sigaction __Context::Act_SIGFPE;
struct sigaction __Context::Act_SIGSEGV;
struct sigaction __Context::Act_SIGTERM;

void __Context::StaticInit()
{
	appUnixBootstrapDiagnosticRngSeed();

	// Only try once.
	INT DefaultFlag = SA_RESETHAND;

	// Install a handler for all signals.
	Act_SIGHUP.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGHUP.sa_mask );
	Act_SIGHUP.sa_flags = DefaultFlag;
	sigaction( SIGHUP, &Act_SIGHUP, 0 );

	Act_SIGQUIT.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGQUIT.sa_mask );
	Act_SIGQUIT.sa_flags = DefaultFlag;
	sigaction( SIGQUIT, &Act_SIGQUIT, 0 );

	Act_SIGILL.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGILL.sa_mask );
	Act_SIGILL.sa_flags = DefaultFlag;
	sigaction( SIGILL, &Act_SIGILL, 0 );

	Act_SIGTRAP.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGTRAP.sa_mask );
	Act_SIGTRAP.sa_flags = DefaultFlag;
	sigaction( SIGTRAP, &Act_SIGTRAP, 0 );

	Act_SIGIOT.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGIOT.sa_mask );
	Act_SIGIOT.sa_flags = DefaultFlag;
	sigaction( SIGIOT, &Act_SIGIOT, 0 );

	Act_SIGBUS.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGBUS.sa_mask );
	Act_SIGBUS.sa_flags = DefaultFlag;
	sigaction( SIGBUS, &Act_SIGBUS, 0 );

	Act_SIGFPE.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGFPE.sa_mask );
	Act_SIGFPE.sa_flags = DefaultFlag;
	sigaction( SIGFPE, &Act_SIGFPE, 0 );

	Act_SIGSEGV.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGSEGV.sa_mask );
	Act_SIGSEGV.sa_flags = DefaultFlag;
	sigaction( SIGSEGV, &Act_SIGSEGV, 0 );

	Act_SIGTERM.sa_handler = HandleSignal;
	sigemptyset( &Act_SIGTERM.sa_mask );
	Act_SIGTERM.sa_flags = DefaultFlag;
	sigaction( SIGTERM, &Act_SIGTERM, 0 );

	signal( SIGPIPE, SIG_IGN );
}

void __Context::HandleSignal( int Sig )
{
	switch (Sig)
	{
		case SIGHUP:
			printf( "Signal: SIGHUP [hangup]\n" );
			SignalExit++;
			break;
		case SIGQUIT:
			printf( "Signal: SIGQUIT [quit]\n" );
			SignalExit++;
			break;
		case SIGILL:
			printf( "Signal: SIGILL [illegal instruction]\n" );
			SignalCritical++;
			break;
		case SIGTRAP:
			printf( "Signal: SIGTRAP [trap]\n" );
			SignalCritical++;
			break;
		case SIGIOT:
			printf( "Signal: SIGIOT [iot trap]\n" );
			SignalCritical++;
			break;
		case SIGBUS:
			printf( "Signal: SIGBUS [bus error]\n" );
			SignalCritical++;
			break;
		case SIGFPE:
			printf( "Signal: SIGFPE [floating point exception]\n" );
			SignalCritical++;
			break;
		case SIGSEGV:
			printf( "Signal: SIGSEGV [segmentation fault]\n" );
			SignalCritical++;
			break;
		case SIGTERM:
			printf( "Signal: SIGTERM [terminate]\n" );
			SignalExit++;
			break;
	}

	if ( (SignalCritical > 0) || (SignalExit > 1) )
	{
		if (AlreadyAborting)
		{
			// Avoid calling appExit again.
			AlreadyAborting = true;
			printf( "Aborting.\n" );
			appExit();
		}
		exit(1);
	}
	if ( SignalExit == 1 )
	{
		printf("Requesting Exit.\n");
		appRequestExit( 0 );
		// Recover into the innermost live guard only while the guarded
		// engine region is active. Outside that window (early startup,
		// launcher rounds, or the teardown tail) __Context::Env is either
		// zeroed or points at an already-exited frame; resuming there
		// cascades into SIGSEGV/SIGIOT terminate noise instead of exiting.
		// A second arrival takes the exit(1) branch above.
		if( GIsGuarded )
			longjmp( Env, 1 );
		return;
	}
}

#endif
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
