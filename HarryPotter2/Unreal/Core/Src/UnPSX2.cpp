/*=============================================================================
	UnPSX2: PSX2 port of UnVcWin32.cpp.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Brandon Reinhart.
=============================================================================*/
#if __GNUG__

// Standard includes.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <float.h>
#include <time.h>

// System includes.
#include <unistd.h>
#include <utime.h>
#include <sys/time.h>
#include <sys/stat.h>

// Core includes.
#include "CorePrivate.h"

// PSX2 includes.
#include "eekernel.h"

/*-----------------------------------------------------------------------------
	Globals
-----------------------------------------------------------------------------*/

// Module
ANSICHAR GModule[32];

// Environment
extern char **environ;

// Signal
static bool HandlingSignal = false;

// Time
static volatile DWORD GCycAcc[2] = {0, 0};
static INT GFrameCount = 0;

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
	else if( ParseCommand(&Cmd,TEXT("EXIT")) )
	{
		Ar.Log( TEXT("Closing by request") );
		appRequestExit( 0 );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("APP")) )
	{
		//!UNIX No APP command.
		Ar.Logf( TEXT("APP command not available.") );
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
	#if __STATIC_LINK
		INT* FalseHandle;
		return (void*) FalseHandle;
	#endif
}

//
// Free a library.
//
void appFreeDllHandle( void* DllHandle )
{
	// Don't do anything.  The handle is garbage.
}

//
// Lookup the address of a shared library function.
//
void* appGetDllExport( void* DllHandle, const TCHAR* ProcName )
{
	// Do not do anything, this is never called.
}

//
// Break the debugger.
//
#ifndef DEFINED_appDebugBreak
void appDebugBreak()
{
	guard(appDebugBreak);
	//!UNIX Not implemented in UNIX.
	unguard;
}
#endif

/*-----------------------------------------------------------------------------
	External processes.
-----------------------------------------------------------------------------*/

void* appCreateProc( const TCHAR* URL, const TCHAR* Parms , UBOOL bRealTime )
{
	guard(appCreateProc);
/*
	debugf( TEXT("Create Proc: %s %s"), URL, Parms );

	TCHAR LocalParms[PATH_MAX] = TEXT("");
	appStrcpy(LocalParms, URL);
	appStrcat(LocalParms, " ");
	appStrcat(LocalParms, Parms);

	pid_t pid = fork(); 
	if (pid == 0) {
		system( LocalParms );
		_exit(1);
	}
*/
	return NULL;
	unguard;
}

UBOOL appGetProcReturnCode( void* ProcHandle, INT* ReturnCode )
{
	guard(appGetProcReturnCode);
	return 0;
	unguard;
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
/*
	TCHAR* 		Result = appStaticString1024();
	time_t		CurTime;
	struct tm*	SysTime;

	CurTime = time( NULL );
	SysTime = localtime( &CurTime );
	appSprintf( Result, asctime( SysTime ) );
	// Strip newline.
	Result[appStrlen( Result )-1] = '\0';
	return Result;
*/
	return TEXT("Unsupported.");
	unguard;
}

//
// Get file time.
//
CORE_API DWORD appGetTime( const TCHAR* Filename )
{
	guard(appGetTime);

	// FIXME: Implement for PSX2
/*
	struct utimbuf FileTime;
	if( utime(Filename,&FileTime)!=0 )
		return 0;
	//FIXME Is this the right "time" to return?
	return (DWORD)FileTime.modtime;
*/
	unguard;
}

//
// Get time in seconds.
//
CORE_API FTime appSecondsSlow()
{
	return FTime();
}

CORE_API FTime appSeconds()
{
	volatile DWORD Cyc[2];
	*(QWORD*) Cyc = *(QWORD*)GCycAcc + appCycles();

	volatile FLOAT f = 4294967296.f;
	volatile FLOAT l = (FLOAT) Cyc[0];
	volatile FLOAT h = (FLOAT) Cyc[1];
	volatile FLOAT r;
	asm volatile(
	"
		lwc1  $f04, 0x00(%0)
		lwc1  $f05, 0x00(%1)
		lwc1  $f06, 0x00(%2)
		lwc1  $f07, 0x00(%3)
		mul.s $f01, $f05, $f06
		add.s $f02, $f01, $f04
		mul.s $f03, $f02, $f07
		swc1  $f03, 0x00(%4)
	"
	:
	: "r" (&l), "r" (&h), "r" (&f), "r" (&GSecondsPerCycle), "r" (&r)
	);
	return FTime(r);
}

CORE_API DWORD appCycles()
{
	DWORD cyc;
	asm volatile("mfc0 %0,$9\n" : "=r" (cyc) );
	
	return cyc;
}

int BlankHandler(int ca)
{
	*(QWORD*)GCycAcc += appCycles();

	DWORD cyc = 0;
	asm volatile("mtc0 %0,$9\n" : : "r" (cyc) );
}

//
// Return the system time.
//
CORE_API void appSystemTime( INT& Year, INT& Month, INT& DayOfWeek, INT& Day, INT& Hour, INT& Min, INT& Sec, INT& MSec )
{
	guard(appSystemTime);

	unguard;
}

CORE_API void appSleep( FLOAT Seconds )
{
	guard(appSleep);

	FTime CurTime = appSeconds();
	while(appSeconds() - CurTime < Seconds);

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
static FString ClipboardText;
CORE_API void appClipboardCopy( const TCHAR* Str )
{
	guard(appClipboardCopy);
	ClipboardText = FString( Str );
	unguard;
}

CORE_API FString appClipboardPaste()
{
	guard(appClipboardPaste);
	return ClipboardText;
	unguard;
}
/*-----------------------------------------------------------------------------
	Command line.
-----------------------------------------------------------------------------*/

// Get startup directory.
CORE_API const TCHAR* appBaseDir()
{
	guard(appBaseDir);
	static TCHAR BaseDir[32]=TEXT("");
	BaseDir[0] = '\0';
	return BaseDir;
	unguard;
}

// Get computer name.
CORE_API const TCHAR* appComputerName()
{
	guard(appComputerName);

	return "Playstation2";
	
	unguard;
}

// Get user name.
CORE_API const TCHAR* appUserName()
{
	guard(appUserName);

	return "Playstation2 user.";
	
	unguard;
}

// Get launch package base name.
CORE_API const TCHAR* appPackage()
{
	guard(appPackage);
	return GModule;
	unguard;
}

/*-----------------------------------------------------------------------------
	App init/exit.
-----------------------------------------------------------------------------*/

//
// Platform specific initialization.
//
void appPlatformPreInit()
{
}

void appPlatformInit()
{
	guard(appPlatformInit);

	// System initialization.
	GSys = new USystem;
	GSys->AddToRoot();
	for( INT i=0; i<GSys->Suppress.Num(); i++ )
		GSys->Suppress(i).SetFlags( RF_Suppress );

	// CPU speed.
	GTimestamp = 1;
	GSecondsPerCycle = 1.0f / 300000000.0f;

	// Determine # of Cycles per HSync.
	AddIntcHandler(INTC_VBLANK_S, BlankHandler, 0);
	EnableIntc(INTC_VBLANK_S);

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
char* appUnixPath( const TCHAR* Path )
{
	guard(appUnixPath);
	TCHAR* UnixPath = appStaticString1024();
	TCHAR* Cur = UnixPath;
	appStrncpy( UnixPath, Path, 1024 );
	while( Cur = strchr( Cur, '\\' ) )
		*Cur = '/';
	return UnixPath;
	unguard;
}

/*-----------------------------------------------------------------------------
	Networking.
-----------------------------------------------------------------------------*/

unsigned long appGetLocalIP( void )
{
/*
	static unsigned long LocalIP = 0;
	struct hostent *Hostinfo;
	char Hostname[256];

	if( LocalIP==0 )
	{
		gethostname( Hostname, sizeof(Hostname) );
		Hostinfo = gethostbyname( Hostname );
		LocalIP = *(unsigned long*)Hostinfo->h_addr_list[0];
	}
	
	return LocalIP;
*/
	return 0;
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
}

void __Context::HandleSignal( int Sig )
{
	longjmp( Env, 1 );
}

#endif
/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
